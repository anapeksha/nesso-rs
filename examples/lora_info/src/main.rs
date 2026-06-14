#![no_std]
#![no_main]

use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use nesso::Nesso;

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

    if nesso.display.clear(Rgb565::BLACK).is_err()
        || nesso
            .display
            .print_centered("LoRa Info", 62, Rgb565::CYAN)
            .is_err()
        || nesso
            .display
            .print_centered("SX1262", 96, Rgb565::WHITE)
            .is_err()
        || nesso
            .display
            .print_centered("No TX in this test", 130, Rgb565::GREEN)
            .is_err()
    {
        abort()
    }

    let begin_ok = nesso.lora.begin().is_ok();
    let status_ok = begin_ok && nesso.lora.status().is_ok();
    let (status, color) = match (begin_ok, status_ok) {
        (true, true) => ("Radio ready", Rgb565::GREEN),
        (false, _) => ("Begin failed", Rgb565::RED),
        (true, false) => ("Status failed", Rgb565::RED),
    };
    let _ = nesso.display.print_centered(status, 162, color);

    loop {
        delay.delay_ms(1000);
    }
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
