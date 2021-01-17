// This is free and unencumbered software released into the public domain.
// balsoft 2020

extern crate simple_osd_common as osd;
extern crate sysfs_class;

use osd::config::Config;
use osd::notify::{OSDContents, OSDProgressText, OSD};
use std::path::PathBuf;
use log;
use sysfs_class::{Backlight, Brightness, SysClass};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BrightnessError {
    #[error("Failed to initialite backlight (possibly invalid backend): {0:?}")]
    BacklightInitError(std::io::Error),
    #[error("Failed to get maximum brightness: {0:?}")]
    BacklightMaxBrightnessError(std::io::Error),
    #[error("Failed to get brightness: {0:?}")]
    BacklightBrightnessError(std::io::Error),
}

fn brightness_daemon() -> Result<(), BrightnessError> {
    let mut config = Config::new("brightness");

    let refresh_interval = config.get_default("default", "refresh interval", 1);

    let backend = config.get_default(
        "default",
        "backlight backend",
        String::from("intel_backlight"),
    );

    let brightness: Backlight = SysClass::from_path(&PathBuf::from(backend))
        .map_err(BrightnessError::BacklightInitError)?;

    let m = brightness
        .max_brightness()
        .map(|b| b as f32)
        .map_err(BrightnessError::BacklightMaxBrightnessError)?;

    let mut osd = OSD::new();
    osd.title = Some(String::from("Screen brightness"));

    let mut b: f32;

    let mut last_b: f32 = 0.;

    loop {
        b = brightness
            .brightness()
            .map(|b| b as f32)
            .map_err(BrightnessError::BacklightBrightnessError)?;

        if (b - last_b).abs() > 0.1 {
            osd.contents = OSDContents::Progress(b / m, OSDProgressText::Percentage);
            osd.update().unwrap();
        }

        last_b = b;

        std::thread::sleep(std::time::Duration::from_secs(refresh_interval))
    }
}

fn main() {
    match brightness_daemon() {
        Ok(_) => { },
        Err(err) => {
            error!(err);
            std::process::exit(1);
        }
    }
}
