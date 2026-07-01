//! Board support package for the Arduino Nesso N1.
//!
//! This crate owns Nesso N1 pin mappings, fixed bus configuration, and concrete
//! board bring-up helpers. It is intentionally specific to the Nesso N1 and does
//! not provide a generic board abstraction layer.

use core::cell::RefCell;

use crate::audio::Buzzer;
use crate::display::{
    BusConfig, Display, DisplayError, DisplayGeometry, NullOutputPin, PanelConfig,
};
#[cfg(feature = "env")]
use crate::env::{EnvError, EnvMeasurement, EnvPro};
use critical_section::Mutex;
use defmt::Format;
use embedded_hal::{
    delay::DelayNs,
    i2c::{ErrorKind, ErrorType, I2c, NoAcknowledgeSource, Operation},
};
use embedded_hal_bus::{i2c, spi};
use esp_hal::{
    Blocking,
    delay::Delay,
    gpio::{DriveMode, Flex, Level, Output, OutputConfig},
    i2c::master::{Config as I2cConfig, I2c as EspI2c},
    spi::{
        Mode,
        master::{Config as SpiConfig, Spi},
    },
    time::Rate,
};
use static_cell::StaticCell;

static SPI2_BUS: StaticCell<Mutex<RefCell<NessoRawSpi>>> = StaticCell::new();
static I2C0_BUS: StaticCell<Mutex<RefCell<NessoRawI2c>>> = StaticCell::new();

#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub enum BoardError {
    /// A board resource was requested more than once.
    ResourceConflict,
}

/// Errors returned while constructing concrete Nesso N1 peripherals.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub enum BoardInitError {
    /// I2C controller setup failed.
    I2c,
    /// SPI controller setup failed.
    Spi,
    /// I/O expander setup failed.
    Expander,
    /// Display initialization failed.
    Display,
    /// A static shared bus was already initialized.
    SharedBus,
}

/// AW9523-compatible expander output-enable register.
pub const EXPANDER_OUTPUT_ENABLE: u8 = 0x03;
/// Expander global-control register used by the official Nesso N1 bring-up.
pub const EXPANDER_GLOBAL_CONTROL: u8 = 0x01;
/// Global-control value used before configuring Nesso N1 expander pins.
pub const EXPANDER_GLOBAL_CONTROL_ENABLE: u8 = 0x01;
/// AW9523-compatible expander output-state register.
pub const EXPANDER_OUTPUT_STATE: u8 = 0x05;
/// AW9523-compatible expander high-impedance register.
pub const EXPANDER_HIGH_IMPEDANCE: u8 = 0x07;
/// AW9523-compatible expander default-output register.
pub const EXPANDER_DEFAULT_OUTPUT: u8 = 0x09;
/// PI4IOE5V6408 pull-enable register.
pub const EXPANDER_PULL_ENABLE: u8 = 0x0B;
/// PI4IOE5V6408 pull-select register. A set bit selects pull-up.
pub const EXPANDER_PULL_SELECT: u8 = 0x0D;
/// PI4IOE5V6408 input-state register.
pub const EXPANDER_INPUT_STATE: u8 = 0x0F;
/// AW9523-compatible expander interrupt-mask register.
pub const EXPANDER_INTERRUPT_MASK: u8 = 0x11;
/// AW9523-compatible expander interrupt-status register.
pub const EXPANDER_INTERRUPT_STATUS: u8 = 0x13;

/// 7-bit I2C device address used by a board component.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub struct I2cAddress(pub u8);

/// ESP32-C6 GPIO number used by a board signal.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub struct Gpio(pub u8);

/// Pin exposed by one of the Nesso N1 I/O expanders.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub struct ExpanderPin {
    /// I2C address of the expander.
    pub address: I2cAddress,
    /// Expander pin number.
    pub pin: u8,
}

/// Current logical state of the two board buttons.
#[derive(Clone, Copy, Debug, Format, Default, Eq, PartialEq)]
pub struct ButtonLevels {
    /// True when KEY1 is pressed.
    pub key1_pressed: bool,
    /// True when KEY2 is pressed.
    pub key2_pressed: bool,
    /// Raw PI4IOE5V6408 input-state register value.
    pub raw_input: u8,
}

/// Location of a board signal.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub enum Signal {
    /// Signal connected directly to an ESP32-C6 GPIO.
    Native(Gpio),
    /// Signal connected through an I/O expander.
    Expander(ExpanderPin),
    /// Named signal whose mapping is documented but not directly controlled here.
    Named(&'static str),
}

/// Fixed Nesso N1 LCD configuration.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct DisplayConfig {
    /// Visible panel width in pixels.
    pub width: u16,
    /// Visible panel height in pixels.
    pub height: u16,
    /// Display bus color depth in bits.
    pub color_depth_bits: u8,
    /// Display controller model.
    pub controller: &'static str,
    /// SPI MOSI GPIO.
    pub spi_mosi: Gpio,
    /// SPI MISO GPIO.
    pub spi_miso: Gpio,
    /// SPI SCK GPIO.
    pub spi_sck: Gpio,
    /// SPI write frequency.
    pub spi_write_hz: u32,
    /// Display RAM X offset.
    pub offset_x: u16,
    /// Display RAM Y offset.
    pub offset_y: u16,
    /// Whether panel colors must be inverted.
    pub invert_colors: bool,
    /// LCD chip-select signal.
    pub chip_select: Signal,
    /// LCD data/command signal.
    pub data_command: Signal,
    /// LCD reset signal.
    pub reset: Signal,
    /// LCD backlight signal.
    pub backlight: Signal,
}

/// Tracks logical resource claims for board-level construction helpers.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub struct BoardResources {
    claimed: u32,
}

impl BoardResources {
    /// Creates an empty resource claim set.
    #[must_use]
    pub const fn new() -> Self {
        Self { claimed: 0 }
    }

    /// Claims one board resource and fails if it was already claimed.
    pub fn claim(&mut self, resource: Resource) -> Result<(), BoardError> {
        let mask = 1_u32 << resource as u8;
        if self.claimed & mask != 0 {
            return Err(BoardError::ResourceConflict);
        }
        self.claimed |= mask;
        Ok(())
    }
}

impl Default for BoardResources {
    fn default() -> Self {
        Self::new()
    }
}

/// Logical resources that should not be configured twice.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub enum Resource {
    /// Main I2C bus used by touch, IMU, power, and expanders.
    I2cMain = 0,
    /// Shared SPI bus used by the display and future SPI peripherals.
    SpiShared = 1,
}

/// Static Nesso N1 board description and resource claims.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub struct NessoN1 {
    resources: BoardResources,
}

/// Concrete blocking I2C bus type used before it is placed behind a shared bus.
pub type NessoRawI2c = EspI2c<'static, Blocking>;
/// Shared I2C device type used by Nesso N1 facade helpers.
pub type NessoI2c = i2c::CriticalSectionDevice<'static, NessoRawI2c>;
/// Software I2C bus used to probe Grove-attached sensors on GPIO5/GPIO4.
#[cfg(feature = "env")]
type NessoGroveI2c = GroveI2c;
/// Concrete blocking SPI bus type used before it is placed behind a shared bus.
pub type NessoRawSpi = Spi<'static, Blocking>;
/// Shared SPI device type used by Nesso N1 SPI peripherals.
pub type NessoSpiDevice = spi::CriticalSectionDevice<'static, NessoRawSpi, NessoOutput, Delay>;
/// Concrete output-pin type used by Nesso N1 helpers.
pub type NessoOutput = Output<'static>;
/// Concrete display type returned by the board support package.
pub type NessoDisplay = Display<NessoSpiDevice, NessoOutput, NullOutputPin, NullOutputPin>;
/// Concrete passive-buzzer type returned by the board support package.
pub type NessoBuzzer = Buzzer<NessoOutput>;
/// Board-owned ENV Pro driver selected by the board auto-detect helper.
///
/// `NessoN1Board::into_display_and_env()` powers the Grove rail, probes the
/// Grove GPIO5/GPIO4 path first, then falls back to the shared Qwiic/main I2C
/// bus. `NessoEnv` hides that transport detail and exposes a single
/// measurement API.
#[cfg(feature = "env")]
pub struct NessoEnv {
    inner: NessoEnvInner,
}
/// Concrete LoRa SPI-device type used by the onboard SX1262 driver.
#[cfg(feature = "lora")]
pub type NessoLoraSpiDevice = NessoSpiDevice;
/// Board-owned radio resources required by `nesso::wifi`.
#[cfg(feature = "wifi")]
pub type WifiResources = crate::wifi::RadioResources;
/// Board-owned Bluetooth resources required by `nesso::ble`.
#[cfg(feature = "ble")]
pub type BleResources = crate::ble::BluetoothResources;

/// Shared ESP radio runtime resources used by Wi-Fi or BLE.
#[cfg(any(feature = "wifi", feature = "ble"))]
pub struct RadioRuntimeResources {
    /// Timer group used by the ESP radio runtime.
    pub timer_group0: esp_hal::peripherals::TIMG0<'static>,
    /// Software interrupt peripheral used by the ESP radio runtime.
    pub software_interrupt: esp_hal::peripherals::SW_INTERRUPT<'static>,
}

/// Core peripherals assembled by [`NessoN1Board::into_core_parts`].
pub struct NessoCoreParts {
    /// Initialized ST7789P3 display.
    pub display: NessoDisplay,
    /// Main I2C bus for shared I2C devices.
    pub i2c: NessoI2c,
    /// Passive buzzer on the documented buzzer GPIO.
    pub buzzer: NessoBuzzer,
    /// ESP32-C6 radio resources for Wi-Fi.
    #[cfg(feature = "wifi")]
    pub wifi: WifiResources,
    /// ESP32-C6 Bluetooth controller resources.
    #[cfg(feature = "ble")]
    pub ble: BleResources,
    /// Shared ESP radio runtime resources.
    #[cfg(any(feature = "wifi", feature = "ble"))]
    pub radio_runtime: RadioRuntimeResources,
    /// Flash peripheral for application storage.
    pub flash: esp_hal::peripherals::FLASH<'static>,
    /// Onboard SX1262 LoRa driver.
    #[cfg(feature = "lora")]
    pub lora: crate::lora::NessoLora,
}

#[cfg(feature = "env")]
impl NessoEnv {
    /// Reads one ENV Pro sample from the detected board path.
    ///
    /// # Errors
    ///
    /// Returns the underlying I2C error kind if the active Grove or Qwiic path
    /// stops acknowledging transfers, or one of the BME688 probe/read errors
    /// from `nesso::env` if the device responds with invalid data.
    pub fn measure(&mut self) -> Result<EnvMeasurement, EnvError<embedded_hal::i2c::ErrorKind>> {
        match &mut self.inner {
            NessoEnvInner::Grove(env) => env.measure().map_err(map_env_error),
            NessoEnvInner::Qwiic(env) => env.measure().map_err(map_env_error),
        }
    }
}

/// Board-owned resources for the onboard SX1262 LoRa transceiver.
#[cfg(feature = "lora")]
#[doc(hidden)]
pub struct LoraResources {
    /// Dedicated LoRa chip-select pin.
    pub chip_select: esp_hal::peripherals::GPIO23<'static>,
    /// SX1262 BUSY pin.
    pub busy: esp_hal::peripherals::GPIO19<'static>,
    /// SX1262 DIO1 interrupt pin.
    pub irq: esp_hal::peripherals::GPIO15<'static>,
}

/// Owns ESP-HAL peripherals before they are split into Nesso N1 services.
pub struct NessoN1Board {
    peripherals: esp_hal::peripherals::Peripherals,
}

#[cfg(feature = "env")]
struct GroveI2c {
    sda: Flex<'static>,
    scl: Flex<'static>,
    delay: Delay,
}

#[cfg(feature = "env")]
enum NessoEnvInner {
    Grove(EnvPro<NessoGroveI2c, Delay>),
    Qwiic(EnvPro<NessoI2c, Delay>),
}

#[cfg(feature = "env")]
impl GroveI2c {
    /// Conservative half-period for the software I2C master used on Grove.
    const HALF_PERIOD_US: u32 = 5;
    /// Number of clock pulses used to recover a stuck bus before probing.
    const BUS_RECOVERY_PULSES: u8 = 9;

    fn new(
        sda: esp_hal::peripherals::GPIO5<'static>,
        scl: esp_hal::peripherals::GPIO4<'static>,
    ) -> Self {
        let mut sda = Flex::new(sda);
        let mut scl = Flex::new(scl);
        let open_drain = OutputConfig::default().with_drive_mode(DriveMode::OpenDrain);

        sda.apply_output_config(&open_drain);
        sda.set_input_enable(true);
        sda.set_high();
        sda.set_output_enable(true);

        scl.apply_output_config(&open_drain);
        scl.set_input_enable(true);
        scl.set_high();
        scl.set_output_enable(true);

        let mut bus = Self {
            sda,
            scl,
            delay: Delay::new(),
        };
        bus.recover_bus();
        bus
    }

    fn delay_half_period(&mut self) {
        self.delay.delay_us(Self::HALF_PERIOD_US);
    }

    fn release_sda(&mut self) {
        self.sda.set_high();
    }

    fn drive_sda_low(&mut self) {
        self.sda.set_low();
    }

    fn release_scl(&mut self) {
        self.scl.set_high();
    }

    fn drive_scl_low(&mut self) {
        self.scl.set_low();
    }

    fn start_condition(&mut self) {
        self.release_sda();
        self.release_scl();
        self.delay_half_period();
        self.drive_sda_low();
        self.delay_half_period();
        self.drive_scl_low();
        self.delay_half_period();
    }

    fn stop_condition(&mut self) {
        self.drive_sda_low();
        self.delay_half_period();
        self.release_scl();
        self.delay_half_period();
        self.release_sda();
        self.delay_half_period();
    }

    fn write_bit(&mut self, high: bool) {
        if high {
            self.release_sda();
        } else {
            self.drive_sda_low();
        }
        self.delay_half_period();
        self.release_scl();
        self.delay_half_period();
        self.drive_scl_low();
        self.delay_half_period();
    }

    fn read_bit(&mut self) -> bool {
        self.release_sda();
        self.delay_half_period();
        self.release_scl();
        self.delay_half_period();
        let high = self.sda.is_high();
        self.drive_scl_low();
        self.delay_half_period();
        high
    }

    fn write_byte(&mut self, byte: u8) -> bool {
        for shift in (0..8).rev() {
            self.write_bit((byte & (1u8 << shift)) != 0);
        }
        !self.read_bit()
    }

    fn read_byte(&mut self, acknowledge: bool) -> u8 {
        let mut byte = 0u8;
        for shift in (0..8).rev() {
            if self.read_bit() {
                byte |= 1u8 << shift;
            }
        }
        self.write_bit(!acknowledge);
        byte
    }

    fn recover_bus(&mut self) {
        self.release_sda();
        self.release_scl();
        self.delay_half_period();

        if self.sda.is_high() {
            return;
        }

        for _ in 0..Self::BUS_RECOVERY_PULSES {
            self.drive_scl_low();
            self.delay_half_period();
            self.release_scl();
            self.delay_half_period();
            if self.sda.is_high() {
                break;
            }
        }

        self.stop_condition();
    }
}

#[cfg(feature = "env")]
impl ErrorType for GroveI2c {
    type Error = ErrorKind;
}

#[cfg(feature = "env")]
impl I2c for GroveI2c {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        if operations.is_empty() {
            return Ok(());
        }

        let mut previous_was_read = None;

        for operation in operations.iter_mut() {
            let read_phase = matches!(operation, Operation::Read(_));
            if previous_was_read != Some(read_phase) {
                self.start_condition();
                let address_byte = (address << 1) | u8::from(read_phase);
                if !self.write_byte(address_byte) {
                    self.stop_condition();
                    return Err(ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address));
                }
                previous_was_read = Some(read_phase);
            }

            match operation {
                Operation::Read(buffer) => {
                    let last_index = buffer.len().saturating_sub(1);
                    for (index, slot) in buffer.iter_mut().enumerate() {
                        *slot = self.read_byte(index != last_index);
                    }
                }
                Operation::Write(buffer) => {
                    for &byte in *buffer {
                        if !self.write_byte(byte) {
                            self.stop_condition();
                            return Err(ErrorKind::NoAcknowledge(NoAcknowledgeSource::Data));
                        }
                    }
                }
            }
        }

        self.stop_condition();
        Ok(())
    }
}

impl NessoN1 {
    /// Visible LCD width in pixels.
    pub const DISPLAY_WIDTH: u16 = 135;
    /// Visible LCD height in pixels.
    pub const DISPLAY_HEIGHT: u16 = 240;
    /// ST7789 column offset for the visible Nesso N1 area.
    pub const DISPLAY_OFFSET_X: u16 = 52;
    /// ST7789 row offset for the visible Nesso N1 area.
    pub const DISPLAY_OFFSET_Y: u16 = 40;
    /// Display SPI write frequency.
    pub const DISPLAY_SPI_WRITE_HZ: u32 = 40_000_000;
    /// Main I2C bus frequency.
    pub const I2C_MAX_HZ: u32 = 400_000;

    pub const GPIO_I2C_SDA: Gpio = Gpio(10);
    pub const GPIO_I2C_SCL: Gpio = Gpio(8);
    pub const GPIO_SPI_MOSI: Gpio = Gpio(21);
    pub const GPIO_SPI_MISO: Gpio = Gpio(22);
    pub const GPIO_SPI_SCK: Gpio = Gpio(20);
    pub const GPIO_LCD_CS: Gpio = Gpio(17);
    pub const GPIO_LCD_DC: Gpio = Gpio(16);
    pub const GPIO_SYS_IRQ: Gpio = Gpio(3);
    pub const GPIO_TOUCH_INT: Gpio = Self::GPIO_SYS_IRQ;
    pub const GPIO_IMU_INT: Gpio = Self::GPIO_SYS_IRQ;

    pub const ADDR_TOUCH_FT6336U: I2cAddress = I2cAddress(0x38);
    pub const ADDR_BMI270_LOW: I2cAddress = I2cAddress(0x68);
    pub const ADDR_BMI270_HIGH: I2cAddress = I2cAddress(0x69);
    pub const ADDR_BQ27220: I2cAddress = I2cAddress(0x55);
    pub const ADDR_AW32001: I2cAddress = I2cAddress(0x49);
    pub const ADDR_EXPANDER_0: I2cAddress = I2cAddress(0x43);
    pub const ADDR_EXPANDER_1: I2cAddress = I2cAddress(0x44);

    pub const GPIO_LORA_CS: Gpio = Gpio(23);
    pub const GPIO_LORA_BUSY: Gpio = Gpio(19);
    pub const GPIO_LORA_IRQ: Gpio = Gpio(15);
    /// Onboard LoRa transceiver model.
    pub const LORA_CONTROLLER: &'static str = "SX1262";
    /// Documented onboard LoRa RF range.
    pub const LORA_MIN_FREQUENCY_HZ: u32 = 850_000_000;
    /// Documented onboard LoRa RF range.
    pub const LORA_MAX_FREQUENCY_HZ: u32 = 960_000_000;
    pub const GPIO_GROVE_IO0: Gpio = Gpio(5);
    pub const GPIO_GROVE_IO1: Gpio = Gpio(4);
    pub const GPIO_HAT_IO1: Gpio = Gpio(2);
    pub const GPIO_HAT_IO2: Gpio = Gpio(6);
    pub const GPIO_HAT_IO3: Gpio = Gpio(7);
    pub const GPIO_BUZZER: Gpio = Gpio(11);
    pub const GPIO_IR: Gpio = Gpio(9);

    pub const KEY1: ExpanderPin = ExpanderPin {
        address: Self::ADDR_EXPANDER_0,
        pin: 0,
    };
    pub const KEY2: ExpanderPin = ExpanderPin {
        address: Self::ADDR_EXPANDER_0,
        pin: 1,
    };
    pub const LORA_LNA_ENABLE: ExpanderPin = ExpanderPin {
        address: Self::ADDR_EXPANDER_0,
        pin: 5,
    };
    pub const LORA_ANTENNA_SWITCH: ExpanderPin = ExpanderPin {
        address: Self::ADDR_EXPANDER_0,
        pin: 6,
    };
    pub const LORA_ENABLE: ExpanderPin = ExpanderPin {
        address: Self::ADDR_EXPANDER_0,
        pin: 7,
    };
    pub const POWEROFF: ExpanderPin = ExpanderPin {
        address: Self::ADDR_EXPANDER_1,
        pin: 0,
    };
    pub const LCD_RESET: ExpanderPin = ExpanderPin {
        address: Self::ADDR_EXPANDER_1,
        pin: 1,
    };
    pub const GROVE_POWER_ENABLE: ExpanderPin = ExpanderPin {
        address: Self::ADDR_EXPANDER_1,
        pin: 2,
    };
    pub const VIN_DETECT: ExpanderPin = ExpanderPin {
        address: Self::ADDR_EXPANDER_1,
        pin: 5,
    };
    pub const LCD_BACKLIGHT: ExpanderPin = ExpanderPin {
        address: Self::ADDR_EXPANDER_1,
        pin: 6,
    };
    pub const LED_BUILTIN: ExpanderPin = ExpanderPin {
        address: Self::ADDR_EXPANDER_1,
        pin: 7,
    };

    /// Creates a board descriptor after claiming core resources.
    pub fn new(mut resources: BoardResources) -> Result<Self, BoardError> {
        resources.claim(Resource::I2cMain)?;
        resources.claim(Resource::SpiShared)?;
        Ok(Self { resources })
    }

    #[must_use]
    /// Returns the fixed Nesso N1 display configuration.
    pub const fn display_config() -> DisplayConfig {
        DisplayConfig {
            width: Self::DISPLAY_WIDTH,
            height: Self::DISPLAY_HEIGHT,
            color_depth_bits: 18,
            controller: "ST7789P3",
            spi_mosi: Self::GPIO_SPI_MOSI,
            spi_miso: Self::GPIO_SPI_MISO,
            spi_sck: Self::GPIO_SPI_SCK,
            spi_write_hz: Self::DISPLAY_SPI_WRITE_HZ,
            offset_x: Self::DISPLAY_OFFSET_X,
            offset_y: Self::DISPLAY_OFFSET_Y,
            invert_colors: true,
            chip_select: Signal::Native(Self::GPIO_LCD_CS),
            data_command: Signal::Native(Self::GPIO_LCD_DC),
            reset: Signal::Expander(Self::LCD_RESET),
            backlight: Signal::Expander(Self::LCD_BACKLIGHT),
        }
    }

    /// Configures the LCD reset and backlight expander pins.
    pub fn init_lcd_expander<I2C, Delay, Error>(
        i2c: &mut I2C,
        delay: &mut Delay,
    ) -> Result<(), Error>
    where
        I2C: I2c<Error = Error>,
        Delay: DelayNs,
    {
        let expander = Self::ADDR_EXPANDER_1.0;
        let _discarded = read_register(i2c, expander, EXPANDER_GLOBAL_CONTROL)?;
        write_register(
            i2c,
            expander,
            EXPANDER_GLOBAL_CONTROL,
            EXPANDER_GLOBAL_CONTROL_ENABLE,
        )?;
        write_register(i2c, expander, EXPANDER_DEFAULT_OUTPUT, 0xFF)?;
        write_register(i2c, expander, EXPANDER_INTERRUPT_MASK, 0xFF)?;
        write_register(i2c, expander, EXPANDER_OUTPUT_ENABLE, 0x00)?;
        let _interrupt_status = read_register(i2c, expander, EXPANDER_INTERRUPT_STATUS)?;

        configure_output(i2c, expander, Self::LCD_BACKLIGHT.pin)?;
        configure_output(i2c, expander, Self::LCD_RESET.pin)?;
        write_bit(
            i2c,
            expander,
            EXPANDER_OUTPUT_STATE,
            Self::LCD_BACKLIGHT.pin,
            true,
        )?;
        write_bit(
            i2c,
            expander,
            EXPANDER_OUTPUT_STATE,
            Self::LCD_RESET.pin,
            false,
        )?;
        delay.delay_ms(100);
        write_bit(
            i2c,
            expander,
            EXPANDER_OUTPUT_STATE,
            Self::LCD_RESET.pin,
            true,
        )
    }

    /// Configures KEY1 and KEY2 as pull-up inputs on the button expander.
    pub fn init_button_inputs<I2C, Error>(i2c: &mut I2C) -> Result<(), Error>
    where
        I2C: I2c<Error = Error>,
    {
        let address = Self::ADDR_EXPANDER_0.0;
        let _discarded = read_register(i2c, address, EXPANDER_GLOBAL_CONTROL)?;
        write_register(
            i2c,
            address,
            EXPANDER_GLOBAL_CONTROL,
            EXPANDER_GLOBAL_CONTROL_ENABLE,
        )?;
        write_register(i2c, address, EXPANDER_DEFAULT_OUTPUT, 0xFF)?;
        write_register(i2c, address, EXPANDER_INTERRUPT_MASK, 0xFF)?;
        configure_input_pull_up(i2c, address, Self::KEY1.pin)?;
        configure_input_pull_up(i2c, address, Self::KEY2.pin)?;
        let _interrupt_status = read_register(i2c, address, EXPANDER_INTERRUPT_STATUS)?;
        Ok(())
    }

    /// Reads KEY1 and KEY2 from the button expander.
    pub fn read_button_levels<I2C, Error>(i2c: &mut I2C) -> Result<ButtonLevels, Error>
    where
        I2C: I2c<Error = Error>,
    {
        let pins = read_register(i2c, Self::ADDR_EXPANDER_0.0, EXPANDER_INPUT_STATE)?;
        Ok(ButtonLevels {
            key1_pressed: pins & (1u8 << Self::KEY1.pin) == 0,
            key2_pressed: pins & (1u8 << Self::KEY2.pin) == 0,
            raw_input: pins,
        })
    }
}

impl NessoN1Board {
    /// Wraps ESP-HAL peripherals for board-specific construction.
    #[must_use]
    pub const fn new(peripherals: esp_hal::peripherals::Peripherals) -> Self {
        Self { peripherals }
    }

    /// Initializes and returns only the display.
    pub fn into_display(self) -> Result<NessoDisplay, BoardInitError> {
        let (display, _i2c) = self.into_display_and_i2c()?;
        Ok(display)
    }

    /// Initializes the display and returns it with the main I2C bus.
    pub fn into_display_and_i2c(self) -> Result<(NessoDisplay, NessoI2c), BoardInitError> {
        let mut delay = Delay::new();
        let mut raw_i2c = Self::configure_i2c(
            self.peripherals.I2C0,
            self.peripherals.GPIO10,
            self.peripherals.GPIO8,
        )?;
        NessoN1::init_lcd_expander(&mut raw_i2c, &mut delay)
            .map_err(|_| BoardInitError::Expander)?;
        let i2c_bus = Self::share_i2c(raw_i2c)?;
        let i2c = i2c::CriticalSectionDevice::new(i2c_bus);

        let spi_bus = Self::configure_spi_bus(
            self.peripherals.SPI2,
            self.peripherals.GPIO20,
            self.peripherals.GPIO21,
            self.peripherals.GPIO22,
        )?;
        let display =
            Self::configure_display(spi_bus, self.peripherals.GPIO17, self.peripherals.GPIO16)?;
        Ok((display, i2c))
    }

    /// Initializes the display and auto-detects ENV Pro on Grove first, then Qwiic.
    #[cfg(feature = "env")]
    pub fn into_display_and_env(self) -> Result<(NessoDisplay, NessoEnv), BoardInitError> {
        let mut delay = Delay::new();
        let mut raw_i2c0 = Self::configure_i2c(
            self.peripherals.I2C0,
            self.peripherals.GPIO10,
            self.peripherals.GPIO8,
        )?;
        NessoN1::init_lcd_expander(&mut raw_i2c0, &mut delay)
            .map_err(|_| BoardInitError::Expander)?;

        let spi_bus = Self::configure_spi_bus(
            self.peripherals.SPI2,
            self.peripherals.GPIO20,
            self.peripherals.GPIO21,
            self.peripherals.GPIO22,
        )?;
        let display =
            Self::configure_display(spi_bus, self.peripherals.GPIO17, self.peripherals.GPIO16)?;

        configure_output(
            &mut raw_i2c0,
            NessoN1::GROVE_POWER_ENABLE.address.0,
            NessoN1::GROVE_POWER_ENABLE.pin,
        )
        .map_err(|_| BoardInitError::Expander)?;
        write_bit(
            &mut raw_i2c0,
            NessoN1::GROVE_POWER_ENABLE.address.0,
            EXPANDER_OUTPUT_STATE,
            NessoN1::GROVE_POWER_ENABLE.pin,
            true,
        )
        .map_err(|_| BoardInitError::Expander)?;
        delay.delay_ms(10);

        let mut grove_i2c =
            Self::configure_grove_i2c(self.peripherals.GPIO5, self.peripherals.GPIO4);
        if EnvPro::probe(&mut grove_i2c, &mut delay).is_ok() {
            let env = EnvPro::new(grove_i2c, Delay::new()).map_err(|_| BoardInitError::I2c)?;
            return Ok((
                display,
                NessoEnv {
                    inner: NessoEnvInner::Grove(env),
                },
            ));
        }

        let i2c_bus = Self::share_i2c(raw_i2c0)?;
        let mut qwiic_i2c = i2c::CriticalSectionDevice::new(i2c_bus);
        EnvPro::probe(&mut qwiic_i2c, &mut delay).map_err(|_| BoardInitError::I2c)?;
        let env = EnvPro::new(qwiic_i2c, Delay::new()).map_err(|_| BoardInitError::I2c)?;
        Ok((
            display,
            NessoEnv {
                inner: NessoEnvInner::Qwiic(env),
            },
        ))
    }

    /// Initializes the display and returns board-owned Wi-Fi resources.
    #[cfg(feature = "wifi")]
    pub fn into_display_and_wifi(
        self,
    ) -> Result<(NessoDisplay, WifiResources, RadioRuntimeResources), BoardInitError> {
        let mut delay = Delay::new();
        {
            let mut raw_i2c = Self::configure_i2c(
                self.peripherals.I2C0,
                self.peripherals.GPIO10,
                self.peripherals.GPIO8,
            )?;
            NessoN1::init_lcd_expander(&mut raw_i2c, &mut delay)
                .map_err(|_| BoardInitError::Expander)?;
            let _i2c_bus = Self::share_i2c(raw_i2c)?;
        }

        let spi_bus = Self::configure_spi_bus(
            self.peripherals.SPI2,
            self.peripherals.GPIO20,
            self.peripherals.GPIO21,
            self.peripherals.GPIO22,
        )?;
        let display =
            Self::configure_display(spi_bus, self.peripherals.GPIO17, self.peripherals.GPIO16)?;
        Ok((
            display,
            WifiResources {
                wifi: self.peripherals.WIFI,
            },
            RadioRuntimeResources {
                timer_group0: self.peripherals.TIMG0,
                software_interrupt: self.peripherals.SW_INTERRUPT,
            },
        ))
    }

    /// Initializes the core services used by the public `nesso` facade.
    pub fn into_core_parts(self) -> Result<NessoCoreParts, BoardInitError> {
        let mut delay = Delay::new();
        let mut raw_i2c = Self::configure_i2c(
            self.peripherals.I2C0,
            self.peripherals.GPIO10,
            self.peripherals.GPIO8,
        )?;
        NessoN1::init_lcd_expander(&mut raw_i2c, &mut delay)
            .map_err(|_| BoardInitError::Expander)?;
        let i2c_bus = Self::share_i2c(raw_i2c)?;
        let i2c = i2c::CriticalSectionDevice::new(i2c_bus);

        let spi_bus = Self::configure_spi_bus(
            self.peripherals.SPI2,
            self.peripherals.GPIO20,
            self.peripherals.GPIO21,
            self.peripherals.GPIO22,
        )?;
        let display =
            Self::configure_display(spi_bus, self.peripherals.GPIO17, self.peripherals.GPIO16)?;
        #[cfg(feature = "lora")]
        let lora = Self::configure_lora_from_parts(
            spi_bus,
            i2c::CriticalSectionDevice::new(i2c_bus),
            LoraResources {
                chip_select: self.peripherals.GPIO23,
                busy: self.peripherals.GPIO19,
                irq: self.peripherals.GPIO15,
            },
        )?;
        let buzzer = Buzzer::new(Output::new(
            self.peripherals.GPIO11,
            Level::Low,
            OutputConfig::default(),
        ));

        Ok(NessoCoreParts {
            display,
            i2c,
            buzzer,
            #[cfg(feature = "wifi")]
            wifi: WifiResources {
                wifi: self.peripherals.WIFI,
            },
            #[cfg(feature = "ble")]
            ble: BleResources {
                bluetooth: self.peripherals.BT,
            },
            #[cfg(any(feature = "wifi", feature = "ble"))]
            radio_runtime: RadioRuntimeResources {
                timer_group0: self.peripherals.TIMG0,
                software_interrupt: self.peripherals.SW_INTERRUPT,
            },
            flash: self.peripherals.FLASH,
            #[cfg(feature = "lora")]
            lora,
        })
    }

    #[must_use]
    /// Returns a standalone passive-buzzer driver.
    pub fn into_buzzer(self) -> NessoBuzzer {
        Buzzer::new(Output::new(
            self.peripherals.GPIO11,
            Level::Low,
            OutputConfig::default(),
        ))
    }

    fn configure_i2c(
        i2c0: esp_hal::peripherals::I2C0<'static>,
        sda: esp_hal::peripherals::GPIO10<'static>,
        scl: esp_hal::peripherals::GPIO8<'static>,
    ) -> Result<NessoRawI2c, BoardInitError> {
        EspI2c::new(
            i2c0,
            I2cConfig::default().with_frequency(Rate::from_khz(400)),
        )
        .map(|i2c| i2c.with_sda(sda).with_scl(scl))
        .map_err(|_| BoardInitError::I2c)
    }

    #[cfg(feature = "env")]
    fn configure_grove_i2c(
        sda: esp_hal::peripherals::GPIO5<'static>,
        scl: esp_hal::peripherals::GPIO4<'static>,
    ) -> NessoGroveI2c {
        GroveI2c::new(sda, scl)
    }

    fn share_i2c(i2c: NessoRawI2c) -> Result<&'static Mutex<RefCell<NessoRawI2c>>, BoardInitError> {
        I2C0_BUS
            .try_init_with(|| Mutex::new(RefCell::new(i2c)))
            .map(|bus| &*bus)
            .ok_or(BoardInitError::SharedBus)
    }

    fn configure_spi_bus(
        spi2: esp_hal::peripherals::SPI2<'static>,
        sck: esp_hal::peripherals::GPIO20<'static>,
        mosi: esp_hal::peripherals::GPIO21<'static>,
        miso: esp_hal::peripherals::GPIO22<'static>,
    ) -> Result<&'static Mutex<RefCell<NessoRawSpi>>, BoardInitError> {
        let spi = Spi::new(
            spi2,
            SpiConfig::default()
                .with_frequency(Rate::from_mhz(40))
                .with_mode(Mode::_0),
        )
        .map_err(|_| BoardInitError::Spi)?
        .with_sck(sck)
        .with_mosi(mosi)
        .with_miso(miso);

        SPI2_BUS
            .try_init_with(|| Mutex::new(RefCell::new(spi)))
            .map(|bus| &*bus)
            .ok_or(BoardInitError::SharedBus)
    }

    fn configure_display(
        spi_bus: &'static Mutex<RefCell<NessoRawSpi>>,
        cs_pin: esp_hal::peripherals::GPIO17<'static>,
        dc_pin: esp_hal::peripherals::GPIO16<'static>,
    ) -> Result<NessoDisplay, BoardInitError> {
        let cs = Output::new(cs_pin, Level::High, OutputConfig::default());
        let dc = Output::new(dc_pin, Level::Low, OutputConfig::default());
        let spi_device = spi::CriticalSectionDevice::new(spi_bus, cs, Delay::new())
            .map_err(|_| BoardInitError::Spi)?;

        let mut display = Display::new(
            spi_device,
            dc,
            NullOutputPin,
            NullOutputPin,
            BusConfig {
                write_hz: NessoN1::DISPLAY_SPI_WRITE_HZ,
                use_dma: true,
            },
            PanelConfig {
                invert_colors: true,
                geometry: DisplayGeometry {
                    width: NessoN1::DISPLAY_WIDTH,
                    height: NessoN1::DISPLAY_HEIGHT,
                    offset_x: NessoN1::DISPLAY_OFFSET_X,
                    offset_y: NessoN1::DISPLAY_OFFSET_Y,
                },
            },
        );

        display
            .init()
            .map_err(|_: DisplayError<_, _>| BoardInitError::Display)?;
        Ok(display)
    }

    /// Builds the onboard SX1262 driver from the shared SPI bus and LoRa pins.
    #[cfg(feature = "lora")]
    #[doc(hidden)]
    pub fn configure_lora_from_parts(
        spi_bus: &'static Mutex<RefCell<NessoRawSpi>>,
        i2c: NessoI2c,
        resources: LoraResources,
    ) -> Result<crate::lora::NessoLora, BoardInitError> {
        let cs = Output::new(resources.chip_select, Level::High, OutputConfig::default());
        let busy = esp_hal::gpio::Input::new(resources.busy, esp_hal::gpio::InputConfig::default());
        let irq = esp_hal::gpio::Input::new(resources.irq, esp_hal::gpio::InputConfig::default());
        let spi_device = spi::CriticalSectionDevice::new(spi_bus, cs, Delay::new())
            .map_err(|_| BoardInitError::Spi)?;
        Ok(crate::lora::Sx1262::new_nesso(spi_device, i2c, busy, irq))
    }
}

/// Compatibility alias for the Nesso N1 board description.
pub type Board = NessoN1;

impl NessoN1 {
    /// Returns the resource claim state associated with this board descriptor.
    #[must_use]
    pub fn resources(&self) -> &BoardResources {
        &self.resources
    }
}

/// Configures an expander pin as a driven output.
pub fn configure_output<I2C, Error>(i2c: &mut I2C, address: u8, bit: u8) -> Result<(), Error>
where
    I2C: I2c<Error = Error>,
{
    write_bit(i2c, address, EXPANDER_OUTPUT_ENABLE, bit, true)?;
    write_bit(i2c, address, EXPANDER_HIGH_IMPEDANCE, bit, false)
}

/// Configures an expander pin as a pull-up input.
pub fn configure_input_pull_up<I2C, Error>(i2c: &mut I2C, address: u8, bit: u8) -> Result<(), Error>
where
    I2C: I2c<Error = Error>,
{
    write_bit(i2c, address, EXPANDER_OUTPUT_ENABLE, bit, false)?;
    write_bit(i2c, address, EXPANDER_HIGH_IMPEDANCE, bit, true)?;
    write_bit(i2c, address, EXPANDER_PULL_ENABLE, bit, true)?;
    write_bit(i2c, address, EXPANDER_PULL_SELECT, bit, true)
}

/// Reads one byte from an I2C expander register.
pub fn read_register<I2C, Error>(i2c: &mut I2C, address: u8, register: u8) -> Result<u8, Error>
where
    I2C: I2c<Error = Error>,
{
    let mut register_byte = [0u8];
    i2c.write_read(address, &[register], &mut register_byte)?;
    Ok(register_byte[0])
}

/// Writes one byte to an I2C expander register.
pub fn write_register<I2C, Error>(
    i2c: &mut I2C,
    address: u8,
    register: u8,
    value: u8,
) -> Result<(), Error>
where
    I2C: I2c<Error = Error>,
{
    i2c.write(address, &[register, value])
}

/// Updates one bit in an I2C expander register.
pub fn write_bit<I2C, Error>(
    i2c: &mut I2C,
    address: u8,
    register: u8,
    bit: u8,
    value: bool,
) -> Result<(), Error>
where
    I2C: I2c<Error = Error>,
{
    let current = read_register(i2c, address, register)?;
    let mask = 1u8 << bit;
    let next = if value {
        current | mask
    } else {
        current & !mask
    };
    write_register(i2c, address, register, next)
}

/// Creates an ENV Pro driver on the shared Nesso N1 external I2C bus.
///
/// The helper enables the Grove sensor rail before probing the shared
/// Qwiic/main I2C bus. Use `NessoN1Board::into_display_and_env()` when the
/// application wants the board to auto-detect ENV Pro on Grove first and then
/// fall back to Qwiic.
#[cfg(feature = "env")]
pub fn init_env_pro<I2C, Delay>(
    mut i2c: I2C,
    mut delay: Delay,
) -> Result<EnvPro<I2C, Delay>, EnvError<I2C::Error>>
where
    I2C: I2c,
    Delay: DelayNs,
{
    configure_output(
        &mut i2c,
        NessoN1::GROVE_POWER_ENABLE.address.0,
        NessoN1::GROVE_POWER_ENABLE.pin,
    )
    .map_err(EnvError::Bus)?;
    write_bit(
        &mut i2c,
        NessoN1::GROVE_POWER_ENABLE.address.0,
        EXPANDER_OUTPUT_STATE,
        NessoN1::GROVE_POWER_ENABLE.pin,
        true,
    )
    .map_err(EnvError::Bus)?;
    delay.delay_ms(10);
    EnvPro::new(i2c, delay)
}

#[cfg(feature = "env")]
fn map_env_error<E>(error: EnvError<E>) -> EnvError<embedded_hal::i2c::ErrorKind>
where
    E: embedded_hal::i2c::Error,
{
    match error {
        EnvError::Bus(bus) => EnvError::Bus(bus.kind()),
        EnvError::InvalidChipId(chip_id) => EnvError::InvalidChipId(chip_id),
        EnvError::NoNewData => EnvError::NoNewData,
        EnvError::UnsupportedVariant(variant) => EnvError::UnsupportedVariant(variant),
    }
}
