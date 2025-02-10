// See the "macOS permissions note" in README.md before running this on macOS
// Big Sur or later.
#[macro_use]
extern crate log;

use std::error::Error;
use std::time::Duration;
use btleplug::api::bleuuid::uuid_from_u16;
use notify_rust::Notification;
use tokio::time;
use chrono::Local;
use directories::UserDirs;
use std::fs::OpenOptions;
use std::io::{Write, BufWriter};

use btleplug::api::{Central, Manager as _, Peripheral, ScanFilter};
use btleplug::platform::Manager;


const DEVICE_INFORMATION_UUID: uuid::Uuid = uuid_from_u16(0x180A);
const MODEL_NUMBER_CHARACTERISTIC_UUID: uuid::Uuid = uuid_from_u16(0x2A24);
const MANUFACTURER_CHARACTERISTIC_UUID: uuid::Uuid = uuid_from_u16(0x2A29);
const BATTERY_SERVICE_UUID: uuid::Uuid = uuid_from_u16(0x180F);
const BATTERY_LEVEL_CHARACTERISTIC_UUID: uuid::Uuid = uuid_from_u16(0x2A19);

#[derive(Debug)]
struct DeviceStatus {
    manufacturer: String,
    model: String,
    battery_level: f32
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let user_dirs = UserDirs::new().expect("Failed to get user directories");
    let file_path = user_dirs.document_dir().expect("Failed to get documents directory").join("devices-monitor-log.csv");

    // Open or create the file in append mode
    let mut file = OpenOptions::new().create(true).append(true).open(&file_path).expect("Failed to open or create file");
    let is_new_file = file.metadata().expect("Failed to read metadata").len() == 0;

    let mut writer = BufWriter::new(&mut file);

    // Write a header if the file is newly created
    if is_new_file {
        writeln!(writer, "Timestamp,Message").expect("Failed to write header");
    }

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let mut device_statuses: Vec<DeviceStatus> = Vec::new();
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
                    let services = peripheral.services(); 
                    
                    let device_info_serv = services.iter().find(|c| c.uuid == DEVICE_INFORMATION_UUID).expect("Unable to find device information service");
                    let model_char = device_info_serv.characteristics.iter().find(|c| c.uuid == MODEL_NUMBER_CHARACTERISTIC_UUID).expect("Unable to find model number characteristic");
                    let manufacturer_char = device_info_serv.characteristics.iter().find(|c| c.uuid == MANUFACTURER_CHARACTERISTIC_UUID).expect("Unable to find manufacturer characteristic");
                    
                    let batt_serv = services.iter().find(|c| c.uuid == BATTERY_SERVICE_UUID).expect("Unable to find battery service");
                    let batt_char = batt_serv.characteristics.iter().find(|c| c.uuid == BATTERY_LEVEL_CHARACTERISTIC_UUID).expect("Unable to find battery level characteristic");
                    debug!("  {:?}", batt_char);
                    let batt_lvl_value = peripheral.read(&batt_char).await?;
                    info!("{:} Battery value: {:?}", local_name, batt_lvl_value);

                    let device_status = DeviceStatus {
                        manufacturer: String::from_utf8(peripheral.read(&manufacturer_char).await?).unwrap_or_else(|e| format!("Invalid UTF-8 sequence for manufacturer: {}", e)),
                        model: String::from_utf8(peripheral.read(&model_char).await?).unwrap_or_else(|e| format!("Invalid UTF-8 sequence for model: {}", e)),
                        battery_level: batt_lvl_value[0] as f32
                    };

                    device_statuses.push(device_status);
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
            info!("{:?}", device_statuses);
            let mut content: String = String::new();
            for device_status in device_statuses.iter() {
                content = format!("{} {} : {}%\n", device_status.manufacturer, device_status.model,device_status.battery_level);
            }
            writeln!(writer, "{},{}", timestamp, content).expect("Failed to write data");
            Notification::new()
                .summary("Battery update")
                .body(&content)
                .icon("battery")
                .appname("Devices monitor")
                .timeout(0) // this however is
                .show()?;
        }
    }
    Ok(())
}
