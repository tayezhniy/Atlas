use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::channel;

/// Событие наблюдателя за файлами.
#[derive(Debug, Clone)]
pub enum WatchEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
    Renamed(PathBuf, PathBuf),
}

/// Наблюдатель за изменениями файловой системы (раздел 4.3 ТЗ).
pub struct FileWatcher {
    watcher: Option<RecommendedWatcher>,
    rx: std::sync::mpsc::Receiver<notify::Result<Event>>,
}

impl FileWatcher {
    pub fn new() -> Result<Self, anyhow::Error> {
        let (tx, rx) = channel();
        let watcher = RecommendedWatcher::new(
            move |res: notify::Result<Event>| {
                let _ = tx.send(res);
            },
            notify::Config::default(),
        )?;
        Ok(Self {
            watcher: Some(watcher),
            rx,
        })
    }

    pub fn watch<P: Into<PathBuf>>(&mut self, path: P) -> Result<(), anyhow::Error> {
        if let Some(w) = &mut self.watcher {
            let path_buf: PathBuf = path.into();
            w.watch(path_buf.as_path(), RecursiveMode::Recursive)?;
        }
        Ok(())
    }

    pub fn next_event(&self) -> Option<WatchEvent> {
        let event = self.rx.try_iter().last()?;
        match event {
            Ok(e) => {
                match e.kind {
                    EventKind::Create(_) => {
                        for path in e.paths {
                            return Some(WatchEvent::Created(path));
                        }
                    }
                    EventKind::Modify(_) => {
                        for path in e.paths {
                            return Some(WatchEvent::Modified(path));
                        }
                    }
                    EventKind::Remove(_) => {
                        for path in e.paths {
                            return Some(WatchEvent::Deleted(path));
                        }
                    }
                    _ => {}
                }
                None
            }
            Err(_) => None,
        }
    }
}