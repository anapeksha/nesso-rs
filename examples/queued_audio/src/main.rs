#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main, time::Instant};
use nesso::{Nesso, audio::Tone};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let delay = Delay::new();
    let mut nesso = match Nesso::new(peripherals) {
        Ok(nesso) => nesso,
        Err(_) => abort(),
    };
    let mut next_tone_at_us = 0;
    loop {
        let now_us = Instant::now().duration_since_epoch().as_micros();
        if now_us >= next_tone_at_us && !nesso.audio.is_busy() {
            if nesso.audio.enqueue(Tone::new(3600, 40)).is_err() {
                abort()
            }
            next_tone_at_us = now_us.saturating_add(700_000);
        }
        if nesso.audio.poll(now_us).is_err() {
            abort()
        }
        delay.delay_millis(1);
    }
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
