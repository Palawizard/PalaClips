use rsmpeg::avutil::AVFrame;
use rsmpeg::ffi;
use rsmpeg::swscale::SwsContext;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_NV12};

use super::convert::{GpuConverter, Size, fit_rect};
use super::d3d11::{ffmpeg_error, frame_texture, gpu_error, render_texture, staging_texture};
use super::video::VideoEncoder;
use crate::error::{EngineError, Result};

#[derive(Debug)]
pub enum FrameConverter {
    Hardware(GpuConverter),
    System(SystemConverter),
}

impl FrameConverter {
    pub fn new(
        device: &ID3D11Device,
        input: Size,
        output: Size,
        fps: u32,
        encoder: &VideoEncoder,
    ) -> Result<Self> {
        if encoder.is_hardware() {
            GpuConverter::new(device, input, output, fps).map(Self::Hardware)
        } else {
            SystemConverter::new(device, input, output, fps, encoder.pixel_format())
                .map(Self::System)
        }
    }

    pub fn input(&self) -> Size {
        match self {
            Self::Hardware(converter) => converter.input(),
            Self::System(converter) => converter.input,
        }
    }

    pub fn convert(&mut self, source: &ID3D11Texture2D, frame: &mut AVFrame) -> Result<()> {
        match self {
            Self::Hardware(converter) => {
                let (texture, slice) = frame_texture(frame)
                    .ok_or_else(|| EngineError::Encoder("image matérielle sans texture".into()))?;
                converter.convert(source, texture, slice)
            }
            Self::System(converter) => converter.convert(source, frame),
        }
    }
}

enum Readback {
    VideoProcessor {
        converter: GpuConverter,
        nv12: ID3D11Texture2D,
        staging: ID3D11Texture2D,
    },
    Bgra {
        staging: ID3D11Texture2D,
    },
}

pub struct SystemConverter {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    readback: Readback,
    scaler: SwsContext,
    input: Size,
    output: Size,
    format: ffi::AVPixelFormat,
    letterbox: Option<(i32, i32, Size)>,
}

impl std::fmt::Debug for SystemConverter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemConverter")
            .field("input", &self.input)
            .field("output", &self.output)
            .field("video_processor", &self.uses_video_processor())
            .finish_non_exhaustive()
    }
}

impl SystemConverter {
    pub fn new(
        device: &ID3D11Device,
        input: Size,
        output: Size,
        fps: u32,
        format: ffi::AVPixelFormat,
    ) -> Result<Self> {
        match video_processor_readback(device, input, output, fps) {
            Ok(readback) => Self::build(device, readback, input, output, format),
            Err(error) => {
                tracing::info!(%error, "pas de Video Processor : conversion des images par le processeur");
                Self::without_video_processor(device, input, output, format)
            }
        }
    }

    fn without_video_processor(
        device: &ID3D11Device,
        input: Size,
        output: Size,
        format: ffi::AVPixelFormat,
    ) -> Result<Self> {
        let staging = staging_texture(device, input, DXGI_FORMAT_B8G8R8A8_UNORM)?;
        Self::build(device, Readback::Bgra { staging }, input, output, format)
    }

    fn build(
        device: &ID3D11Device,
        readback: Readback,
        input: Size,
        output: Size,
        format: ffi::AVPixelFormat,
    ) -> Result<Self> {
        // SAFETY: GetImmediateContext has no preconditions on a valid device.
        let context = unsafe { device.GetImmediateContext() }.map_err(gpu_error)?;
        let (scaler, letterbox) = match &readback {
            Readback::VideoProcessor { .. } => {
                (scaler(output, ffi::AV_PIX_FMT_NV12, output, format)?, None)
            }
            Readback::Bgra { .. } => {
                let letterbox = letterbox(input, output);
                let size = letterbox.map_or(output, |(_, _, size)| size);
                let mut scaler = scaler(input, ffi::AV_PIX_FMT_BGRA, size, format)?;
                bt709_limited_from_full_rgb(&mut scaler);
                (scaler, letterbox)
            }
        };
        Ok(Self {
            device: device.clone(),
            context,
            readback,
            scaler,
            input,
            output,
            format,
            letterbox,
        })
    }

    pub fn uses_video_processor(&self) -> bool {
        matches!(self.readback, Readback::VideoProcessor { .. })
    }

    pub fn convert(&mut self, source: &ID3D11Texture2D, frame: &mut AVFrame) -> Result<()> {
        match self.read_into(source, frame) {
            Err(error) if self.uses_video_processor() => {
                tracing::warn!(%error, "Video Processor en échec : conversion des images par le processeur");
                *self = Self::without_video_processor(
                    &self.device,
                    self.input,
                    self.output,
                    self.format,
                )?;
                self.read_into(source, frame)
            }
            outcome => outcome,
        }
    }

    fn read_into(&mut self, source: &ID3D11Texture2D, frame: &mut AVFrame) -> Result<()> {
        let staging = match &self.readback {
            Readback::VideoProcessor {
                converter,
                nv12,
                staging,
            } => {
                converter.convert(source, nv12, 0)?;
                // SAFETY: both NV12 textures live on this device and have the same size and format.
                unsafe { self.context.CopyResource(staging, nv12) };
                staging.clone()
            }
            Readback::Bgra { staging } => {
                // SAFETY: the staging texture was created with the source's size and BGRA8 format.
                unsafe { self.context.CopyResource(staging, source) };
                staging.clone()
            }
        };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: the staging texture is CPU-readable; Map waits for the copy and fills `mapped`.
        unsafe {
            self.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
        }
        .map_err(gpu_error)?;
        let result = self.scale(&mapped, frame);
        // SAFETY: subresource 0 was mapped just above and is unmapped exactly once.
        unsafe { self.context.Unmap(&staging, 0) };
        result
    }

    fn scale(&mut self, mapped: &D3D11_MAPPED_SUBRESOURCE, frame: &mut AVFrame) -> Result<()> {
        let pitch = mapped.RowPitch as i32;
        let base = mapped.pData.cast::<u8>().cast_const();
        let (sources, height): ([*const u8; 4], i32) = match self.readback {
            Readback::VideoProcessor { .. } => (
                [
                    base,
                    // SAFETY: an NV12 staging texture stores its interleaved chroma plane right
                    // after `height` rows of luma, inside the same mapping.
                    unsafe { base.add(mapped.RowPitch as usize * self.output.height as usize) },
                    std::ptr::null(),
                    std::ptr::null(),
                ],
                self.output.height as i32,
            ),
            Readback::Bgra { .. } => (
                [base, std::ptr::null(), std::ptr::null(), std::ptr::null()],
                self.input.height as i32,
            ),
        };
        let strides = [pitch, pitch, 0, 0];
        let mut destinations = frame.data;
        if let Some((left, top, _)) = self.letterbox {
            fill_black(frame, self.format)?;
            destinations = offset_planes(frame, self.format, left, top);
        }
        // SAFETY: `sources`/`strides` describe the mapped staging image, valid until Unmap;
        // `destinations` point inside the frame's own buffers with its line sizes.
        unsafe {
            self.scaler.scale(
                sources.as_ptr(),
                strides.as_ptr(),
                0,
                height,
                destinations.as_ptr(),
                frame.linesize.as_ptr(),
            )
        }
        .map_err(ffmpeg_error)
    }
}

fn letterbox(input: Size, output: Size) -> Option<(i32, i32, Size)> {
    let rect = fit_rect(input, output);
    let (left, top) = (rect.left.max(0) & !1, rect.top.max(0) & !1);
    let size = Size::new(
        ((rect.right - rect.left) as u32).min(output.width - left as u32) & !1,
        ((rect.bottom - rect.top) as u32).min(output.height - top as u32) & !1,
    );
    ((left, top, size) != (0, 0, output)).then_some((left, top, size))
}

fn video_processor_readback(
    device: &ID3D11Device,
    input: Size,
    output: Size,
    fps: u32,
) -> Result<Readback> {
    Ok(Readback::VideoProcessor {
        converter: GpuConverter::new(device, input, output, fps)?,
        nv12: render_texture(device, output, DXGI_FORMAT_NV12)?,
        staging: staging_texture(device, output, DXGI_FORMAT_NV12)?,
    })
}

fn scaler(
    input: Size,
    input_format: ffi::AVPixelFormat,
    output: Size,
    output_format: ffi::AVPixelFormat,
) -> Result<SwsContext> {
    SwsContext::get_context(
        input.width as i32,
        input.height as i32,
        input_format,
        output.width as i32,
        output.height as i32,
        output_format,
        ffi::SWS_BILINEAR,
        None,
        None,
        None,
    )
    .ok_or_else(|| EngineError::Encoder("conversion d'image par le processeur impossible".into()))
}

fn bt709_limited_from_full_rgb(scaler: &mut SwsContext) {
    // SAFETY: the context is valid; sws_getCoefficients returns a static table.
    unsafe {
        let bt709 = ffi::sws_getCoefficients(ffi::SWS_CS_ITU709 as i32);
        ffi::sws_setColorspaceDetails(scaler.as_mut_ptr(), bt709, 1, bt709, 0, 0, 1 << 16, 1 << 16);
    }
}

fn fill_black(frame: &mut AVFrame, format: ffi::AVPixelFormat) -> Result<()> {
    let linesizes = frame.linesize.map(|size| size as isize);
    // SAFETY: the frame owns writable buffers of its width, height and format.
    let status = unsafe {
        ffi::av_image_fill_black(
            frame.data.as_ptr(),
            linesizes.as_ptr(),
            format,
            ffi::AVCOL_RANGE_MPEG,
            frame.width,
            frame.height,
        )
    };
    if status < 0 {
        return Err(EngineError::Encoder("image noire impossible".into()));
    }
    Ok(())
}

fn offset_planes(frame: &AVFrame, format: ffi::AVPixelFormat, left: i32, top: i32) -> [*mut u8; 8] {
    let mut planes = frame.data;
    let offset = |plane: usize, x: i32, y: i32| (y * frame.linesize[plane] + x) as usize;
    // SAFETY: `left`/`top` lie inside the frame (even values within the output size), so every
    // offset stays inside its plane.
    unsafe {
        planes[0] = planes[0].add(offset(0, left, top));
        if format == ffi::AV_PIX_FMT_NV12 {
            planes[1] = planes[1].add(offset(1, left, top / 2));
        } else {
            planes[1] = planes[1].add(offset(1, left / 2, top / 2));
            planes[2] = planes[2].add(offset(2, left / 2, top / 2));
        }
    }
    planes
}

#[cfg(test)]
mod tests {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_WARP;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_SDK_VERSION,
        D3D11CreateDevice,
    };

    use super::*;

    const RED_BT709_LIMITED: (u8, u8, u8) = (63, 102, 240);

    fn warp_device() -> ID3D11Device {
        let mut device = None;
        for flags in [
            D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        ] {
            // SAFETY: all out-pointers are valid for the duration of the call.
            let created = unsafe {
                D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_WARP,
                    HMODULE::default(),
                    flags,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    None,
                    None,
                )
            };
            if created.is_ok() {
                break;
            }
        }
        device.expect("WARP is always available")
    }

    fn solid_bgra(device: &ID3D11Device, size: Size, bgra: [u8; 4]) -> ID3D11Texture2D {
        let texture = render_texture(device, size, DXGI_FORMAT_B8G8R8A8_UNORM).expect("texture");
        let pixels = bgra.repeat((size.width * size.height) as usize);
        // SAFETY: the buffer holds exactly width × height BGRA pixels with the given row pitch.
        unsafe {
            device
                .GetImmediateContext()
                .expect("context")
                .UpdateSubresource(&texture, 0, None, pixels.as_ptr().cast(), size.width * 4, 0);
        }
        texture
    }

    fn centre_yuv(frame: &AVFrame) -> (u8, u8, u8) {
        let (x, y) = (frame.width as usize / 2, frame.height as usize / 2);
        let at = |plane: usize, column: usize, row: usize| {
            // SAFETY: the coordinates lie inside the plane of an allocated frame.
            unsafe { *frame.data[plane].add(row * frame.linesize[plane] as usize + column) }
        };
        if frame.format == ffi::AV_PIX_FMT_NV12 {
            (
                at(0, x, y),
                at(1, x & !1, y / 2),
                at(1, (x & !1) + 1, y / 2),
            )
        } else {
            (at(0, x, y), at(1, x / 2, y / 2), at(2, x / 2, y / 2))
        }
    }

    fn convert_red(
        converter: &mut SystemConverter,
        output: Size,
        format: ffi::AVPixelFormat,
    ) -> (u8, u8, u8) {
        let source = solid_bgra(&converter.device.clone(), converter.input, [0, 0, 255, 255]);
        let mut frame = AVFrame::new();
        frame.set_width(output.width as i32);
        frame.set_height(output.height as i32);
        frame.set_format(format);
        frame.alloc_buffer().expect("frame");
        converter.convert(&source, &mut frame).expect("conversion");
        centre_yuv(&frame)
    }

    fn close_to(actual: (u8, u8, u8), expected: (u8, u8, u8)) -> bool {
        let near = |a: u8, b: u8| a.abs_diff(b) <= 3;
        near(actual.0, expected.0) && near(actual.1, expected.1) && near(actual.2, expected.2)
    }

    #[test]
    fn processor_conversion_writes_bt709_limited_colours_in_both_layouts() {
        let device = warp_device();
        let (input, output) = (Size::new(128, 72), Size::new(64, 36));
        for format in [ffi::AV_PIX_FMT_NV12, ffi::AV_PIX_FMT_YUV420P] {
            let mut converter =
                SystemConverter::without_video_processor(&device, input, output, format)
                    .expect("converter");
            let yuv = convert_red(&mut converter, output, format);
            assert!(close_to(yuv, RED_BT709_LIMITED), "{format:?}: {yuv:?}");
        }
    }

    #[test]
    fn letterboxed_processor_conversion_keeps_black_bars() {
        let device = warp_device();
        let (input, output) = (Size::new(64, 64), Size::new(128, 64));
        let mut converter =
            SystemConverter::without_video_processor(&device, input, output, ffi::AV_PIX_FMT_NV12)
                .expect("converter");
        let source = solid_bgra(&device, input, [0, 0, 255, 255]);
        let mut frame = AVFrame::new();
        frame.set_width(128);
        frame.set_height(64);
        frame.set_format(ffi::AV_PIX_FMT_NV12);
        frame.alloc_buffer().expect("frame");
        converter.convert(&source, &mut frame).expect("conversion");
        // SAFETY: column 2 of row 32 lies inside the luma plane.
        let bar = unsafe { *frame.data[0].add(32 * frame.linesize[0] as usize + 2) };
        assert_eq!(bar, 16);
        assert!(close_to(centre_yuv(&frame), RED_BT709_LIMITED));
    }

    #[test]
    fn video_processor_conversion_matches_when_the_device_has_one() {
        let device = warp_device();
        let (input, output) = (Size::new(128, 72), Size::new(64, 36));
        let mut converter = SystemConverter::new(&device, input, output, 30, ffi::AV_PIX_FMT_NV12)
            .expect("converter");
        let yuv = convert_red(&mut converter, output, ffi::AV_PIX_FMT_NV12);
        assert!(close_to(yuv, RED_BT709_LIMITED), "{yuv:?}");
    }

    #[test]
    fn same_aspect_ratio_needs_no_letterbox() {
        assert_eq!(
            letterbox(Size::new(2560, 1440), Size::new(1920, 1080)),
            None
        );
        assert_eq!(letterbox(Size::new(1280, 720), Size::new(1280, 720)), None);
    }

    #[test]
    fn letterbox_is_centred_on_even_pixels_and_stays_inside() {
        let (left, top, size) =
            letterbox(Size::new(1280, 1024), Size::new(1920, 1080)).expect("4:5 into 16:9");
        assert_eq!((left, top), (284, 0));
        assert_eq!(size, Size::new(1350, 1080));
        assert!(left as u32 + size.width <= 1920);
        let (left, top, size) = letterbox(Size::new(1921, 1081), Size::new(1280, 720)).unwrap_or((
            0,
            0,
            Size::new(1280, 720),
        ));
        assert_eq!(left % 2, 0);
        assert_eq!(top % 2, 0);
        assert!(size.width <= 1280 && size.height <= 720);
    }
}
