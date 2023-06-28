#![feature(if_let_guard)]

use std::error::Error;
use btleplug::api::{
    Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter,
};
use btleplug::platform::{Manager, Peripheral};
use futures::stream::StreamExt;


mod flipper_manager;

async fn data_sender(flipper: Peripheral) {
    
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let manager = Manager::new().await?;

    let central = flipper_manager::get_central(&manager).await;

    let mut events = central.events().await?;

    central.start_scan(ScanFilter::default()).await?;

    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(id) => {
                if let Some(flp) = flipper_manager::get_flipper(&central, &id).await {
                    flp.connect().await.expect(format!("Failed to connect to Flipper {}", id.to_string()).as_str());
                }
            }
            CentralEvent::DeviceConnected(id) => {
                if let Some(flp) = flipper_manager::get_flipper(&central, &id).await {
                    flp.discover_services().await?;
                    tokio::spawn(data_sender(flp));
                };
            }
            _ => {}
        }
    }

    Ok(())
}
