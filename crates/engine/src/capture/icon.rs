use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDIBits, HBITMAP, HDC,
};
use windows::Win32::UI::Shell::SHDefExtractIconW;
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};
use windows::core::PCWSTR;

use crate::encode::encode_png_rgba;

pub const ICON_SIZE: u32 = 48;

pub fn app_icon_png(exe: &Path) -> Option<Vec<u8>> {
    let wide: Vec<u16> = exe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut icon = HICON::default();
    // SAFETY: `wide` is a NUL-terminated path that outlives the call; the out-pointer is valid.
    unsafe {
        SHDefExtractIconW(
            PCWSTR(wide.as_ptr()),
            0,
            0,
            Some(&mut icon),
            None,
            ICON_SIZE,
        )
    }
    .ok()
    .ok()?;
    let pixels = icon_rgba(icon);
    // SAFETY: the icon was created by SHDefExtractIconW above and is destroyed exactly once.
    let _ = unsafe { DestroyIcon(icon) };
    encode_png_rgba(&pixels?, ICON_SIZE, ICON_SIZE).ok()
}

fn icon_rgba(icon: HICON) -> Option<Vec<u8>> {
    let mut info = ICONINFO::default();
    // SAFETY: `icon` is a valid icon handle and `info` a valid out-pointer.
    unsafe { GetIconInfo(icon, &mut info) }.ok()?;
    // SAFETY: a memory DC compatible with the screen has no preconditions.
    let dc = unsafe { CreateCompatibleDC(None) };
    let color = bitmap_bgra(dc, info.hbmColor);
    let mask = bitmap_bgra(dc, info.hbmMask);
    // SAFETY: the DC and both bitmaps belong to this function and are released exactly once.
    unsafe {
        let _ = DeleteDC(dc);
        let _ = DeleteObject(info.hbmColor.into());
        let _ = DeleteObject(info.hbmMask.into());
    }
    let mut pixels = color?;
    let has_alpha = pixels.chunks_exact(4).any(|pixel| pixel[3] != 0);
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        pixel.swap(0, 2);
        if !has_alpha {
            let transparent = mask
                .as_ref()
                .and_then(|mask| mask.get(index * 4))
                .is_some_and(|value| *value != 0);
            pixel[3] = if transparent { 0 } else { 255 };
        }
    }
    Some(pixels)
}

fn bitmap_bgra(dc: HDC, bitmap: HBITMAP) -> Option<Vec<u8>> {
    if bitmap.is_invalid() {
        return None;
    }
    let mut header = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: ICON_SIZE as i32,
            biHeight: -(ICON_SIZE as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];
    // SAFETY: `pixels` holds ICON_SIZE rows of 32-bit pixels as described by `header`.
    let rows = unsafe {
        GetDIBits(
            dc,
            bitmap,
            0,
            ICON_SIZE,
            Some(pixels.as_mut_ptr().cast()),
            &mut header,
            DIB_RGB_COLORS,
        )
    };
    (rows > 0).then_some(pixels)
}
