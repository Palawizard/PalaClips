use windows::Wdk::Graphics::Direct3D::{
    D3DKMT_SCHEDULINGPRIORITYCLASS_REALTIME, D3DKMTSetProcessSchedulingPriorityClass,
};
use windows::Win32::System::Threading::GetCurrentProcess;

pub fn raise_gpu_priority() -> bool {
    // SAFETY: the pseudo handle of the current process is always valid.
    let status = unsafe {
        D3DKMTSetProcessSchedulingPriorityClass(
            GetCurrentProcess(),
            D3DKMT_SCHEDULINGPRIORITYCLASS_REALTIME,
        )
    };
    if status.is_ok() {
        tracing::info!("priorité GPU temps réel");
    } else {
        tracing::warn!(
            status = format!("{:#010x}", status.0),
            "priorité GPU temps réel refusée"
        );
    }
    status.is_ok()
}
