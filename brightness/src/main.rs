// This is free and unencumbered software released into the public domain.
// balsoft 2020

extern crate simple_osd_common as osd;
extern crate sysfs_class;
#[macro_use]
extern crate log;

use osd::config::Config;
use osd::daemon::run;
use osd::notify::{OSDContents, OSDProgressText, OSD};
use std::path::PathBuf;
use sysfs_class::{Backlight, Brightness, SysClass};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BrightnessError {
    #[error("Failed to initialite backlight (possibly invalid backend): {0}")]
    BacklightInitError(std::io::Error),
    #[error("Failed to get maximum brightness: {0}")]
    MaxBrightnessError(std::io::Error),
    #[error("Failed to get brightness: {0}")]
    BrightnessError(std::io::Error),
    #[error("Failed to update a notification: {0}")]
    OSDUpdate(#[from] osd::notify::UpdateError),
}

fn brightness_daemon() -> Result<(), BrightnessError> {
    let mut osd = OSD::new();
    osd.title = Some(String::from("Screen brightness"));

    let mut config = Config::new("brightness");

    let refresh_interval = config.get_default("default", "refresh interval", 500);

    let backend = config.get_default(
        "default",
        "backlight backend",
        String::from("/var/lib/backlight/intel_backlight"),
    );

    let brightness: Backlight = SysClass::from_path(&PathBuf::from(backend))
        .map_err(BrightnessError::BacklightInitError)?;

    let m = brightness
        .max_brightness()
        .map(|b| b as f32)
        .map_err(BrightnessError::MaxBrightnessError)?;

    debug!("Maximum brightness: {0}", m);

    let mut b: f32;

    let mut last_b: f32 = 0.;

    loop {
        b = brightness
            .brightness()
            .map(|b| b as f32)
            .map_err(BrightnessError::BrightnessError)?;

        if (b - last_b).abs() > 0.1 {
            osd.icon = Some(String::from(if b / m < 0.33 {
                "display-brightness-low"
            } else if b / m < 0.66 {
                "display-brightness-medium"
            } else {
                "display-brightness-high"
            }));
            osd.contents = OSDContents::Progress(b / m, OSDProgressText::Percentage);
            osd.update()?;
        }

        last_b = b;

        std::thread::sleep(std::time::Duration::from_millis(refresh_interval))
    }
}

fn main() {
    run("simple-osd-brightness", brightness_daemon)
}
