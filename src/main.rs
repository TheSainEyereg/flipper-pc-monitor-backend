#![feature(if_let_guard)]

use btleplug::api::{
    bleuuid::BleUuid, Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter,
};
use btleplug::platform::{Adapter, Manager, Peripheral, PeripheralId};
use futures::stream::StreamExt;
use tokio::sync::RwLock;
use std::borrow::BorrowMut;
use std::cell::{RefCell};
use std::sync::{Arc, Mutex};
use std::error::Error;
use std::rc::Rc;
use uuid::Uuid;

const FLIPPER_CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x19ed82ae_ed21_4c9d_4145_228e62fe0000);

async fn get_central(manager: &Manager) -> Adapter {
    manager
        .adapters()
        .await
        .unwrap()
        .into_iter()
        .nth(0)
        .unwrap()
}

async fn get_flipper(central: &Adapter, id: PeripheralId) -> Option<Peripheral> {
    for p in central.peripherals().await.unwrap().iter().filter(|p| p.id() == id) {
        if p.properties()
            .await
            .unwrap()
            .unwrap()
            .local_name
            .iter()
            .any(|name| name.contains("Flipper"))
        {
            return Some(p.clone());
        }
    }
    None
}

async fn data_sender(flipper: Peripheral) {
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let manager = Manager::new().await?;

    let central = get_central(&manager).await;

    let mut events = central.events().await?;

    central.start_scan(ScanFilter::default()).await?;

    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(id) => {
                if let Some(flp) = get_flipper(&central, id).await {
                    flp.connect().await.expect(format!("Failed to connect to Flipper {}", id).as_str());
                }
            }
            CentralEvent::DeviceConnected(id) => {
                if let Some(flp) = get_flipper(&central, id).await {
                    flp.discover_services().await?;
                    tokio::spawn(data_sender(flp));
                };
            }
            _ => {}
        }
    }

    Ok(())
}
