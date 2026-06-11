#![no_std]

use core::convert::Infallible;

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
    text::{Alignment, Text},
};
use embedded_hal::{
    delay::DelayNs,
    digital::{Error as DigitalError, OutputPin},
    spi::{Error as SpiError, ErrorKind as SpiErrorKind, Operation, SpiBus, SpiDevice},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayError<SpiError, PinError> {
    Spi(SpiError),
    Pin(PinError),
    Text,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rotation {
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplayGeometry {
    pub width: u16,
    pub height: u16,
    pub offset_x: u16,
    pub offset_y: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BusConfig {
    pub write_hz: u32,
    pub use_dma: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PanelConfig {
    pub geometry: DisplayGeometry,
    pub invert_colors: bool,
}

pub struct Display<SPI, DC, RST, BL> {
    spi: SPI,
    dc: DC,
    reset: RST,
    backlight: BL,
    panel: PanelConfig,
    bus: BusConfig,
    rotation: Rotation,
}

#[derive(Debug)]
pub enum LcdSpiDeviceError<BusError, CsError> {
    Bus(BusError),
    ChipSelect(CsError),
}

impl<BusError, CsError> SpiError for LcdSpiDeviceError<BusError, CsError>
where
    BusError: SpiError,
    CsError: DigitalError,
{
    fn kind(&self) -> SpiErrorKind {
        match self {
            Self::Bus(error) => error.kind(),
            Self::ChipSelect(_) => SpiErrorKind::ChipSelectFault,
        }
    }
}

pub struct LcdSpiDevice<Bus, Cs, Delay> {
    bus: Bus,
    cs: Cs,
    delay: Delay,
}

impl<Bus, Cs, Delay> LcdSpiDevice<Bus, Cs, Delay> {
    #[must_use]
    pub const fn new(bus: Bus, cs: Cs, delay: Delay) -> Self {
        Self { bus, cs, delay }
    }

    pub fn release(self) -> (Bus, Cs, Delay) {
        (self.bus, self.cs, self.delay)
    }
}

impl<Bus, Cs, Delay> embedded_hal::spi::ErrorType for LcdSpiDevice<Bus, Cs, Delay>
where
    Bus: SpiBus<u8>,
    Cs: OutputPin,
    Delay: DelayNs,
    Bus::Error: SpiError,
    Cs::Error: DigitalError,
{
    type Error = LcdSpiDeviceError<Bus::Error, Cs::Error>;
}

impl<Bus, Cs, Delay> SpiDevice for LcdSpiDevice<Bus, Cs, Delay>
where
    Bus: SpiBus<u8>,
    Cs: OutputPin,
    Delay: DelayNs,
    Bus::Error: SpiError,
    Cs::Error: DigitalError,
{
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        self.cs.set_low().map_err(LcdSpiDeviceError::ChipSelect)?;
        for operation in operations {
            match operation {
                Operation::Read(buffer) => self.bus.read(buffer).map_err(LcdSpiDeviceError::Bus)?,
                Operation::Write(buffer) => {
                    self.bus.write(buffer).map_err(LcdSpiDeviceError::Bus)?;
                }
                Operation::Transfer(read, write) => self
                    .bus
                    .transfer(read, write)
                    .map_err(LcdSpiDeviceError::Bus)?,
                Operation::TransferInPlace(buffer) => self
                    .bus
                    .transfer_in_place(buffer)
                    .map_err(LcdSpiDeviceError::Bus)?,
                Operation::DelayNs(delay) => self.delay.delay_ns(*delay),
            }
        }
        self.bus.flush().map_err(LcdSpiDeviceError::Bus)?;
        self.cs.set_high().map_err(LcdSpiDeviceError::ChipSelect)
    }
}

impl<SPI, DC, RST, BL> Display<SPI, DC, RST, BL> {
    #[must_use]
    pub const fn new(
        spi: SPI,
        dc: DC,
        reset: RST,
        backlight: BL,
        bus: BusConfig,
        panel: PanelConfig,
    ) -> Self {
        Self {
            spi,
            dc,
            reset,
            backlight,
            panel,
            bus,
            rotation: Rotation::Portrait,
        }
    }

    #[must_use]
    pub const fn geometry(&self) -> DisplayGeometry {
        self.panel.geometry
    }

    #[must_use]
    pub const fn bus_config(&self) -> BusConfig {
        self.bus
    }

    #[must_use]
    pub const fn panel_config(&self) -> PanelConfig {
        self.panel
    }

    #[must_use]
    pub const fn rotation(&self) -> Rotation {
        self.rotation
    }

    pub fn set_rotation(&mut self, rotation: Rotation) {
        self.rotation = rotation;
    }

    pub fn release(self) -> (SPI, DC, RST, BL) {
        (self.spi, self.dc, self.reset, self.backlight)
    }
}

impl<SPI, DC, RST, BL, SpiError, PinError> Display<SPI, DC, RST, BL>
where
    SPI: SpiDevice<Error = SpiError>,
    DC: OutputPin<Error = PinError>,
    RST: OutputPin<Error = PinError>,
    BL: OutputPin<Error = PinError>,
{
    pub fn init(&mut self) -> Result<(), DisplayError<SpiError, PinError>> {
        self.reset.set_low().map_err(DisplayError::Pin)?;
        self.reset.set_high().map_err(DisplayError::Pin)?;
        self.command(0x01, &[])?;
        self.command(0x11, &[])?;
        self.command(0x3A, &[0x55])?;
        if self.panel.invert_colors {
            self.command(0x21, &[])?;
        }
        self.command(0x29, &[])
    }

    pub fn set_backlight(&mut self, enabled: bool) -> Result<(), DisplayError<SpiError, PinError>> {
        if enabled {
            self.backlight.set_high().map_err(DisplayError::Pin)
        } else {
            self.backlight.set_low().map_err(DisplayError::Pin)
        }
    }

    pub fn command(
        &mut self,
        command: u8,
        data: &[u8],
    ) -> Result<(), DisplayError<SpiError, PinError>> {
        self.dc.set_low().map_err(DisplayError::Pin)?;
        self.spi.write(&[command]).map_err(DisplayError::Spi)?;
        if !data.is_empty() {
            self.dc.set_high().map_err(DisplayError::Pin)?;
            self.spi.write(data).map_err(DisplayError::Spi)?;
        }
        Ok(())
    }

    pub fn clear(&mut self, color: Rgb565) -> Result<(), DisplayError<SpiError, PinError>> {
        self.fill_solid(
            &Rectangle::new(
                Point::zero(),
                Size::new(
                    u32::from(self.panel.geometry.width),
                    u32::from(self.panel.geometry.height),
                ),
            ),
            color,
        )
    }

    pub fn print_centered(
        &mut self,
        text: &str,
        y: i32,
        color: Rgb565,
    ) -> Result<(), DisplayError<SpiError, PinError>> {
        let style = MonoTextStyle::new(&FONT_6X10, color);
        Text::with_alignment(
            text,
            Point::new(i32::from(self.panel.geometry.width) / 2, y),
            style,
            Alignment::Center,
        )
        .draw(self)
        .map(|_| ())
    }
}

impl<SPI, DC, RST, BL, SpiError, PinError> DrawTarget for Display<SPI, DC, RST, BL>
where
    SPI: SpiDevice<Error = SpiError>,
    DC: OutputPin<Error = PinError>,
    RST: OutputPin<Error = PinError>,
    BL: OutputPin<Error = PinError>,
{
    type Color = Rgb565;
    type Error = DisplayError<SpiError, PinError>;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if self.bounding_box().contains(point) {
                self.fill_solid(&Rectangle::new(point, Size::new(1, 1)), color)?;
            }
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let clipped = area.intersection(&self.bounding_box());
        if clipped.is_zero_sized() {
            return Ok(());
        }
        let raw = color.into_storage().to_be_bytes();
        let geometry = self.panel.geometry;
        let x0 = geometry.offset_x + clipped.top_left.x.max(0) as u16;
        let y0 = geometry.offset_y + clipped.top_left.y.max(0) as u16;
        let x1 = x0 + clipped.size.width.saturating_sub(1) as u16;
        let y1 = y0 + clipped.size.height.saturating_sub(1) as u16;
        self.command(
            0x2A,
            &[(x0 >> 8) as u8, x0 as u8, (x1 >> 8) as u8, x1 as u8],
        )?;
        self.command(
            0x2B,
            &[(y0 >> 8) as u8, y0 as u8, (y1 >> 8) as u8, y1 as u8],
        )?;
        self.dc.set_low().map_err(DisplayError::Pin)?;
        self.spi.write(&[0x2C]).map_err(DisplayError::Spi)?;
        self.dc.set_high().map_err(DisplayError::Pin)?;
        for _ in 0..clipped.size.width.saturating_mul(clipped.size.height) {
            self.spi.write(&raw).map_err(DisplayError::Spi)?;
        }
        Ok(())
    }
}

impl<SPI, DC, RST, BL> OriginDimensions for Display<SPI, DC, RST, BL> {
    fn size(&self) -> Size {
        Size::new(
            u32::from(self.panel.geometry.width),
            u32::from(self.panel.geometry.height),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NullOutputPin;

impl embedded_hal::digital::ErrorType for NullOutputPin {
    type Error = Infallible;
}

impl OutputPin for NullOutputPin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NullSpi;

impl embedded_hal::spi::ErrorType for NullSpi {
    type Error = Infallible;
}

impl SpiDevice for NullSpi {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        for operation in operations {
            match operation {
                Operation::Read(buffer) => buffer.fill(0),
                Operation::Write(_)
                | Operation::Transfer(_, _)
                | Operation::TransferInPlace(_)
                | Operation::DelayNs(_) => {}
            }
        }
        Ok(())
    }
}
