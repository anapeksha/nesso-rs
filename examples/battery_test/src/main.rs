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

    if nesso.enable_battery_charging().is_err() {
        let _ = nesso
            .display
            .print_centered("Charge enable failed", 88, Rgb565::RED);
    }

    let mut previous = BatteryLines::new();

    loop {
        match nesso.battery_status() {
            Ok(status) => {
                if !render_battery_status(&mut nesso.display, &mut previous, &status) {
                    abort()
                }
            }
            Err(_) => {
                if !render_read_error(&mut nesso.display) {
                    abort()
                }
            }
        }

        delay.delay_ms(1000);
    }
}

fn render_read_error(display: &mut NessoDisplay) -> bool {
    display
        .clear_region(
            &Rectangle::new(Point::new(0, 88), Size::new(135, 100)),
            Rgb565::BLACK,
        )
        .is_ok()
        && display
            .print_centered("Battery read", 112, Rgb565::RED)
            .is_ok()
        && display.print_centered("failed", 136, Rgb565::RED).is_ok()
}

struct BatteryLines {
    voltage: String<32>,
    current: String<32>,
    percent: String<32>,
    charge: String<32>,
}

impl BatteryLines {
    const fn new() -> Self {
        Self {
            voltage: String::new(),
            current: String::new(),
            percent: String::new(),
            charge: String::new(),
        }
    }
}

fn render_battery_status(
    display: &mut NessoDisplay,
    previous: &mut BatteryLines,
    status: &nesso::power::BatteryStatus,
) -> bool {
    let mut voltage_line = String::<32>::new();
    let mut current_line = String::<32>::new();
    let mut percent_line = String::<32>::new();
    let mut charge_line = String::<32>::new();
    let _ = write!(voltage_line, "Voltage {} mV", status.voltage_mv);
    let _ = write!(current_line, "Current {} mA", status.current_ma);
    let _ = write!(percent_line, "Battery {}%", status.percentage);
    let charge = match status.charge {
        ChargeStatus::Unknown => "Charge unknown",
        ChargeStatus::Discharging => "Discharging",
        ChargeStatus::Charging => "Charging",
        ChargeStatus::Full => "Full",
    };
    let _ = write!(charge_line, "{}", charge);

    render_changed_line(
        display,
        &mut previous.voltage,
        96,
        &voltage_line,
        Rgb565::WHITE,
    ) && render_changed_line(
        display,
        &mut previous.current,
        116,
        &current_line,
        Rgb565::WHITE,
    ) && render_changed_line(
        display,
        &mut previous.percent,
        136,
        &percent_line,
        Rgb565::YELLOW,
    ) && render_changed_line(
        display,
        &mut previous.charge,
        164,
        &charge_line,
        Rgb565::GREEN,
    )
}

fn render_changed_line(
    display: &mut NessoDisplay,
    previous: &mut String<32>,
    y: i32,
    text: &str,
    color: Rgb565,
) -> bool {
    if previous.as_str() == text {
        return true;
    }

    previous.clear();
    if previous.push_str(text).is_err() {
        return false;
    }

    let area = Rectangle::new(Point::new(0, y - 10), Size::new(135, 16));
    display.clear_region(&area, Rgb565::BLACK).is_ok()
        && display.print_centered(text, y, color).is_ok()
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
