// This is free and unencumbered software released into the public domain.
// balsoft 2020

extern crate configparser;
extern crate dbus;
extern crate notify_rust;
extern crate xdg;
#[macro_use]
extern crate log;
extern crate pretty_env_logger;
extern crate thiserror;

pub static APPNAME: &str = "simple-osd";

pub mod daemon {
    use std::fmt::Display;
    use std::ops::FnOnce;
    use std::process::exit;
    macro_rules! good_panic {
        (target: $target:expr, $($tts:tt)*) => {{
            error!(target: $target, $($tts)*);
            exit(1);
        }}
    }
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
            Err(err) => good_panic!(target: daemon, "{}", err),
        };
    }
}

pub mod config {
    use configparser::ini::Ini;
    use std::fs::{metadata, File};
    use xdg::BaseDirectories;

    use std::default::Default;
    use std::fmt::Debug;
    use std::fmt::Display;
    use std::str::FromStr;

    pub struct Config {
        config_path: Option<String>,
        config: Ini,
    }

    impl Config {
        fn get_config_path(name: &'static str, config: &mut Ini) -> Option<String> {
            BaseDirectories::with_prefix(crate::APPNAME)
                .map_err(|err| warn!("Failed to set up XDG Base Directories: {0:?}", err))
                .ok()
                .and_then(|xdg_dirs| {
                    let config_path_option = xdg_dirs.place_config_file(name).ok();

                    if let Some(config_path_buf) = config_path_option.clone() {
                        if let Ok(true) = metadata(config_path_buf.clone()).map(|m| m.is_file()) {
                            let conf = config_path_buf.to_str()?;
                            debug!("Loading config file from {0}", conf);
                            if let Err(err) = config.load(conf) {
                                warn!("Failed to load config from {0}: {1}", conf, err);
                            }
                            trace!("Loaded config file:\n{0}", config.writes())
                        } else {
                            debug!("Creating a config file at {0:?}", config_path_buf);
                            if let Err(err) = File::create(config_path_buf.clone()) {
                                warn!(
                                    "Failed to create a config file at {0:?}: {1}",
                                    config_path_buf, err
                                )
                            }
                        };
                    }

                    config_path_option.map(|p| p.to_str().unwrap().to_string())
                })
        }

        pub fn new(name: &'static str) -> Config {
            let mut config = Ini::new();

            let config_path = Self::get_config_path(name, &mut config);

            Config {
                config,
                config_path,
            }
        }

        pub fn get<T>(&mut self, section: &str, key: &str) -> Option<T>
        where
            T: FromStr,
            <T as FromStr>::Err: Debug,
        {
            self.config
                .get(section, key)
                .and_then(|s: String| {
                    s.parse()
                        .map_err(|err| {
                            warn!(
                                "Failed to parse the config variable {0}.{1} ({2}): {3:?}",
                                section, key, s, err
                            )
                        })
                        .ok()
                })
                .or_else(|| {
                    self.config.set(section, key, None);
                    self.config_path
                        .as_ref()
                        .map(|path| self.config.write(path.as_str()));
                    None
                })
        }

        pub fn get_default<T>(&mut self, section: &str, key: &str, default: T) -> T
        where
            T: FromStr,
            T: Display,
            <T as FromStr>::Err: Debug,
        {
            let val: Option<T> = self.get(section, key);

            val.unwrap_or_else(|| {
                self.config.set(section, key, Some(format!("{}", default)));
                self.config_path
                    .as_ref()
                    .map(|path| self.config.write(path.as_str()));
                default
            })
        }

        pub fn get_default_from_trait<T>(&mut self, section: &str, key: &str) -> T
        where
            T: FromStr,
            T: Display,
            T: Default,
            <T as FromStr>::Err: Debug,
        {
            self.get_default(section, key, T::default())
        }

        pub fn get_override(&mut self, section: &str, key_and_value: &str) -> String
        {
            self.get_default(section, key_and_value, String::from(key_and_value))
        }
    }
}
pub mod notify {
    use crate::config::Config;
    use dbus::ffidisp::{BusType, Connection};
    pub use notify_rust::Urgency;
    use notify_rust::{Hint, Notification, NotificationHandle};
    use std::default::Default;
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
        id: Option<u32>,
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
            let hint =
                config.get_default("progressbar", "use freedesktop notification hint", false);

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
                id: None,
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
            Self::construct_fake_handle(self.id.unwrap_or(0), self.notification.clone())
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

            self.id.map(|i| self.notification.id(i));
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
            let handle = notification
                .finalize()
                .show()
                .map_err(UpdateError::NotificationShowError)?;
            self.id = Some(handle.id());
            Ok(())
        }

        pub fn on_close<F: 'static>(&mut self, callback: F) -> Result<(), CloseCallbackError>
        where
            F: std::ops::FnOnce() + Send,
        {
            if let Some(id) = self.id {
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
}
