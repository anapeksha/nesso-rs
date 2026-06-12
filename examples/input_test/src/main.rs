#![no_std]
#![no_main]

use core::fmt::Write as _;

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{Point, RgbColor, Size},
    primitives::Rectangle,
};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use heapless::String;
use nesso::{
    Nesso,
    bsp::{ButtonLevels, NessoDisplay},
    input::{Button, ButtonEvent},
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
    if nesso.display.clear(Rgb565::BLACK).is_err()
        || nesso
            .display
            .print_centered("Input Test", 54, Rgb565::CYAN)
            .is_err()
    {
        abort()
    }

    let mut key1 = Button::default();
    let mut key2 = Button::default();
    let mut now_ms = 0u32;
    let mut last_event = "Waiting";
    let mut previous_levels = ButtonLevels {
        key1_pressed: true,
        key2_pressed: true,
        raw_input: 0,
    };
    let mut previous_event = "";

    loop {
        let levels = match nesso.button_levels() {
            Ok(levels) => levels,
            Err(_) => abort(),
        };

        if let Some(event) = key1.update(levels.key1_pressed, now_ms) {
            last_event = event_name("KEY1", event);
        }
        if let Some(event) = key2.update(levels.key2_pressed, now_ms) {
            last_event = event_name("KEY2", event);
        }

        if (levels != previous_levels || last_event != previous_event)
            && !render_input_state(&mut nesso.display, levels, last_event)
        {
            abort()
        }
        previous_levels = levels;
        previous_event = last_event;

        delay.delay_ms(100);
        now_ms = now_ms.saturating_add(100);
    }
}

fn render_input_state(display: &mut NessoDisplay, levels: ButtonLevels, last_event: &str) -> bool {
    let pressed_line = match (levels.key1_pressed, levels.key2_pressed) {
        (true, true) => "Pressed: KEY1+KEY2",
        (true, false) => "Pressed: KEY1",
        (false, true) => "Pressed: KEY2",
        (false, false) => "Pressed: none",
    };
    let mut key1_line = String::<32>::new();
    let mut key2_line = String::<32>::new();
    let mut raw_line = String::<32>::new();
    let _ = write!(
        key1_line,
        "KEY1 {}",
        if levels.key1_pressed {
            "pressed"
        } else {
            "released"
        }
    );
    let _ = write!(
        key2_line,
        "KEY2 {}",
        if levels.key2_pressed {
            "pressed"
        } else {
            "released"
        }
    );
    let _ = write!(raw_line, "Raw 0x{:02X}", levels.raw_input);

    clear_line(display, 78)
        && display
            .print_centered(pressed_line, 78, Rgb565::GREEN)
            .is_ok()
        && clear_line(display, 104)
        && display
            .print_centered(&key1_line, 104, Rgb565::WHITE)
            .is_ok()
        && clear_line(display, 126)
        && display
            .print_centered(&key2_line, 126, Rgb565::WHITE)
            .is_ok()
        && clear_line(display, 148)
        && display
            .print_centered(&raw_line, 148, Rgb565::YELLOW)
            .is_ok()
        && clear_line(display, 176)
        && display
            .print_centered(last_event, 176, Rgb565::CYAN)
            .is_ok()
}

fn clear_line(display: &mut NessoDisplay, baseline_y: i32) -> bool {
    display
        .clear_region(
            &Rectangle::new(Point::new(0, baseline_y - 12), Size::new(135, 16)),
            Rgb565::BLACK,
        )
        .is_ok()
}

fn event_name(key: &'static str, event: ButtonEvent) -> &'static str {
    match (key, event) {
        ("KEY1", ButtonEvent::Pressed) => "KEY1 pressed",
        ("KEY1", ButtonEvent::Released) => "KEY1 released",
        ("KEY1", ButtonEvent::Held) => "KEY1 held",
        ("KEY1", ButtonEvent::Clicked) => "KEY1 clicked",
        ("KEY1", ButtonEvent::DoubleClicked) => "KEY1 double",
        ("KEY2", ButtonEvent::Pressed) => "KEY2 pressed",
        ("KEY2", ButtonEvent::Released) => "KEY2 released",
        ("KEY2", ButtonEvent::Held) => "KEY2 held",
        ("KEY2", ButtonEvent::Clicked) => "KEY2 clicked",
        ("KEY2", ButtonEvent::DoubleClicked) => "KEY2 double",
        _ => "Input event",
    }
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
