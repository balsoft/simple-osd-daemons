// This is free and unencumbered software released into the public domain.
// balsoft 2020

extern crate configparser;
extern crate dbus;
extern crate notify_rust;
extern crate xdg;

pub static APPNAME: &str = "simple-osd";

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
        pub fn new(name: &'static str) -> Config {
            let mut config = Ini::new();

            let xdg_dirs = BaseDirectories::with_prefix(crate::APPNAME).unwrap();

            let config_path_option = xdg_dirs.place_config_file(name).ok();

            if let Some(config_path_buf) = config_path_option.clone() {
                if metadata(config_path_buf.clone())
                    .map(|m| m.is_file())
                    .unwrap_or(false)
                {
                    let _ = config.load(config_path_buf.to_str().unwrap());
                } else {
                    let _ = File::create(config_path_buf);
                };
            }

            let config_path = config_path_option.map(|p| p.to_str().unwrap().to_string());

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
                .map(|s: String| s.parse().unwrap())
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
    }
}
pub mod notify {
    use crate::config::Config;
    use dbus::ffidisp::{BusType, Connection};
    pub use notify_rust::Urgency;
    use notify_rust::{Notification, NotificationHandle};
    use std::default::Default;
    use std::thread;

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
        length: i32,

        full: String,
        empty: String,

        start: String,
        end: String,

        // Internal notification
        notification: Notification,
        id: Option<u32>,
    }

    impl OSD {
        pub fn new() -> OSD {
            let mut config = Config::new("common");

            let timeout = config.get("notification", "default timeout").unwrap_or(-1); // -1 means the default timeout of the notification server

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
                length,
                full,
                empty,
                start,
                end,
                notification,
            }
        }

        fn construct_fake_handle(id: u32, notification: Notification) -> NotificationHandle {
            let h = CustomHandle {
                id,
                connection: Connection::get_private(BusType::Session).unwrap(),
                notification,
            };
            unsafe {
                let handle: NotificationHandle = std::mem::transmute(h);
                handle
            }
        }

        fn fake_handle(&mut self) -> NotificationHandle {
            Self::construct_fake_handle(self.id.unwrap_or(0), self.notification.clone())
        }

        pub fn update(&mut self) -> Result<(), String> {
            let text = match &self.contents {
                OSDContents::Simple(text) => text.clone(),
                OSDContents::Progress(value, text) => {
                    let mut s = String::new();

                    s.push_str(self.start.as_str());

                    for _ in 0..(value * self.length as f32) as i32 {
                        s.push_str(self.full.as_str())
                    }

                    for _ in (value * self.length as f32) as i32..self.length {
                        s.push_str(self.empty.as_str())
                    }

                    s.push_str(self.end.as_str());

                    s.push(' ');

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
            let handle = self
                .notification
                .summary(self.title.as_deref().unwrap_or(""))
                .body(&text.unwrap_or_else(String::new))
                .icon(self.icon.as_deref().unwrap_or(""))
                .urgency(self.urgency)
                .finalize()
                .show()
                .or(Err("Failed to show the notification"))?;
            self.id = Some(handle.id());
            Ok(())
        }

        pub fn on_close<F: 'static>(&mut self, callback: F)
        where
            F: std::ops::FnOnce() + Send,
        {
            if let Some(id) = self.id {
                let notification = self.notification.clone();

                thread::spawn(move || {
                    let fake_handle = Self::construct_fake_handle(id, notification);
                    fake_handle.on_close(callback);
                });
            } else {
                callback();
            }
        }

        pub fn close(&mut self) {
            self.fake_handle().close();
        }
    }

    impl Default for OSD {
        fn default() -> Self {
            Self::new()
        }
    }
}
