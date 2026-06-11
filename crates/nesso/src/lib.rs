#![no_std]
#![doc = include_str!("../../../README.md")]

/// Audio support for the Nesso N1 passive buzzer.
pub use nesso_audio as audio;
/// Display support for the Nesso N1 ST7789P3 LCD.
pub use nesso_display as display;
/// IMU support for the Nesso N1 BMI270.
pub use nesso_imu as imu;
/// Button and input event helpers.
pub use nesso_input as input;
/// Board constants and board-specific setup helpers.
pub use nesso_n1 as bsp;
/// Battery, charger, and power-management support.
pub use nesso_power as power;
/// Settings and key/value storage primitives.
pub use nesso_storage as storage;
/// Touch support for the Nesso N1 FT6336U controller.
pub use nesso_touch as touch;
/// Wi-Fi support for the ESP32-C6 radio.
pub use nesso_wifi as wifi;

use bsp::Board;

/// Aggregated Nesso N1 SDK facade.
///
/// This type owns the board marker and all initialized subsystems supplied
/// through [`NessoParts`]. The current constructor is explicit so applications
/// can decide how hardware buses are shared. A higher-level `begin` constructor
/// is planned once the I2C ownership model is finalized.
#[derive(Debug)]
pub struct Nesso<DISPLAY, TOUCH, IMU, POWER, STORAGE, WIFI, AUDIO> {
    /// Board marker and verified Nesso N1 resource state.
    pub board: Board,
    /// Display driver instance.
    pub display: DISPLAY,
    /// Touch controller instance.
    pub touch: TOUCH,
    /// IMU driver instance.
    pub imu: IMU,
    /// Power-management driver instance.
    pub power: POWER,
    /// Settings or flash storage instance.
    pub storage: STORAGE,
    /// Wi-Fi controller instance.
    pub wifi: WIFI,
    /// Audio output instance.
    pub audio: AUDIO,
}

/// Parts used to construct a [`Nesso`] facade.
#[derive(Debug)]
pub struct NessoParts<DISPLAY, TOUCH, IMU, POWER, STORAGE, WIFI, AUDIO> {
    /// Board marker and verified Nesso N1 resource state.
    pub board: Board,
    /// Display driver instance.
    pub display: DISPLAY,
    /// Touch controller instance.
    pub touch: TOUCH,
    /// IMU driver instance.
    pub imu: IMU,
    /// Power-management driver instance.
    pub power: POWER,
    /// Settings or flash storage instance.
    pub storage: STORAGE,
    /// Wi-Fi controller instance.
    pub wifi: WIFI,
    /// Audio output instance.
    pub audio: AUDIO,
}

impl<DISPLAY, TOUCH, IMU, POWER, STORAGE, WIFI, AUDIO>
    Nesso<DISPLAY, TOUCH, IMU, POWER, STORAGE, WIFI, AUDIO>
{
    /// Builds the public facade from already-initialized subsystem parts.
    #[must_use]
    pub fn from_parts(parts: NessoParts<DISPLAY, TOUCH, IMU, POWER, STORAGE, WIFI, AUDIO>) -> Self {
        Self {
            board: parts.board,
            display: parts.display,
            touch: parts.touch,
            imu: parts.imu,
            power: parts.power,
            storage: parts.storage,
            wifi: parts.wifi,
            audio: parts.audio,
        }
    }
}
