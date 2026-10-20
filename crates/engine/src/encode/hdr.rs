use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, ID3DBlob};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_VIEWPORT, ID3D11Device, ID3D11DeviceContext, ID3D11PixelShader, ID3D11Texture2D,
    ID3D11VertexShader,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::core::{PCSTR, s};

use super::convert::Size;
use super::d3d11::{gpu_error, render_texture};
use crate::error::{EngineError, Result};

const ROLL_OFF_START: f32 = 0.9;

const SHADER: &str = r"
Texture2D<float4> source : register(t0);

float4 fullscreen(uint id : SV_VertexID) : SV_Position {
    float2 corner = float2((id << 1) & 2, id & 2);
    return float4(corner * float2(2, -2) + float2(-1, 1), 0, 1);
}

float3 encode_srgb(float3 linear_rgb) {
    return linear_rgb <= 0.0031308
        ? linear_rgb * 12.92
        : 1.055 * pow(linear_rgb, 1.0 / 2.4) - 0.055;
}

float3 roll_off(float3 rgb) {
    float peak = max(max(rgb.r, rgb.g), rgb.b);
    if (peak <= ROLL_OFF_START) {
        return rgb;
    }
    float headroom = 1.0 - ROLL_OFF_START;
    float excess = peak - ROLL_OFF_START;
    return rgb * ((ROLL_OFF_START + headroom * excess / (excess + headroom)) / peak);
}

float4 scrgb_to_sdr(float4 position : SV_Position) : SV_Target {
    float3 linear_rgb = saturate(roll_off(source.Load(int3(position.xy, 0)).rgb / SDR_WHITE));
    return float4(encode_srgb(linear_rgb), 1);
}
";

#[derive(Debug)]
pub struct HdrToSdr {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    vertex_shader: ID3D11VertexShader,
    pixel_shader: ID3D11PixelShader,
    sdr: Option<ID3D11Texture2D>,
}

impl HdrToSdr {
    pub fn new(device: &ID3D11Device, sdr_white_scrgb: f32) -> Result<Self> {
        let source = shader_source(sdr_white_scrgb);
        let vertex = compile_shader(&source, s!("fullscreen"), s!("vs_5_0"))?;
        let pixel = compile_shader(&source, s!("scrgb_to_sdr"), s!("ps_5_0"))?;
        let mut vertex_shader = None;
        let mut pixel_shader = None;
        // SAFETY: the bytecode slices come from successful D3DCompile blobs alive for the calls.
        unsafe {
            device
                .CreateVertexShader(blob_bytes(&vertex), None, Some(&mut vertex_shader))
                .map_err(gpu_error)?;
            device
                .CreatePixelShader(blob_bytes(&pixel), None, Some(&mut pixel_shader))
                .map_err(gpu_error)?;
        }
        Ok(Self {
            device: device.clone(),
            // SAFETY: GetImmediateContext has no preconditions on a valid device.
            context: unsafe { device.GetImmediateContext() }.map_err(gpu_error)?,
            vertex_shader: vertex_shader.ok_or_else(missing_shader)?,
            pixel_shader: pixel_shader.ok_or_else(missing_shader)?,
            sdr: None,
        })
    }

    pub fn convert(&mut self, scrgb: &ID3D11Texture2D) -> Result<ID3D11Texture2D> {
        let size = Size::of(scrgb);
        let sdr = match self.sdr.take() {
            Some(sdr) if Size::of(&sdr) == size => sdr,
            _ => render_texture(&self.device, size, DXGI_FORMAT_B8G8R8A8_UNORM)?,
        };
        let mut source_view = None;
        let mut target_view = None;
        // SAFETY: both textures live on `device` with SHADER_RESOURCE / RENDER_TARGET binds; views
        // and state are set on the multithread-protected immediate context and unbound after the draw.
        unsafe {
            self.device
                .CreateShaderResourceView(scrgb, None, Some(&mut source_view))
                .map_err(gpu_error)?;
            self.device
                .CreateRenderTargetView(&sdr, None, Some(&mut target_view))
                .map_err(gpu_error)?;
            self.context.IASetInputLayout(None);
            self.context
                .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            self.context.VSSetShader(&self.vertex_shader, None);
            self.context.PSSetShader(&self.pixel_shader, None);
            self.context.PSSetShaderResources(0, Some(&[source_view]));
            self.context.OMSetRenderTargets(Some(&[target_view]), None);
            self.context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: size.width as f32,
                Height: size.height as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            self.context.Draw(3, 0);
            self.context.PSSetShaderResources(0, Some(&[None]));
            self.context.OMSetRenderTargets(Some(&[None]), None);
        }
        self.sdr = Some(sdr.clone());
        Ok(sdr)
    }
}

fn shader_source(sdr_white_scrgb: f32) -> String {
    SHADER
        .replace("SDR_WHITE", &format!("{sdr_white_scrgb:.4}"))
        .replace("ROLL_OFF_START", &format!("{ROLL_OFF_START:.4}"))
}

pub fn compile_shader(source: &str, entry: PCSTR, target: PCSTR) -> Result<ID3DBlob> {
    let mut code = None;
    let mut errors = None;
    // SAFETY: `source` outlives the call and its exact length is passed; out-pointers are valid.
    let result = unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            s!("palaclips.hlsl"),
            None,
            None,
            entry,
            target,
            0,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    if let Err(error) = result {
        let detail = errors
            .as_ref()
            .map(|blob| String::from_utf8_lossy(blob_bytes(blob)).into_owned())
            .unwrap_or_else(|| error.message());
        return Err(EngineError::Encoder(format!("shader : {detail}")));
    }
    code.ok_or_else(missing_shader)
}

pub fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
    // SAFETY: a D3D blob owns GetBufferSize() bytes at GetBufferPointer() for its whole lifetime.
    unsafe { std::slice::from_raw_parts(blob.GetBufferPointer().cast(), blob.GetBufferSize()) }
}

fn missing_shader() -> EngineError {
    EngineError::Encoder("shader HDR → SDR non créé".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hdr_to_sdr_shaders_compile() -> Result<()> {
        let source = shader_source(3.5);
        compile_shader(&source, s!("fullscreen"), s!("vs_5_0"))?;
        compile_shader(&source, s!("scrgb_to_sdr"), s!("ps_5_0"))?;
        Ok(())
    }
}
