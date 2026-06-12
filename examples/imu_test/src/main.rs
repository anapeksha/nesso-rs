#![no_std]
#![no_main]

use core::fmt::Write as _;

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{Point, RgbColor, Size},
};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use heapless::String;
use nesso::{Nesso, bsp::NessoDisplay};

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

    if nesso.display.clear(Rgb565::BLACK).is_err() {
        abort()
    }

    show_imu_status(
        &mut nesso.display,
        "BMI270",
        "Uploading config",
        Rgb565::YELLOW,
    );
    if nesso.init_imu().is_err() {
        show_imu_status(
            &mut nesso.display,
            "BMI270 config",
            "Init failed",
            Rgb565::RED,
        );
        abort()
    }
    show_imu_status(&mut nesso.display, "BMI270", "Configured", Rgb565::GREEN);

    delay.delay_ms(100);
    if nesso.display.clear(Rgb565::BLACK).is_err()
        || nesso
            .display
            .print_centered("IMU Tilt Test", 42, Rgb565::CYAN)
            .is_err()
        || nesso
            .display
            .print_centered("BMI270 live", 64, Rgb565::GREEN)
            .is_err()
    {
        abort()
    }

    let mut previous = ImuLines::new();

    loop {
        let acceleration = match nesso.acceleration() {
            Ok(acceleration) => acceleration,
            Err(_) => {
                show_imu_status(&mut nesso.display, "BMI270", "Read error", Rgb565::RED);
                delay.delay_ms(500);
                continue;
            }
        };

        if !render_imu_data(
            &mut nesso.display,
            &mut previous,
            acceleration.x_mg,
            acceleration.y_mg,
            acceleration.z_mg,
        ) {
            abort()
        }

        delay.delay_ms(150);
    }
}

struct ImuLines {
    x: String<32>,
    y: String<32>,
    z: String<32>,
    axis: String<32>,
}

impl ImuLines {
    const fn new() -> Self {
        Self {
            x: String::new(),
            y: String::new(),
            z: String::new(),
            axis: String::new(),
        }
    }
}

fn render_imu_data(
    display: &mut NessoDisplay,
    previous: &mut ImuLines,
    x: i16,
    y: i16,
    z: i16,
) -> bool {
    let mut x_line = String::<32>::new();
    let mut y_line = String::<32>::new();
    let mut z_line = String::<32>::new();
    let mut axis_line = String::<32>::new();
    let _ = write!(x_line, "X raw {}", x);
    let _ = write!(y_line, "Y raw {}", y);
    let _ = write!(z_line, "Z raw {}", z);
    let _ = write!(axis_line, "Gravity {}", dominant_axis(x, y, z));

    render_changed_line(display, &mut previous.x, 102, &x_line, Rgb565::WHITE)
        && render_changed_line(display, &mut previous.y, 122, &y_line, Rgb565::WHITE)
        && render_changed_line(display, &mut previous.z, 142, &z_line, Rgb565::WHITE)
        && render_changed_line(display, &mut previous.axis, 172, &axis_line, Rgb565::YELLOW)
}

fn render_changed_line(
    display: &mut NessoDisplay,
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

    let area =
        embedded_graphics::primitives::Rectangle::new(Point::new(0, y - 10), Size::new(135, 16));
    display.clear_region(&area, Rgb565::BLACK).is_ok()
        && display.print_centered(text, y, color).is_ok()
}

fn show_imu_status(display: &mut NessoDisplay, status: &str, detail: &str, color: Rgb565) {
    let _ = display.clear(Rgb565::BLACK);
    let _ = display.print_centered("IMU Test", 76, Rgb565::CYAN);
    let _ = display.print_centered(status, 108, color);
    let _ = display.print_centered(detail, 128, Rgb565::WHITE);
}

fn dominant_axis(x: i16, y: i16, z: i16) -> &'static str {
    let x = i32::from(x);
    let y = i32::from(y);
    let z = i32::from(z);
    let abs_x = x.abs();
    let abs_y = y.abs();
    let abs_z = z.abs();

    if abs_x >= abs_y && abs_x >= abs_z {
        if x >= 0 { "+X" } else { "-X" }
    } else if abs_y >= abs_z {
        if y >= 0 { "+Y" } else { "-Y" }
    } else if z >= 0 {
        "+Z"
    } else {
        "-Z"
    }
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
