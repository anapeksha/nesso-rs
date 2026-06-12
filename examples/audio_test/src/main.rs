#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use nesso::{Nesso, audio::Tone};

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
    let tone = Tone::new(Tone::DEFAULT_BUZZER_HZ, 250);

    loop {
        let _ = nesso.audio.play_blocking(tone, &mut delay);
        delay.delay_millis(750);
    }
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
