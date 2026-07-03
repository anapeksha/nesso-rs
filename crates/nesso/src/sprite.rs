//! Caller-owned RGB565 sprites and dirty-region helpers.
//!
//! ```rust,ignore
//! let mut pixels = [Rgb565::BLACK; 64 * 64];
//! let sprite = nesso::sprite::Sprite::new(64, 64, &mut pixels)?;
//! let mut dirty = nesso::sprite::DirtyRegions::<8>::new();
//! dirty.mark_clipped(area, sprite.bounds())?;
//! dirty.flush_sprite_at(&sprite, &mut display, Point::zero())?;
//! ```

use embedded_graphics::{Drawable, pixelcolor::Rgb565, prelude::*, primitives::Rectangle};
use heapless::Vec;

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

    /// Returns one pixel from the sprite.
    #[must_use]
    pub fn pixel(&self, point: Point) -> Option<Rgb565> {
        self.pixel_index(point).map(|index| self.pixels[index])
    }

    /// Copies a full-frame RGB565 slice into the sprite.
    pub fn copy_from_slice(&mut self, pixels: &[Rgb565]) -> Result<(), SpriteError> {
        if pixels.len() < self.pixels.len() {
            return Err(SpriteError::BufferTooSmall);
        }
        self.pixels.copy_from_slice(&pixels[..self.pixels.len()]);
        Ok(())
    }

    /// Draws the sprite into another draw target at `top_left`.
    pub fn draw_at<D>(&self, target: &mut D, top_left: Point) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        target.fill_contiguous(
            &Rectangle::new(top_left, self.size()),
            self.pixels.iter().copied(),
        )
    }

    /// Draws a clipped sprite region into another draw target.
    ///
    /// `source_area` is interpreted in sprite-local coordinates. Pixels outside
    /// the sprite are clipped before drawing, and `dest_top_left` is the target
    /// coordinate for the clipped region's top-left corner.
    pub fn draw_region_at<D>(
        &self,
        target: &mut D,
        source_area: &Rectangle,
        dest_top_left: Point,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let clipped = source_area.intersection(&self.bounds());
        if clipped.is_zero_sized() {
            return Ok(());
        }

        let offset = clipped.top_left - source_area.top_left;
        let target_area = Rectangle::new(dest_top_left + offset, clipped.size);
        target.fill_contiguous(&target_area, self.region_pixels(&clipped))
    }

    /// Clears a sprite-local overlay rectangle.
    pub fn clear_overlay(&mut self, area: Rectangle, color: Rgb565) -> Result<(), SpriteError> {
        self.fill_solid(&area, color)
    }

    /// Returns an iterator over a clipped region in row-major order.
    pub fn region_pixels(&self, area: &Rectangle) -> SpriteRegionPixels<'_> {
        let clipped = area.intersection(&self.bounds());
        SpriteRegionPixels {
            pixels: self.pixels,
            stride: usize::from(self.width),
            x: clipped.top_left.x.max(0) as usize,
            y: clipped.top_left.y.max(0) as usize,
            width: clipped.size.width as usize,
            height: clipped.size.height as usize,
            current_x: 0,
            current_y: 0,
        }
    }

    fn pixel_index(&self, point: Point) -> Option<usize> {
        if !self.bounds().contains(point) {
            return None;
        }
        Some(point.y as usize * usize::from(self.width) + point.x as usize)
    }
}

/// Iterator over a sprite region in row-major order.
pub struct SpriteRegionPixels<'a> {
    pixels: &'a [Rgb565],
    stride: usize,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    current_x: usize,
    current_y: usize,
}

impl Iterator for SpriteRegionPixels<'_> {
    type Item = Rgb565;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_y >= self.height {
            return None;
        }

        let index = (self.y + self.current_y) * self.stride + self.x + self.current_x;
        let color = self.pixels.get(index).copied();
        self.current_x += 1;
        if self.current_x >= self.width {
            self.current_x = 0;
            self.current_y += 1;
        }
        color
    }
}

/// Fixed-capacity list of dirty rectangles.
pub struct DirtyRegions<const N: usize> {
    regions: Vec<Rectangle, N>,
}

impl<const N: usize> DirtyRegions<N> {
    /// Creates an empty dirty-region list.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    /// Removes all tracked regions.
    pub fn clear(&mut self) {
        self.regions.clear();
    }

    /// Returns the number of tracked regions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.regions.len()
    }

    /// Returns true when no dirty regions are tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// Adds a dirty region after clipping it to `bounds`.
    pub fn push_clipped(
        &mut self,
        region: Rectangle,
        bounds: Rectangle,
    ) -> Result<(), SpriteError> {
        self.mark(region, bounds)
    }

    /// Marks a dirty region after clipping it to `bounds`.
    pub fn mark_clipped(
        &mut self,
        region: Rectangle,
        bounds: Rectangle,
    ) -> Result<(), SpriteError> {
        self.mark(region, bounds)
    }

    /// Marks a dirty region after clipping and coalescing it with overlaps.
    pub fn mark(&mut self, region: Rectangle, bounds: Rectangle) -> Result<(), SpriteError> {
        let clipped = region.intersection(&bounds);
        if clipped.is_zero_sized() {
            return Ok(());
        }
        if let Some(existing) = self
            .regions
            .iter_mut()
            .find(|existing| rectangles_touch_or_overlap(**existing, clipped))
        {
            *existing = bounding_union(*existing, clipped);
            return Ok(());
        }
        self.regions
            .push(clipped)
            .map_err(|_| SpriteError::DirtyRegionCapacity)
    }

    /// Coalesces overlapping or touching dirty regions.
    pub fn coalesce(&mut self) {
        let mut index = 0;
        while index < self.regions.len() {
            let mut other = index + 1;
            let mut merged = false;
            while other < self.regions.len() {
                if rectangles_touch_or_overlap(self.regions[index], self.regions[other]) {
                    self.regions[index] = bounding_union(self.regions[index], self.regions[other]);
                    let _removed = self.regions.remove(other);
                    merged = true;
                } else {
                    other += 1;
                }
            }
            if !merged {
                index += 1;
            }
        }
    }

    /// Returns an iterator over tracked regions.
    pub fn iter(&self) -> impl Iterator<Item = &Rectangle> {
        self.regions.iter()
    }

    /// Copies all dirty regions from a sprite to a draw target.
    pub fn flush_sprite<D>(&self, sprite: &Sprite<'_>, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        for region in self.iter() {
            sprite.draw_region_at(target, region, region.top_left)?;
        }
        Ok(())
    }

    /// Copies all dirty regions from a sprite to a draw target at `origin`.
    pub fn flush_sprite_at<D>(
        &self,
        sprite: &Sprite<'_>,
        target: &mut D,
        origin: Point,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        for region in self.iter() {
            sprite.draw_region_at(target, region, origin + region.top_left)?;
        }
        Ok(())
    }

    /// Flushes dirty sprite regions at `origin` and clears this dirty list.
    pub fn flush_and_clear_sprite_at<D>(
        &mut self,
        sprite: &Sprite<'_>,
        target: &mut D,
        origin: Point,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        self.flush_sprite_at(sprite, target, origin)?;
        self.clear();
        Ok(())
    }
}

impl<const N: usize> Default for DirtyRegions<N> {
    fn default() -> Self {
        Self::new()
    }
}

fn rectangles_touch_or_overlap(a: Rectangle, b: Rectangle) -> bool {
    let a_x0 = a.top_left.x;
    let a_y0 = a.top_left.y;
    let a_x1 = a.top_left.x + a.size.width as i32;
    let a_y1 = a.top_left.y + a.size.height as i32;
    let b_x0 = b.top_left.x;
    let b_y0 = b.top_left.y;
    let b_x1 = b.top_left.x + b.size.width as i32;
    let b_y1 = b.top_left.y + b.size.height as i32;

    a_x0 <= b_x1 && b_x0 <= a_x1 && a_y0 <= b_y1 && b_y0 <= a_y1
}

fn bounding_union(a: Rectangle, b: Rectangle) -> Rectangle {
    let x0 = a.top_left.x.min(b.top_left.x);
    let y0 = a.top_left.y.min(b.top_left.y);
    let x1 = (a.top_left.x + a.size.width as i32).max(b.top_left.x + b.size.width as i32);
    let y1 = (a.top_left.y + a.size.height as i32).max(b.top_left.y + b.size.height as i32);
    Rectangle::new(
        Point::new(x0, y0),
        Size::new((x1 - x0) as u32, (y1 - y0) as u32),
    )
}

/// Returns the intersection of `region` and `bounds`.
#[must_use]
pub fn clipped_region(region: Rectangle, bounds: Rectangle) -> Rectangle {
    region.intersection(&bounds)
}

/// Returns the smallest rectangle containing both input rectangles.
#[must_use]
pub fn union_region(a: Rectangle, b: Rectangle) -> Rectangle {
    bounding_union(a, b)
}

/// Clears an overlay rectangle on any RGB565 draw target.
pub fn clear_overlay<D>(target: &mut D, area: Rectangle, color: Rgb565) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    area.into_styled(embedded_graphics::primitives::PrimitiveStyle::with_fill(
        color,
    ))
    .draw(target)
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
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpriteError {
    /// The provided pixel buffer is smaller than `width * height`.
    BufferTooSmall,
    /// The dirty-region list is full.
    DirtyRegionCapacity,
}
