use std::mem::ManuallyDrop;

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_TEX2D_ARRAY_VPOV, D3D11_TEX2D_VPIV, D3D11_TEX2D_VPOV, D3D11_TEXTURE2D_DESC,
    D3D11_VIDEO_COLOR, D3D11_VIDEO_COLOR_0, D3D11_VIDEO_COLOR_RGBA,
    D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE, D3D11_VIDEO_PROCESSOR_CONTENT_DESC,
    D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC, D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0,
    D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC, D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0,
    D3D11_VIDEO_PROCESSOR_STREAM, D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
    D3D11_VPIV_DIMENSION_TEXTURE2D, D3D11_VPOV_DIMENSION_TEXTURE2D,
    D3D11_VPOV_DIMENSION_TEXTURE2DARRAY, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    ID3D11VideoContext1, ID3D11VideoDevice, ID3D11VideoProcessor, ID3D11VideoProcessorEnumerator,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709, DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709,
    DXGI_RATIONAL,
};
use windows::core::Interface;

use super::d3d11::gpu_error;
use crate::config::OutputResolution;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn of(texture: &ID3D11Texture2D) -> Self {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: GetDesc only writes into `desc`.
        unsafe { texture.GetDesc(&mut desc) };
        Self::new(desc.Width, desc.Height)
    }
}

pub fn output_size(input: Size, resolution: OutputResolution) -> Size {
    let even = |value: f64| ((value / 2.0).round() as u32).max(1) * 2;
    match resolution {
        OutputResolution::Native => Size::new(even(input.width as f64), even(input.height as f64)),
        OutputResolution::Height { height } => {
            let height = height.min(input.height.max(1));
            let width = input.width as f64 * height as f64 / input.height.max(1) as f64;
            Size::new(even(width), even(height as f64))
        }
    }
}

pub fn fit_rect(input: Size, output: Size) -> RECT {
    let scale = f64::min(
        output.width as f64 / input.width.max(1) as f64,
        output.height as f64 / input.height.max(1) as f64,
    );
    let width = (input.width as f64 * scale).round() as i32;
    let height = (input.height as f64 * scale).round() as i32;
    let left = (output.width as i32 - width) / 2;
    let top = (output.height as i32 - height) / 2;
    RECT {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
}

#[derive(Debug)]
pub struct GpuConverter {
    video_device: ID3D11VideoDevice,
    video_context: ID3D11VideoContext1,
    enumerator: ID3D11VideoProcessorEnumerator,
    processor: ID3D11VideoProcessor,
    input: Size,
    output: Size,
}

impl GpuConverter {
    pub fn new(device: &ID3D11Device, input: Size, output: Size, fps: u32) -> Result<Self> {
        let video_device: ID3D11VideoDevice = device.cast().map_err(gpu_error)?;
        // SAFETY: GetImmediateContext has no preconditions on a valid device.
        let context: ID3D11DeviceContext =
            unsafe { device.GetImmediateContext() }.map_err(gpu_error)?;
        let video_context: ID3D11VideoContext1 = context.cast().map_err(gpu_error)?;
        let rate = DXGI_RATIONAL {
            Numerator: fps,
            Denominator: 1,
        };
        let content = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
            InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
            InputFrameRate: rate,
            InputWidth: input.width,
            InputHeight: input.height,
            OutputFrameRate: rate,
            OutputWidth: output.width,
            OutputHeight: output.height,
            Usage: D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
        };
        // SAFETY: `content` is a fully initialised descriptor living across the calls.
        let (enumerator, processor) = unsafe {
            let enumerator = video_device
                .CreateVideoProcessorEnumerator(&content)
                .map_err(gpu_error)?;
            let processor = video_device
                .CreateVideoProcessor(&enumerator, 0)
                .map_err(gpu_error)?;
            (enumerator, processor)
        };
        let black = D3D11_VIDEO_COLOR {
            Anonymous: D3D11_VIDEO_COLOR_0 {
                RGBA: D3D11_VIDEO_COLOR_RGBA {
                    R: 0.0,
                    G: 0.0,
                    B: 0.0,
                    A: 1.0,
                },
            },
        };
        let source = RECT {
            left: 0,
            top: 0,
            right: input.width as i32,
            bottom: input.height as i32,
        };
        let destination = fit_rect(input, output);
        // SAFETY: state setters on a processor created from this video context's device.
        unsafe {
            video_context.VideoProcessorSetStreamFrameFormat(
                &processor,
                0,
                D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
            );
            video_context.VideoProcessorSetStreamAutoProcessingMode(&processor, 0, false);
            video_context.VideoProcessorSetStreamColorSpace1(
                &processor,
                0,
                DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709,
            );
            video_context.VideoProcessorSetOutputColorSpace1(
                &processor,
                DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709,
            );
            video_context.VideoProcessorSetOutputBackgroundColor(&processor, false, &black);
            video_context.VideoProcessorSetStreamSourceRect(&processor, 0, true, Some(&source));
            video_context.VideoProcessorSetStreamDestRect(&processor, 0, true, Some(&destination));
        }
        Ok(Self {
            video_device,
            video_context,
            enumerator,
            processor,
            input,
            output,
        })
    }

    pub fn input(&self) -> Size {
        self.input
    }

    pub fn output(&self) -> Size {
        self.output
    }

    pub fn convert(
        &self,
        input: &ID3D11Texture2D,
        output: &ID3D11Texture2D,
        output_index: u32,
    ) -> Result<()> {
        let input_desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
            FourCC: 0,
            ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPIV {
                    MipSlice: 0,
                    ArraySlice: 0,
                },
            },
        };
        let mut output_texture_desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: GetDesc only writes into the descriptor.
        unsafe { output.GetDesc(&mut output_texture_desc) };
        let output_desc = if output_texture_desc.ArraySize > 1 {
            D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
                ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2DARRAY,
                Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 {
                    Texture2DArray: D3D11_TEX2D_ARRAY_VPOV {
                        MipSlice: 0,
                        FirstArraySlice: output_index,
                        ArraySize: 1,
                    },
                },
            }
        } else {
            D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
                ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
                Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 {
                    Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 },
                },
            }
        };
        let mut input_view = None;
        let mut output_view = None;
        // SAFETY: both textures belong to the device of `video_device`; descriptors outlive the calls.
        unsafe {
            self.video_device
                .CreateVideoProcessorInputView(
                    input,
                    &self.enumerator,
                    &input_desc,
                    Some(&mut input_view),
                )
                .map_err(gpu_error)?;
            self.video_device
                .CreateVideoProcessorOutputView(
                    output,
                    &self.enumerator,
                    &output_desc,
                    Some(&mut output_view),
                )
                .map_err(gpu_error)?;
        }
        let mut stream = D3D11_VIDEO_PROCESSOR_STREAM {
            Enable: true.into(),
            pInputSurface: ManuallyDrop::new(input_view),
            ..Default::default()
        };
        // SAFETY: the stream references a live input view; the output view is alive for the call.
        let result = unsafe {
            self.video_context.VideoProcessorBlt(
                &self.processor,
                output_view.as_ref(),
                0,
                std::slice::from_ref(&stream),
            )
        };
        // SAFETY: the input view was moved into the stream above and is released exactly once here.
        unsafe { ManuallyDrop::drop(&mut stream.pInputSurface) };
        result.map_err(gpu_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_output_keeps_size_rounded_to_even() {
        assert_eq!(
            output_size(Size::new(2560, 1440), OutputResolution::Native),
            Size::new(2560, 1440)
        );
        assert_eq!(
            output_size(Size::new(1273, 711), OutputResolution::Native),
            Size::new(1274, 712)
        );
    }

    #[test]
    fn scaled_output_keeps_aspect_ratio() {
        assert_eq!(
            output_size(
                Size::new(2560, 1440),
                OutputResolution::Height { height: 1080 }
            ),
            Size::new(1920, 1080)
        );
        assert_eq!(
            output_size(
                Size::new(3440, 1440),
                OutputResolution::Height { height: 720 }
            ),
            Size::new(1720, 720)
        );
    }

    #[test]
    fn scaled_output_never_exceeds_the_source() {
        assert_eq!(
            output_size(
                Size::new(2560, 1440),
                OutputResolution::Height { height: 2160 }
            ),
            Size::new(2560, 1440)
        );
        assert_eq!(
            output_size(
                Size::new(1920, 1080),
                OutputResolution::Height { height: 1081 }
            ),
            Size::new(1920, 1080)
        );
        assert_eq!(
            output_size(
                Size::new(2560, 1440),
                OutputResolution::Height { height: 1439 }
            ),
            Size::new(2558, 1440)
        );
    }

    #[test]
    fn fit_rect_letterboxes_a_new_aspect_ratio() {
        let rect = fit_rect(Size::new(1280, 1024), Size::new(1920, 1080));
        assert_eq!(
            (rect.left, rect.top, rect.right, rect.bottom),
            (285, 0, 1635, 1080)
        );
        let same = fit_rect(Size::new(2560, 1440), Size::new(1920, 1080));
        assert_eq!(
            (same.left, same.top, same.right, same.bottom),
            (0, 0, 1920, 1080)
        );
    }
}
