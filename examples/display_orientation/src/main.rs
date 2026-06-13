#![no_std]
#![no_main]

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{Point, RgbColor},
};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use nesso::{Nesso, display::DisplayOrientation, ui};

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

    if nesso
        .display
        .set_orientation(DisplayOrientation::LandscapeClockwise)
        .is_err()
        || nesso.display.clear(Rgb565::BLACK).is_err()
        || nesso
            .display
            .print_centered("Landscape CW", 24, Rgb565::WHITE)
            .is_err()
        || ui::draw_line(
            &mut nesso.display,
            Point::new(24, 96),
            Point::new(216, 96),
            Rgb565::GREEN,
            2,
        )
        .is_err()
        || ui::draw_line(
            &mut nesso.display,
            Point::new(216, 96),
            Point::new(196, 84),
            Rgb565::GREEN,
            2,
        )
        .is_err()
        || ui::draw_line(
            &mut nesso.display,
            Point::new(216, 96),
            Point::new(196, 108),
            Rgb565::GREEN,
            2,
        )
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
