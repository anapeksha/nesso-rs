#![no_std]
#![no_main]

use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use nesso_n1::NessoN1Board;

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut delay = Delay::new();
    let mut display = match NessoN1Board::new(peripherals).into_display() {
        Ok(display) => display,
        Err(_) => abort(),
    };

    if display.clear(Rgb565::BLACK).is_err()
        || display
            .print_centered("Hello World", 94, Rgb565::WHITE)
            .is_err()
        || display
            .print_centered("Nesso N1", 116, Rgb565::CYAN)
            .is_err()
        || display
            .print_centered("ESP32-C6", 130, Rgb565::CYAN)
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
