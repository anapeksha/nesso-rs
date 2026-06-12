use embedded_graphics::{pixelcolor::Rgb565, prelude::*, primitives::Rectangle};

/// Caller-owned RGB565 sprite buffer.
///
/// The SDK does not allocate sprite memory. Applications provide a mutable
/// pixel slice sized to `width * height`, draw into the sprite, then push it to
/// the display with `display.blit_pixels(...)`.
pub struct Sprite<'a> {
    width: u16,
    height: u16,
    pixels: &'a mut [Rgb565],
}

impl<'a> Sprite<'a> {
    /// Creates a sprite backed by caller-owned RGB565 memory.
    pub fn new(width: u16, height: u16, pixels: &'a mut [Rgb565]) -> Result<Self, SpriteError> {
        let required_len = usize::from(width) * usize::from(height);
        if pixels.len() < required_len {
            return Err(SpriteError::BufferTooSmall);
        }
        Ok(Self {
            width,
            height,
            pixels: &mut pixels[..required_len],
        })
    }

    /// Returns the sprite width in pixels.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Returns the sprite height in pixels.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Returns the sprite bounds with origin at `(0, 0)`.
    #[must_use]
    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(
            Point::zero(),
            Size::new(u32::from(self.width), u32::from(self.height)),
        )
    }

    /// Clears the entire sprite to one color.
    pub fn clear(&mut self, color: Rgb565) {
        self.pixels.fill(color);
    }

    /// Returns the contiguous RGB565 pixel buffer.
    #[must_use]
    pub fn pixels(&self) -> &[Rgb565] {
        self.pixels
    }

    /// Returns the mutable contiguous RGB565 pixel buffer.
    pub fn pixels_mut(&mut self) -> &mut [Rgb565] {
        self.pixels
    }

    fn pixel_index(&self, point: Point) -> Option<usize> {
        if !self.bounds().contains(point) {
            return None;
        }
        Some(point.y as usize * usize::from(self.width) + point.x as usize)
    }
}

impl DrawTarget for Sprite<'_> {
    type Color = Rgb565;
    type Error = SpriteError;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if let Some(index) = self.pixel_index(point) {
                self.pixels[index] = color;
            }
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let clipped = area.intersection(&self.bounds());
        if clipped.is_zero_sized() {
            return Ok(());
        }

        let x_start = clipped.top_left.x as usize;
        let y_start = clipped.top_left.y as usize;
        let width = clipped.size.width as usize;
        let height = clipped.size.height as usize;
        let stride = usize::from(self.width);

        for row in y_start..y_start + height {
            let start = row * stride + x_start;
            let end = start + width;
            if let Some(line) = self.pixels.get_mut(start..end) {
                line.fill(color);
            }
        }
        Ok(())
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let area_width = area.size.width as i32;
        if area_width <= 0 {
            return Ok(());
        }

        for (index, color) in colors.into_iter().enumerate() {
            let index = index as i32;
            let point = Point::new(
                area.top_left.x + index % area_width,
                area.top_left.y + index / area_width,
            );
            if let Some(pixel_index) = self.pixel_index(point) {
                self.pixels[pixel_index] = color;
            }
        }
        Ok(())
    }
}

impl OriginDimensions for Sprite<'_> {
    fn size(&self) -> Size {
        Size::new(u32::from(self.width), u32::from(self.height))
    }
}

/// Errors returned by sprite construction and drawing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpriteError {
    /// The provided pixel buffer is smaller than `width * height`.
    BufferTooSmall,
}
