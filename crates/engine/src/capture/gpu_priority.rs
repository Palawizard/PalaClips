use windows::Wdk::Graphics::Direct3D::{
    D3DKMT_SCHEDULINGPRIORITYCLASS_NORMAL, D3DKMT_SCHEDULINGPRIORITYCLASS_REALTIME,
    D3DKMTSetProcessSchedulingPriorityClass,
};
use windows::Win32::System::Threading::GetCurrentProcess;

use crate::config::GpuPriority;

pub fn set_gpu_priority(priority: GpuPriority) -> bool {
    let class = match priority {
        GpuPriority::Game => D3DKMT_SCHEDULINGPRIORITYCLASS_NORMAL,
        GpuPriority::Capture => D3DKMT_SCHEDULINGPRIORITYCLASS_REALTIME,
    };
    // SAFETY: the pseudo handle of the current process is always valid.
    let status = unsafe { D3DKMTSetProcessSchedulingPriorityClass(GetCurrentProcess(), class) };
    if status.is_ok() {
        tracing::info!(?priority, "priorité GPU appliquée");
    } else {
        tracing::warn!(
            ?priority,
            status = format!("{:#010x}", status.0),
            "priorité GPU refusée"
        );
    }
    status.is_ok()
}
