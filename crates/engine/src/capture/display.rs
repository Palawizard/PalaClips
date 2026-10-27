use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
    DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
    DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_DEVICE_INFO_HEADER,
    DISPLAYCONFIG_DEVICE_INFO_TYPE, DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_MODE_INFO,
    DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SDR_WHITE_LEVEL, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
    DISPLAYCONFIG_TARGET_DEVICE_NAME, DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes,
    QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::Graphics::Gdi::{DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW};
use windows::core::HSTRING;

const SCRGB_REFERENCE_NITS: f32 = 80.0;
const ADVANCED_COLOR_ENABLED: u32 = 0b10;
const WIDE_COLOR_ENFORCED: u32 = 0b100;
const GET_ADVANCED_COLOR_INFO_2: DISPLAYCONFIG_DEVICE_INFO_TYPE =
    DISPLAYCONFIG_DEVICE_INFO_TYPE(15);
pub const ADVANCED_COLOR_MODE_HDR: i32 = 2;

#[repr(C)]
#[derive(Default)]
struct AdvancedColorInfo2Request {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    value: u32,
    color_encoding: i32,
    bits_per_color_channel: u32,
    active_color_mode: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdvancedColorInfo2 {
    pub flags: u32,
    pub bits_per_color_channel: u32,
    pub active_color_mode: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayColorInfo {
    pub legacy_flags: Option<u32>,
    pub info2: Option<AdvancedColorInfo2>,
    pub sdr_white_level: Option<u32>,
}

impl DisplayColorInfo {
    pub fn hdr(&self) -> bool {
        match (self.info2, self.legacy_flags) {
            (Some(info2), _) => info2.active_color_mode == ADVANCED_COLOR_MODE_HDR,
            (None, Some(flags)) => {
                flags & ADVANCED_COLOR_ENABLED != 0 && flags & WIDE_COLOR_ENFORCED == 0
            }
            (None, None) => false,
        }
    }

    pub fn sdr_white_nits(&self) -> f32 {
        self.sdr_white_level.map_or(SCRGB_REFERENCE_NITS, |level| {
            level as f32 * SCRGB_REFERENCE_NITS / 1_000.0
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayColor {
    pub hdr: bool,
    pub sdr_white_nits: f32,
}

impl DisplayColor {
    pub fn sdr_white_scrgb(&self) -> f32 {
        self.sdr_white_nits / SCRGB_REFERENCE_NITS
    }
}

pub fn refresh_hz(device_name: &str) -> Option<u32> {
    let mut mode = DEVMODEW {
        dmSize: size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    // SAFETY: the device name is a valid wide string and the mode is sized for the call.
    unsafe {
        EnumDisplaySettingsW(
            &HSTRING::from(device_name),
            ENUM_CURRENT_SETTINGS,
            &mut mode,
        )
    }
    .as_bool()
    .then_some(mode.dmDisplayFrequency)
    .filter(|hz| *hz > 1)
}

pub fn display_color(device_name: &str) -> Option<DisplayColor> {
    let info = display_color_info(device_name)?;
    if info.legacy_flags.is_none() && info.info2.is_none() {
        return None;
    }
    Some(DisplayColor {
        hdr: info.hdr(),
        sdr_white_nits: info.sdr_white_nits(),
    })
}

pub fn monitor_path(device_name: &str) -> Option<String> {
    active_paths()?
        .iter()
        .find(|path| source_name(path).is_some_and(|name| name.eq_ignore_ascii_case(device_name)))
        .and_then(target_path)
}

pub fn device_name_of(monitor_path: &str) -> Option<String> {
    active_paths()?
        .iter()
        .find(|path| {
            target_path(path).is_some_and(|target| target.eq_ignore_ascii_case(monitor_path))
        })
        .and_then(source_name)
}

fn source_name(path: &DISPLAYCONFIG_PATH_INFO) -> Option<String> {
    let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
        header: header::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>(
            DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
            path.sourceInfo.adapterId,
            path.sourceInfo.id,
        ),
        ..Default::default()
    };
    query(&mut source.header).then(|| wide_string(&source.viewGdiDeviceName))
}

fn target_path(path: &DISPLAYCONFIG_PATH_INFO) -> Option<String> {
    let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME {
        header: header::<DISPLAYCONFIG_TARGET_DEVICE_NAME>(
            DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
            path.targetInfo.adapterId,
            path.targetInfo.id,
        ),
        ..Default::default()
    };
    query(&mut target.header)
        .then(|| wide_string(&target.monitorDevicePath))
        .filter(|path| !path.is_empty())
}

fn wide_string(text: &[u16]) -> String {
    String::from_utf16_lossy(text)
        .trim_end_matches('\0')
        .to_owned()
}

pub fn display_color_info(device_name: &str) -> Option<DisplayColorInfo> {
    let path = active_paths()?.into_iter().find(|path| {
        source_name(path).is_some_and(|name| name.eq_ignore_ascii_case(device_name))
    })?;

    let (adapter, id) = (path.targetInfo.adapterId, path.targetInfo.id);
    let mut legacy = DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO {
        header: header::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>(
            DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
            adapter,
            id,
        ),
        ..Default::default()
    };
    let mut info2 = AdvancedColorInfo2Request {
        header: header::<AdvancedColorInfo2Request>(GET_ADVANCED_COLOR_INFO_2, adapter, id),
        ..Default::default()
    };
    let mut white = DISPLAYCONFIG_SDR_WHITE_LEVEL {
        header: header::<DISPLAYCONFIG_SDR_WHITE_LEVEL>(
            DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL,
            adapter,
            id,
        ),
        SDRWhiteLevel: 1_000,
    };
    Some(DisplayColorInfo {
        // SAFETY: reading the bitfield union written by DisplayConfigGetDeviceInfo.
        legacy_flags: query(&mut legacy.header).then_some(unsafe { legacy.Anonymous.value }),
        info2: query(&mut info2.header).then_some(AdvancedColorInfo2 {
            flags: info2.value,
            bits_per_color_channel: info2.bits_per_color_channel,
            active_color_mode: info2.active_color_mode,
        }),
        sdr_white_level: query(&mut white.header).then_some(white.SDRWhiteLevel),
    })
}

fn active_paths() -> Option<Vec<DISPLAYCONFIG_PATH_INFO>> {
    let (mut path_count, mut mode_count) = (0, 0);
    // SAFETY: both counters are valid out-pointers.
    let sized = unsafe {
        GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
    };
    if sized != ERROR_SUCCESS {
        return None;
    }
    let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
    let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
    // SAFETY: the arrays hold exactly the element counts passed alongside them.
    let queried = unsafe {
        QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            None,
        )
    };
    if queried != ERROR_SUCCESS {
        return None;
    }
    paths.truncate(path_count as usize);
    Some(paths)
}

fn header<T>(
    kind: DISPLAYCONFIG_DEVICE_INFO_TYPE,
    adapter: windows::Win32::Foundation::LUID,
    id: u32,
) -> DISPLAYCONFIG_DEVICE_INFO_HEADER {
    DISPLAYCONFIG_DEVICE_INFO_HEADER {
        r#type: kind,
        size: size_of::<T>() as u32,
        adapterId: adapter,
        id,
    }
}

fn query(header: &mut DISPLAYCONFIG_DEVICE_INFO_HEADER) -> bool {
    // SAFETY: `header` is the first field of a request struct whose size it declares.
    unsafe { DisplayConfigGetDeviceInfo(header) == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(legacy: Option<u32>, mode: Option<i32>) -> DisplayColorInfo {
        DisplayColorInfo {
            legacy_flags: legacy,
            info2: mode.map(|active_color_mode| AdvancedColorInfo2 {
                flags: 0,
                bits_per_color_channel: 8,
                active_color_mode,
            }),
            sdr_white_level: None,
        }
    }

    #[test]
    fn hdr_follows_active_color_mode_when_available() {
        assert!(info(Some(0b0011), Some(ADVANCED_COLOR_MODE_HDR)).hdr());
        assert!(!info(Some(0b0111), Some(1)).hdr());
        assert!(!info(Some(0b0011), Some(0)).hdr());
    }

    #[test]
    fn legacy_fallback_ignores_auto_color_management() {
        assert!(info(Some(0b0011), None).hdr());
        assert!(!info(Some(0b0111), None).hdr());
        assert!(!info(Some(0b0001), None).hdr());
        assert!(!info(None, None).hdr());
    }

    #[test]
    fn sdr_white_defaults_to_scrgb_reference() {
        assert_eq!(info(None, None).sdr_white_nits(), 80.0);
        let bright = DisplayColorInfo {
            sdr_white_level: Some(2_500),
            ..info(None, None)
        };
        assert_eq!(bright.sdr_white_nits(), 200.0);
    }
}
