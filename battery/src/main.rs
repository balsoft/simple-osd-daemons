// This is free and unencumbered software released into the public domain.
// balsoft 2020

extern crate battery;
extern crate simple_osd_common as osd;

use std::io;
use std::thread;
use std::time::Duration;

use osd::notify::{OSD, Urgency};
use osd::config::Config;

#[derive(Debug, Eq, PartialEq)]
enum Threshold {
    Percentage(i32),
    Minutes(i32)
}

#[derive(Debug, Eq, PartialEq)]
enum State {
    Low,
    Critical,
    Charging,
    Normal
}

fn threshold_sane(thresh: Threshold) -> Option<Threshold> {
    match thresh {
        Threshold::Percentage(p) => {
            if p < 0 || p > 100 { return None; }
            Some(thresh)
        },
        Threshold::Minutes(m) => {
            if m < 0 { return None; }
            Some(thresh)
        }
    }
}

fn parse_threshold(thresh: String) -> Option<Threshold> {
    let mut s = thresh.clone();

    let last = s.pop();

    let parsed = s.parse();

    match last {
        Some('%') => parsed.map(Threshold::Percentage).ok().and_then(threshold_sane),
        Some('m') => parsed.map(Threshold::Minutes).ok().and_then(threshold_sane),
        _ => None
    }
}

#[cfg(test)]
mod parse_threshold_tests {
    use super::parse_threshold;
    use super::Threshold;
    #[test]
    fn parses_percentage() {
        assert_eq!(parse_threshold("15%".to_string()), Some(Threshold::Percentage(15)));
    }
    #[test]
    fn parses_minutes() {
        assert_eq!(parse_threshold("10m".to_string()), Some(Threshold::Minutes(10)));
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
        s.push_str("-");
        d = -d;
    }
    let hours = d / 3600;
    let minutes = (d % 3600) / 60;
    let seconds = d % 60;


    if hours > 0 {
        s.push_str(&format!("{}h", hours));
    }
    if minutes > 0 {
        if hours > 0 { s.push_str(" "); }
        s.push_str(&format!("{}m", minutes));
    }
    if seconds > 0 {
        if hours > 0 || minutes > 0 { s.push_str(" "); }
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

fn main() -> battery::Result<()> {
    let mut config = Config::new("battery");

    let low_threshold_str = config.get_default("threshold", "low", String::from("30m"));
    let critical_threshold_str = config.get_default("threshold", "critical", String::from("10m"));

    let low_threshold = parse_threshold(low_threshold_str).expect("Low threshold is incorrect: must be either a percentage or minutes");
    let critical_threshold = parse_threshold(critical_threshold_str).expect("Critical threshold is incorrect: must be either a percentage or minutes");

    let refresh_interval = config.get_default("default", "refresh interval", 30);

    let mut osd = OSD::new();
    osd.icon = Some(String::from("battery"));

    let manager = battery::Manager::new()?;
    let mut battery = match manager.batteries()?.next() {
        Some(Ok(battery)) => battery,
        Some(Err(e)) => {
            eprintln!("Unable to access battery information");
            return Err(e);
        }
        None => {
            eprintln!("Unable to find any batteries");
            return Err(io::Error::from(io::ErrorKind::NotFound).into());
        }
    };

    let mut state: State;
    let mut last_state: State = State::Normal;


    loop {
        state = match battery.state() {
            battery::State::Charging => State::Charging,
            battery::State::Full => State::Normal,
            _ => {
                let soc = (battery.state_of_charge().value * 100.) as i32;
                let tte = battery.time_to_empty().map(|q| q.value).unwrap_or(0.) as i32 / 60;
                println!("{:?}, {:?}", soc, tte);
                let low = match low_threshold {
                    Threshold::Percentage(p) => if soc <= p { State::Low } else { State::Normal },
                    Threshold::Minutes(m) => if tte <= m { State::Low } else { State::Normal }
                };
                match critical_threshold {
                    Threshold::Percentage(p) => if soc <= p { State::Critical } else { low },
                    Threshold::Minutes(m) => if tte <= m { State::Critical } else { low }
                }
            }
        };

        if state != last_state {
            match state {
                State::Charging => {
                    battery.time_to_full().map(|ttf| {
                        osd.title = Some(format!("Charging, {} until full", format_duration(ttf.value)));
                        osd.urgency = Urgency::Low;
                        osd.update();
                    });
                }
                State::Low => {
                    battery.time_to_empty().map(|tte| {
                        osd.title = Some(format!("Low battery, {} remaining", format_duration(tte.value)));
                        osd.urgency = Urgency::Normal;
                        osd.update();
                    });
                },
                State::Normal | State::Critical => { }
            }
        }

        if state == State::Critical {
            battery.time_to_empty().map(|tte| {
                osd.title = Some(format!("Critically low battery, {} remaining", format_duration(tte.value)));
                osd.urgency = Urgency::Critical;
                osd.update();
            });
        }

        thread::sleep(Duration::from_secs(refresh_interval));
        manager.refresh(&mut battery)?;
        last_state = state;
    }
}
