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
use nesso::{Nesso, bsp::NessoDisplay, power::ChargeStatus};

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
            .print_centered("Battery Test", 56, Rgb565::CYAN)
            .is_err()
    {
        abort()
    }

    loop {
        let status = match nesso.battery_status() {
            Ok(status) => status,
            Err(_) => abort(),
        };

        if !render_battery_status(&mut nesso.display, &status) {
            abort()
        }

        delay.delay_ms(1000);
    }
}

fn render_battery_status(display: &mut NessoDisplay, status: &nesso::power::BatteryStatus) -> bool {
    if display
        .clear_region(
            &Rectangle::new(Point::new(0, 82), Size::new(135, 110)),
            Rgb565::BLACK,
        )
        .is_err()
    {
        return false;
    }

    let mut voltage_line = String::<32>::new();
    let mut current_line = String::<32>::new();
    let mut percent_line = String::<32>::new();
    let _ = write!(voltage_line, "Voltage {} mV", status.voltage_mv);
    let _ = write!(current_line, "Current {} mA", status.current_ma);
    let _ = write!(percent_line, "Battery {}%", status.percentage);
    let charge_line = match status.charge {
        ChargeStatus::Unknown => "Charge unknown",
        ChargeStatus::Discharging => "Discharging",
        ChargeStatus::Charging => "Charging",
        ChargeStatus::Full => "Full",
    };

    display
        .print_centered(&voltage_line, 96, Rgb565::WHITE)
        .is_ok()
        && display
            .print_centered(&current_line, 116, Rgb565::WHITE)
            .is_ok()
        && display
            .print_centered(&percent_line, 136, Rgb565::YELLOW)
            .is_ok()
        && display
            .print_centered(charge_line, 164, Rgb565::GREEN)
            .is_ok()
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
