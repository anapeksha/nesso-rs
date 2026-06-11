#![no_std]

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
use nesso_audio::Buzzer;
use nesso_display::{
    BusConfig, Display, DisplayError, DisplayGeometry, LcdSpiDevice, NullOutputPin, PanelConfig,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardError {
    ResourceConflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardInitError {
    I2c,
    Spi,
    Expander,
    Display,
}

pub const EXPANDER_OUTPUT_ENABLE: u8 = 0x03;
pub const EXPANDER_OUTPUT_STATE: u8 = 0x05;
pub const EXPANDER_HIGH_IMPEDANCE: u8 = 0x07;
pub const EXPANDER_DEFAULT_OUTPUT: u8 = 0x09;
pub const EXPANDER_INTERRUPT_MASK: u8 = 0x11;
pub const EXPANDER_INTERRUPT_STATUS: u8 = 0x13;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct I2cAddress(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gpio(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpanderPin {
    pub address: I2cAddress,
    pub pin: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Signal {
    Native(Gpio),
    Expander(ExpanderPin),
    Named(&'static str),
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct DisplayConfig {
    pub width: u16,
    pub height: u16,
    pub color_depth_bits: u8,
    pub controller: &'static str,
    pub spi_mosi: Gpio,
    pub spi_miso: Gpio,
    pub spi_sck: Gpio,
    pub spi_write_hz: u32,
    pub offset_x: u16,
    pub offset_y: u16,
    pub invert_colors: bool,
    pub chip_select: Signal,
    pub data_command: Signal,
    pub reset: Signal,
    pub backlight: Signal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardResources {
    claimed: u32,
}

impl BoardResources {
    #[must_use]
    pub const fn new() -> Self {
        Self { claimed: 0 }
    }

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Resource {
    I2cMain = 0,
    SpiShared = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NessoN1 {
    resources: BoardResources,
}

pub type NessoI2c = EspI2c<'static, Blocking>;
pub type NessoSpi = Spi<'static, Blocking>;
pub type NessoOutput = Output<'static>;
pub type NessoDisplay =
    Display<LcdSpiDevice<NessoSpi, NessoOutput, Delay>, NessoOutput, NullOutputPin, NullOutputPin>;
pub type NessoBuzzer = Buzzer<NessoOutput>;

pub struct WifiResources {
    pub wifi: esp_hal::peripherals::WIFI<'static>,
    pub timer_group0: esp_hal::peripherals::TIMG0<'static>,
    pub software_interrupt: esp_hal::peripherals::SW_INTERRUPT<'static>,
}

pub struct NessoN1Board {
    peripherals: esp_hal::peripherals::Peripherals,
}

impl NessoN1 {
    pub const DISPLAY_WIDTH: u16 = 135;
    pub const DISPLAY_HEIGHT: u16 = 240;
    pub const DISPLAY_OFFSET_X: u16 = 52;
    pub const DISPLAY_OFFSET_Y: u16 = 40;
    pub const DISPLAY_SPI_WRITE_HZ: u32 = 40_000_000;
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

    pub fn new(mut resources: BoardResources) -> Result<Self, BoardError> {
        resources.claim(Resource::I2cMain)?;
        resources.claim(Resource::SpiShared)?;
        Ok(Self { resources })
    }

    #[must_use]
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
}

impl NessoN1Board {
    #[must_use]
    pub const fn new(peripherals: esp_hal::peripherals::Peripherals) -> Self {
        Self { peripherals }
    }

    pub fn into_display(self) -> Result<NessoDisplay, BoardInitError> {
        let (display, _i2c) = self.into_display_and_i2c()?;
        Ok(display)
    }

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

    #[must_use]
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

pub type Board = NessoN1;

impl NessoN1 {
    #[must_use]
    pub fn resources(&self) -> &BoardResources {
        &self.resources
    }
}

pub fn configure_output<I2C, Error>(i2c: &mut I2C, address: u8, bit: u8) -> Result<(), Error>
where
    I2C: I2c<Error = Error>,
{
    write_bit(i2c, address, EXPANDER_OUTPUT_ENABLE, bit, true)?;
    write_bit(i2c, address, EXPANDER_HIGH_IMPEDANCE, bit, false)
}

pub fn read_register<I2C, Error>(i2c: &mut I2C, address: u8, register: u8) -> Result<u8, Error>
where
    I2C: I2c<Error = Error>,
{
    let mut value = [0u8];
    i2c.write_read(address, &[register], &mut value)?;
    Ok(value[0])
}

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
