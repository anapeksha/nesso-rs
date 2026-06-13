#![no_std]
#![no_main]

use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use nesso::{Nesso, storage::SettingsStore};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut delay = Delay::new();
    let mut nesso = match Nesso::new(peripherals) {
        Ok(nesso) => nesso,
        Err(_) => abort(),
    };
    let mut settings = SettingsStore::new();
    if settings.set("brightness", &[80]).is_err()
        || settings.set("sound", &[1]).is_err()
        || settings.get("brightness") != Some(&[80][..])
        || !settings.remove("sound")
    {
        abort()
    }
    if nesso.display.clear(Rgb565::BLACK).is_err()
        || nesso
            .display
            .print_centered("Storage Settings", 92, Rgb565::WHITE)
            .is_err()
        || nesso
            .display
            .print_centered("set/get/remove ok", 118, Rgb565::GREEN)
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
