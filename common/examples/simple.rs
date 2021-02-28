extern crate simple_osd_common as osd;
extern crate thiserror;
#[macro_use] extern crate log;

use osd::daemon::run;
use osd::config::Config;
use osd::notify::{OSDContents, OSDProgressText, Urgency, OSD};
use std::thread::sleep;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
enum SimpleError {
    #[error("Failed to update a notification: {0}")]
    OSDUpdate(#[from] osd::notify::UpdateError),
    #[error("Failed to close a notification: {0}")]
    OSDClose(#[from] osd::notify::CloseError),
    #[error("Failed to set a notification close callback: {0}")]
    OSDOnClose(#[from] osd::notify::CloseCallbackError),
}

fn simple_daemon() -> Result<(), SimpleError> {
    let mut config = Config::new("simple-example");

    let foo = config.get_default("example section", "foo", "bar baz".to_string());

    info!("Value of foo is {}", foo);

    let example_no_default =
        config.get::<i32>("example section", "example variable with no default");

    info!(
        "Value of example variable with no default is {:?}",
        example_no_default
    );

    let refresh_interval = config.get_default("default", "refresh interval", 1);

    let mut osd_simple = OSD::new();
    osd_simple.title = Some("Simple (but urgent) notification".to_string());
    osd_simple.contents = OSDContents::Simple(Some("Just simple contents".to_string()));
    osd_simple.urgency = Urgency::Critical;

    let mut percentage = 0.;

    let mut osd_progress_bar_percentage = OSD::new();
    osd_progress_bar_percentage.title =
        Some("A progress bar showing important percentage!".to_string());

    let eta = 15.;
    let mut elapsed = 0.;

    let mut osd_progress_bar_text = OSD::new();
    osd_progress_bar_text.title =
        Some("Nuclear warhead launch in progress, time left:".to_string());
    osd_progress_bar_text.urgency = Urgency::Low;

    loop {
        percentage = (percentage + 0.123) % 1.;

        elapsed = (elapsed + refresh_interval as f32) % eta;

        osd_progress_bar_percentage.contents =
            OSDContents::Progress(percentage, OSDProgressText::Percentage);

        osd_progress_bar_text.contents = OSDContents::Progress(
            elapsed / eta,
            OSDProgressText::Text(Some(format!("{}s / {}s", elapsed, eta))),
        );

        osd_simple.update()?;
        osd_progress_bar_percentage.update()?;
        osd_progress_bar_text.update()?;

        sleep(Duration::from_secs(refresh_interval));
    }
}

fn main() {
    run("simple-osd-simple", simple_daemon)
}
