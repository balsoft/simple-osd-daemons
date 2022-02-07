use std::fmt::Display;
use std::ops::FnOnce;
use std::process::exit;
pub fn run<F, E>(daemon: &str, f: F)
where
    F: FnOnce() -> Result<(), E>,
    E: Display,
{
    pretty_env_logger::init();
    info!(target: daemon, "Starting");
    match f() {
        Ok(_) => {
            info!(target: daemon, "Exiting normally")
        }
        Err(err) => {
            error!(target: daemon, "{}", err);
            exit(1)
        }
    };
}
