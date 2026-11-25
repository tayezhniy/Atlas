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

    /// Возвращает следующее событие. Обрабатывает все накопившиеся события,
    /// возвращая первое значимое (create/modify/delete/rename).
    pub fn next_event(&self) -> Option<WatchEvent> {
        for event in self.rx.try_iter() {
            let Ok(e) = event else { continue };
            match e.kind {
                EventKind::Create(_) => {
                    if let Some(p) = e.paths.first() {
                        return Some(WatchEvent::Created(p.clone()));
                    }
                }
                EventKind::Modify(notify::event::ModifyKind::Name(notify::event::RenameMode::Both)) => {
                    if e.paths.len() == 2 {
                        return Some(WatchEvent::Renamed(e.paths[0].clone(), e.paths[1].clone()));
                    }
                }
                EventKind::Modify(_) => {
                    if let Some(p) = e.paths.first() {
                        return Some(WatchEvent::Modified(p.clone()));
                    }
                }
                EventKind::Remove(_) => {
                    if let Some(p) = e.paths.first() {
                        return Some(WatchEvent::Deleted(p.clone()));
                    }
                }
                _ => {}
            }
        }
        None
    }
}