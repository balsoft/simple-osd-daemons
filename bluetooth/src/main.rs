// This is free and unencumbered software released into the public domain.
// balsoft 2020

extern crate blurz;
extern crate simple_osd_common as osd;

use blurz::bluetooth_session::BluetoothSession;
use blurz::bluetooth_adapter::BluetoothAdapter;

use osd::config::Config;
use osd::notify::{OSD, Urgency};

fn main() {

    let mut config = Config::new("bluetooth");

    let refresh_interval = config.get_default("default", "refresh interval", 15);

    let path: Option<String> = config.get("session", "path");

    let session = BluetoothSession::create_session(path.as_deref()).unwrap();

    let adapter = BluetoothAdapter::init(&session).unwrap();

    let mut osd = OSD::new();
    osd.icon = Some(String::from("bluetooth"));
    osd.urgency = Urgency::Low;

    let mut device_name: String;
    let mut last_device_name: String = String::new();

    loop {
        let device = adapter.get_first_device().unwrap();

        device_name = device.get_name().unwrap_or(String::from(""));

        if device_name != last_device_name {
            osd.title = Some(format!("Bluetooth: connected to {}", device_name));
            osd.update();
        }

        last_device_name = device_name;

        std::thread::sleep(std::time::Duration::from_secs(refresh_interval))
    }
}
