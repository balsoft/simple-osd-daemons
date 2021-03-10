// This is free and unencumbered software released into the public domain.
// balsoft 2020

extern crate battery;
extern crate simple_osd_common as osd;
extern crate thiserror;
#[macro_use]
extern crate log;

use std::thread;
use std::time::Duration;

use osd::config::Config;
use osd::daemon::run;
use osd::notify::{Urgency, OSD};
use thiserror::Error;

#[derive(Debug, Eq, PartialEq)]
enum Threshold {
    Percentage(i32),
    Minutes(i32),
}

#[derive(Debug, Eq, PartialEq)]
enum State {
    Low,
    Critical,
    Charging,
    Normal,
}

fn threshold_sane(thresh: Threshold) -> Option<Threshold> {
    match thresh {
        Threshold::Percentage(p) => {
            if p < 0 || p > 100 {
                return None;
            }
            Some(thresh)
        }
        Threshold::Minutes(m) => {
            if m < 0 {
                return None;
            }
            Some(thresh)
        }
    }
}

fn parse_threshold(thresh: String) -> Option<Threshold> {
    let mut s = thresh;

    let last = s.pop();

    let parsed = s.parse();

    match last {
        Some('%') => parsed
            .map(Threshold::Percentage)
            .ok()
            .and_then(threshold_sane),
        Some('m') => parsed.map(Threshold::Minutes).ok().and_then(threshold_sane),
        _ => None,
    }
}

#[cfg(test)]
mod parse_threshold_tests {
    use super::parse_threshold;
    use super::Threshold;
    #[test]
    fn parses_percentage() {
        assert_eq!(
            parse_threshold("15%".to_string()),
            Some(Threshold::Percentage(15))
        );
    }
    #[test]
    fn parses_minutes() {
        assert_eq!(
            parse_threshold("10m".to_string()),
            Some(Threshold::Minutes(10))
        );
    }
    #[test]
    fn fails_on_incorrect_percentage() {
        assert_eq!(parse_threshold("foo%".to_string()), None);
    }
    #[test]
    fn fails_on_incorrect_minutes() {
        assert_eq!(parse_threshold("foom".to_string()), None);
    }
    #[test]
    fn fails_on_high_percentage() {
        assert_eq!(parse_threshold("110%".to_string()), None);
    }
    #[test]
    fn fails_on_negative_percentage() {
        assert_eq!(parse_threshold("-10%".to_string()), None);
    }
    #[test]
    fn fails_on_negative_minutes() {
        assert_eq!(parse_threshold("-10m".to_string()), None);
    }
}

fn format_duration(duration: f32) -> String {
    let mut d = duration as i32;
    if d == 0 {
        return "0s".to_string();
    }
    let mut s = String::new();
    if d < 0 {
        s.push('-');
        d = -d;
    }
    let hours = d / 3600;
    let minutes = (d % 3600) / 60;
    let seconds = d % 60;

    if hours > 0 {
        s.push_str(&format!("{}h", hours));
    }
    if minutes > 0 {
        if hours > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{}m", minutes));
    }
    if seconds > 0 {
        if hours > 0 || minutes > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{}s", seconds));
    }
    s
}

#[cfg(test)]
mod format_duration_tests {
    use super::format_duration;
    #[test]
    fn no_time() {
        assert_eq!(&format_duration(0.), "0s");
    }
    #[test]
    fn seconds() {
        assert_eq!(&format_duration(12.), "12s");
    }
    #[test]
    fn minutes_seconds() {
        assert_eq!(&format_duration(123.), "2m 3s");
    }
    #[test]
    fn minutes() {
        assert_eq!(&format_duration(120.), "2m");
    }
    #[test]
    fn hours_minutes_seconds() {
        assert_eq!(&format_duration(12345.), "3h 25m 45s");
    }
    #[test]
    fn hours_minutes() {
        assert_eq!(&format_duration(9000.), "2h 30m")
    }
    #[test]
    fn hours() {
        assert_eq!(&format_duration(3600.), "1h")
    }
    #[test]
    fn negative() {
        assert_eq!(&format_duration(-12345.), "-3h 25m 45s");
    }
}

#[derive(Error, Debug)]
enum BatteryError {
    #[error("Unable to access battery information")]
    BatteryInformationAccess(#[from] battery::errors::Error),
    #[error("No batteries detected")]
    NoBatteriesDetected,
    #[error("Failed to update a notification: {0}")]
    OSDUpdate(#[from] osd::notify::UpdateError),
    #[error("No time-to-empty estimation available")]
    TTEEstimationUnavailable,
}

fn battery_daemon() -> Result<(), BatteryError> {
    let mut config = Config::new("battery");

    let low_threshold_str = config.get_default("threshold", "low", String::from("30m"));
    let critical_threshold_str = config.get_default("threshold", "critical", String::from("10m"));

    let low_threshold = parse_threshold(low_threshold_str)
        .expect("Low threshold is incorrect: must be either a percentage or minutes");
    let critical_threshold = parse_threshold(critical_threshold_str)
        .expect("Critical threshold is incorrect: must be either a percentage or minutes");

    let show_battery_charge = config.get_default("default", "show battery charge", false);

    let refresh_interval = config.get_default("default", "refresh interval", 30);

    let mut osd = OSD::new();
    osd.icon = Some(String::from("battery"));

    let manager = battery::Manager::new()?;
    let mut battery = manager
        .batteries()?
        .next()
        .ok_or(BatteryError::NoBatteriesDetected)??;

    let mut state: State;
    let mut last_state: State = State::Normal;

    loop {
        let soc = (battery.state_of_charge().value * 100.) as i32;

        state = match battery.state() {
            battery::State::Charging => State::Charging,
            battery::State::Full => State::Normal,
            _ => {
                let tte = battery.time_to_empty().map(|q| q.value as i32 / 60);
                debug!("{:?}, {:?}", soc, tte);
                let low = match low_threshold {
                    Threshold::Percentage(p) if soc <= p => State::Low,
                    Threshold::Minutes(m) if tte.ok_or(BatteryError::TTEEstimationUnavailable)? <= m => State::Low,
                    Threshold::Percentage(_) | Threshold::Minutes(_) => State::Normal,
                };
                match critical_threshold {
                    Threshold::Percentage(p) if soc <= p => State::Critical,
                    Threshold::Minutes(m) if tte.ok_or(BatteryError::TTEEstimationUnavailable)? <= m => State::Critical,
                    Threshold::Percentage(_) | Threshold::Minutes(_) => low,
                }
            }
        };

        debug!("State: {:?}, Charge: {:#?}", state, battery.state_of_charge());

        if state != last_state {
            match state {
                State::Charging => {
                    osd.icon = if show_battery_charge {
                        let icon_name = format!("battery-{:03}-charging", (soc / 10) * 10);
                        Some(config.get_override("icons", icon_name.as_str()))
                    } else {
                        Some(config.get_override("icons", "battery-good-charging"))
                    };
                    osd.urgency = Urgency::Low;
                    osd.title = Some(
                        match battery.time_to_full() {
                            Some(ttf) => format!(
                                "Charging {}%, {} until full",
                                soc,
                                format_duration(ttf.value)
                            ),
                            None => {
                                warn!("No time-to-full estimation available");
                                format!("Charging {}%", soc)
                            },
                        });
                    osd.update()?;
                }
                State::Low => {
                    osd.icon = Some(config.get_override("icons", "battery-low"));
                    osd.urgency = Urgency::Normal;
                    osd.title = Some(
                        match battery.time_to_empty() {
                            Some(tte) => format!(
                                "Low battery {}%, {} remaining",
                                soc,
                                format_duration(tte.value)
                            ),
                            None => {
                                warn!("No time-to-empty estimation available");
                                format!("Low battery {}%", soc)
                            },
                        });
                    osd.update()?;
                }
                State::Normal if show_battery_charge => {
                    let icon_name = format!("battery-{:03}", (soc / 10) * 10);
                    osd.icon = Some(config.get_override("icons", icon_name.as_str()));
                    osd.urgency = Urgency::Normal;
                    osd.title = Some(
                        match battery.time_to_empty() {
                            Some(tte) => format!(
                                "Adapter disconnected, charge {}%, {} remaining",
                                soc,
                                format_duration(tte.value)
                            ),
                            None => {
                                warn!("No time-to-empty estimation available");
                                format!("Adapter disconnected, charge {}%", soc)
                            },
                        });
                    osd.update()?;
                },
                _ => {},
            }
        }

        if state == State::Critical {
            osd.icon = Some(config.get_override("icons", "battery-caution"));
            osd.urgency = Urgency::Critical;
            osd.title = Some(
                match battery.time_to_empty() {
                    Some(tte) => format!(
                        "Critically low battery {}%, {} remaining",
                        soc,
                        format_duration(tte.value)
                    ),
                    None => {
                        warn!("No time-to-empty estimation available");
                        format!("Critically low battery {}%", soc)
                    },
                });
            osd.update()?;
        }

        thread::sleep(Duration::from_secs(refresh_interval));
        manager.refresh(&mut battery)?;
        last_state = state;
    }
}

fn main() {
    run("simple-osd-battery", battery_daemon);
}
