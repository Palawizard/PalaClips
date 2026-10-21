use std::ffi::c_void;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use palaclips_engine::encode::{
    GpuConverter, Size, blob_bytes, compile_shader, create_device, render_texture,
};
use windows::Wdk::Graphics::Direct3D::{
    D3DKMT_SCHEDULINGPRIORITYCLASS, D3DKMT_SCHEDULINGPRIORITYCLASS_ABOVE_NORMAL,
    D3DKMT_SCHEDULINGPRIORITYCLASS_HIGH, D3DKMT_SCHEDULINGPRIORITYCLASS_NORMAL,
    D3DKMT_SCHEDULINGPRIORITYCLASS_REALTIME, D3DKMTGetProcessSchedulingPriorityClass,
    D3DKMTSetProcessSchedulingPriorityClass,
};
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_CONSTANT_BUFFER, D3D11_BIND_RENDER_TARGET, D3D11_BUFFER_DESC, D3D11_QUERY_DESC,
    D3D11_QUERY_EVENT, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_VIEWPORT, ID3D11Device,
    ID3D11DeviceContext, ID3D11Query, ID3D11RenderTargetView, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_NV12, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_PRESENT, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_DISCARD,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGIFactory2, IDXGISwapChain1,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow, WINDOW_EX_STYLE,
    WS_OVERLAPPEDWINDOW,
};

use crate::probe_av::{create_window, pump_messages};
use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::UI::Shell::IsUserAnAdmin;
use windows::core::{BOOL, Interface, s, w};

const LOAD_SHADER: &str = r"
cbuffer Load : register(b0) { uint iterations; uint3 padding; };

float4 fullscreen(uint id : SV_VertexID) : SV_Position {
    float2 corner = float2((id << 1) & 2, id & 2);
    return float4(corner * float2(2, -2) + float2(-1, 1), 0, 1);
}

float4 burn(float4 position : SV_Position) : SV_Target {
    float value = position.x * 0.001 + position.y * 0.0007;
    [loop] for (uint i = 0; i < iterations; i++) {
        value = frac(sin(value) * 43758.5453 + 0.1);
    }
    return value.xxxx;
}
";

const LOAD_SIZE: Size = Size::new(1920, 1080);
const SOURCE_SIZE: Size = Size::new(2560, 1440);
const FPS: u32 = 60;

#[derive(Debug, Clone, Copy)]
enum Priority {
    Default,
    Thread7,
    Class(D3DKMT_SCHEDULINGPRIORITYCLASS),
    Thread7Class(D3DKMT_SCHEDULINGPRIORITYCLASS),
}

impl Priority {
    fn label(self) -> String {
        match self {
            Self::Default => "défaut".into(),
            Self::Thread7 => "SetGPUThreadPriority(7)".into(),
            Self::Class(class) => format!("classe {}", class_name(class)),
            Self::Thread7Class(class) => format!("thread 7 + classe {}", class_name(class)),
        }
    }
}

fn class_name(class: D3DKMT_SCHEDULINGPRIORITYCLASS) -> &'static str {
    [
        "IDLE",
        "BELOW_NORMAL",
        "NORMAL",
        "ABOVE_NORMAL",
        "HIGH",
        "REALTIME",
    ]
    .get(class.0 as usize)
    .copied()
    .unwrap_or("?")
}

pub fn probe_gpu_priority(seconds: u64, rounds: u32, load_ms: f64, window: bool) -> Result<()> {
    // SAFETY: IsUserAnAdmin has no preconditions.
    let admin = unsafe { IsUserAnAdmin() }.as_bool();
    println!(
        "HAGS (HwSchMode) : {}",
        match hags_mode() {
            Some(2) => "activé".to_owned(),
            Some(1) => "désactivé".to_owned(),
            Some(other) => format!("valeur {other}"),
            None => "absent (désactivé)".to_owned(),
        }
    );
    println!("administrateur : {admin}");

    let device = create_device()?;
    // SAFETY: GetImmediateContext has no preconditions on a valid device.
    let context = unsafe { device.GetImmediateContext() }?;
    let source = render_texture(&device, SOURCE_SIZE, DXGI_FORMAT_B8G8R8A8_UNORM)?;
    let output = nv12_texture(&device, SOURCE_SIZE)?;
    let converter = GpuConverter::new(&device, SOURCE_SIZE, SOURCE_SIZE, FPS)?;
    let query = event_query(&device)?;
    let measure = |label: &str| -> Result<()> {
        let latencies = convert_latencies(&converter, &context, &query, &source, &output, seconds)?;
        println!("  {label:<34} {}", summary(latencies));
        Ok(())
    };

    println!("conversion 1440p → NV12 à {FPS} i/s, {seconds} s par essai");
    measure("sans charge, défaut")?;

    let load = GpuLoad::spawn(seconds * u64::from(rounds) * 7 + 10, load_ms, window)?;
    std::thread::sleep(Duration::from_secs(3));
    println!(
        "charge au premier plan : {}",
        foreground_pid() == load.0.id()
    );
    let priorities = [
        Priority::Default,
        Priority::Thread7,
        Priority::Class(D3DKMT_SCHEDULINGPRIORITYCLASS_ABOVE_NORMAL),
        Priority::Class(D3DKMT_SCHEDULINGPRIORITYCLASS_HIGH),
        Priority::Class(D3DKMT_SCHEDULINGPRIORITYCLASS_REALTIME),
        Priority::Thread7Class(D3DKMT_SCHEDULINGPRIORITYCLASS_HIGH),
    ];
    for round in 1..=rounds {
        println!("charge GPU, tour {round}");
        for priority in priorities {
            let applied = apply(&device, priority);
            measure(&format!("{} [{applied}]", priority.label()))?;
            reset(&device);
        }
    }
    Ok(())
}

fn apply(device: &ID3D11Device, priority: Priority) -> String {
    let (thread, class) = match priority {
        Priority::Default => (None, None),
        Priority::Thread7 => (Some(7), None),
        Priority::Class(class) => (None, Some(class)),
        Priority::Thread7Class(class) => (Some(7), Some(class)),
    };
    let mut parts = Vec::new();
    if let Some(value) = thread {
        parts.push(set_thread_priority(device, value));
    }
    if let Some(class) = class {
        parts.push(set_class(class));
    }
    if parts.is_empty() {
        parts.push(format!(
            "lu : thread {}, classe {}",
            thread_priority(device).map_or("?".into(), |value| value.to_string()),
            current_class().map_or("?", class_name)
        ));
    }
    parts.join(", ")
}

fn reset(device: &ID3D11Device) {
    set_thread_priority(device, 0);
    set_class(D3DKMT_SCHEDULINGPRIORITYCLASS_NORMAL);
}

fn set_thread_priority(device: &ID3D11Device, value: i32) -> String {
    let Ok(dxgi) = device.cast::<IDXGIDevice>() else {
        return "IDXGIDevice absent".into();
    };
    // SAFETY: plain COM call on a valid DXGI device.
    let result = unsafe { dxgi.SetGPUThreadPriority(value) };
    match result {
        Ok(()) => format!(
            "thread {value} OK, relu {}",
            thread_priority(device).map_or("?".into(), |read| read.to_string())
        ),
        Err(error) => format!("thread {value} refusé {:#010x}", error.code().0),
    }
}

fn thread_priority(device: &ID3D11Device) -> Option<i32> {
    let dxgi = device.cast::<IDXGIDevice>().ok()?;
    // SAFETY: plain COM call on a valid DXGI device.
    unsafe { dxgi.GetGPUThreadPriority() }.ok()
}

fn set_class(class: D3DKMT_SCHEDULINGPRIORITYCLASS) -> String {
    // SAFETY: the pseudo handle of the current process is always valid.
    let status = unsafe { D3DKMTSetProcessSchedulingPriorityClass(GetCurrentProcess(), class) };
    if status.is_ok() {
        format!(
            "classe {} OK, relue {}",
            class_name(class),
            current_class().map_or("?", class_name)
        )
    } else {
        format!("classe {} refusée {:#010x}", class_name(class), status.0)
    }
}

fn current_class() -> Option<D3DKMT_SCHEDULINGPRIORITYCLASS> {
    let mut class = D3DKMT_SCHEDULINGPRIORITYCLASS::default();
    // SAFETY: the pseudo handle is valid and `class` is a valid out-pointer.
    let status =
        unsafe { D3DKMTGetProcessSchedulingPriorityClass(GetCurrentProcess(), &mut class) };
    status.is_ok().then_some(class)
}

fn hags_mode() -> Option<u32> {
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: `value` and `size` are valid out-pointers sized for a REG_DWORD.
    unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!(r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers"),
            w!("HwSchMode"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast::<c_void>()),
            Some(&mut size),
        )
    }
    .ok()
    .ok()?;
    Some(value)
}

fn convert_latencies(
    converter: &GpuConverter,
    context: &ID3D11DeviceContext,
    query: &ID3D11Query,
    source: &ID3D11Texture2D,
    output: &ID3D11Texture2D,
    seconds: u64,
) -> Result<Vec<f64>> {
    let period = Duration::from_secs_f64(1.0 / f64::from(FPS));
    let start = Instant::now();
    let mut next = start;
    let mut latencies = Vec::new();
    while start.elapsed() < Duration::from_secs(seconds) {
        if let Some(wait) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
        let submitted = Instant::now();
        converter.convert(source, output, 0)?;
        // SAFETY: the query and the context belong to the same device.
        unsafe {
            context.End(query);
            context.Flush();
        }
        wait_for(context, query);
        latencies.push(submitted.elapsed().as_secs_f64() * 1_000.0);
        next = (next + period).max(Instant::now());
    }
    Ok(latencies)
}

fn summary(mut latencies: Vec<f64>) -> String {
    latencies.sort_by(f64::total_cmp);
    let at = |q: f64| latencies[((latencies.len() - 1) as f64 * q) as usize];
    let frame_ms = 1_000.0 / f64::from(FPS);
    let late = latencies.iter().filter(|ms| **ms > frame_ms).count();
    format!(
        "p50 {:6.2} ms  p95 {:6.2} ms  max {:7.2} ms  > 1 image : {:5.1} %  ({} conv.)",
        at(0.5),
        at(0.95),
        at(1.0),
        late as f64 * 100.0 / latencies.len() as f64,
        latencies.len()
    )
}

fn wait_for(context: &ID3D11DeviceContext, query: &ID3D11Query) {
    loop {
        let mut done = BOOL(0);
        // SAFETY: an event query writes one BOOL into `done` once the GPU has reached it.
        let _ = unsafe {
            context.GetData(
                query,
                Some((&raw mut done).cast::<c_void>()),
                size_of::<BOOL>() as u32,
                0,
            )
        };
        if done.as_bool() {
            return;
        }
        std::thread::sleep(Duration::from_micros(200));
    }
}

fn event_query(device: &ID3D11Device) -> Result<ID3D11Query> {
    let desc = D3D11_QUERY_DESC {
        Query: D3D11_QUERY_EVENT,
        MiscFlags: 0,
    };
    let mut query = None;
    // SAFETY: `desc` is a valid event query descriptor and the out-pointer is valid.
    unsafe { device.CreateQuery(&desc, Some(&mut query)) }?;
    query.ok_or_else(|| anyhow!("requête d'événement non créée"))
}

fn nv12_texture(device: &ID3D11Device, size: Size) -> Result<ID3D11Texture2D> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: size.width,
        Height: size.height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_NV12,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut texture = None;
    // SAFETY: `desc` describes a valid NV12 render target and the out-pointer is valid.
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }?;
    texture.ok_or_else(|| anyhow!("texture NV12 non créée"))
}

struct GpuLoad(Child);

impl GpuLoad {
    fn spawn(seconds: u64, load_ms: f64, window: bool) -> Result<Self> {
        let mut command = Command::new(std::env::current_exe()?);
        command.args([
            "gpu-load",
            "--seconds",
            &seconds.to_string(),
            "--draw-ms",
            &load_ms.to_string(),
        ]);
        if window {
            command.arg("--window");
        }
        Ok(Self(command.stdout(Stdio::null()).spawn()?))
    }
}

impl Drop for GpuLoad {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn load_window(device: &ID3D11Device) -> Result<(IDXGISwapChain1, ID3D11RenderTargetView)> {
    let window = create_window(
        WINDOW_EX_STYLE(0),
        WS_OVERLAPPEDWINDOW,
        Some((200, 200, 640, 360)),
    )?;
    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: 640,
        Height: 360,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        ..Default::default()
    };
    let mut view = None;
    // SAFETY: the window belongs to this thread; the factory is the device's own; out-pointers are valid.
    let swap_chain = unsafe {
        let _ = SetForegroundWindow(window);
        let factory: IDXGIFactory2 = device.cast::<IDXGIDevice>()?.GetAdapter()?.GetParent()?;
        let swap_chain = factory.CreateSwapChainForHwnd(device, window, &desc, None, None)?;
        let back: ID3D11Texture2D = swap_chain.GetBuffer(0)?;
        device.CreateRenderTargetView(&back, None, Some(&mut view))?;
        swap_chain
    };
    Ok((
        swap_chain,
        view.ok_or_else(|| anyhow!("vue de la fenêtre non créée"))?,
    ))
}

fn foreground_pid() -> u32 {
    let mut pid = 0;
    // SAFETY: GetForegroundWindow has no preconditions and `pid` is a valid out-pointer.
    unsafe { GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut pid)) };
    pid
}

pub fn gpu_load(seconds: u64, draw_ms: f64, window: bool) -> Result<()> {
    let device = create_device()?;
    // SAFETY: GetImmediateContext has no preconditions on a valid device.
    let context = unsafe { device.GetImmediateContext() }?;
    let presented = window.then(|| load_window(&device)).transpose()?;
    let vertex = compile_shader(LOAD_SHADER, s!("fullscreen"), s!("vs_5_0"))?;
    let pixel = compile_shader(LOAD_SHADER, s!("burn"), s!("ps_5_0"))?;
    let target = render_texture(&device, LOAD_SIZE, DXGI_FORMAT_B8G8R8A8_UNORM)?;
    let constants_desc = D3D11_BUFFER_DESC {
        ByteWidth: 16,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
        ..Default::default()
    };
    let mut vertex_shader = None;
    let mut pixel_shader = None;
    let mut target_view = None;
    let mut constants = None;
    // SAFETY: bytecode comes from live D3DCompile blobs; descriptors and out-pointers are valid.
    unsafe {
        device.CreateVertexShader(blob_bytes(&vertex), None, Some(&mut vertex_shader))?;
        device.CreatePixelShader(blob_bytes(&pixel), None, Some(&mut pixel_shader))?;
        device.CreateRenderTargetView(&target, None, Some(&mut target_view))?;
        device.CreateBuffer(&constants_desc, None, Some(&mut constants))?;
        context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        context.VSSetShader(vertex_shader.as_ref(), None);
        context.PSSetShader(pixel_shader.as_ref(), None);
        context.PSSetConstantBuffers(0, Some(&[constants.clone()]));
        context.OMSetRenderTargets(Some(&[target_view]), None);
        context.RSSetViewports(Some(&[D3D11_VIEWPORT {
            Width: LOAD_SIZE.width as f32,
            Height: LOAD_SIZE.height as f32,
            MaxDepth: 1.0,
            ..Default::default()
        }]));
    }
    let constants = constants.ok_or_else(|| anyhow!("tampon de constantes non créé"))?;
    let set_iterations = |iterations: u32| {
        let data = [iterations, 0, 0, 0];
        // SAFETY: `data` is 16 bytes, the size of the constant buffer.
        unsafe { context.UpdateSubresource(&constants, 0, None, data.as_ptr().cast(), 0, 0) };
    };
    let queries = [event_query(&device)?, event_query(&device)?];
    let timed_draw = |iterations: u32| {
        set_iterations(iterations);
        let start = Instant::now();
        // SAFETY: the pipeline state set above is complete for a fullscreen triangle.
        unsafe {
            context.Draw(3, 0);
            context.End(&queries[0]);
            context.Flush();
        }
        wait_for(&context, &queries[0]);
        start.elapsed().as_secs_f64() * 1_000.0
    };

    let mut iterations = 16u32;
    let mut measured = timed_draw(iterations);
    while measured < draw_ms / 4.0 && iterations < 1 << 20 {
        iterations *= 2;
        measured = timed_draw(iterations);
    }
    let iterations = ((f64::from(iterations) * draw_ms / measured.max(0.01)) as u32).max(1);
    set_iterations(iterations);
    tracing::info!(
        iterations,
        draw_ms = timed_draw(iterations),
        "charge GPU calibrée"
    );

    let end = Instant::now() + Duration::from_secs(seconds);
    let mut index = 0;
    while Instant::now() < end {
        let query = &queries[index % 2];
        if index >= 2 {
            wait_for(&context, query);
        }
        // SAFETY: same complete pipeline state; the query belongs to this device.
        unsafe {
            context.Draw(3, 0);
            context.End(query);
            context.Flush();
        }
        if let Some((swap_chain, view)) = &presented {
            // SAFETY: the view targets the swap chain's back buffer on this device.
            unsafe {
                context.ClearRenderTargetView(view, &[0.1, 0.1, 0.1, 1.0]);
                let _ = swap_chain.Present(0, DXGI_PRESENT(0));
            }
            pump_messages();
        }
        index += 1;
    }
    Ok(())
}
