extern crate battery;
extern crate simple_osd_common as osd;

use std::io;
use std::thread;
use std::time::Duration;

use osd::notify::{OSD, OSDContents, OSDProgressText, Urgency};
use osd::config::Config;

use battery::units::Time;

#[derive(Debug)]
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

fn parse_threshold(thresh: String) -> Option<Threshold> {
    let mut s = thresh.clone();

    let last = s.pop();

    match last {
        Some('%') => s.parse().map(Threshold::Percentage).ok(),
        Some('m') => s.parse().map(Threshold::Minutes).ok(),
        _ => None
    }
}

fn main() -> battery::Result<()> {
    let mut config = Config::new("battery");

    let mut low_threshold_str = config.get_default("threshold", "low", String::from("30m"));
    let mut critical_threshold_str = config.get_default("threshold", "critical", String::from("10m"));

    let low_threshold = parse_threshold(low_threshold_str).expect("Low threshold is incorrect: must be either a percentage or minutes");
    let critical_threshold = parse_threshold(critical_threshold_str).expect("Critical threshold is incorrect: must be either a percentage or minutes");

    let refresh_interval = config.get_default("default", "refresh interval", 30);

    println!("{:?}, {:?}", low_threshold, critical_threshold);

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
                        osd.title = Some(format!("Charging, {:?} until full", ttf));
                        osd.urgency = Urgency::Low;
                        osd.update();
                    });
                }
                State::Low => {
                    battery.time_to_empty().map(|tte| {
                        osd.title = Some(format!("Low battery, {:?} remaining", tte));
                        osd.urgency = Urgency::Normal;
                        osd.update();
                    });
                },
                State::Normal | State::Critical => { }
            }
        }

        if state == State::Critical {
            battery.time_to_empty().map(|tte| {
                osd.title = Some(format!("Critically low battery, {:?} remaining", tte));
                osd.urgency = Urgency::Critical;
                osd.update();
            });
        }

        thread::sleep(Duration::from_secs(refresh_interval));
        manager.refresh(&mut battery)?;
        last_state = state;
    }
}
