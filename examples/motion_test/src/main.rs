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
use nesso::{
    Nesso,
    motion::{MotionDetector, MotionState, Pose},
};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    run(peripherals)
}

fn run(peripherals: esp_hal::peripherals::Peripherals) -> ! {
    let mut delay = Delay::new();
    let mut nesso = match Nesso::new(peripherals) {
        Ok(nesso) => nesso,
        Err(_) => abort(),
    };
    if nesso.init_imu().is_err()
        || nesso.display.clear(Rgb565::BLACK).is_err()
        || nesso
            .display
            .print_centered("Motion Context", 56, Rgb565::CYAN)
            .is_err()
    {
        abort()
    }

    let mut detector = MotionDetector::new();
    let mut previous = MotionLines::new();
    loop {
        let acceleration = match nesso.acceleration() {
            Ok(acceleration) => acceleration,
            Err(_) => abort(),
        };
        let context = detector.update(acceleration);
        let mut pose = String::<32>::new();
        let mut state = String::<32>::new();
        let _ = write!(pose, "Pose {}", pose_label(context.pose));
        let _ = write!(state, "State {}", state_label(context.state));

        if !render_changed_line(
            &mut nesso.display,
            &mut previous.pose,
            102,
            &pose,
            Rgb565::WHITE,
        ) || !render_changed_line(
            &mut nesso.display,
            &mut previous.state,
            128,
            &state,
            Rgb565::YELLOW,
        ) {
            abort()
        }

        delay.delay_ms(180);
    }
}

struct MotionLines {
    pose: String<32>,
    state: String<32>,
}

impl MotionLines {
    const fn new() -> Self {
        Self {
            pose: String::new(),
            state: String::new(),
        }
    }
}

fn render_changed_line(
    display: &mut nesso::bsp::NessoDisplay,
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

fn pose_label(pose: Pose) -> &'static str {
    match pose {
        Pose::FaceUp => "face up",
        Pose::FaceDown => "face down",
        Pose::PortraitUp => "portrait up",
        Pose::PortraitDown => "portrait down",
        Pose::LandscapeLeft => "landscape left",
        Pose::LandscapeRight => "landscape right",
    }
}

fn state_label(state: MotionState) -> &'static str {
    match state {
        MotionState::Unknown => "warming",
        MotionState::Still => "still",
        MotionState::Moving => "moving",
    }
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
