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

    pub fn get_override(&mut self, section: &str, key_and_value: &str) -> String {
        self.get_default(section, key_and_value, String::from(key_and_value))
    }
}
