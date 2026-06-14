#![no_std]
#![no_main]

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{Point, RgbColor, Size},
    primitives::Rectangle,
};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use heapless::String;
use nesso::{
    Nesso,
    motion::{MotionDetector, orientation_kind},
};

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
    if nesso.init_imu().is_err() {
        abort()
    }
    if nesso.display.clear(Rgb565::BLACK).is_err()
        || nesso
            .display
            .print_centered("Motion Status", 84, Rgb565::WHITE)
            .is_err()
    {
        abort()
    }
    let mut detector = MotionDetector::new();
    let mut previous_line = String::<64>::new();
    loop {
        let acceleration = match nesso.acceleration() {
            Ok(acceleration) => acceleration,
            Err(_) => abort(),
        };
        let context = detector.update(acceleration);
        let mut line = String::<64>::new();
        let _ = core::fmt::write(
            &mut line,
            format_args!("{:?} {:?}", orientation_kind(context.pose), context.state),
        );
        if !render_changed_line(&mut nesso.display, &mut previous_line, &line) {
            abort()
        }
        delay.delay_ms(250);
    }
}

fn render_changed_line(
    display: &mut nesso::bsp::NessoDisplay,
    previous: &mut String<64>,
    text: &str,
) -> bool {
    if previous.as_str() == text {
        return true;
    }

    previous.clear();
    if previous.push_str(text).is_err() {
        return false;
    }

    display
        .clear_region(
            &Rectangle::new(Point::new(0, 100), Size::new(135, 20)),
            Rgb565::BLACK,
        )
        .is_ok()
        && display.print_centered(text, 112, Rgb565::CYAN).is_ok()
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
