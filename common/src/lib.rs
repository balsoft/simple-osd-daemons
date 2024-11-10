// This is free and unencumbered software released into the public domain.
// balsoft 2020

extern crate configparser;
extern crate notify_rust;
extern crate xdg;
#[macro_use]
extern crate log;
extern crate pretty_env_logger;
extern crate thiserror;

pub static APPNAME: &str = "simple-osd";

pub mod daemon;

pub mod config;

pub mod notify;
