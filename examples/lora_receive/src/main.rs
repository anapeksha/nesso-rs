#![no_std]
#![no_main]

use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use nesso::{Nesso, lora::LoraConfig};

esp_bootloader_esp_idf::esp_app_desc!();

const FREQUENCY_HZ: u32 = 915_000_000;

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
            .print_centered("LoRa Receive", 62, Rgb565::CYAN)
            .is_err()
        || nesso
            .display
            .print_centered("RX continuous", 96, Rgb565::WHITE)
            .is_err()
        || nesso
            .display
            .print_centered("No transmit", 130, Rgb565::GREEN)
            .is_err()
    {
        abort()
    }

    let mut lora = match nesso.into_lora() {
        Ok(lora) => lora,
        Err(_) => loop {
            delay.delay_ms(1000);
        },
    };

    let receive_ready =
        lora.configure(LoraConfig::new(FREQUENCY_HZ)).is_ok() && lora.start_receive().is_ok();

    let mut packet = [0u8; 64];
    loop {
        if receive_ready {
            let _received = lora.read_packet(&mut packet);
        }
        delay.delay_ms(100);
    }
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
