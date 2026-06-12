//! Board support package for the Arduino Nesso N1.
//!
//! This crate owns Nesso N1 pin mappings, fixed bus configuration, and concrete
//! board bring-up helpers. It is intentionally specific to the Nesso N1 and does
//! not provide a generic board abstraction layer.

use crate::audio::Buzzer;
use crate::display::{
    BusConfig, Display, DisplayError, DisplayGeometry, LcdSpiDevice, NullOutputPin, PanelConfig,
};
use embedded_hal::{delay::DelayNs, i2c::I2c};
use esp_hal::{
    Blocking,
    delay::Delay,
    gpio::{Level, Output, OutputConfig},
    i2c::master::{Config as I2cConfig, I2c as EspI2c},
    spi::{
        Mode,
        master::{Config as SpiConfig, Spi},
    },
    time::Rate,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardError {
    /// A board resource was requested more than once.
    ResourceConflict,
}

/// Errors returned while constructing concrete Nesso N1 peripherals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardInitError {
    /// I2C controller setup failed.
    I2c,
    /// SPI controller setup failed.
    Spi,
    /// I/O expander setup failed.
    Expander,
    /// Display initialization failed.
    Display,
}

/// AW9523-compatible expander output-enable register.
pub const EXPANDER_OUTPUT_ENABLE: u8 = 0x03;
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct I2cAddress(pub u8);

/// ESP32-C6 GPIO number used by a board signal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gpio(pub u8);

/// Pin exposed by one of the Nesso N1 I/O expanders.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpanderPin {
    /// I2C address of the expander.
    pub address: I2cAddress,
    /// Expander pin number.
    pub pin: u8,
}

/// Current logical state of the two board buttons.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ButtonLevels {
    /// True when KEY1 is pressed.
    pub key1_pressed: bool,
    /// True when KEY2 is pressed.
    pub key2_pressed: bool,
    /// Raw PI4IOE5V6408 input-state register value.
    pub raw_input: u8,
}

/// Location of a board signal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Resource {
    /// Main I2C bus used by touch, IMU, power, and expanders.
    I2cMain = 0,
    /// Shared SPI bus used by the display and future SPI peripherals.
    SpiShared = 1,
}

/// Static Nesso N1 board description and resource claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NessoN1 {
    resources: BoardResources,
}

/// Concrete blocking I2C type used by Nesso N1 helpers.
pub type NessoI2c = EspI2c<'static, Blocking>;
/// Concrete blocking SPI type used by Nesso N1 helpers.
pub type NessoSpi = Spi<'static, Blocking>;
/// Concrete output-pin type used by Nesso N1 helpers.
pub type NessoOutput = Output<'static>;
/// Concrete display type returned by the board support package.
pub type NessoDisplay =
    Display<LcdSpiDevice<NessoSpi, NessoOutput, Delay>, NessoOutput, NullOutputPin, NullOutputPin>;
/// Concrete passive-buzzer type returned by the board support package.
pub type NessoBuzzer = Buzzer<NessoOutput>;
/// Board-owned radio resources required by `nesso::wifi`.
#[cfg(feature = "wifi")]
pub type WifiResources = crate::wifi::RadioResources;

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
    /// Flash peripheral for application storage.
    pub flash: esp_hal::peripherals::FLASH<'static>,
}

/// Owns ESP-HAL peripherals before they are split into Nesso N1 services.
pub struct NessoN1Board {
    peripherals: esp_hal::peripherals::Peripherals,
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
        let _discarded = read_register(i2c, expander, 0x01)?;
        write_register(i2c, expander, 0x01, 0x01)?;
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
        let _discarded = read_register(i2c, address, 0x01)?;
        write_register(i2c, address, 0x01, 0x01)?;
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
        let mut i2c = Self::configure_i2c(
            self.peripherals.I2C0,
            self.peripherals.GPIO10,
            self.peripherals.GPIO8,
        )?;
        NessoN1::init_lcd_expander(&mut i2c, &mut delay).map_err(|_| BoardInitError::Expander)?;

        let display = Self::configure_display(
            self.peripherals.SPI2,
            self.peripherals.GPIO20,
            self.peripherals.GPIO21,
            self.peripherals.GPIO22,
            self.peripherals.GPIO17,
            self.peripherals.GPIO16,
        )?;
        Ok((display, i2c))
    }

    /// Initializes the display and returns board-owned Wi-Fi resources.
    #[cfg(feature = "wifi")]
    pub fn into_display_and_wifi(self) -> Result<(NessoDisplay, WifiResources), BoardInitError> {
        let mut delay = Delay::new();
        {
            let mut i2c = Self::configure_i2c(
                self.peripherals.I2C0,
                self.peripherals.GPIO10,
                self.peripherals.GPIO8,
            )?;
            NessoN1::init_lcd_expander(&mut i2c, &mut delay)
                .map_err(|_| BoardInitError::Expander)?;
        }

        let display = Self::configure_display(
            self.peripherals.SPI2,
            self.peripherals.GPIO20,
            self.peripherals.GPIO21,
            self.peripherals.GPIO22,
            self.peripherals.GPIO17,
            self.peripherals.GPIO16,
        )?;
        Ok((
            display,
            WifiResources {
                wifi: self.peripherals.WIFI,
                timer_group0: self.peripherals.TIMG0,
                software_interrupt: self.peripherals.SW_INTERRUPT,
            },
        ))
    }

    /// Initializes the core services used by the public `nesso` facade.
    pub fn into_core_parts(self) -> Result<NessoCoreParts, BoardInitError> {
        let mut delay = Delay::new();
        let mut i2c = Self::configure_i2c(
            self.peripherals.I2C0,
            self.peripherals.GPIO10,
            self.peripherals.GPIO8,
        )?;
        NessoN1::init_lcd_expander(&mut i2c, &mut delay).map_err(|_| BoardInitError::Expander)?;

        let display = Self::configure_display(
            self.peripherals.SPI2,
            self.peripherals.GPIO20,
            self.peripherals.GPIO21,
            self.peripherals.GPIO22,
            self.peripherals.GPIO17,
            self.peripherals.GPIO16,
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
                timer_group0: self.peripherals.TIMG0,
                software_interrupt: self.peripherals.SW_INTERRUPT,
            },
            flash: self.peripherals.FLASH,
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
    ) -> Result<NessoI2c, BoardInitError> {
        EspI2c::new(
            i2c0,
            I2cConfig::default().with_frequency(Rate::from_khz(400)),
        )
        .map(|i2c| i2c.with_sda(sda).with_scl(scl))
        .map_err(|_| BoardInitError::I2c)
    }

    fn configure_display(
        spi2: esp_hal::peripherals::SPI2<'static>,
        sck: esp_hal::peripherals::GPIO20<'static>,
        mosi: esp_hal::peripherals::GPIO21<'static>,
        miso: esp_hal::peripherals::GPIO22<'static>,
        cs_pin: esp_hal::peripherals::GPIO17<'static>,
        dc_pin: esp_hal::peripherals::GPIO16<'static>,
    ) -> Result<NessoDisplay, BoardInitError> {
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

        let cs = Output::new(cs_pin, Level::High, OutputConfig::default());
        let dc = Output::new(dc_pin, Level::Low, OutputConfig::default());
        let spi_device = LcdSpiDevice::new(spi, cs, Delay::new());

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
    let mut value = [0u8];
    i2c.write_read(address, &[register], &mut value)?;
    Ok(value[0])
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
