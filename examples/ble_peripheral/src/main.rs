#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use bt_hci::{
    cmd::le::{LeSetAdvData, LeSetAdvEnable, LeSetAdvParams, LeSetScanResponseData},
    controller::ControllerCmdSync,
};
use core::{future::Future, pin::Pin};
use embassy_futures::{
    block_on,
    select::{Either, select},
};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embedded_graphics::{geometry::Point, pixelcolor::Rgb565, prelude::RgbColor};
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, main};
use nesso::{Nesso, ble::service, bsp::NessoDisplay};
use trouble_host::prelude::*;

esp_bootloader_esp_idf::esp_app_desc!();

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 1;
const ATTRIBUTE_MAX: usize = 14;
const CCCD_MAX: usize = 1;
const PACKET_MTU: usize = 64;
const PACKET_CAPACITY: usize = 8;
const DEVICE_NAME: &[u8] = b"Nesso N1";
const APPEARANCE: [u8; 2] = [0x80, 0x07];
const SERVICE_UUID: [u8; 2] = service::NESSO_SERVICE_UUID16.to_le_bytes();

struct ExamplePacket(Box<[u8; PACKET_MTU]>);

impl AsRef<[u8]> for ExamplePacket {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl AsMut<[u8]> for ExamplePacket {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut()
    }
}

impl Packet for ExamplePacket {}

struct ExamplePacketPool;

impl PacketPool for ExamplePacketPool {
    type Packet = ExamplePacket;

    const MTU: usize = PACKET_MTU;

    fn allocate() -> Option<Self::Packet> {
        Some(ExamplePacket(Box::new([0_u8; PACKET_MTU])))
    }

    fn capacity() -> usize {
        PACKET_CAPACITY
    }
}

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 64 * 1024);
    esp_alloc::heap_allocator!(size: 64 * 1024);
    run(peripherals)
}

fn run(peripherals: esp_hal::peripherals::Peripherals) -> ! {
    let mut nesso = match Nesso::new(peripherals) {
        Ok(nesso) => nesso,
        Err(_) => abort(),
    };

    show_status(
        &mut nesso.display,
        "BLE Peripheral",
        "Starting controller",
        "nRF Connect ready",
    );

    let ble = match nesso.init_ble() {
        Ok(ble) => ble,
        Err(_) => abort(),
    };
    let connector = match ble.into_hci_connector() {
        Ok(connector) => connector,
        Err(_) => abort(),
    };

    block_on(run_ble(connector, &mut nesso.display));
    abort()
}

fn run_ble<'a>(
    connector: esp_radio::ble::controller::BleConnector<'static>,
    display: &'a mut NessoDisplay,
) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
    Box::pin(async move {
        let controller = ExternalController::<_, 1>::new(connector);
        let mut resources: HostResources<ExamplePacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
            HostResources::new();
        let stack = trouble_host::new(controller, &mut resources)
            .set_random_address(Address::random([0xC6, 0xF0, 0xA0, 0x01, 0x4E, 0x51]));
        let Host {
            mut peripheral,
            mut runner,
            ..
        } = stack.build();

        let mut command_storage = [0_u8; 20];
        let mut status_storage = [0_u8; 20];
        let command_value = padded_value(b"idle");
        let status_value = padded_value(b"ready");

        let mut table: AttributeTable<'_, NoopRawMutex, ATTRIBUTE_MAX> = AttributeTable::new();
        let mut gap = table.add_service(Service::new(0x1800_u16));
        let _name = gap.add_characteristic_ro(0x2A00_u16, DEVICE_NAME);
        let _appearance = gap.add_characteristic_ro(0x2A01_u16, &APPEARANCE);
        gap.build();
        table.add_service(Service::new(0x1801_u16)).build();

        let mut nesso_service = table.add_service(Service::new(service::NESSO_SERVICE_UUID16));
        let command = nesso_service
            .add_characteristic(
                service::COMMAND_CHARACTERISTIC_UUID16,
                [CharacteristicProp::Read, CharacteristicProp::Write],
                command_value,
                &mut command_storage,
            )
            .build();
        let status = nesso_service
            .add_characteristic(
                service::NOTIFY_CHARACTERISTIC_UUID16,
                [CharacteristicProp::Read, CharacteristicProp::Notify],
                status_value,
                &mut status_storage,
            )
            .build();
        nesso_service.build();

        let server = AttributeServer::<
            NoopRawMutex,
            ExamplePacketPool,
            ATTRIBUTE_MAX,
            CCCD_MAX,
            CONNECTIONS_MAX,
        >::new(table);

        let peripheral_task = peripheral_loop(&mut peripheral, &server, command, status, display);
        let runner_task = runner.run();
        match select(runner_task, peripheral_task).await {
            Either::First(_result) => {}
            Either::Second(_result) => {}
        }
    })
}

fn peripheral_loop<'a, 'd, 'values, C>(
    peripheral: &'a mut Peripheral<'d, C, ExamplePacketPool>,
    server: &'a AttributeServer<
        '_,
        NoopRawMutex,
        ExamplePacketPool,
        ATTRIBUTE_MAX,
        CCCD_MAX,
        CONNECTIONS_MAX,
    >,
    command: Characteristic<[u8; 20]>,
    status: Characteristic<[u8; 20]>,
    display: &'a mut NessoDisplay,
) -> Pin<Box<dyn Future<Output = ()> + 'a>>
where
    C: Controller
        + for<'t> ControllerCmdSync<LeSetAdvData>
        + ControllerCmdSync<LeSetAdvParams>
        + for<'t> ControllerCmdSync<LeSetAdvEnable>
        + for<'t> ControllerCmdSync<LeSetScanResponseData>
        + 'a,
    'd: 'a,
    'values: 'a,
{
    Box::pin(async move {
        let mut adv_data = [0_u8; 31];
        let mut scan_data = [0_u8; 31];

        let adv_len = match AdStructure::encode_slice(
            &[
                AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                AdStructure::ServiceUuids16(&[SERVICE_UUID]),
            ],
            &mut adv_data,
        ) {
            Ok(len) => len,
            Err(_) => return,
        };
        let scan_len = match AdStructure::encode_slice(
            &[AdStructure::CompleteLocalName(DEVICE_NAME)],
            &mut scan_data,
        ) {
            Ok(len) => len,
            Err(_) => return,
        };

        loop {
            show_status(
                display,
                "BLE Peripheral",
                "Advertising as Nesso N1",
                "Open nRF Connect",
            );

            let advertiser = match peripheral
                .advertise(
                    &AdvertisementParameters::default(),
                    Advertisement::ConnectableScannableUndirected {
                        adv_data: &adv_data[..adv_len],
                        scan_data: &scan_data[..scan_len],
                    },
                )
                .await
            {
                Ok(advertiser) => advertiser,
                Err(_) => continue,
            };

            let connection = match advertiser.accept().await {
                Ok(connection) => connection,
                Err(_) => continue,
            };
            show_status(display, "BLE Peripheral", "Connected", "Read/write F0A1");

            let gatt = match connection.with_attribute_server(server) {
                Ok(gatt) => gatt,
                Err(_) => continue,
            };

            let notify_value = padded_value(b"connected");
            let _notified = status.notify(&gatt, &notify_value).await;

            loop {
                match gatt.next().await {
                    GattConnectionEvent::Disconnected { .. } => break,
                    GattConnectionEvent::Gatt {
                        event: GattEvent::Write(event),
                    } => {
                        let is_command = event.handle() == command.handle;
                        if let Ok(reply) = event.accept() {
                            let _sent = reply.send().await;
                        }
                        if is_command {
                            show_status(
                                display,
                                "BLE Peripheral",
                                "Command written",
                                "Notify updated",
                            );
                            let updated = padded_value(b"command received");
                            let _notified = status.notify(&gatt, &updated).await;
                        }
                    }
                    GattConnectionEvent::Gatt { event } => {
                        if let Ok(reply) = event.accept() {
                            let _sent = reply.send().await;
                        }
                    }
                    _ => {}
                }
            }
        }
    })
}

fn padded_value(bytes: &[u8]) -> [u8; 20] {
    let mut value = [0_u8; 20];
    let len = bytes.len().min(value.len());
    value[..len].copy_from_slice(&bytes[..len]);
    value
}

fn show_status(display: &mut NessoDisplay, title: &str, line1: &str, line2: &str) {
    let _cleared = display.clear(Rgb565::BLACK);
    let _title = display.print_centered(title, 62, Rgb565::CYAN);
    let _line1 = display.print_at(line1, Point::new(28, 104), Rgb565::GREEN);
    let _line2 = display.print_at(line2, Point::new(28, 130), Rgb565::WHITE);
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
