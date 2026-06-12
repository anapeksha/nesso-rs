#![no_std]
#![no_main]

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{RgbColor, Size},
};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use nesso::{
    Nesso,
    ui::{Insets, LabelStyle, ScreenLayout, draw_label, draw_progress_bar},
};

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

    let layout = ScreenLayout::new(Size::new(135, 240));
    if nesso.display.clear(Rgb565::BLACK).is_err()
        || draw_label(
            &mut nesso.display,
            layout.row(44, 24, Insets::symmetric(8, 0)),
            "UI Test",
            LabelStyle::centered(Rgb565::CYAN),
        )
        .is_err()
        || draw_label(
            &mut nesso.display,
            layout.row(84, 24, Insets::symmetric(8, 0)),
            "Layout + text",
            LabelStyle::centered(Rgb565::WHITE),
        )
        .is_err()
        || draw_progress_bar(
            &mut nesso.display,
            layout.row(126, 12, Insets::symmetric(18, 0)),
            72,
            Rgb565::GREEN,
            Rgb565::new(2, 2, 2),
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
