extern crate libnotify;
extern crate xdg;
extern crate configparser;

pub static APPNAME: &str = "simple-osd";

pub mod config {
    use configparser::ini::Ini;
    use xdg::BaseDirectories;
    use std::fs::{File, metadata};

    pub struct Config {
        config_path: Option<String>,
        config: Ini
    }

    impl Config {

        pub fn new(name: &'static str) -> Config {
            let mut config = Ini::new();

            let xdg_dirs = BaseDirectories::with_prefix(crate::APPNAME).unwrap();

            let config_path_option = xdg_dirs.place_config_file(name).ok();

            if let Some(config_path_buf) = config_path_option.clone() {
                if metadata(config_path_buf.clone()).map(|m| m.is_file()).unwrap_or(false) {
                    config.load(config_path_buf.to_str().unwrap());
                } else {
                    File::create(config_path_buf);
                }
            }

            let config_path = config_path_option.map(|p| p.to_str().unwrap().to_string());

            Config { config, config_path }
        }

        pub fn get<T: std::str::FromStr>(&mut self, section: &str, key: &str) -> Option<T>
          where
            T: std::str::FromStr,
            <T as std::str::FromStr>::Err: std::fmt::Debug
        {
            self.config.get(section, key).map(|s: String| { s.parse().unwrap() }).or_else(|| {
                self.config.set(section, key, None);
                self.config_path.as_ref().map(|path| self.config.write(path.as_str()));
                None
            })
        }

        pub fn get_default<T>(&mut self, section: &str, key: &str, default: T) -> T
          where
            T: std::str::FromStr,
            T: std::fmt::Display,
            <T as std::str::FromStr>::Err: std::fmt::Debug
        {
            let val: Option<T> = self.get(section, key);

            val.unwrap_or_else(|| {
                self.config.set(section, key, Some(format!("{}", default)));
                self.config_path.as_ref().map(|path| self.config.write(path.as_str()));
                default
            })
        }
    }
}
pub mod notify {
    use libnotify::Notification;
    pub use libnotify::Urgency;
    use crate::config::Config;

    fn init_if_not_already() {
        if ! libnotify::is_initted() {
            println!("Initializing libnotify");
            libnotify::init(crate::APPNAME).unwrap()
        }
    }

    pub enum OSDProgressText {
        Percentage,
        Text(Option<String>)
    }

    pub enum OSDContents {
        Simple(Option<String>),
        Progress(f32, OSDProgressText)
    }

    pub struct OSD {
        pub title: Option<String>,

        pub icon: Option<String>,

        pub contents: OSDContents,

        pub urgency: Urgency,

        // Progress bar stuff
        length: i32,

        full: String,
        empty: String,

        start: String,
        end: String,

        // Internal notification
        notification: Notification
    }

    impl OSD {
        pub fn new() -> OSD {
            init_if_not_already();

            let mut config = Config::new("common");

            let length = config.get_default("progressbar", "length", 20);

            let full = config.get_default("progressbar", "full", String::from("█"));
            let empty = config.get_default("progressbar", "empty", String::from("░"));

            let start = config.get_default("progressbar", "start", String::new());
            let end = config.get_default("progressbar", "end", String::new());

            let notification = Notification::new("", None, None);

            return OSD {
                title: None, icon: None,
                contents: OSDContents::Simple(None),
                urgency: Urgency::Normal,
                length, full, empty, start, end,
                notification
            };
        }

        fn get_full_text(&self) -> Option<String> {
            match &self.contents {
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

                    s.push_str(" ");

                    match text {
                        OSDProgressText::Percentage => {
                            s.push_str(((value * 100.) as i32).to_string().as_str());

                            s.push_str("%");
                        },
                        OSDProgressText::Text(text) => {
                            text.as_ref().map(|text| s.push_str(text.as_str()));
                        }
                    }


                    Some(s)
                }
            }
        }

        pub fn update(&mut self) {
            self.notification.update(self.title.as_deref().unwrap_or(""), self.get_full_text().as_deref(), self.icon.as_deref()).unwrap();
            self.notification.set_urgency(self.urgency);
            self.notification.show().unwrap();
        }
    }
}
