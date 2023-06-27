#![feature(if_let_guard)]

use btleplug::api::{
    bleuuid::BleUuid, Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter,
};
use btleplug::platform::{Adapter, Manager, Peripheral, PeripheralId};
use futures::stream::StreamExt;
use std::error::Error;
use uuid::Uuid;
use std::rc::Rc;
use std::cell::RefCell;

const FLIPPER_CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x19ed82ae_ed21_4c9d_4145_228e62fe0000);

async fn get_central(manager: &Manager) -> Adapter {
    let adapters = manager.adapters().await.unwrap();
    adapters.into_iter().nth(0).unwrap()
}

#[derive(Clone)]
enum FlipperState {
    Discovered,
    Connected,
    Lost,
}

#[derive(Clone)]
struct FlipperManager {
    flipper: Rc<RefCell<Option<Peripheral>>>,
    flipper_state: Rc<RefCell<FlipperState>>
}

impl FlipperManager {
    pub fn new() -> Self {
        FlipperManager { flipper: ((Rc::new(RefCell::from(None)))), flipper_state: (Rc::new(RefCell::from(FlipperState::Lost))) }
    }

    pub fn set_flipper(&mut self, ph: Peripheral) {
        *self.flipper.borrow_mut() = Some(ph);
    }

    pub fn is_state(&self, _state: FlipperState) -> bool {
        matches!(self.flipper_state.borrow(), _state)
    }

    pub fn set_state(&mut self, state: FlipperState) {
        *self.flipper_state.borrow_mut() = state;
    }

    pub fn is_flipper_id(&self, id: PeripheralId) -> bool {
        self.flipper.borrow().as_ref().and_then(|flp| Some(flp.id() == id)).unwrap()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let manager = Manager::new().await?;

    let central = get_central(&manager).await;

    let mut events = central.events().await?;

    central.start_scan(ScanFilter::default()).await?;

    let mut manager: FlipperManager = FlipperManager::new();

    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(id) => {
				if manager.is_state(FlipperState::Discovered) { continue }
				manager.set_state(FlipperState::Discovered);

				manager.set_flipper(find_flipper(&central).await.unwrap());

                manager.flipper.borrow().as_ref().unwrap().connect().await.expect("Cannot connect to device");
            }
            CentralEvent::DeviceConnected(id) => {
				if manager.is_state(FlipperState::Connected) || manager.is_flipper_id(id) { continue }
				manager.set_state(FlipperState::Connected);
                
                if let Some(flp) = manager.flipper.borrow_mut().as_mut() {
                    flp.discover_services().await?;
                }
				
			}
            CentralEvent::DeviceDisconnected(id) => {
				if manager.is_state(FlipperState::Lost) || manager.is_flipper_id(id) { continue }
				manager.set_state(FlipperState::Lost);
            }
            _ => {}
        }
    }

    Ok(())
}

async fn find_flipper(central: &Adapter) -> Option<Peripheral> {
    for p in central.peripherals().await.unwrap() {
        if p.properties()
            .await
            .unwrap()
            .unwrap()
            .local_name
            .iter()
            .any(|name| name.contains("Flipper"))
        {
            return Some(p);
        }
    }
    None
}
