use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossbeam_channel::{RecvTimeoutError, unbounded};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::error::{EngineError, Result};

const QUIET: Duration = Duration::from_millis(1_500);

pub struct LibraryWatch {
    _watcher: RecommendedWatcher,
}

impl std::fmt::Debug for LibraryWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryWatch").finish_non_exhaustive()
    }
}

pub fn watch(
    root: &Path,
    on_change: impl Fn(Vec<PathBuf>) + Send + 'static,
) -> Result<LibraryWatch> {
    std::fs::create_dir_all(root)?;
    let (changes, received) = unbounded::<PathBuf>();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Ok(event) = event {
            for path in event.paths {
                changes.send(path).ok();
            }
        }
    })
    .map_err(watch_error)?;
    watcher
        .watch(root, RecursiveMode::Recursive)
        .map_err(watch_error)?;
    std::thread::Builder::new()
        .name("palaclips-library-watch".into())
        .spawn(move || {
            while let Ok(first) = received.recv() {
                let mut batch = BTreeSet::from([first]);
                loop {
                    match received.recv_timeout(QUIET) {
                        Ok(path) => {
                            batch.insert(path);
                        }
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => {
                            on_change(batch.into_iter().collect());
                            return;
                        }
                    }
                }
                on_change(batch.into_iter().collect());
            }
        })?;
    Ok(LibraryWatch { _watcher: watcher })
}

fn watch_error(error: notify::Error) -> EngineError {
    EngineError::Library(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_arrive_together_once_the_folder_is_quiet() {
        let dir = std::env::temp_dir()
            .join("palaclips-watch-tests")
            .join(std::process::id().to_string());
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("dir");
        let (batches, received) = unbounded();
        let watch = watch(&dir, move |paths| {
            batches.send(paths).ok();
        })
        .expect("watch");
        std::fs::write(dir.join("a.mp4"), b"a").expect("write");
        std::fs::write(dir.join("b.mp4"), b"b").expect("write");
        let batch = received
            .recv_timeout(Duration::from_secs(10))
            .expect("one batch");
        assert!(
            batch.contains(&dir.join("a.mp4")) && batch.contains(&dir.join("b.mp4")),
            "{batch:?}"
        );
        drop(watch);
        std::fs::remove_dir_all(&dir).ok();
    }
}
