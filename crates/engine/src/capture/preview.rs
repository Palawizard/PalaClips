use std::time::{Duration, Instant};

use rsmpeg::avutil::AVFrame;
use rsmpeg::ffi;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, ID3D11Multithread, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_NV12;
use windows::core::Interface;
use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    GraphicsCaptureItemType, MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

use super::targets::{ResolvedTarget, resolve_target};
use super::video::keep_mta_alive;
use crate::config::CaptureTarget;
use crate::encode::{GpuConverter, Size, encode_jpeg, render_texture, staging_texture};
use crate::error::{EngineError, Result};

pub const PREVIEW_BOX: Size = Size::new(384, 216);
const PREVIEW_INTERVAL: Duration = Duration::from_millis(900);

pub type PreviewSink = Box<dyn FnMut(Vec<u8>) + Send>;

pub struct PreviewCapture {
    control: Option<CaptureControl<PreviewHandler, EngineError>>,
}

impl PreviewCapture {
    pub fn start(target: &CaptureTarget, sink: PreviewSink) -> Result<Self> {
        keep_mta_alive();
        let control = match resolve_target(target)? {
            ResolvedTarget::Monitor(monitor) => {
                PreviewHandler::start_free_threaded(preview_settings(monitor, sink))
            }
            ResolvedTarget::Window(window) => {
                PreviewHandler::start_free_threaded(preview_settings(window, sink))
            }
        }
        .map_err(|error| EngineError::Capture(error.to_string()))?;
        Ok(Self {
            control: Some(control),
        })
    }
}

impl std::fmt::Debug for PreviewCapture {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreviewCapture")
            .field("running", &self.control.is_some())
            .finish()
    }
}

impl Drop for PreviewCapture {
    fn drop(&mut self) {
        if let Some(control) = self.control.take()
            && let Err(error) = control.stop()
        {
            tracing::debug!(%error, "aperçu déjà arrêté");
        }
    }
}

pub fn preview_size(input: Size) -> Size {
    let scale = f64::min(
        PREVIEW_BOX.width as f64 / input.width.max(1) as f64,
        PREVIEW_BOX.height as f64 / input.height.max(1) as f64,
    )
    .min(1.0);
    let even = |value: u32| ((f64::from(value) * scale / 2.0).round() as u32).max(1) * 2;
    Size::new(even(input.width), even(input.height))
}

fn preview_settings<T: TryInto<GraphicsCaptureItemType>>(
    item: T,
    sink: PreviewSink,
) -> Settings<PreviewSink, T> {
    Settings::new(
        item,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Custom(PREVIEW_INTERVAL),
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        sink,
    )
}

struct Scaler {
    input: Size,
    copy: ID3D11Texture2D,
    converter: GpuConverter,
    output: ID3D11Texture2D,
    readback: ID3D11Texture2D,
}

struct PreviewHandler {
    sink: PreviewSink,
    scaler: Option<Scaler>,
    last: Option<Instant>,
}

impl GraphicsCaptureApiHandler for PreviewHandler {
    type Flags = PreviewSink;
    type Error = EngineError;

    fn new(context: Context<Self::Flags>) -> Result<Self> {
        let multithread: ID3D11Multithread = context
            .device
            .cast()
            .map_err(|error| EngineError::Capture(error.message()))?;
        // SAFETY: plain COM call on a valid interface, before any concurrent use of the context.
        let _was_protected = unsafe { multithread.SetMultithreadProtected(true) };
        Ok(Self {
            sink: context.flags,
            scaler: None,
            last: None,
        })
    }

    fn on_frame_arrived(&mut self, frame: &mut Frame, _: InternalCaptureControl) -> Result<()> {
        if self
            .last
            .is_some_and(|last| last.elapsed() < PREVIEW_INTERVAL)
        {
            return Ok(());
        }
        self.last = Some(Instant::now());
        match self.snapshot(frame) {
            Ok(jpeg) => (self.sink)(jpeg),
            Err(error) => tracing::debug!(%error, "aperçu non produit"),
        }
        Ok(())
    }
}

impl PreviewHandler {
    fn snapshot(&mut self, frame: &mut Frame) -> Result<Vec<u8>> {
        let input = Size::new(frame.width(), frame.height());
        if self
            .scaler
            .as_ref()
            .is_none_or(|scaler| scaler.input != input)
        {
            let output = preview_size(input);
            let device = frame.device();
            self.scaler = Some(Scaler {
                input,
                copy: render_texture(device, input, frame.desc().Format)?,
                converter: GpuConverter::new(device, input, output, 1)?,
                output: render_texture(device, output, DXGI_FORMAT_NV12)?,
                readback: staging_texture(device, output, DXGI_FORMAT_NV12)?,
            });
        }
        let Some(scaler) = self.scaler.as_ref() else {
            return Err(EngineError::Capture("aperçu sans convertisseur".into()));
        };
        let context = frame.device_context();
        // SAFETY: both textures live on the capture device and have the same size and format.
        unsafe { context.CopyResource(&scaler.copy, frame.as_raw_texture()) };
        scaler.converter.convert(&scaler.copy, &scaler.output, 0)?;
        // SAFETY: same device, same size and NV12 format on both sides.
        unsafe { context.CopyResource(&scaler.readback, &scaler.output) };

        let output = scaler.converter.output();
        let mut picture = AVFrame::new();
        picture.set_width(output.width as i32);
        picture.set_height(output.height as i32);
        picture.set_format(ffi::AV_PIX_FMT_NV12);
        picture
            .alloc_buffer()
            .map_err(|error| EngineError::Capture(error.to_string()))?;

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: the staging texture is CPU-readable and mapped once, then unmapped below.
        unsafe { context.Map(&scaler.readback, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) }
            .map_err(|error| EngineError::Capture(error.message()))?;
        let pitch = mapped.RowPitch as usize;
        let width = output.width as usize;
        let rows = output.height as usize;
        for (plane, plane_rows, offset) in [(0, rows, 0), (1, rows / 2, pitch * rows)] {
            let stride = picture.linesize[plane] as usize;
            for row in 0..plane_rows {
                // SAFETY: the mapped NV12 surface holds `rows` luma rows then `rows / 2` chroma
                // rows of `pitch` bytes; the frame planes were allocated for this size.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        mapped.pData.cast::<u8>().add(offset + row * pitch),
                        picture.data[plane].add(row * stride),
                        width,
                    );
                }
            }
        }
        // SAFETY: matches the successful Map above.
        unsafe { context.Unmap(&scaler.readback, 0) };
        encode_jpeg(&picture, output.width as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_fits_the_box_and_keeps_the_aspect_ratio() {
        assert_eq!(preview_size(Size::new(2560, 1440)), Size::new(384, 216));
        assert_eq!(preview_size(Size::new(5120, 1440)), Size::new(384, 108));
        assert_eq!(preview_size(Size::new(1080, 1920)), Size::new(122, 216));
    }

    #[test]
    fn preview_never_upscales_a_small_window() {
        assert_eq!(preview_size(Size::new(300, 150)), Size::new(300, 150));
        assert_eq!(preview_size(Size::new(1, 1)), Size::new(2, 2));
    }
}
