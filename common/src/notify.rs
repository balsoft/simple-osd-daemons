use crate::config::Config;
pub use notify_rust::Urgency;
use notify_rust::{CloseHandler, CloseReason, Hint, Notification, NotificationHandle};
use std::default::Default;
use std::sync::{Arc, Mutex};
use std::thread;
use thiserror::Error;

pub enum OSDProgressText {
    Percentage,
    Text(Option<String>),
}

pub enum OSDContents {
    Simple(Option<String>),
    Progress(f32, OSDProgressText),
}

impl Default for OSDContents {
    fn default() -> OSDContents {
        OSDContents::Simple(None)
    }
}

pub struct OSD {
    pub title: Option<String>,

    pub icon: Option<String>,

    pub contents: OSDContents,

    pub urgency: Urgency,

    pub timeout: i32,

    // Progress bar stuff
    hint: bool,

    length: i32,

    full: String,
    empty: String,

    start: String,
    end: String,

    // Internal notification
    notification: Notification,
    id: Arc<Mutex<Option<u32>>>,
    on_close_handler: Arc<Mutex<Box<dyn CloseHandler<CloseReason> + Send + Sync>>>,
}

#[derive(Error, Debug)]
pub enum NotificationHandleError {
    #[error("Failed to initialize a ZBUS connection")]
    ZBUSConnectionError(#[from] zbus::Error),
}

#[derive(Error, Debug)]
pub enum CloseCallbackError {
    #[error("Failed to get a notification handle")]
    NotificationHandleError(#[from] NotificationHandleError),
}

#[derive(Error, Debug)]
pub enum CloseError {
    #[error("Failed to get a notification handle")]
    NotificationHandleError(#[from] NotificationHandleError),
}

#[derive(Error, Debug)]
pub enum UpdateError {
    #[error("Failed to show the notification")]
    NotificationShowError(#[from] notify_rust::error::Error),
}

impl OSD {
    pub fn new() -> OSD {
        let mut config = Config::new("common");

        // -1 means the default timeout of the notification server
        let timeout = config.get("notification", "default timeout").unwrap_or(-1);

        // Progress doesn't go down for the same notification, at least in mako, so disable it by default
        let hint = config.get_default("progressbar", "use freedesktop notification hint", false);

        let length = config.get_default("progressbar", "length", 20);

        let full = config.get_default("progressbar", "full", String::from("█"));
        let empty = config.get_default("progressbar", "empty", String::from("░"));

        let start = config.get_default("progressbar", "start", String::new());
        let end = config.get_default("progressbar", "end", String::new());

        let notification = Notification::new();

        OSD {
            title: None,
            icon: None,
            contents: OSDContents::default(),
            urgency: Urgency::Normal,
            id: Arc::new(Mutex::new(None)),
            timeout,
            hint,
            length,
            full,
            empty,
            start,
            end,
            notification,
            on_close_handler: Arc::new(Mutex::new(Box::new(|_| {}))),
        }
    }

    pub fn update(&mut self) -> Result<(), UpdateError> {
        let text = match &self.contents {
            OSDContents::Simple(text) => text.clone(),
            OSDContents::Progress(value, text) => {
                let mut s = String::new();

                if !self.hint {
                    trace!("Hint is false, generating progressbar");

                    s.push_str(self.start.as_str());

                    for _ in 0..(value * self.length as f32) as i32 {
                        s.push_str(self.full.as_str())
                    }

                    for _ in (value * self.length as f32) as i32..self.length {
                        s.push_str(self.empty.as_str())
                    }

                    s.push_str(self.end.as_str());

                    s.push(' ');
                }

                match text {
                    OSDProgressText::Percentage => {
                        s.push_str(((value * 100.) as i32).to_string().as_str());

                        s.push('%');
                    }
                    OSDProgressText::Text(text) => {
                        if let Some(text) = text.as_ref() {
                            s.push_str(text.as_str())
                        };
                    }
                }

                Some(s)
            }
        };

        self.notification = Notification::new();

        let notification = self
            .notification
            .summary(self.title.as_deref().unwrap_or(""))
            .body(&text.unwrap_or_else(String::new))
            .icon(self.icon.as_deref().unwrap_or(""))
            .hint(Hint::Category("osd".to_owned()))
            .urgency(self.urgency);
        if self.hint {
            if let OSDContents::Progress(value, _) = self.contents {
                let percentage = (value * 100.0).round() as i32;
                notification.hint(Hint::CustomInt(String::from("value"), percentage));
            }
        }
        if let Some(id) = *self.id.lock().unwrap() {
            trace!("Replaces {}", id);
            notification.id(id);
        }
        let handle: NotificationHandle = notification
            .finalize()
            .show()
            .map_err(UpdateError::NotificationShowError)?;
        trace!("Handle {:?}", handle);
        self.id = Arc::new(Mutex::new(Some(handle.id())));
        let id = self.id.clone();
        let on_close_handler = self.on_close_handler.clone();
        thread::spawn(move || {
            handle.on_close(|reason| {
                trace!("Notification has been closed, resetting id to None");
                let mut id = id.lock().unwrap();
                *id = None;
                let mut on_close_handler = on_close_handler.lock().unwrap();
                on_close_handler.call(reason);
                *on_close_handler = Box::new(|_| {});
            });
        });
        Ok(())
    }

    pub fn update_(&mut self) {
        self.update().unwrap_or_else(|err| { warn!("{}", err); });
    }

    pub fn on_close(&mut self, callback: Box<dyn CloseHandler<CloseReason> + Send + Sync>) -> Result<(), CloseCallbackError>
    {
        if let Some(id) = *self.id.lock().unwrap() {
            trace!("Setting up a close callback on notification {}", id);
            self.on_close_handler = Arc::new(Mutex::new(callback));
            Ok(())
        } else {
            debug!("Notification is already closed, calling immediately");
            callback.call(CloseReason::Other(0));
            Ok(())
        }
    }
}

impl Default for OSD {
    fn default() -> Self {
        Self::new()
    }
}
