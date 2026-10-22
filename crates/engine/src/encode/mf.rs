use std::ffi::c_void;
use std::mem::ManuallyDrop;

use rsmpeg::avcodec::AVCodecContext;
use rsmpeg::ffi;
use windows::Win32::Media::MediaFoundation::{
    CODECAPI_AVEncNumWorkerThreads, ICodecAPI, IMFTransform,
};
use windows::Win32::System::Memory::{
    MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_NOACCESS, VirtualQuery,
};
use windows::Win32::System::Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_UI4};
use windows::core::Interface;

use super::convert::Size;
use super::threads::module_of;

const ENCODER_MODULE: &str = "mfh264enc.dll";

pub fn mf_worker_threads(size: Size, fps: u32) -> u32 {
    if u64::from(size.width) * u64::from(size.height) * u64::from(fps) <= 1920 * 1080 * 60 {
        2
    } else {
        4
    }
}

pub fn limit_mf_workers(context: &mut AVCodecContext, workers: u32) -> bool {
    let Some(api) = codec_api(context) else {
        tracing::warn!("encodeur H.264 de Windows introuvable : nombre de fils par défaut");
        return false;
    };
    let value = VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_UI4,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 { ulVal: workers },
            }),
        },
    };
    // The MFT reads AVEncNumWorkerThreads only when its types are set, so the same types are set again.
    // SAFETY: `api` is a live ICodecAPI of the Microsoft H.264 encoder, borrowed for this call.
    let applied = unsafe {
        api.SetValue(&CODECAPI_AVEncNumWorkerThreads, &value)
            .is_ok()
            && api.cast::<IMFTransform>().is_ok_and(|transform| {
                let output = transform.GetOutputCurrentType(0);
                let input = transform.GetInputCurrentType(0);
                matches!((output, input), (Ok(output), Ok(input))
                    if transform.SetOutputType(0, &output, 0).is_ok()
                        && transform.SetInputType(0, &input, 0).is_ok())
            })
    };
    if applied {
        tracing::debug!(workers, "encodeur H.264 de Windows limité");
    } else {
        tracing::warn!(
            workers,
            "nombre de fils de l'encodeur H.264 de Windows refusé"
        );
    }
    applied
}

fn codec_api(context: &mut AVCodecContext) -> Option<ICodecAPI> {
    // SAFETY: priv_data is the MFContext allocated with this h264_mf codec context; av_opt_find
    // only reads its AVClass option table.
    let option = unsafe {
        let private = (*context.as_mut_ptr()).priv_data;
        let option = ffi::av_opt_find(private, c"rate_control".as_ptr(), std::ptr::null(), 0, 0);
        option
            .as_ref()
            .map(|option| (private, option.offset as usize))
    };
    let (private, offset) = option?;
    // In FFmpeg's MFContext the ICodecAPI pointer is the field right before `opt_enc_rc`,
    // the field behind "rate_control" (https://github.com/FFmpeg/FFmpeg/blob/n8.1/libavcodec/mfenc.c).
    let field = offset.checked_sub(size_of::<*mut c_void>())?;
    // SAFETY: `field` lies inside the MFContext, which starts at `private` and extends past `offset`.
    let pointer = unsafe { private.cast::<u8>().add(field).cast::<*mut c_void>().read() };
    if !readable(pointer) {
        return None;
    }
    // SAFETY: `pointer` was checked to be readable; a COM object starts with its vtable pointer.
    let vtable = unsafe { pointer.cast::<*mut c_void>().read() };
    if !readable(vtable) || module_of(vtable).as_deref() != Some(ENCODER_MODULE) {
        return None;
    }
    // SAFETY: the pointer is a live COM object implemented by the Microsoft H.264 encoder.
    unsafe { ICodecAPI::from_raw_borrowed(&pointer) }.cloned()
}

fn readable(address: *mut c_void) -> bool {
    if address.is_null() || !(address as usize).is_multiple_of(align_of::<*mut c_void>()) {
        return false;
    }
    let mut info = MEMORY_BASIC_INFORMATION::default();
    // SAFETY: VirtualQuery only describes the address space into `info`.
    let filled = unsafe {
        VirtualQuery(
            Some(address),
            &mut info,
            size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };
    filled != 0 && info.State == MEM_COMMIT && info.Protect != PAGE_NOACCESS && info.Protect.0 != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_workers_up_to_1080p60_then_four() {
        assert_eq!(mf_worker_threads(Size::new(1280, 720), 30), 2);
        assert_eq!(mf_worker_threads(Size::new(1920, 1080), 60), 2);
        assert_eq!(mf_worker_threads(Size::new(2560, 1440), 60), 4);
    }

    #[test]
    fn unreadable_addresses_are_rejected() {
        assert!(!readable(std::ptr::null_mut()));
        assert!(!readable(8usize as *mut c_void));
        let value = 0u64;
        assert!(readable((&raw const value).cast_mut().cast()));
    }
}
