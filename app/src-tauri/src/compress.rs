use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use palaclips_engine::events::Localized;
use palaclips_engine::library::{
    CompressCodec, CompressPlan, CompressRequest, Quality, SourceInfo, compress, compressed_name,
    copy_file_to_clipboard, plan_compression, probe_source,
};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use crate::{EngineSlot, internal_error};

pub const COMPRESS_EVENT: &str = "compress-changed";
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "mov", "webm", "avi", "m4v", "wmv", "flv", "ts",
];
const PROGRESS_STEP: f64 = 0.01;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running {
        attempt: u32,
        fraction: f64,
    },
    Done {
        path: String,
        bytes: u64,
        attempts: u32,
    },
    Failed {
        error: Localized,
    },
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompressJob {
    id: u32,
    source: String,
    target_bytes: u64,
    codec: CompressCodec,
    plan: CompressPlan,
    #[serde(flatten)]
    state: JobState,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlanInput {
    pub path: String,
    pub target_bytes: u64,
    pub codec: CompressCodec,
    pub all_tracks: bool,
    #[serde(default)]
    pub start_secs: f64,
    #[serde(default)]
    pub end_secs: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompressInput {
    #[serde(flatten)]
    pub source: PlanInput,
    pub plan: CompressPlan,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProbeView {
    source: SourceInfo,
    thumbnail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanView {
    plan: CompressPlan,
    quality: Quality,
    estimated_bytes: u64,
    duration_secs: f64,
}

#[derive(Debug, Default)]
pub struct CompressQueue {
    jobs: Vec<CompressJob>,
    requests: HashMap<u32, CompressRequest>,
    running: Option<(u32, Arc<AtomicBool>)>,
    worker: bool,
    next: u32,
}

fn duration_of(source: &SourceInfo, input: &PlanInput) -> f64 {
    let end = input
        .end_secs
        .unwrap_or(source.duration_secs)
        .min(source.duration_secs);
    (end - input.start_secs.max(0.0)).max(0.0)
}

pub fn probe(path: &str) -> Result<ProbeView, Localized> {
    let path = PathBuf::from(path);
    let source = probe_source(&path)?;
    let thumbnail = palaclips_engine::paths::thumbnail_path(&path)
        .ok()
        .filter(|thumbnail| {
            thumbnail.exists()
                || palaclips_engine::library::thumbnail_from_file(&path, thumbnail).is_ok()
        })
        .map(|thumbnail| thumbnail.to_string_lossy().into_owned());
    Ok(ProbeView { source, thumbnail })
}

pub fn plan(input: &PlanInput) -> Result<PlanView, Localized> {
    let source = probe_source(&PathBuf::from(&input.path))?;
    let duration_secs = duration_of(&source, input);
    let plan = plan_compression(
        &source,
        input.target_bytes,
        input.codec,
        input.all_tracks,
        duration_secs,
    )?;
    Ok(view(plan, input.codec, duration_secs))
}

pub fn preview(plan: CompressPlan, codec: CompressCodec, duration_secs: f64) -> PlanView {
    view(plan, codec, duration_secs)
}

fn view(plan: CompressPlan, codec: CompressCodec, duration_secs: f64) -> PlanView {
    PlanView {
        quality: plan.quality(codec),
        estimated_bytes: plan.estimated_bytes(duration_secs),
        plan,
        duration_secs,
    }
}

impl EngineSlot {
    pub fn compress_jobs(&self) -> Vec<CompressJob> {
        self.compress
            .lock()
            .map(|queue| queue.jobs.clone())
            .unwrap_or_default()
    }

    pub fn compress_add(
        &self,
        app: &tauri::AppHandle,
        inputs: Vec<CompressInput>,
    ) -> Result<Vec<CompressJob>, Localized> {
        let mut queue = self.compress.lock().map_err(|_| internal_error())?;
        let mut added = Vec::new();
        for input in inputs {
            let source = PathBuf::from(&input.source.path);
            probe_source(&source)?;
            queue.next += 1;
            let id = queue.next;
            let request = CompressRequest {
                output: compressed_name(&source, input.source.target_bytes),
                source,
                target_bytes: input.source.target_bytes,
                codec: input.source.codec,
                all_tracks: input.source.all_tracks,
                start_secs: input.source.start_secs,
                end_secs: input.source.end_secs,
                plan: input.plan.clone(),
            };
            let job = CompressJob {
                id,
                source: input.source.path.clone(),
                target_bytes: input.source.target_bytes,
                codec: input.source.codec,
                plan: input.plan,
                state: JobState::Queued,
            };
            queue.requests.insert(id, request);
            queue.jobs.push(job.clone());
            added.push(job);
        }
        if !queue.worker && !added.is_empty() {
            queue.worker = true;
            let app = app.clone();
            std::thread::Builder::new()
                .name("palaclips-compress".into())
                .spawn(move || run_queue(&app))
                .map_err(|_| internal_error())?;
        }
        for job in &added {
            app.emit(COMPRESS_EVENT, job).ok();
        }
        Ok(added)
    }

    pub fn compress_cancel(&self, app: &tauri::AppHandle, id: u32) {
        let Ok(mut queue) = self.compress.lock() else {
            return;
        };
        if let Some((running, cancel)) = &queue.running
            && *running == id
        {
            cancel.store(true, Ordering::Relaxed);
            return;
        }
        queue.requests.remove(&id);
        if let Some(job) = queue
            .jobs
            .iter_mut()
            .find(|job| job.id == id && matches!(job.state, JobState::Queued))
        {
            job.state = JobState::Cancelled;
            app.emit(COMPRESS_EVENT, &*job).ok();
        }
    }

    pub fn compress_clear(&self) {
        if let Ok(mut queue) = self.compress.lock() {
            queue
                .jobs
                .retain(|job| matches!(job.state, JobState::Queued | JobState::Running { .. }));
        }
    }

    fn finished_output(&self, id: u32) -> Result<PathBuf, Localized> {
        let queue = self.compress.lock().map_err(|_| internal_error())?;
        queue
            .jobs
            .iter()
            .find_map(|job| match &job.state {
                JobState::Done { path, .. } if job.id == id => Some(PathBuf::from(path)),
                _ => None,
            })
            .ok_or_else(|| Localized::new("clip_unknown"))
    }

    pub fn compress_copy(&self, id: u32) -> Result<(), Localized> {
        Ok(copy_file_to_clipboard(&self.finished_output(id)?)?)
    }

    pub fn compress_reveal(&self, id: u32) -> Result<(), Localized> {
        crate::clips::reveal(&self.finished_output(id)?)
    }

    fn update_job(&self, app: &tauri::AppHandle, id: u32, state: JobState) {
        let Ok(mut queue) = self.compress.lock() else {
            return;
        };
        if let Some(job) = queue.jobs.iter_mut().find(|job| job.id == id) {
            job.state = state;
            app.emit(COMPRESS_EVENT, &*job).ok();
        }
    }

    fn next_request(&self) -> Option<(u32, CompressRequest, Arc<AtomicBool>)> {
        let mut queue = self.compress.lock().ok()?;
        let id = queue
            .jobs
            .iter()
            .find(|job| matches!(job.state, JobState::Queued))
            .map(|job| job.id);
        let Some(id) = id else {
            queue.worker = false;
            queue.running = None;
            return None;
        };
        let Some(request) = queue.requests.remove(&id) else {
            queue.worker = false;
            return None;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        queue.running = Some((id, cancel.clone()));
        Some((id, request, cancel))
    }
}

fn run_queue(app: &tauri::AppHandle) {
    loop {
        let Some(slot) = app.try_state::<EngineSlot>() else {
            return;
        };
        let Some((id, request, cancel)) = slot.next_request() else {
            return;
        };
        slot.update_job(
            app,
            id,
            JobState::Running {
                attempt: 1,
                fraction: 0.0,
            },
        );
        let encoders = crate::commands::cached_encoders(&slot).unwrap_or_default();
        let mut reported = (0, -1.0);
        let result = compress(
            &request,
            &encoders,
            &mut |step| {
                if step.attempt != reported.0 || step.fraction - reported.1 >= PROGRESS_STEP {
                    reported = (step.attempt, step.fraction);
                    slot.update_job(
                        app,
                        id,
                        JobState::Running {
                            attempt: step.attempt,
                            fraction: step.fraction,
                        },
                    );
                }
            },
            &cancel,
        );
        let state = match result {
            Ok(result) => {
                slot.library_changed(app, std::slice::from_ref(&request.output));
                JobState::Done {
                    path: result.path,
                    bytes: result.bytes,
                    attempts: result.attempts,
                }
            }
            Err(palaclips_engine::EngineError::CompressCancelled) => JobState::Cancelled,
            Err(error) => {
                tracing::warn!(%error, source = %request.source.display(), "compression impossible");
                JobState::Failed {
                    error: error.localized(),
                }
            }
        };
        slot.update_job(app, id, state);
    }
}
