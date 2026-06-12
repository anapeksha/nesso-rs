#![no_std]
#![no_main]

use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use nesso::{Nesso, bsp::NessoDisplay, storage::SettingsStore};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let storage_ok = validate_settings();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut delay = Delay::new();
    let mut nesso = match Nesso::new(peripherals) {
        Ok(nesso) => nesso,
        Err(_) => abort(),
    };

    if !storage_ok {
        abort()
    }

    if !render_storage_state(&mut nesso.display) {
        abort()
    }

    loop {
        delay.delay_ms(1000);
    }
}

fn validate_settings() -> bool {
    let mut settings = SettingsStore::new();
    settings.set("mode", b"demo").is_ok()
        && settings.set("boot", b"ok").is_ok()
        && settings.len() == 2
        && settings.get("mode").and_then(as_utf8) == Some("demo")
        && settings.get("boot").and_then(as_utf8) == Some("ok")
}

fn render_storage_state(display: &mut NessoDisplay) -> bool {
    display.clear(Rgb565::BLACK).is_ok()
        && display
            .print_centered("Storage Test", 54, Rgb565::CYAN)
            .is_ok()
        && display
            .print_centered("Storage OK", 94, Rgb565::GREEN)
            .is_ok()
        && display
            .print_centered("mode=demo", 116, Rgb565::WHITE)
            .is_ok()
        && display
            .print_centered("boot=ok", 138, Rgb565::WHITE)
            .is_ok()
}

fn as_utf8(bytes: &[u8]) -> Option<&str> {
    core::str::from_utf8(bytes).ok()
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
