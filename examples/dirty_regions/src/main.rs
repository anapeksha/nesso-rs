#![no_std]
#![no_main]

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point, RgbColor, Size},
    primitives::Rectangle,
};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use nesso::{
    Nesso,
    sprite::{DirtyRegions, Sprite},
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
    let mut pixels = [Rgb565::BLACK; 96 * 48];
    let mut sprite = match Sprite::new(96, 48, &mut pixels) {
        Ok(sprite) => sprite,
        Err(_) => abort(),
    };
    let mut dirty = DirtyRegions::<4>::new();
    let bounds = sprite.bounds();
    sprite.clear(Rgb565::BLACK);
    if sprite
        .fill_solid(
            &Rectangle::new(Point::new(8, 8), Size::new(36, 18)),
            Rgb565::RED,
        )
        .is_err()
        || sprite
            .fill_solid(
                &Rectangle::new(Point::new(28, 14), Size::new(44, 22)),
                Rgb565::BLUE,
            )
            .is_err()
        || dirty
            .mark(Rectangle::new(Point::new(8, 8), Size::new(36, 18)), bounds)
            .is_err()
        || dirty
            .mark(
                Rectangle::new(Point::new(28, 14), Size::new(44, 22)),
                bounds,
            )
            .is_err()
        || nesso.display.clear(Rgb565::BLACK).is_err()
        || dirty.flush_sprite(&sprite, &mut nesso.display).is_err()
        || nesso
            .display
            .print_centered("Coalesced dirty regions", 120, Rgb565::WHITE)
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
