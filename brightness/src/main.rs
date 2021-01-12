// This is free and unencumbered software released into the public domain.
// balsoft 2020

extern crate backlight;
extern crate simple_osd_common as osd;

use osd::config::Config;
use osd::notify::{OSD, OSDContents, OSDProgressText};

use backlight::Brightness;

fn main() {
    let mut config = Config::new("brightness");

    let refresh_interval = config.get_default("default", "refresh interval", 1);

    let brightness = Brightness::default();

    let m = brightness.get_max_brightness().unwrap() as f32;

    let mut osd = OSD::new();
    osd.title = Some(String::from("Screen brightness"));

    let mut b : f32;

    let mut last_b : f32 = 0.;

    loop {
        b = brightness.get_brightness().unwrap() as f32;

        if (b - last_b).abs() > 0.1 {
            osd.contents = OSDContents::Progress(b/m, OSDProgressText::Percentage);
            osd.update().unwrap();
        }

        last_b = b;

        std::thread::sleep(std::time::Duration::from_secs(refresh_interval))
    }
}
