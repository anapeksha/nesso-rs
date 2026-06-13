#![no_std]
//! Public Rust facade crate for the Arduino Nesso N1.
//!
//! `nesso` is the crate applications should normally depend on. It owns the
//! Nesso N1 board bring-up path and exposes lower-level hardware modules for
//! advanced use.
//!
//! # Example
//!
//! ```rust,ignore
//! #![no_std]
//! #![no_main]
//!
//! use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
//! use embedded_hal::delay::DelayNs;
//! use esp_hal::{clock::CpuClock, delay::Delay, main};
//! use nesso::Nesso;
//!
//! #[main]
//! fn main() -> ! {
//!     let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
//!     let peripherals = esp_hal::init(config);
//!     let mut delay = Delay::new();
//!     let mut nesso = match Nesso::new(peripherals) {
//!         Ok(nesso) => nesso,
//!         Err(_) => esp_hal::system::software_reset(),
//!     };
//!
//!     if nesso.display.clear(Rgb565::BLACK).is_err()
//!         || nesso
//!             .display
//!             .print_centered("Hello from nesso", 120, Rgb565::WHITE)
//!             .is_err()
//!     {
//!         esp_hal::system::software_reset()
//!     }
//!
//!     loop {
//!         delay.delay_ms(1000);
//!     }
//! }
//! ```

/// Audio support for the Nesso N1 passive buzzer.
pub mod audio;
/// Bluetooth Low Energy controller support for the ESP32-C6 radio.
#[cfg(feature = "ble")]
pub mod ble;
/// Board constants and board-specific setup helpers.
pub mod bsp;
/// Display support for the Nesso N1 ST7789P3 LCD.
pub mod display;
/// Environmental sensor unit support.
#[cfg(feature = "env")]
pub mod env;
/// IMU support for the Nesso N1 BMI270.
pub mod imu;
/// Button and input event helpers.
pub mod input;
/// Motion classification helpers built on BMI270 accelerometer samples.
pub mod motion;
/// Battery, charger, and power-management support.
pub mod power;
/// ESP radio runtime startup helpers.
#[cfg(any(feature = "wifi", feature = "ble"))]
pub mod runtime;
/// Caller-owned RGB565 sprite/framebuffer support.
pub mod sprite;
/// Settings and key/value storage primitives.
pub mod storage;
/// Touch support for the Nesso N1 FT6336U controller.
pub mod touch;
/// Small graphics and UI primitives for `embedded-graphics` draw targets.
pub mod ui;
/// Wi-Fi support for the ESP32-C6 radio.
#[cfg(feature = "wifi")]
pub mod wifi;

#[cfg(feature = "ble")]
use crate::ble::Ble;
#[cfg(any(feature = "wifi", feature = "ble"))]
use crate::bsp::RadioRuntimeResources;
#[cfg(feature = "wifi")]
use crate::bsp::WifiResources;
use crate::bsp::{
    BoardInitError, ButtonLevels, NessoBuzzer, NessoDisplay, NessoI2c, NessoN1, NessoN1Board,
};
use crate::imu::{Acceleration, Bmi270, Gyroscope};
use crate::power::{BatteryStatus, Power};
use crate::storage::{EspFlashSettingsStore, SettingsPartition};
use crate::touch::{Touch, TouchEvent, TouchState};
#[cfg(feature = "wifi")]
use crate::wifi::EspRadioWifi;
use esp_hal::delay::Delay;

/// SDK-level error type for facade operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NessoError {
    /// Board resource initialization failed.
    Board(BoardInitError),
    /// Touch controller I2C access failed.
    Touch,
    /// BMI270 initialization or read failed.
    Imu,
    /// Power-management I2C access failed.
    Power,
    /// The flash peripheral has already been moved out of the facade.
    FlashUnavailable,
    /// Flash settings partition configuration is invalid.
    Storage,
    /// The Wi-Fi radio resources have already been moved out of the facade.
    #[cfg(feature = "wifi")]
    WifiUnavailable,
    /// The BLE controller resources have already been moved out of the facade.
    #[cfg(feature = "ble")]
    BleUnavailable,
    /// Shared radio runtime resources have already been moved out of the facade.
    #[cfg(any(feature = "wifi", feature = "ble"))]
    RadioRuntimeUnavailable,
    /// BMI270 has not been initialized with [`Nesso::init_imu`].
    ImuNotInitialized,
    /// Button expander setup or read failed.
    Input,
}

/// Board-owned Nesso N1 SDK.
///
/// `display`, `audio`, and `wifi` own independent hardware resources and are
/// exposed as fields. Touch, IMU, and power share the board I2C bus, so they are
/// exposed as methods that borrow the bus internally.
pub struct Nesso {
    /// ST7789P3 display driver.
    pub display: NessoDisplay,
    /// Passive buzzer driver.
    pub audio: NessoBuzzer,
    i2c: NessoI2c,
    #[cfg(feature = "wifi")]
    wifi: Option<WifiResources>,
    #[cfg(feature = "ble")]
    ble: Option<crate::bsp::BleResources>,
    #[cfg(any(feature = "wifi", feature = "ble"))]
    radio_runtime: Option<RadioRuntimeResources>,
    #[cfg(any(feature = "wifi", feature = "ble"))]
    radio_runtime_started: bool,
    flash: Option<esp_hal::peripherals::FLASH<'static>>,
    imu_initialized: bool,
    previous_touch: TouchState,
}

impl Nesso {
    /// Initializes board-owned Nesso N1 resources from ESP-HAL peripherals.
    pub fn new(peripherals: esp_hal::peripherals::Peripherals) -> Result<Self, NessoError> {
        let parts = NessoN1Board::new(peripherals)
            .into_core_parts()
            .map_err(NessoError::Board)?;
        let nesso = Self {
            display: parts.display,
            audio: parts.buzzer,
            i2c: parts.i2c,
            #[cfg(feature = "wifi")]
            wifi: Some(parts.wifi),
            #[cfg(feature = "ble")]
            ble: Some(parts.ble),
            #[cfg(any(feature = "wifi", feature = "ble"))]
            radio_runtime: Some(parts.radio_runtime),
            #[cfg(any(feature = "wifi", feature = "ble"))]
            radio_runtime_started: false,
            flash: Some(parts.flash),
            imu_initialized: false,
            previous_touch: TouchState::default(),
        };
        Ok(nesso)
    }

    /// Uploads BMI270 configuration and enables accelerometer/gyroscope reads.
    pub fn init_imu(&mut self) -> Result<(), NessoError> {
        Bmi270::new(&mut self.i2c, Delay::new())
            .init()
            .map_err(|_| NessoError::Imu)?;
        self.imu_initialized = true;
        Ok(())
    }

    /// Returns the current touch state from the FT6336U controller.
    pub fn touch_state(&mut self) -> Result<TouchState, NessoError> {
        Touch::new(&mut self.i2c)
            .read_state()
            .map_err(|_| NessoError::Touch)
    }

    /// Polls a touch event while preserving previous touch state in the facade.
    pub fn touch_event(&mut self) -> Result<TouchEvent, NessoError> {
        let current = self.touch_state()?;
        let event = match (self.previous_touch.primary(), current.primary()) {
            (None, Some(point)) => TouchEvent::Pressed(point),
            (Some(_), None) => TouchEvent::Released,
            (Some(previous), Some(point)) if previous != point => TouchEvent::Moved(point),
            _ => TouchEvent::Idle,
        };
        self.previous_touch = current;
        Ok(event)
    }

    /// Configures the board KEY1/KEY2 expander pins as button inputs.
    pub fn init_buttons(&mut self) -> Result<(), NessoError> {
        NessoN1::init_button_inputs(&mut self.i2c).map_err(|_| NessoError::Input)
    }

    /// Returns the current KEY1/KEY2 pressed state.
    pub fn button_levels(&mut self) -> Result<ButtonLevels, NessoError> {
        NessoN1::read_button_levels(&mut self.i2c).map_err(|_| NessoError::Input)
    }

    /// Returns raw BMI270 accelerometer data.
    pub fn acceleration(&mut self) -> Result<Acceleration, NessoError> {
        if !self.imu_initialized {
            return Err(NessoError::ImuNotInitialized);
        }
        Bmi270::new(&mut self.i2c, Delay::new())
            .acceleration()
            .map_err(|_| NessoError::Imu)
    }

    /// Returns raw BMI270 gyroscope data.
    pub fn gyroscope(&mut self) -> Result<Gyroscope, NessoError> {
        if !self.imu_initialized {
            return Err(NessoError::ImuNotInitialized);
        }
        Bmi270::new(&mut self.i2c, Delay::new())
            .gyroscope()
            .map_err(|_| NessoError::Imu)
    }

    /// Returns battery and charger status from the BQ27220/AW32001 devices.
    pub fn battery_status(&mut self) -> Result<BatteryStatus, NessoError> {
        Power::new(&mut self.i2c)
            .battery_status()
            .map_err(|_| NessoError::Power)
    }

    /// Takes the ESP32-C6 radio resources and creates a Wi-Fi station driver.
    ///
    /// Wi-Fi needs the ESP allocator because `esp-radio` returns heap-backed
    /// scan results. Keeping Wi-Fi behind this method lets display, input,
    /// touch, IMU, audio, power, and storage applications avoid a global heap.
    #[cfg(feature = "wifi")]
    pub fn init_wifi(&mut self) -> Result<EspRadioWifi, NessoError> {
        let wifi = self.wifi.take().ok_or(NessoError::WifiUnavailable)?;
        if self.radio_runtime_started {
            return Ok(EspRadioWifi::new_started(wifi));
        }
        let runtime = self
            .radio_runtime
            .take()
            .ok_or(NessoError::RadioRuntimeUnavailable)?;
        Ok(EspRadioWifi::new(wifi, runtime))
    }

    /// Takes the ESP32-C6 Bluetooth resources and creates a BLE controller.
    ///
    /// The SDK owns the board lifecycle and returns an HCI connector wrapper.
    /// GATT services, phone app protocol, and notification semantics are built
    /// above this layer by a BLE host stack.
    #[cfg(feature = "ble")]
    pub fn init_ble(&mut self) -> Result<Ble, NessoError> {
        let ble = self.ble.take().ok_or(NessoError::BleUnavailable)?;
        if self.radio_runtime_started {
            return Ok(Ble::new_started(ble));
        }
        let runtime = self
            .radio_runtime
            .take()
            .ok_or(NessoError::RadioRuntimeUnavailable)?;
        Ok(Ble::new(ble, runtime))
    }

    /// Starts the shared ESP radio runtime before Wi-Fi or BLE tasks run.
    ///
    /// Call this once in async applications that use Wi-Fi, BLE, or both. After
    /// it succeeds, [`Nesso::init_wifi`] and [`Nesso::init_ble`] create
    /// controllers that reuse the already-started runtime. Simple blocking
    /// examples may skip this and let Wi-Fi or BLE start the runtime lazily.
    #[cfg(any(feature = "wifi", feature = "ble"))]
    pub fn start_async_runtime(&mut self) -> Result<(), NessoError> {
        if self.radio_runtime_started {
            return Ok(());
        }

        let runtime = self
            .radio_runtime
            .take()
            .ok_or(NessoError::RadioRuntimeUnavailable)?;
        runtime::start_radio_runtime(runtime);
        self.radio_runtime_started = true;
        Ok(())
    }

    /// Creates a flash-backed settings store at the SDK default partition.
    ///
    /// The default region is documented as [`storage::SettingsPartition::DEFAULT`].
    /// Applications with a custom partition table may prefer
    /// [`Nesso::take_flash_settings_partition`] or [`Nesso::take_flash_settings`].
    pub fn take_default_flash_settings(
        &mut self,
    ) -> Result<EspFlashSettingsStore<'static>, NessoError> {
        self.take_flash_settings_partition(SettingsPartition::DEFAULT)
    }

    /// Creates a flash-backed settings store from a documented flash partition.
    pub fn take_flash_settings_partition(
        &mut self,
        partition: SettingsPartition,
    ) -> Result<EspFlashSettingsStore<'static>, NessoError> {
        let flash = self.flash.take().ok_or(NessoError::FlashUnavailable)?;
        EspFlashSettingsStore::from_partition(flash, partition).map_err(|_| NessoError::Storage)
    }

    /// Creates a flash-backed settings store at an application-selected offset.
    ///
    /// Prefer [`Nesso::take_default_flash_settings`] unless the application has
    /// its own partition table or flash allocation policy.
    pub fn take_flash_settings(
        &mut self,
        offset: u32,
    ) -> Result<EspFlashSettingsStore<'static>, NessoError> {
        let flash = self.flash.take().ok_or(NessoError::FlashUnavailable)?;
        Ok(EspFlashSettingsStore::from_flash(flash, offset))
    }
}
