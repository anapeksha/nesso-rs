#![no_std]
#![no_main]

use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use heapless::String;
use nesso::{
    Nesso,
    input::{Button, ButtonEvent, TouchGesture},
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
    if nesso.init_buttons().is_err() {
        abort()
    }
    let mut key1 = Button::default();
    let mut touch = TouchGesture::default();
    let mut now_ms = 0u32;
    loop {
        now_ms = now_ms.saturating_add(20);
        let levels = match nesso.button_levels() {
            Ok(levels) => levels,
            Err(_) => abort(),
        };
        let button_event = key1.update(levels.key1_pressed, now_ms);
        let touch_state = match nesso.touch_state() {
            Ok(state) => state,
            Err(_) => abort(),
        };
        let touch_event = touch.update(touch_state.primary().map(|point| (point.x, point.y)));
        let rendered_button_event = match button_event {
            Some(event) => event,
            None => ButtonEvent::Released,
        };
        let mut line = String::<64>::new();
        let _ = core::fmt::write(
            &mut line,
            format_args!("{rendered_button_event:?} {touch_event:?}"),
        );
        if nesso.display.clear(Rgb565::BLACK).is_err()
            || nesso
                .display
                .print_centered("Input Events", 84, Rgb565::WHITE)
                .is_err()
            || nesso
                .display
                .print_centered(&line, 116, Rgb565::YELLOW)
                .is_err()
        {
            abort()
        }
        delay.delay_ms(20);
    }
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
