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
use heapless::String;
use nesso::Nesso;

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

    if nesso.display.clear(Rgb565::BLACK).is_err()
        || nesso
            .display
            .print_centered("Power Status", 84, Rgb565::WHITE)
            .is_err()
    {
        abort()
    }

    let mut previous_line = String::<48>::new();
    loop {
        let status = match nesso.battery_status() {
            Ok(status) => status,
            Err(_) => abort(),
        };
        let mut line = String::<48>::new();
        let _ = core::fmt::write(
            &mut line,
            format_args!("{}% {}mV", status.percentage, status.voltage_mv),
        );
        if !render_changed_line(&mut nesso.display, &mut previous_line, &line) {
            abort()
        }
        delay.delay_ms(1000);
    }
}

fn render_changed_line(
    display: &mut nesso::bsp::NessoDisplay,
    previous: &mut String<48>,
    text: &str,
) -> bool {
    if previous.as_str() == text {
        return true;
    }

    previous.clear();
    if previous.push_str(text).is_err() {
        return false;
    }

    display
        .clear_region(
            &Rectangle::new(Point::new(0, 100), Size::new(135, 20)),
            Rgb565::BLACK,
        )
        .is_ok()
        && display.print_centered(text, 112, Rgb565::GREEN).is_ok()
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
