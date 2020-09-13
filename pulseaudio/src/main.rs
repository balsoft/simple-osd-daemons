// This is free and unencumbered software released into the public domain.
// balsoft 2020

extern crate libpulse_binding as pulse;

extern crate simple_osd_common as osd;

use pulse::mainloop::standard::Mainloop;
use pulse::context::Context;
use osd::config::Config;
use osd::notify::{OSD, OSDContents, OSDProgressText};

use pulse::context::subscribe::{subscription_masks, Operation, Facility};

use pulse::callbacks::ListResult;
use pulse::context::introspect::SinkInfo;

fn main() {
    let mut mainloop = Mainloop::new().expect("Failed to create mainloop");

    let mut config = Config::new("pulseaudio");

    let mut context = Context::new(
        &mainloop, osd::APPNAME
    ).expect("Failed to create new context");

    context.connect(config.get::<String>("default", "server").as_deref(), 0, None)
        .expect("Failed to connect context");

    // Wait for context to be ready
    loop {
        mainloop.iterate(false);
        match context.get_state() {
            pulse::context::State::Ready => { break; },
            pulse::context::State::Failed |
            pulse::context::State::Unconnected |
            pulse::context::State::Terminated => {
                eprintln!("Context state failed/terminated, quitting...");
                return;
            },
            _ => {}
        }
    }

    eprintln!("connected");

    context.subscribe(subscription_masks::SINK, |success| {
        if ! success {
            eprintln!("failed to subscribe to events");
            return;
        }
    });

    let introspector = context.introspect();


    // Explanation for the unsafe:
    // Both subscribe_callback and sink_info_handler shall not outlive mainloop, but the borrow checker can't know that.
    // Thus, it moves osd into the subscribe_callback and then tries to move it into the sink_info_handler, but that's impossible.
    // In reality, both closures will be destroyed when mainloop quits, and osd's lifetime is the same as mainloop's.
    unsafe {
        let osd: *mut OSD = &mut OSD::new();
        (*osd).icon = Some(String::from("multimedia-volume-control"));

        let sink_info_handler = move |results: ListResult<&SinkInfo>| {
            if let ListResult::Item(i) = results {
                let volume = i.volume.avg();
                let sink_name = i.description.as_deref().unwrap_or("Unnamed sink");
                let muted_message = if i.mute { " [MUTED]" } else { "" };
                (*osd).title = Some(format!("Volume on {}{}", sink_name, muted_message));
                (*osd).contents = OSDContents::Progress(volume.0 as f32 / 65536., OSDProgressText::Percentage);
                (*osd).update();
            }
        };

        let subscribe_callback = move |facility, operation, index| {
            if facility == Some(Facility::Sink) && operation == Some(Operation::Changed) {
                introspector.get_sink_info_by_index(index, sink_info_handler);
            }
        };

        context.set_subscribe_callback(Some(Box::new(subscribe_callback)));

        // We need to run mainloop here for reasons I don't understand. It crashes otherwise.
        mainloop.run().unwrap();
    }
}
