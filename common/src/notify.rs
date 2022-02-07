use crate::config::Config;
use dbus::ffidisp::{BusType, Connection};
pub use notify_rust::Urgency;
use notify_rust::{Hint, Notification, NotificationHandle};
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

struct CustomHandle {
    pub id: u32,
    pub connection: Connection,
    pub notification: Notification,
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
}

#[derive(Error, Debug)]
pub enum NotificationHandleError {
    #[error("Failed to initialize a DBUS connection")]
    DBUSConnectionError(#[from] dbus::Error),
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
        }
    }

    fn construct_fake_handle(
        id: u32,
        notification: Notification,
    ) -> Result<NotificationHandle, NotificationHandleError> {
        let h = CustomHandle {
            id,
            connection: Connection::get_private(BusType::Session)?,
            notification,
        };
        unsafe {
            let handle: NotificationHandle = std::mem::transmute(h);
            Ok(handle)
        }
    }

    fn fake_handle(&mut self) -> Result<NotificationHandle, NotificationHandleError> {
        Self::construct_fake_handle(
            self.id.lock().unwrap().unwrap_or(0),
            self.notification.clone(),
        )
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
        let notification = self.notification.clone();
        let id = self.id.clone();
        thread::spawn(move || {
            let fake_handle =
                Self::construct_fake_handle(id.lock().unwrap().unwrap_or(0), notification).unwrap();
            fake_handle.on_close(|| {
                trace!("Notification has been closed, resetting id to None");
                let mut id = id.lock().unwrap();
                *id = None;
            })
        });
        Ok(())
    }

    pub fn on_close<F: 'static>(&mut self, callback: F) -> Result<(), CloseCallbackError>
    where
        F: std::ops::FnOnce() + Send,
    {
        if let Some(id) = *self.id.lock().unwrap() {
            let notification = self.notification.clone();

            thread::spawn(move || {
                let fake_handle = Self::construct_fake_handle(id, notification);
                trace!("Setting up a close callback on notification {}", id);
                fake_handle.map(|handle| {
                    handle.on_close(|| {
                        debug!("Notification {} closed, calling back", id);
                        callback();
                    });
                })
            });

            Ok(())
        } else {
            debug!("Notification is already closed, calling immediately");
            callback();
            Ok(())
        }
    }

    pub fn close(&mut self) -> Result<(), CloseError> {
        self.fake_handle().map(|handle| handle.close())?;
        Ok(())
    }
}

impl Default for OSD {
    fn default() -> Self {
        Self::new()
    }
}
