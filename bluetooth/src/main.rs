// This is free and unencumbered software released into the public domain.
// balsoft 2020

extern crate simple_osd_common as osd;

#[macro_use]
extern crate log;

use thiserror::Error;

use osd::daemon::run;

use osd::notify::{OSDContents, OSDProgressText, OSD};

use bluer::{self, AdapterEvent};

use futures_lite::stream::StreamExt;

#[derive(Error, Debug)]
enum BluetoothError {
    #[error("Bluer error")]
    BluerError(#[from] bluer::Error),
}

async fn bluetooth_daemon() -> Result<(), BluetoothError> {
    let mut osd = OSD::new();
    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await?;
    let mut event_stream = adapter.events().await?;
    loop {
        match event_stream.next().await {
            Some(AdapterEvent::DeviceAdded(addr)) => {
                let device = adapter.device(addr)?;
                osd.title = Some(String::from("Connected to"));
                osd.icon = Some(String::from("network-bluetooth-activated"));
                osd.contents = OSDContents::Simple(device.name().await?);
                trace!("DeviceAdded {:?}", device.name().await?);
                osd.update_();
            },
            Some(AdapterEvent::DeviceRemoved(addr)) => {
                let device = adapter.device(addr)?;
                osd.title = Some(String::from("Bluetooth device disconnected"));
                osd.icon = Some(String::from("network-bluetooth"));
                osd.contents = OSDContents::Simple(device.name().await?);
                trace!("DeviceRemoved {:?}", device.name().await?);
                osd.update_();
            },
            None => { return Ok(()); },
            _ => {},
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    pretty_env_logger::init();
    bluetooth_daemon().await;
}
