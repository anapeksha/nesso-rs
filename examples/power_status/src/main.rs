#![no_std]
#![no_main]

use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
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
        if nesso.display.clear(Rgb565::BLACK).is_err()
            || nesso
                .display
                .print_centered("Power Status", 84, Rgb565::WHITE)
                .is_err()
            || nesso
                .display
                .print_centered(&line, 112, Rgb565::GREEN)
                .is_err()
        {
            abort()
        }
        delay.delay_ms(1000);
    }
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
