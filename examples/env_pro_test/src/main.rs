#![no_std]
#![no_main]

use core::fmt::Write as _;

use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use heapless::String;
use nesso::{
    bsp::{NessoDisplay, NessoEnv, NessoN1Board},
    env::EnvMeasurement,
};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    run(peripherals)
}

fn run(peripherals: esp_hal::peripherals::Peripherals) -> ! {
    let (display, env) = match NessoN1Board::new(peripherals).into_display_and_env() {
        Ok(parts) => parts,
        Err(_) => abort(),
    };
    init_env_and_loop(display, env)
}

fn init_env_and_loop(mut display: NessoDisplay, env: NessoEnv) -> ! {
    let mut delay = Delay::new();
    show_status(&mut display, "ENV Pro", "BME688 ready", Rgb565::GREEN);
    delay.delay_ms(500);
    read_loop(display, env)
}

fn read_loop(mut display: NessoDisplay, mut env: NessoEnv) -> ! {
    let mut delay = Delay::new();
    loop {
        let measurement = match env.measure() {
            Ok(measurement) => measurement,
            Err(_) => {
                show_status(&mut display, "ENV Pro", "Read error", Rgb565::RED);
                delay.delay_ms(1000);
                continue;
            }
        };

        if !render_measurement(&mut display, measurement) {
            abort()
        }

        delay.delay_ms(2500);
    }
}

fn render_measurement(display: &mut NessoDisplay, measurement: EnvMeasurement) -> bool {
    if display.clear(Rgb565::BLACK).is_err()
        || display.print_centered("ENV Pro", 34, Rgb565::CYAN).is_err()
    {
        return false;
    }

    let mut temperature = String::<32>::new();
    let mut humidity = String::<32>::new();
    let mut pressure = String::<32>::new();
    let mut gas = String::<32>::new();
    let _ = write!(temperature, "Temp {:.1} C", measurement.temperature_c);
    let _ = write!(humidity, "Humidity {:.1}%", measurement.humidity_percent);
    let _ = write!(pressure, "Pressure {:.1} hPa", measurement.pressure_hpa);
    match measurement.gas_resistance_ohm {
        Some(value) => {
            let _ = write!(gas, "Gas {:.0} ohm", value);
        }
        None => {
            let _ = write!(gas, "Gas warming");
        }
    }

    display
        .print_centered(&temperature, 74, Rgb565::WHITE)
        .is_ok()
        && display.print_centered(&humidity, 98, Rgb565::WHITE).is_ok()
        && display
            .print_centered(&pressure, 122, Rgb565::WHITE)
            .is_ok()
        && display.print_centered(&gas, 154, Rgb565::YELLOW).is_ok()
}

fn show_status(display: &mut NessoDisplay, title: &str, detail: &str, color: Rgb565) {
    let _ = display.clear(Rgb565::BLACK);
    let _ = display.print_centered(title, 86, Rgb565::CYAN);
    let _ = display.print_centered(detail, 118, color);
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
