#![no_std]
#![no_main]

use core::fmt::Write as _;

use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use heapless::String;
use nesso_n1::{NessoDisplay, NessoN1Board};
use nesso_power::{ChargeStatus, Power};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut delay = Delay::new();
    let (mut display, i2c) = match NessoN1Board::new(peripherals).into_display_and_i2c() {
        Ok(parts) => parts,
        Err(_) => abort(),
    };

    let mut power = Power::new(i2c);

    loop {
        let status = match power.battery_status() {
            Ok(status) => status,
            Err(_) => abort(),
        };

        if !render_battery_status(&mut display, &status) {
            abort()
        }

        delay.delay_ms(1000);
    }
}

fn render_battery_status(display: &mut NessoDisplay, status: &nesso_power::BatteryStatus) -> bool {
    if display.clear(Rgb565::BLACK).is_err()
        || display
            .print_centered("Battery Test", 56, Rgb565::CYAN)
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
