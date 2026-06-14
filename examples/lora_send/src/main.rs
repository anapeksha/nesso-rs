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
            .print_centered("LoRa Send", 56, Rgb565::CYAN)
            .is_err()
        || nesso
            .display
            .print_centered("Attach antenna", 92, Rgb565::YELLOW)
            .is_err()
    {
        abort()
    }

    if option_env!("NESSO_LORA_ALLOW_TX") != Some("1") {
        if nesso
            .display
            .print_centered("TX disabled", 126, Rgb565::GREEN)
            .is_err()
        {
            abort()
        }
        loop {
            delay.delay_ms(1000);
        }
    }

    if nesso
        .display
        .print_centered("Sending packets", 126, Rgb565::WHITE)
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

    let transmit_ready = lora.configure(LoraConfig::new(FREQUENCY_HZ)).is_ok();

    loop {
        if transmit_ready && lora.transmit(b"nesso lora").is_ok() {
            while matches!(lora.tx_done(), Ok(false)) {
                delay.delay_ms(10);
            }
        }
        delay.delay_ms(3000);
    }
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
