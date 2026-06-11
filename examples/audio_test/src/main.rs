#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use nesso_audio::Tone;
use nesso_n1::NessoN1Board;

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut delay = Delay::new();
    let mut buzzer = NessoN1Board::new(peripherals).into_buzzer();
    let tone = Tone::new(Tone::DEFAULT_BUZZER_HZ, 250);

    loop {
        let _ = buzzer.play_blocking(tone, &mut delay);
        delay.delay_millis(750);
    }
}
