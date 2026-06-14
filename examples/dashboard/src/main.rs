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
    display::DisplayOrientation,
    motion::{MotionDetector, orientation_kind},
    power::ChargeStatus,
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

    if nesso
        .display
        .set_orientation(DisplayOrientation::LandscapeClockwise)
        .is_err()
        || nesso.display.clear(Rgb565::BLACK).is_err()
        || nesso
            .display
            .print_centered("Nesso Dashboard", 22, Rgb565::CYAN)
            .is_err()
    {
        abort()
    }

    let _charge_enabled = nesso.enable_battery_charging();
    let _imu_ready = nesso.init_imu();
    let mut motion = MotionDetector::new();
    let mut previous = DashboardLines::new();

    loop {
        let mut battery_line = String::<48>::new();
        match nesso.battery_status() {
            Ok(status) => {
                let charge = match status.charge {
                    ChargeStatus::Charging => "charging",
                    ChargeStatus::Full => "full",
                    ChargeStatus::Discharging => "battery",
                    ChargeStatus::Unknown => "unknown",
                };
                let _ = write!(
                    battery_line,
                    "Battery {:3}% {}mV {}",
                    status.percentage, status.voltage_mv, charge
                );
            }
            Err(_) => {
                let _ = battery_line.push_str("Battery unavailable");
            }
        }

        let mut motion_line = String::<48>::new();
        match nesso.acceleration() {
            Ok(acceleration) => {
                let sample = motion.update(acceleration);
                let _ = write!(
                    motion_line,
                    "{:?} {:?}",
                    orientation_kind(sample.pose),
                    sample.state
                );
            }
            Err(_) => {
                let _ = motion_line.push_str("IMU unavailable");
            }
        }

        let mut touch_line = String::<48>::new();
        match nesso.touch_state() {
            Ok(touch) => {
                if let Some(point) = touch.primary() {
                    let point = point.oriented(
                        nesso.display.geometry(),
                        DisplayOrientation::LandscapeClockwise,
                    );
                    let _ = write!(touch_line, "Touch {},{}", point.x, point.y);
                } else {
                    let _ = touch_line.push_str("Touch idle");
                }
            }
            Err(_) => {
                let _ = touch_line.push_str("Touch unavailable");
            }
        }

        if !render_changed_line(
            &mut nesso.display,
            &mut previous.battery,
            58,
            &battery_line,
            Rgb565::GREEN,
        ) || !render_changed_line(
            &mut nesso.display,
            &mut previous.motion,
            90,
            &motion_line,
            Rgb565::YELLOW,
        ) || !render_changed_line(
            &mut nesso.display,
            &mut previous.touch,
            122,
            &touch_line,
            Rgb565::WHITE,
        ) {
            abort()
        }

        delay.delay_ms(500);
    }
}

struct DashboardLines {
    battery: String<48>,
    motion: String<48>,
    touch: String<48>,
}

impl DashboardLines {
    const fn new() -> Self {
        Self {
            battery: String::new(),
            motion: String::new(),
            touch: String::new(),
        }
    }
}

fn render_changed_line(
    display: &mut nesso::bsp::NessoDisplay,
    previous: &mut String<48>,
    baseline_y: i32,
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

    display
        .clear_region(
            &Rectangle::new(Point::new(0, baseline_y - 12), Size::new(240, 20)),
            Rgb565::BLACK,
        )
        .is_ok()
        && display.print_centered(text, baseline_y, color).is_ok()
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
