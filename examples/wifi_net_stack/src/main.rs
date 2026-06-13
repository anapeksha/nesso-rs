#![no_std]
#![no_main]

use core::task::Waker;

use embassy_net::{Config, StackResources};
use embassy_time_driver::Driver;
use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main, time::Instant};
use nesso::{Nesso, wifi::Credentials};

esp_bootloader_esp_idf::esp_app_desc!();
embassy_time_driver::time_driver_impl!(static DRIVER: ExampleTimeDriver = ExampleTimeDriver);

struct ExampleTimeDriver;

impl Driver for ExampleTimeDriver {
    fn now(&self) -> u64 {
        Instant::now().duration_since_epoch().as_micros()
    }

    fn schedule_wake(&self, _at: u64, waker: &Waker) {
        waker.wake_by_ref();
    }
}

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 64 * 1024);
    esp_alloc::heap_allocator!(size: 36 * 1024);
    run(peripherals)
}

fn run(peripherals: esp_hal::peripherals::Peripherals) -> ! {
    let mut delay = Delay::new();
    let mut nesso = match Nesso::new(peripherals) {
        Ok(nesso) => nesso,
        Err(_) => abort(),
    };
    if nesso.start_async_runtime().is_err() {
        abort()
    }

    let ssid = env_or_empty(option_env!("NESSO_WIFI_SSID"));
    let password = env_or_empty(option_env!("NESSO_WIFI_PASSWORD"));
    if ssid.is_empty() {
        let _ = nesso.display.clear(Rgb565::BLACK);
        let _ = nesso
            .display
            .print_centered("Set NESSO_WIFI_SSID", 104, Rgb565::YELLOW);
        loop {
            delay.delay_ms(1000);
        }
    }

    let credentials = match Credentials::new(ssid, password) {
        Ok(credentials) => credentials,
        Err(_) => abort(),
    };
    let mut wifi = match nesso.init_wifi() {
        Ok(wifi) => wifi,
        Err(_) => abort(),
    };

    if wifi.connect(&credentials).is_err() {
        abort()
    }

    let interfaces = match wifi.take_interfaces() {
        Ok(interfaces) => interfaces,
        Err(_) => abort(),
    };

    let mut resources = StackResources::<3>::new();
    let (_stack, _runner) = embassy_net::new(
        interfaces.station,
        Config::dhcpv4(Default::default()),
        &mut resources,
        0x4E45_5353,
    );

    if nesso.display.clear(Rgb565::BLACK).is_err()
        || nesso
            .display
            .print_centered("WiFi Net Stack", 76, Rgb565::CYAN)
            .is_err()
        || nesso
            .display
            .print_centered("embassy-net ready", 112, Rgb565::GREEN)
            .is_err()
    {
        abort()
    }

    loop {
        delay.delay_ms(1000);
    }
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}

fn env_or_empty(value: Option<&'static str>) -> &'static str {
    value.map_or("", core::convert::identity)
}
