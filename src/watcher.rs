use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub struct JournalWatcher {
    _watcher: RecommendedWatcher,
    rx: mpsc::Receiver<()>,
}

impl JournalWatcher {
    pub fn new(journal_path: &Path) -> Option<Self> {
        let (tx, rx) = mpsc::channel();
        let mut last_event = Instant::now();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) => {
                            let now = Instant::now();
                            if now.duration_since(last_event) > Duration::from_millis(500) {
                                last_event = now;
                                let _ = tx.send(());
                            }
                        }
                        _ => {}
                    }
                }
            },
            notify::Config::default(),
        )
        .ok()?;

        let watch_dir = journal_path.parent().unwrap_or(Path::new("."));
        watcher.watch(watch_dir, RecursiveMode::Recursive).ok()?;

        Some(Self {
            _watcher: watcher,
            rx,
        })
    }

    pub fn has_changes(&self) -> bool {
        self.rx.try_recv().is_ok()
    }
}
