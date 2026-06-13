#![no_std]
#![no_main]

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{Point, RgbColor, Size},
    primitives::Rectangle,
};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use nesso::{
    Nesso,
    display::DisplayOrientation,
    sprite::{DirtyRegions, Sprite},
    ui::{LabelStyle, draw_filled_pill, draw_label},
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
    if nesso
        .display
        .set_orientation(DisplayOrientation::LandscapeClockwise)
        .is_err()
    {
        abort()
    }

    let mut pixels = [Rgb565::BLACK; 96 * 32];
    let mut sprite = match Sprite::new(96, 32, &mut pixels) {
        Ok(sprite) => sprite,
        Err(_) => abort(),
    };
    let mut dirty = DirtyRegions::<4>::new();
    let sprite_bounds = sprite.bounds();

    sprite.clear(Rgb565::BLACK);
    if draw_filled_pill(&mut sprite, sprite_bounds, Rgb565::BLUE).is_err()
        || draw_label(
            &mut sprite,
            sprite_bounds,
            "Landscape",
            LabelStyle::centered(Rgb565::WHITE),
        )
        .is_err()
        || dirty.push_clipped(sprite_bounds, sprite_bounds).is_err()
        || nesso.display.clear(Rgb565::BLACK).is_err()
        || dirty.flush_sprite(&sprite, &mut nesso.display).is_err()
        || nesso
            .display
            .fill_rect(
                &Rectangle::new(Point::new(116, 48), Size::new(72, 18)),
                Rgb565::GREEN,
            )
            .is_err()
        || nesso
            .display
            .print_centered("Dirty update", 84, Rgb565::YELLOW)
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
