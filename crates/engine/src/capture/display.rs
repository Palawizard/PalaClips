use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
    DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_DEVICE_INFO_TYPE,
    DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SDR_WHITE_LEVEL, DISPLAYCONFIG_SOURCE_DEVICE_NAME, DisplayConfigGetDeviceInfo,
    GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
};
use windows::Win32::Foundation::ERROR_SUCCESS;

const SCRGB_REFERENCE_NITS: f32 = 80.0;
const ADVANCED_COLOR_ENABLED: u32 = 0b10;

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

pub fn display_color(device_name: &str) -> Option<DisplayColor> {
    let path = active_paths()?.into_iter().find(|path| {
        let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
            header: header::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>(
                DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                path.sourceInfo.adapterId,
                path.sourceInfo.id,
            ),
            ..Default::default()
        };
        query(&mut source.header)
            && String::from_utf16_lossy(&source.viewGdiDeviceName)
                .trim_end_matches('\0')
                .eq_ignore_ascii_case(device_name)
    })?;

    let target = (path.targetInfo.adapterId, path.targetInfo.id);
    let mut color = DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO {
        header: header::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>(
            DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
            target.0,
            target.1,
        ),
        ..Default::default()
    };
    let mut white = DISPLAYCONFIG_SDR_WHITE_LEVEL {
        header: header::<DISPLAYCONFIG_SDR_WHITE_LEVEL>(
            DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL,
            target.0,
            target.1,
        ),
        SDRWhiteLevel: 1_000,
    };
    if !query(&mut color.header) {
        return None;
    }
    // SAFETY: reading the bitfield union written by DisplayConfigGetDeviceInfo.
    let flags = unsafe { color.Anonymous.value };
    let hdr = flags & ADVANCED_COLOR_ENABLED != 0;
    let sdr_white_nits = if query(&mut white.header) {
        white.SDRWhiteLevel as f32 * SCRGB_REFERENCE_NITS / 1_000.0
    } else {
        SCRGB_REFERENCE_NITS
    };
    Some(DisplayColor {
        hdr,
        sdr_white_nits,
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
