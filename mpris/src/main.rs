pub extern crate mpris;
pub extern crate simple_osd_common as osd;

use std::sync::atomic::{AtomicBool, Ordering};
pub use std::sync::{Arc, Mutex};

use std::ops::Deref;

pub use osd::config::Config;
pub use osd::notify::{OSDContents, OSDProgressText, OSD};

use mpris::{PlaybackStatus, PlayerFinder};

pub use std::time::{Duration, SystemTime};

use std::vec::Vec;

fn format_duration(d: Duration) -> String {
    let s = d.as_secs();
    let secs = s % 60;
    let mins = s / 60;
    format!("{:02}:{:02}", mins, secs)
}

#[cfg(test)]
mod format_duration_tests {
    use super::*;
    #[test]
    fn seconds() {
        assert_eq!(&format_duration(Duration::from_secs(10)), "00:10");
    }
    #[test]
    fn minutes_seconds() {
        assert_eq!(&format_duration(Duration::from_secs(70)), "01:10");
    }
    #[test]
    fn many_minutes_seconds() {
        assert_eq!(&format_duration(Duration::from_secs(7210)), "120:10");
    }
}

fn format_artists(artists: Vec<&str>) -> Option<String> {
    let mut v = artists.clone();
    v.reverse();

    if v.len() < 2 {
        return Some(v.pop()?.to_string());
    }

    let mut s = String::new();

    for _ in 0..v.len() - 2 {
        s.push_str(v.pop()?);
        s.push_str(", ")
    }

    s.push_str(v.pop()?);

    s.push_str(" & ");

    s.push_str(v.pop()?);

    Some(s)
}

#[cfg(test)]
mod format_artists_test {
    use super::*;
    #[test]
    fn none() {
        assert_eq!(format_artists([].to_vec()), None);
    }
    #[test]
    fn one() {
        assert_eq!(
            format_artists(["John Doe"].to_vec()),
            Some("John Doe".to_string())
        );
    }
    #[test]
    fn two() {
        assert_eq!(
            format_artists(["John Doe", "Jane Doe"].to_vec()),
            Some("John Doe & Jane Doe".to_string())
        );
    }
    #[test]
    fn many() {
        assert_eq!(
            format_artists(["John Doe", "Jane Doe", "Chris P. Bacon", "Seymore Clevarge"].to_vec()),
            Some("John Doe, Jane Doe, Chris P. Bacon & Seymore Clevarge".to_string())
        );
    }
}

#[cfg(feature = "display_on_volume_changes")]
mod volume_changes {
    extern crate libpulse_binding as pulse;

    use super::*;

    use pulse::context::subscribe::{subscription_masks, Facility, Operation};
    use pulse::context::Context;
    use pulse::mainloop::standard::IterateResult;
    use pulse::mainloop::standard::Mainloop;

    pub(super) struct VolumeMonitor {
        mainloop: Arc<Mutex<Mainloop>>,
        #[allow(dead_code)]
        context: Arc<Mutex<Context>>,
    }

    impl VolumeMonitor {
        pub fn new(
            config: Arc<Mutex<Config>>,
            trigger: Arc<Mutex<SystemTime>>,
            dismissed: Arc<Mutex<AtomicBool>>,
        ) -> VolumeMonitor {
            let mainloop = Arc::new(Mutex::new(
                Mainloop::new().expect("Failed to create mainloop"),
            ));

            let context = Arc::new(Mutex::new(
                Context::new(mainloop.lock().unwrap().deref(), osd::APPNAME)
                    .expect("Failed to create new context"),
            ));

            context
                .lock()
                .unwrap()
                .connect(
                    config
                        .lock()
                        .unwrap()
                        .get::<String>("pulseaudio", "server")
                        .as_deref(),
                    0,
                    None,
                )
                .expect("Failed to connect context");

            // Wait for context to be ready
            loop {
                match mainloop.lock().unwrap().iterate(false) {
                    IterateResult::Quit(_) | IterateResult::Err(_) => {
                        panic!("Iterate state was not success, quitting...");
                    }
                    IterateResult::Success(_) => {}
                }
                match context.lock().unwrap().get_state() {
                    pulse::context::State::Ready => {
                        break;
                    }
                    pulse::context::State::Failed
                    | pulse::context::State::Unconnected
                    | pulse::context::State::Terminated => {
                        panic!("Context state failed/terminated, quitting...");
                    }
                    _ => {}
                }
            }

            context
                .lock()
                .unwrap()
                .subscribe(subscription_masks::SINK, |success| {
                    if !success {
                        eprintln!("failed to subscribe to events");
                        return;
                    }
                });

            let subscribe_callback = move |facility, operation, _index| {
                if facility == Some(Facility::Sink) && operation == Some(Operation::Changed) {
                    *trigger.lock().unwrap() = SystemTime::now();
                    dismissed.lock().unwrap().store(false, Ordering::Relaxed);
                }
            };

            context
                .lock()
                .unwrap()
                .set_subscribe_callback(Some(Box::new(subscribe_callback)));

            VolumeMonitor { mainloop, context }
        }
        pub fn tick(&self) {
            match self.mainloop.lock().unwrap().iterate(false) {
                IterateResult::Quit(_) | IterateResult::Err(_) => {
                    panic!("Iterate state was not success, quitting...");
                }
                IterateResult::Success(_) => {}
            }
        }
    }
}

fn main() {
    let config = Arc::new(Mutex::new(Config::new("mpris")));
    let mut osd = OSD::new();
    let mut waiting_on_close = false;
    let dismissed = Arc::new(Mutex::new(AtomicBool::new(false)));

    let player = PlayerFinder::new().unwrap().find_active().unwrap();

    let mut progress_tracker = player.track_progress(100).unwrap();

    let update_on_volume_change =
        config
            .lock()
            .unwrap()
            .get_default("default", "update on volume change", true);
    let timeout = config
        .lock()
        .unwrap()
        .get_default("default", "notification display time", 5);

    let trigger = Arc::new(Mutex::new(SystemTime::now()));

    #[cfg(feature = "display_on_volume_changes")]
    let vc = if update_on_volume_change {
        Some(volume_changes::VolumeMonitor::new(
            config.clone(),
            trigger.clone(),
            dismissed.clone(),
        ))
    } else {
        None
    };

    drop(config);

    let mut title;
    let mut old_title = "".to_string();
    let mut playback_status;
    let mut old_playback_status = PlaybackStatus::Stopped;

    loop {
        let progress = progress_tracker.tick().progress;

        title = progress.metadata().title().unwrap_or("Unknown").to_string();
        playback_status = progress.playback_status();

        if title != old_title || playback_status != old_playback_status {
            *trigger.lock().unwrap() = SystemTime::now();
            dismissed.lock().unwrap().store(false, Ordering::Relaxed);
        }

        let elapsed = trigger
            .lock()
            .unwrap()
            .elapsed()
            .unwrap_or_else(|_| Duration::from_secs(timeout + 1));

        if elapsed.as_secs() < timeout
            && playback_status != PlaybackStatus::Stopped
            && !dismissed.lock().unwrap().load(Ordering::Relaxed)
        {
            let metadata = progress.metadata();
            let artists = metadata
                .artists()
                .and_then(format_artists)
                .unwrap_or_else(|| "Unknown".to_string());
            let position = progress.position();
            let length = progress
                .length()
                .unwrap_or_else(|| Duration::from_secs(100000000));

            osd.title = Some(format!("{:?}: {} - {}", playback_status, title, artists));
            let ratio = position.as_secs_f32() / length.as_secs_f32();
            let text = format!(
                "{} / {}",
                format_duration(position),
                format_duration(length)
            );
            osd.contents = OSDContents::Progress(ratio, OSDProgressText::Text(Some(text)));
            osd.timeout = 1;
            osd.icon = match playback_status {
                PlaybackStatus::Playing => Some("media-playback-start".to_string()),
                PlaybackStatus::Paused => Some("media-playback-pause".to_string()),
                _ => None,
            };
            osd.update().unwrap();
            if !waiting_on_close {
                waiting_on_close = true;
                let dismissed_clone = dismissed.clone();
                osd.on_close(move || {
                    dismissed_clone
                        .lock()
                        .unwrap()
                        .store(true, Ordering::Relaxed);
                });
            }
        } else {
            waiting_on_close = false;
            osd.close()
        }

        old_title = title;
        old_playback_status = playback_status;

        #[cfg(feature = "display_on_volume_changes")]
        if let Some(v) = vc.as_ref() {
            v.tick()
        };
    }
}
