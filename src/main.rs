// See the "macOS permissions note" in README.md before running this on macOS
// Big Sur or later.
#[macro_use]
extern crate log;

use std::error::Error;
use std::time::Duration;
use btleplug::api::bleuuid::uuid_from_u16;
use notify_rust::Notification;
use tokio::time;

use btleplug::api::{Central, Manager as _, Peripheral, ScanFilter};
use btleplug::platform::Manager;


const DEVICE_INFORMATION_UUID: uuid::Uuid = uuid_from_u16(0x180A);
const MODEL_NUMBER_CHARACTERISTIC_UUID: uuid::Uuid = uuid_from_u16(0x2A24);
const MANUFACTURER_CHARACTERISTIC_UUID: uuid::Uuid = uuid_from_u16(0x2A29);
const BATTERY_SERVICE_UUID: uuid::Uuid = uuid_from_u16(0x180F);
const BATTERY_LEVEL_CHARACTERISTIC_UUID: uuid::Uuid = uuid_from_u16(0x2A19);

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let manager = Manager::new().await?;
    let adapter_list = manager.adapters().await?;
    if adapter_list.is_empty() {
        error!("No Bluetooth adapters found");
    }

    for adapter in adapter_list.iter() {
        debug!("Starting scan on {}...", adapter.adapter_info().await?);
        adapter
            .start_scan(ScanFilter::default())
            .await
            .expect("Can't scan BLE adapter for connected devices...");
        time::sleep(Duration::from_secs(2)).await;
        let peripherals = adapter.peripherals().await?;
        if peripherals.is_empty() {
            error!("->>> BLE peripheral devices were not found, sorry. Exiting...");
        } else {
            // All peripheral devices in range
            for peripheral in peripherals.iter() {
                let properties = peripheral.properties().await?;
                let is_connected = peripheral.is_connected().await?;
                let local_name = properties
                    .unwrap()
                    .local_name
                    .unwrap_or(String::from("(peripheral name unknown)"));
                
                // if !is_connected {
                //     println!("Connecting to peripheral {:?}...", &local_name);
                //     if let Err(err) = peripheral.connect().await {
                //         eprintln!("Error connecting to peripheral, skipping: {}", err);
                //         continue;
                //     }
                // }
                
                if is_connected {
                    peripheral.discover_services().await?;
                    debug!("Discover peripheral {:?} services...", &local_name);

                    // find the characteristic we want
                    let services = peripheral.services(); // Store the services in a variable
                    let batt_serv = services.iter().find(|c| c.uuid == BATTERY_SERVICE_UUID).expect("Unable to find battery service");
                    debug!("Service UUID {}, primary: {}", batt_serv.uuid, batt_serv.primary);
                    let batt_char = batt_serv.characteristics.iter().find(|c| c.uuid == BATTERY_LEVEL_CHARACTERISTIC_UUID).expect("Unable to find battery level characteristic");
                    debug!("  {:?}", batt_char);
                    let batt_lvl_value = peripheral.read(&batt_char).await?;
                    info!("{:} Battery value: {:?}", local_name, batt_lvl_value);
                    // for service in peripheral.services() {
                    //     for characteristic in service.characteristics.clone() {
                    //         println!("  {:?}", characteristic);
                    //         let res = peripheral.read(&characteristic);
                    //     }
                    //     if service.uuid.to_string().contains("0000180f-0000-1000-8000-00805f9b34fb") {
                    //         println!(
                    //             "Service UUID {}, primary: {}",
                    //             service.uuid, service.primary
                    //         );
                    //         for characteristic in service.characteristics {
                    //             println!("  {:?}", characteristic);
                    //         }
                    //     }
                    // }
                }
            }
            Notification::new()
                .summary("Category:email")
                .body("This has nothing to do with emails.\nIt should not go away until you acknowledge it.")
                .icon("firefox")
                .appname("devices-monitor")
                .timeout(0) // this however is
                .show()?;
        }
    }
    Ok(())
}
