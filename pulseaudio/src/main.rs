// This is free and unencumbered software released into the public domain.
// balsoft 2020

extern crate libpulse_binding as pulse;

extern crate simple_osd_common as osd;

#[macro_use]
extern crate log;

use std::cell::RefCell;
use std::rc::Rc;

use std::collections::HashMap;

use osd::config::Config;
use osd::daemon::run;
use osd::notify::{OSDContents, OSDProgressText, OSD};
use pulse::context::{Context, FlagSet, State};
use pulse::mainloop::standard::Mainloop;

use pulse::context::subscribe::{InterestMaskSet, Facility, Operation};

use pulse::callbacks::ListResult;
use pulse::context::introspect::SinkInfo;
use thiserror::Error;

#[derive(Error, Debug)]
enum PulseaudioError {
    #[error("Failed to create a pulseaudio mainloop")]
    MainloopNewError,
    #[error("Failed to create a pulseaudio context")]
    ContextNewError,
    #[error("Failed to connect a pulseaudio context: {0:?}")]
    ContextConnectError(pulse::error::PAErr),
    #[error("Pulseaudio context state failed/terminated")]
    ContextStateError,
    #[error("Pulseaudio mainloop exited with an error: {0}")]
    MainloopRunErr(pulse::error::PAErr),
}

fn pulseaudio_daemon() -> Result<(), PulseaudioError> {
    let mut mainloop = Mainloop::new().ok_or(PulseaudioError::MainloopNewError)?;

    let mut config = Config::new("pulseaudio");

    let mut context =
        Context::new(&mainloop, osd::APPNAME).ok_or(PulseaudioError::ContextNewError)?;

    trace!("Connecting to a pulseaudio server");
    context
        .connect(
            config.get::<String>("default", "server").as_deref(),
            FlagSet::empty(),
            None,
        )
        .map_err(PulseaudioError::ContextConnectError)?;

    trace!("Waiting for the context to become ready");
    loop {
        mainloop.iterate(false);
        match context.get_state() {
            pulse::context::State::Ready => {
                break;
            }
            pulse::context::State::Failed
            | pulse::context::State::Unconnected
            | pulse::context::State::Terminated => {
                return Err(PulseaudioError::ContextStateError);
            }
            _ => {}
        }
    }

    trace!("Subscribing to SINK events");
    context.subscribe(InterestMaskSet::SINK, |success| {
        if !success {
            error!("Failed to subscribe to pulseaudio events");
            std::process::exit(1);
        }
    });

    let introspector = context.introspect();

    let osd = Rc::new(RefCell::new(OSD::new()));
    let prev_state = Rc::new(RefCell::new(HashMap::<String, (f32, bool)>::new()));

    let sink_info_handler = move |results: ListResult<&SinkInfo>| {
        if let ListResult::Item(i) = results {
            let volume = i.volume.avg().0 as f32 / 65536.;

            let sink_name = i.description.as_deref().unwrap_or("Unnamed sink");
            let show = if let Some((volume_prev, mute_prev)) = prev_state.borrow_mut().insert(sink_name.to_string(), (volume, i.mute)) {
                volume_prev != volume || mute_prev != i.mute
            } else { true };
            if show {
                let muted_message = if i.mute { " [MUTED]" } else { "" };
                osd.borrow_mut().icon = Some(String::from(match (i.mute, volume) {
                    (true, _) => "audio-volume-muted",
                    (false, v) if v < 0.33 => "audio-volume-low",
                    (false, v) if v < 0.66 => "audio-volume-medium",
                    (false, _) => "audio-volume-high",
                }));
                osd.borrow_mut().title = Some(format!("Volume on {}{}", sink_name, muted_message));
                osd.borrow_mut().contents = OSDContents::Progress(volume, OSDProgressText::Percentage);
                osd.borrow_mut().update_();
            }
        }
    };

    let subscribe_callback = move |facility, operation, index| {
        if facility == Some(Facility::Sink) && operation == Some(Operation::Changed) {
            trace!("Sink has been changed");
            introspector.get_sink_info_by_index(index, sink_info_handler.clone());
        }
    };

    context.set_subscribe_callback(Some(Box::new(subscribe_callback)));

    // Kill the process if the server dies (ugly, but I don't think there's any other way)
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            if context.get_state() != State::Ready { std::process::exit(1); }
        }
    });

    mainloop
        .run()
        .map_err(|(paerr, _)| PulseaudioError::MainloopRunErr(paerr))?;

    Ok(())
}

fn main() {
    run("simple-osd-pulseaudio", pulseaudio_daemon);
}
