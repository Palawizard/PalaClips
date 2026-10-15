use std::ffi::CStr;

use rsmpeg::avcodec::{AVCodec, AVCodecContext};
use rsmpeg::avutil::ra;
use rsmpeg::ffi;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareEncoder {
    pub name: String,
    pub description: String,
    pub opens: bool,
}

pub fn ffmpeg_version() -> String {
    // SAFETY: av_version_info returns a static NUL-terminated string owned by libavutil.
    unsafe { CStr::from_ptr(ffi::av_version_info()) }
        .to_string_lossy()
        .into_owned()
}

pub fn hardware_video_encoders() -> Vec<HardwareEncoder> {
    AVCodec::iterate()
        .filter(|codec| is_hardware_video_encoder(codec))
        .map(|codec| HardwareEncoder {
            name: codec.name().to_string_lossy().into_owned(),
            description: codec.long_name().to_string_lossy().into_owned(),
            opens: opens_with_nv12_720p(&codec),
        })
        .collect()
}

fn is_hardware_video_encoder(codec: &AVCodec) -> bool {
    // SAFETY: the codec comes from av_codec_iterate and lives for the whole program.
    let is_encoder = unsafe { ffi::av_codec_is_encoder(codec.as_ptr()) } != 0;
    let hardware =
        (codec.capabilities as u32) & (ffi::AV_CODEC_CAP_HARDWARE | ffi::AV_CODEC_CAP_HYBRID) != 0;
    is_encoder && codec.type_ == ffi::AVMEDIA_TYPE_VIDEO && hardware
}

fn opens_with_nv12_720p(codec: &AVCodec) -> bool {
    let mut context = AVCodecContext::new(codec);
    context.set_width(1280);
    context.set_height(720);
    context.set_pix_fmt(ffi::AV_PIX_FMT_NV12);
    context.set_time_base(ra(1, 60));
    context.set_framerate(ra(60, 1));
    context.open(None).is_ok()
}
