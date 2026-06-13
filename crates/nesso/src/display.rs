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

const COLOR_STREAM_PIXELS: usize = 128;
const PIXEL_STREAM_PIXELS: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayError<SpiError, PinError> {
    Spi(SpiError),
    Pin(PinError),
    Text,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayOrientation {
    /// Native Nesso N1 portrait orientation, 135 x 240 logical pixels.
    Portrait,
    /// Portrait rotated 180 degrees, 135 x 240 logical pixels.
    PortraitInverted,
    /// Landscape orientation with logical pixels rotated clockwise.
    LandscapeClockwise,
    /// Landscape orientation with logical pixels rotated counter-clockwise.
    LandscapeCounterClockwise,
}

/// Backwards-compatible alias for the display orientation type.
pub type Rotation = DisplayOrientation;

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
    orientation: DisplayOrientation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PixelRun {
    y: i32,
    x_start: i32,
    x_end: i32,
    color: Rgb565,
}

impl PixelRun {
    fn new(point: Point, color: Rgb565) -> Self {
        Self {
            y: point.y,
            x_start: point.x,
            x_end: point.x,
            color,
        }
    }

    fn try_extend(&mut self, point: Point, color: Rgb565) -> bool {
        if self.y == point.y && self.x_end.saturating_add(1) == point.x && self.color == color {
            self.x_end = point.x;
            true
        } else {
            false
        }
    }

    fn rectangle(self) -> Rectangle {
        Rectangle::new(
            Point::new(self.x_start, self.y),
            Size::new((self.x_end - self.x_start + 1) as u32, 1),
        )
    }
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
    /// Creates an SPI device wrapper with manual chip-select control.
    #[must_use]
    pub const fn new(bus: Bus, cs: Cs, delay: Delay) -> Self {
        Self { bus, cs, delay }
    }

    /// Releases the wrapped SPI bus, chip-select pin, and delay provider.
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
    /// Creates a display driver from concrete bus and control pins.
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
            orientation: DisplayOrientation::Portrait,
        }
    }

    /// Returns the configured visible display geometry.
    #[must_use]
    pub const fn geometry(&self) -> DisplayGeometry {
        self.panel.geometry
    }

    /// Returns the configured bus parameters.
    #[must_use]
    pub const fn bus_config(&self) -> BusConfig {
        self.bus
    }

    /// Returns the configured panel parameters.
    #[must_use]
    pub const fn panel_config(&self) -> PanelConfig {
        self.panel
    }

    /// Returns the current logical orientation.
    #[must_use]
    pub const fn orientation(&self) -> DisplayOrientation {
        self.orientation
    }

    /// Returns the current logical orientation.
    #[must_use]
    pub const fn rotation(&self) -> Rotation {
        self.orientation
    }

    /// Sets the logical orientation used by the driver.
    ///
    /// All `DrawTarget`, text, fill, and blit coordinates are interpreted in
    /// this logical orientation and clipped before they are mapped to the
    /// native panel memory coordinates.
    pub fn set_orientation(&mut self, orientation: DisplayOrientation) {
        self.orientation = orientation;
    }

    /// Sets the logical rotation used by the driver.
    pub fn set_rotation(&mut self, rotation: Rotation) {
        self.set_orientation(rotation);
    }

    /// Releases the display bus and control pins.
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
    /// Initializes the ST7789-compatible panel.
    pub fn init(&mut self) -> Result<(), DisplayError<SpiError, PinError>> {
        self.reset.set_low().map_err(DisplayError::Pin)?;
        self.delay_ns(10_000_000)?;
        self.reset.set_high().map_err(DisplayError::Pin)?;
        self.delay_ns(120_000_000)?;
        self.command(0x01, &[])?;
        self.delay_ns(150_000_000)?;
        self.command(0x11, &[])?;
        self.delay_ns(120_000_000)?;
        self.command(0x3A, &[0x55])?;
        if self.panel.invert_colors {
            self.command(0x21, &[])?;
        }
        self.command(0x29, &[])?;
        self.delay_ns(20_000_000)
    }

    /// Enables or disables the display backlight pin.
    pub fn set_backlight(&mut self, enabled: bool) -> Result<(), DisplayError<SpiError, PinError>> {
        if enabled {
            self.backlight.set_high().map_err(DisplayError::Pin)
        } else {
            self.backlight.set_low().map_err(DisplayError::Pin)
        }
    }

    /// Sends one display command followed by optional data bytes.
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

    fn delay_ns(&mut self, delay_ns: u32) -> Result<(), DisplayError<SpiError, PinError>> {
        self.spi
            .transaction(&mut [Operation::DelayNs(delay_ns)])
            .map_err(DisplayError::Spi)
    }

    /// Fills the visible display with one color.
    pub fn clear(&mut self, color: Rgb565) -> Result<(), DisplayError<SpiError, PinError>> {
        self.fill_solid(
            &Rectangle::new(
                Point::zero(),
                Size::new(self.logical_size().width, self.logical_size().height),
            ),
            color,
        )
    }

    /// Fills a rectangular region with one color.
    pub fn fill_rect(
        &mut self,
        area: &Rectangle,
        color: Rgb565,
    ) -> Result<(), DisplayError<SpiError, PinError>> {
        self.fill_solid(area, color)
    }

    /// Clears a rectangular region to one color.
    pub fn clear_region(
        &mut self,
        area: &Rectangle,
        color: Rgb565,
    ) -> Result<(), DisplayError<SpiError, PinError>> {
        self.fill_solid(area, color)
    }

    /// Streams a contiguous RGB565 pixel source into a rectangular region.
    ///
    /// This is the preferred path for sprites, image buffers, and other
    /// high-throughput drawing because it opens one panel address window and
    /// streams pixel data in chunks.
    pub fn blit_pixels<I>(
        &mut self,
        area: &Rectangle,
        pixels: I,
    ) -> Result<(), DisplayError<SpiError, PinError>>
    where
        I: IntoIterator<Item = Rgb565>,
    {
        let clipped = area.intersection(&self.bounding_box());
        if clipped.is_zero_sized() {
            return Ok(());
        }

        if self.orientation == DisplayOrientation::Portrait && clipped == *area {
            self.set_address_window(&clipped)?;
            self.write_color_stream(
                pixels,
                clipped.size.width.saturating_mul(clipped.size.height) as usize,
            )
        } else {
            self.fill_contiguous(area, pixels)
        }
    }

    /// Draws a single centered text line using the built-in mono font.
    pub fn print_centered(
        &mut self,
        text: &str,
        y: i32,
        color: Rgb565,
    ) -> Result<(), DisplayError<SpiError, PinError>> {
        let style = MonoTextStyle::new(&FONT_6X10, color);
        Text::with_alignment(
            text,
            Point::new(self.logical_size().width as i32 / 2, y),
            style,
            Alignment::Center,
        )
        .draw(self)
        .map(|_| ())
    }

    /// Draws one text line at a fixed point using the built-in mono font.
    pub fn print_at(
        &mut self,
        text: &str,
        position: Point,
        color: Rgb565,
    ) -> Result<(), DisplayError<SpiError, PinError>> {
        let style = MonoTextStyle::new(&FONT_6X10, color);
        Text::new(text, position, style).draw(self).map(|_| ())
    }

    fn flush_run(&mut self, run: PixelRun) -> Result<(), DisplayError<SpiError, PinError>> {
        self.fill_solid(&run.rectangle(), run.color)
    }

    fn logical_size(&self) -> Size {
        let geometry = self.panel.geometry;
        match self.orientation {
            DisplayOrientation::Portrait | DisplayOrientation::PortraitInverted => {
                Size::new(u32::from(geometry.width), u32::from(geometry.height))
            }
            DisplayOrientation::LandscapeClockwise
            | DisplayOrientation::LandscapeCounterClockwise => {
                Size::new(u32::from(geometry.height), u32::from(geometry.width))
            }
        }
    }

    fn map_rectangle_to_native(&self, area: &Rectangle) -> Rectangle {
        let geometry = self.panel.geometry;
        let native_width = i32::from(geometry.width);
        let native_height = i32::from(geometry.height);
        let x = area.top_left.x;
        let y = area.top_left.y;
        let width = area.size.width as i32;
        let height = area.size.height as i32;

        match self.orientation {
            DisplayOrientation::Portrait => *area,
            DisplayOrientation::PortraitInverted => Rectangle::new(
                Point::new(native_width - x - width, native_height - y - height),
                area.size,
            ),
            DisplayOrientation::LandscapeClockwise => Rectangle::new(
                Point::new(y, native_height - x - width),
                Size::new(area.size.height, area.size.width),
            ),
            DisplayOrientation::LandscapeCounterClockwise => Rectangle::new(
                Point::new(native_width - y - height, x),
                Size::new(area.size.height, area.size.width),
            ),
        }
    }

    fn set_address_window(
        &mut self,
        area: &Rectangle,
    ) -> Result<(), DisplayError<SpiError, PinError>> {
        let geometry = self.panel.geometry;
        let x0 = geometry.offset_x + area.top_left.x.max(0) as u16;
        let y0 = geometry.offset_y + area.top_left.y.max(0) as u16;
        let x1 = x0 + area.size.width.saturating_sub(1) as u16;
        let y1 = y0 + area.size.height.saturating_sub(1) as u16;
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
        self.dc.set_high().map_err(DisplayError::Pin)
    }

    fn write_repeated_color(
        &mut self,
        color: Rgb565,
        pixels: usize,
    ) -> Result<(), DisplayError<SpiError, PinError>> {
        let raw = color.into_storage().to_be_bytes();
        let mut chunk = [0u8; COLOR_STREAM_PIXELS * 2];
        for pixel in chunk.chunks_exact_mut(2) {
            pixel.copy_from_slice(&raw);
        }

        let mut remaining = pixels;
        while remaining > 0 {
            let write_pixels = remaining.min(COLOR_STREAM_PIXELS);
            let write_len = write_pixels * 2;
            self.spi
                .write(&chunk[..write_len])
                .map_err(DisplayError::Spi)?;
            remaining -= write_pixels;
        }
        Ok(())
    }

    fn write_color_stream<I>(
        &mut self,
        pixels: I,
        max_pixels: usize,
    ) -> Result<(), DisplayError<SpiError, PinError>>
    where
        I: IntoIterator<Item = Rgb565>,
    {
        let mut chunk = [0u8; PIXEL_STREAM_PIXELS * 2];
        let mut buffered_pixels = 0usize;

        for (written_pixels, color) in pixels.into_iter().enumerate() {
            if written_pixels >= max_pixels {
                break;
            }
            let raw = color.into_storage().to_be_bytes();
            let offset = buffered_pixels * 2;
            chunk[offset] = raw[0];
            chunk[offset + 1] = raw[1];
            buffered_pixels += 1;

            if buffered_pixels == PIXEL_STREAM_PIXELS {
                self.spi.write(&chunk).map_err(DisplayError::Spi)?;
                buffered_pixels = 0;
            }
        }

        if buffered_pixels > 0 {
            self.spi
                .write(&chunk[..buffered_pixels * 2])
                .map_err(DisplayError::Spi)?;
        }
        Ok(())
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
        let mut run = None;

        for Pixel(point, color) in pixels {
            if !self.bounding_box().contains(point) {
                if let Some(current) = run.take() {
                    self.flush_run(current)?;
                }
                continue;
            }

            if let Some(mut current) = run.take() {
                if current.try_extend(point, color) {
                    run = Some(current);
                } else {
                    self.flush_run(current)?;
                    run = Some(PixelRun::new(point, color));
                }
            } else {
                run = Some(PixelRun::new(point, color));
            }
        }

        if let Some(current) = run {
            self.flush_run(current)?;
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let clipped = area.intersection(&self.bounding_box());
        if clipped.is_zero_sized() {
            return Ok(());
        }
        let native = self.map_rectangle_to_native(&clipped);
        self.set_address_window(&native)?;
        self.write_repeated_color(
            color,
            clipped.size.width.saturating_mul(clipped.size.height) as usize,
        )
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let clipped = area.intersection(&self.bounding_box());
        if clipped.is_zero_sized() {
            return Ok(());
        }

        if self.orientation == DisplayOrientation::Portrait && clipped == *area {
            self.set_address_window(&clipped)?;
            return self.write_color_stream(
                colors,
                clipped.size.width.saturating_mul(clipped.size.height) as usize,
            );
        }

        let area_width = area.size.width as i32;
        if area_width <= 0 {
            return Ok(());
        }

        let mut run = None;
        for (index, color) in colors.into_iter().enumerate() {
            let index = index as i32;
            let point = Point::new(
                area.top_left.x + index % area_width,
                area.top_left.y + index / area_width,
            );
            if !clipped.contains(point) {
                if let Some(current) = run.take() {
                    self.flush_run(current)?;
                }
                continue;
            }

            if let Some(mut current) = run.take() {
                if current.try_extend(point, color) {
                    run = Some(current);
                } else {
                    self.flush_run(current)?;
                    run = Some(PixelRun::new(point, color));
                }
            } else {
                run = Some(PixelRun::new(point, color));
            }
        }

        if let Some(current) = run {
            self.flush_run(current)?;
        }
        Ok(())
    }
}

impl<SPI, DC, RST, BL> OriginDimensions for Display<SPI, DC, RST, BL> {
    fn size(&self) -> Size {
        let geometry = self.panel.geometry;
        match self.orientation {
            DisplayOrientation::Portrait | DisplayOrientation::PortraitInverted => {
                Size::new(u32::from(geometry.width), u32::from(geometry.height))
            }
            DisplayOrientation::LandscapeClockwise
            | DisplayOrientation::LandscapeCounterClockwise => {
                Size::new(u32::from(geometry.height), u32::from(geometry.width))
            }
        }
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
