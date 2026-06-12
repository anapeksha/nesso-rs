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
            acceleration.x_mg,
            acceleration.y_mg,
            acceleration.z_mg,
        ) {
            abort()
        }

        delay.delay_ms(150);
    }
}

fn render_imu_data(display: &mut NessoDisplay, x: i16, y: i16, z: i16) -> bool {
    if display
        .clear_region(
            &Rectangle::new(Point::new(0, 88), Size::new(135, 110)),
            Rgb565::BLACK,
        )
        .is_err()
    {
        return false;
    }

    let mut x_line = String::<32>::new();
    let mut y_line = String::<32>::new();
    let mut z_line = String::<32>::new();
    let mut axis_line = String::<32>::new();
    let _ = write!(x_line, "X raw {}", x);
    let _ = write!(y_line, "Y raw {}", y);
    let _ = write!(z_line, "Z raw {}", z);
    let _ = write!(axis_line, "Gravity {}", dominant_axis(x, y, z));

    display.print_centered(&x_line, 102, Rgb565::WHITE).is_ok()
        && display.print_centered(&y_line, 122, Rgb565::WHITE).is_ok()
        && display.print_centered(&z_line, 142, Rgb565::WHITE).is_ok()
        && display
            .print_centered(&axis_line, 172, Rgb565::YELLOW)
            .is_ok()
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
