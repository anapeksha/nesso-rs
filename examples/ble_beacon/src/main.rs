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
use embedded_graphics::{geometry::Point, pixelcolor::Rgb565, prelude::RgbColor};
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    main,
    time::{Duration, Instant},
};
use nesso::{
    Nesso,
    ble::{Advertisement as NessoAdvertisement, BeaconFrame, BeaconSchedule, service},
    bsp::NessoDisplay,
};
use trouble_host::prelude::*;

esp_bootloader_esp_idf::esp_app_desc!();

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 1;
const PACKET_MTU: usize = 64;
const PACKET_CAPACITY: usize = 4;
const BEACON_FRAMES: usize = 3;
const PAYLOAD_BYTES: usize = 31;

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

    show_status(&mut nesso.display, "BLE Beacon", "Starting", "Scanner only");

    let ble = match nesso.init_ble() {
        Ok(ble) => ble,
        Err(_) => abort(),
    };
    let connector = match ble.into_hci_connector() {
        Ok(connector) => connector,
        Err(_) => abort(),
    };
    let schedule = match build_schedule() {
        Ok(schedule) => schedule,
        Err(_) => abort(),
    };

    block_on(run_ble(connector, schedule, &mut nesso.display));
    abort()
}

fn run_ble<'a>(
    connector: esp_radio::ble::controller::BleConnector<'static>,
    schedule: BeaconSchedule<BEACON_FRAMES, PAYLOAD_BYTES>,
    display: &'a mut NessoDisplay,
) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
    Box::pin(async move {
        let controller = ExternalController::<_, 1>::new(connector);
        let mut resources: HostResources<ExamplePacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
            HostResources::new();
        let stack = trouble_host::new(controller, &mut resources)
            .set_random_address(Address::random([0xC6, 0xF0, 0xA0, 0x02, 0x4E, 0x51]));
        let Host {
            mut peripheral,
            mut runner,
            ..
        } = stack.build();

        let beacon_task = beacon_loop(&mut peripheral, schedule, display);
        let runner_task = runner.run();
        match select(runner_task, beacon_task).await {
            Either::First(_result) => {}
            Either::Second(_result) => {}
        }
    })
}

fn beacon_loop<'a, 'd, C>(
    peripheral: &'a mut Peripheral<'d, C, ExamplePacketPool>,
    mut schedule: BeaconSchedule<BEACON_FRAMES, PAYLOAD_BYTES>,
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
{
    Box::pin(async move {
        let Some(first) = schedule.current() else {
            return;
        };

        let _advertiser = match peripheral
            .advertise(
                &AdvertisementParameters::default(),
                Advertisement::NonconnectableScannableUndirected {
                    adv_data: first.advertisement(),
                    scan_data: first.scan_response(),
                },
            )
            .await
        {
            Ok(advertiser) => advertiser,
            Err(_) => return,
        };

        loop {
            let Some(current) = schedule.current() else {
                return;
            };
            show_status(
                display,
                "BLE Beacon",
                "Rotating payloads",
                "Scan in nRF Connect",
            );
            delay_ms(current.duration_ms());

            let Some(next) = schedule.advance() else {
                return;
            };
            let _updated = peripheral
                .update_adv_data(Advertisement::NonconnectableScannableUndirected {
                    adv_data: next.advertisement(),
                    scan_data: next.scan_response(),
                })
                .await;
        }
    })
}

fn build_schedule() -> Result<BeaconSchedule<BEACON_FRAMES, PAYLOAD_BYTES>, nesso::ble::BleError> {
    let mut schedule = BeaconSchedule::new();
    schedule.push(frame("Nesso setup", &[0x01], 1500)?)?;
    schedule.push(frame("Battery OK", &[0x02], 1500)?)?;
    schedule.push(frame("SDK beacon", &[0x03], 1500)?)?;
    Ok(schedule)
}

fn frame(
    name: &str,
    service_data: &[u8],
    duration_ms: u32,
) -> Result<BeaconFrame<PAYLOAD_BYTES>, nesso::ble::BleError> {
    let mut advertisement = NessoAdvertisement::new();
    advertisement.push_flags()?;
    advertisement.push_service_uuid16(service::NESSO_SERVICE_UUID16)?;
    advertisement.push_service_data16(service::NESSO_SERVICE_UUID16, service_data)?;

    let mut scan_response = NessoAdvertisement::new();
    scan_response.push_complete_name(name)?;

    Ok(BeaconFrame::new(advertisement, scan_response, duration_ms))
}

fn delay_ms(milliseconds: u32) {
    let start = Instant::now();
    let duration = Duration::from_millis(u64::from(milliseconds));
    while start.elapsed() < duration {}
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
