use std::path::Path;

use rsmpeg::avcodec::{AVCodec, AVCodecContext, AVCodecParameters, AVPacket};
use rsmpeg::avutil::{AVFrame, ra};
use rsmpeg::ffi;
use rsmpeg::swscale::SwsContext;

use super::d3d11::ffmpeg_error;
use crate::error::{EngineError, Result};
use crate::replay::packet_data;

pub const THUMBNAIL_WIDTH: i32 = 480;

pub fn write_thumbnail(
    codec_parameters: &AVCodecParameters,
    keyframe: &AVPacket,
    path: &Path,
) -> Result<()> {
    let picture = decode(codec_parameters, keyframe)?;
    let jpeg = encode_jpeg(&picture, THUMBNAIL_WIDTH)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, jpeg)?;
    Ok(())
}

pub fn encode_jpeg(picture: &AVFrame, width: i32) -> Result<Vec<u8>> {
    let height = (picture.height as i64 * width as i64 / picture.width.max(1) as i64) as i32 & !1;
    let mut scaled = AVFrame::new();
    scaled.set_width(width);
    scaled.set_height(height);
    scaled.set_format(ffi::AV_PIX_FMT_YUVJ420P);
    scaled.alloc_buffer().map_err(ffmpeg_error)?;
    let mut scaler = SwsContext::get_context(
        picture.width,
        picture.height,
        picture.format,
        width,
        height,
        ffi::AV_PIX_FMT_YUVJ420P,
        ffi::SWS_BILINEAR,
        None,
        None,
        None,
    )
    .ok_or_else(|| EngineError::Encoder("mise à l'échelle de la miniature impossible".into()))?;
    scaler
        .scale_frame(picture, 0, picture.height, &mut scaled)
        .map_err(ffmpeg_error)?;

    let codec = AVCodec::find_encoder(ffi::AV_CODEC_ID_MJPEG)
        .ok_or_else(|| EngineError::Encoder("encodeur JPEG absent de FFmpeg".into()))?;
    let mut encoder = AVCodecContext::new(&codec);
    encoder.set_width(width);
    encoder.set_height(height);
    encoder.set_pix_fmt(ffi::AV_PIX_FMT_YUVJ420P);
    encoder.set_time_base(ra(1, 1));
    encoder.open(None).map_err(ffmpeg_error)?;
    encoder.send_frame(Some(&scaled)).map_err(ffmpeg_error)?;
    encoder.send_frame(None).map_err(ffmpeg_error)?;
    let jpeg = encoder.receive_packet().map_err(ffmpeg_error)?;
    Ok(packet_data(&jpeg).to_vec())
}

fn decode(codec_parameters: &AVCodecParameters, keyframe: &AVPacket) -> Result<AVFrame> {
    let codec = AVCodec::find_decoder(codec_parameters.codec_id)
        .ok_or_else(|| EngineError::Encoder("décodeur absent pour la miniature".into()))?;
    let mut decoder = AVCodecContext::new(&codec);
    decoder
        .apply_codecpar(codec_parameters)
        .map_err(ffmpeg_error)?;
    decoder.open(None).map_err(ffmpeg_error)?;
    decoder.send_packet(Some(keyframe)).map_err(ffmpeg_error)?;
    decoder.send_packet(None).map_err(ffmpeg_error)?;
    decoder.receive_frame().map_err(ffmpeg_error)
}

pub fn encode_png_rgba(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let row = width as usize * 4;
    if pixels.len() != row * height as usize {
        return Err(EngineError::Encoder(
            "image RGBA de taille inattendue".into(),
        ));
    }
    let mut picture = AVFrame::new();
    picture.set_width(width as i32);
    picture.set_height(height as i32);
    picture.set_format(ffi::AV_PIX_FMT_RGBA);
    picture.alloc_buffer().map_err(ffmpeg_error)?;
    let stride = picture.linesize[0] as usize;
    for (index, source) in pixels.chunks_exact(row).enumerate() {
        // SAFETY: the frame was allocated for `height` rows of at least `row` bytes each.
        unsafe {
            std::ptr::copy_nonoverlapping(
                source.as_ptr(),
                picture.data[0].add(index * stride),
                row,
            );
        }
    }
    let codec = AVCodec::find_encoder(ffi::AV_CODEC_ID_PNG)
        .ok_or_else(|| EngineError::Encoder("encodeur PNG absent de FFmpeg".into()))?;
    let mut encoder = AVCodecContext::new(&codec);
    encoder.set_width(width as i32);
    encoder.set_height(height as i32);
    encoder.set_pix_fmt(ffi::AV_PIX_FMT_RGBA);
    encoder.set_time_base(ra(1, 1));
    encoder.open(None).map_err(ffmpeg_error)?;
    encoder.send_frame(Some(&picture)).map_err(ffmpeg_error)?;
    encoder.send_frame(None).map_err(ffmpeg_error)?;
    let png = encoder.receive_packet().map_err(ffmpeg_error)?;
    Ok(packet_data(&png).to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_encoder_writes_a_png_signature() {
        let pixels = [255u8, 0, 0, 128].repeat(4 * 4);
        let png = encode_png_rgba(&pixels, 4, 4).expect("png");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn png_encoder_refuses_a_wrong_buffer_size() {
        assert!(encode_png_rgba(&[0; 12], 4, 4).is_err());
    }
}
