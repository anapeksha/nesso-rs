#![no_std]
#![no_main]

use core::fmt::Write as _;

use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use heapless::String;
use nesso_n1::NessoN1Board;
use nesso_touch::Touch;

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut delay = Delay::new();
    let (mut display, i2c) = match NessoN1Board::new(peripherals).into_display_and_i2c() {
        Ok(parts) => parts,
        Err(_) => abort(),
    };

    let mut touch = Touch::new(i2c);

    loop {
        let state = match touch.read_state() {
            Ok(state) => state,
            Err(_) => abort(),
        };

        if display.clear(Rgb565::BLACK).is_err()
            || display
                .print_centered("Touch Test", 68, Rgb565::CYAN)
                .is_err()
        {
            abort()
        }

        if let Some(point) = state.primary() {
            let mut line = String::<32>::new();
            let _ = write!(line, "x={} y={}", point.x, point.y);
            if display
                .print_centered("Pressed", 108, Rgb565::GREEN)
                .is_err()
                || display.print_centered(&line, 128, Rgb565::WHITE).is_err()
            {
                abort()
            }
        } else if display
            .print_centered("No touch", 118, Rgb565::WHITE)
            .is_err()
        {
            abort()
        }

        delay.delay_ms(100);
    }
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
