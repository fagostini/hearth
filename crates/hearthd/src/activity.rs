//! Input-activity timestamps from /dev/input/event*. Only the time of the last
//! event is kept; event codes and contents are dropped as soon as they're read.
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone, Default)]
pub struct Activity {
    last: Arc<Mutex<Option<Instant>>>,
    open: Arc<Mutex<HashSet<PathBuf>>>,
}

impl Activity {
    /// Time since the last input event, or `None` if no device could be read (unknown).
    pub fn input_age(&self, now: Instant) -> Option<Duration> {
        if self.open.lock().unwrap().is_empty() {
            return None;
        }
        // `last` is set when the first device opens, so "no event yet" ages from then.
        self.last.lock().unwrap().map(|t| now - t)
    }

    /// Open new devices every 30 s (hot-plug). ponytail: all seats' devices;
    /// filter by udev ID_SEAT if multi-seat PCs ever matter.
    pub async fn watch(self) {
        loop {
            for (path, dev) in evdev::enumerate() {
                let props = dev.properties();
                if props.contains(evdev::PropType::ACCELEROMETER) {
                    continue; // streams events with nobody touching it
                }
                if !self.open.lock().unwrap().insert(path.clone()) {
                    continue;
                }
                self.last.lock().unwrap().get_or_insert_with(Instant::now);
                let me = self.clone();
                tokio::spawn(async move {
                    if let Ok(mut stream) = dev.into_event_stream() {
                        while let Ok(ev) = stream.next_event().await {
                            use evdev::EventType as T;
                            if matches!(ev.event_type(), T::KEY | T::RELATIVE | T::ABSOLUTE) {
                                *me.last.lock().unwrap() = Some(Instant::now());
                            }
                        }
                    }
                    me.open.lock().unwrap().remove(&path); // unplugged or unreadable
                });
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }
}
