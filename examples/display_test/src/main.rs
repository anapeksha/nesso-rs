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
    sprite::{DirtyRectTracker, MaskedSprite},
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

    let mut sprite = MaskedSprite::<96, 32>::new(Rgb565::BLACK);
    let mut dirty = DirtyRectTracker::<4>::new();
    let sprite_bounds = sprite.bounds();

    if draw_filled_pill(&mut sprite, sprite_bounds, Rgb565::BLUE).is_err()
        || draw_label(
            &mut sprite,
            sprite_bounds,
            "MASKED",
            LabelStyle::centered(Rgb565::WHITE),
        )
        .is_err()
        || nesso.display.clear(Rgb565::BLACK).is_err()
        || nesso
            .display
            .print_centered("v0.2.4 display paths", 18, Rgb565::YELLOW)
            .is_err()
    {
        abort()
    }

    let left = Point::new(8, 52);
    let right = Point::new(136, 52);
    let mut current = right;
    let mut next = left;

    loop {
        let old_rect = Rectangle::new(current, Size::new(96, 32));
        let new_rect = Rectangle::new(next, Size::new(96, 32));
        if dirty.register_movement(old_rect, new_rect).is_err()
            || dirty
                .update_screen(&mut nesso.display, Rgb565::BLACK, |display, dirty_area| {
                    if !dirty_area.intersection(&new_rect).is_zero_sized() {
                        display.draw_masked_sprite(next, &sprite)?;
                    }
                    Ok(())
                })
                .is_err()
        {
            abort()
        }

        current = next;
        next = if next == left { right } else { left };
        delay.delay_ms(750);
    }
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
