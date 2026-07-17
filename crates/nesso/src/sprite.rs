//! Caller-owned RGB565 sprites and dirty-region helpers.
//!
//! ```rust,ignore
//! let mut pixels = [Rgb565::BLACK; 64 * 64];
//! let sprite = nesso::sprite::Sprite::new(64, 64, &mut pixels)?;
//! let mut dirty = nesso::sprite::DirtyRegions::<8>::new();
//! dirty.mark_clipped(area, sprite.bounds())?;
//! dirty.flush_sprite_at(&sprite, &mut display, Point::zero())?;
//! ```

use core::convert::Infallible;

use embedded_graphics::{Drawable, pixelcolor::Rgb565, prelude::*, primitives::Rectangle};
use heapless::Vec;

/// Maximum horizontal sprite span supported by [`MaskedSprite`].
///
/// This matches the longest logical row of the 240 x 135 Nesso N1 panel.
pub const MAX_MASKED_SPRITE_WIDTH: usize = 240;
const MASK_BYTES_PER_ROW: usize = MAX_MASKED_SPRITE_WIDTH.div_ceil(8);

/// Owned, fixed-size RGB565 sprite storage.
///
/// Nested rows avoid unstable generic-const expressions while preserving one
/// contiguous row-major memory layout and zero-allocation construction.
pub struct SpriteBuffer<const W: usize, const H: usize> {
    colors: [[Rgb565; W]; H],
}

impl<const W: usize, const H: usize> SpriteBuffer<W, H> {
    /// Creates a sprite with every pixel initialized to `color`.
    #[must_use]
    pub const fn new(color: Rgb565) -> Self {
        Self {
            colors: [[color; W]; H],
        }
    }

    /// Returns the sprite bounds with origin at `(0, 0)`.
    #[must_use]
    pub const fn bounds(&self) -> Rectangle {
        Rectangle::new(Point::zero(), Size::new(W as u32, H as u32))
    }

    /// Returns the row-major color rows.
    #[must_use]
    pub const fn rows(&self) -> &[[Rgb565; W]; H] {
        &self.colors
    }

    /// Returns the mutable row-major color rows.
    pub fn rows_mut(&mut self) -> &mut [[Rgb565; W]; H] {
        &mut self.colors
    }

    /// Returns one color row.
    #[must_use]
    pub fn row(&self, y: usize) -> Option<&[Rgb565; W]> {
        self.colors.get(y)
    }

    /// Clears the complete buffer to `color`.
    pub fn clear(&mut self, color: Rgb565) {
        for row in &mut self.colors {
            row.fill(color);
        }
    }

    fn pixel_index(point: Point) -> Option<(usize, usize)> {
        let x = usize::try_from(point.x).ok()?;
        let y = usize::try_from(point.y).ok()?;
        (x < W && y < H).then_some((x, y))
    }
}

impl<const W: usize, const H: usize> DrawTarget for SpriteBuffer<W, H> {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if let Some((x, y)) = Self::pixel_index(point) {
                self.colors[y][x] = color;
            }
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let clipped = area.intersection(&self.bounds());
        let x = clipped.top_left.x.max(0) as usize;
        let y = clipped.top_left.y.max(0) as usize;
        let width = clipped.size.width as usize;
        let height = clipped.size.height as usize;
        for row in &mut self.colors[y..y + height] {
            row[x..x + width].fill(color);
        }
        Ok(())
    }
}

impl<const W: usize, const H: usize> OriginDimensions for SpriteBuffer<W, H> {
    fn size(&self) -> Size {
        Size::new(W as u32, H as u32)
    }
}

/// Fixed-size RGB565 sprite with a packed one-bit opacity mask.
///
/// Mask bits are stored least-significant bit first and padded at each row.
/// Row padding makes opaque-run discovery independent between rows and avoids
/// division in the display hot path. Only the first `W.div_ceil(8)` bytes of
/// each mask row are used. `W` must not exceed [`MAX_MASKED_SPRITE_WIDTH`].
pub struct MaskedSprite<const W: usize, const H: usize> {
    colors: [[Rgb565; W]; H],
    mask: [[u8; MASK_BYTES_PER_ROW]; H],
}

impl<const W: usize, const H: usize> MaskedSprite<W, H> {
    /// Creates a fully transparent sprite with initialized color storage.
    #[must_use]
    pub const fn new(color: Rgb565) -> Self {
        assert!(W <= MAX_MASKED_SPRITE_WIDTH);
        Self {
            colors: [[color; W]; H],
            mask: [[0; MASK_BYTES_PER_ROW]; H],
        }
    }

    /// Returns the sprite bounds with origin at `(0, 0)`.
    #[must_use]
    pub const fn bounds(&self) -> Rectangle {
        Rectangle::new(Point::zero(), Size::new(W as u32, H as u32))
    }

    /// Returns one contiguous color row.
    #[must_use]
    pub fn color_row(&self, y: usize) -> Option<&[Rgb565; W]> {
        self.colors.get(y)
    }

    /// Returns the packed mask rows.
    #[must_use]
    pub const fn mask_rows(&self) -> &[[u8; MASK_BYTES_PER_ROW]; H] {
        &self.mask
    }

    /// Returns whether a sprite-local pixel is opaque.
    #[must_use]
    pub fn is_opaque(&self, point: Point) -> bool {
        Self::pixel_index(point).is_some_and(|(x, y)| self.opaque_at(x, y))
    }

    /// Changes one sprite-local pixel's opacity without changing its color.
    pub fn set_opaque(&mut self, point: Point, opaque: bool) {
        if let Some((x, y)) = Self::pixel_index(point) {
            self.set_opaque_at(x, y, opaque);
        }
    }

    /// Makes the complete sprite transparent without touching color data.
    pub fn clear_mask(&mut self) {
        for row in &mut self.mask {
            row.fill(0);
        }
    }

    /// Returns the next opaque half-open range in row `y`, beginning at `x`.
    #[must_use]
    pub fn next_opaque_run(&self, y: usize, mut x: usize) -> Option<core::ops::Range<usize>> {
        if y >= H {
            return None;
        }
        while x < W && !self.opaque_at(x, y) {
            x += 1;
        }
        if x == W {
            return None;
        }
        let start = x;
        while x < W && self.opaque_at(x, y) {
            x += 1;
        }
        Some(start..x)
    }

    fn pixel_index(point: Point) -> Option<(usize, usize)> {
        let x = usize::try_from(point.x).ok()?;
        let y = usize::try_from(point.y).ok()?;
        (x < W && y < H).then_some((x, y))
    }

    fn opaque_at(&self, x: usize, y: usize) -> bool {
        self.mask[y][x / 8] & (1 << (x % 8)) != 0
    }

    fn set_opaque_at(&mut self, x: usize, y: usize, opaque: bool) {
        let bit = 1 << (x % 8);
        if opaque {
            self.mask[y][x / 8] |= bit;
        } else {
            self.mask[y][x / 8] &= !bit;
        }
    }
}

impl<const W: usize, const H: usize> DrawTarget for MaskedSprite<W, H> {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if let Some((x, y)) = Self::pixel_index(point) {
                self.colors[y][x] = color;
                self.set_opaque_at(x, y, true);
            }
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let clipped = area.intersection(&self.bounds());
        let x = clipped.top_left.x.max(0) as usize;
        let y = clipped.top_left.y.max(0) as usize;
        let width = clipped.size.width as usize;
        let height = clipped.size.height as usize;
        for row in y..y + height {
            self.colors[row][x..x + width].fill(color);
            for column in x..x + width {
                self.set_opaque_at(column, row, true);
            }
        }
        Ok(())
    }
}

impl<const W: usize, const H: usize> OriginDimensions for MaskedSprite<W, H> {
    fn size(&self) -> Size {
        Size::new(W as u32, H as u32)
    }
}

/// Two fixed-size canvases for ping-pong rendering.
pub struct DoubleBufferedCanvas<const W: usize, const H: usize> {
    buffers: [SpriteBuffer<W, H>; 2],
    display_index: usize,
}

impl<const W: usize, const H: usize> DoubleBufferedCanvas<W, H> {
    /// Creates two equally initialized canvas buffers.
    #[must_use]
    pub const fn new(color: Rgb565) -> Self {
        Self {
            buffers: [SpriteBuffer::new(color), SpriteBuffer::new(color)],
            display_index: 0,
        }
    }

    /// Returns the buffer that is safe for CPU drawing.
    pub fn drawing_buffer(&mut self) -> &mut SpriteBuffer<W, H> {
        &mut self.buffers[1 - self.display_index]
    }

    /// Returns the current display-transfer buffer.
    #[must_use]
    pub fn display_buffer(&self) -> &SpriteBuffer<W, H> {
        &self.buffers[self.display_index]
    }

    /// Publishes the painted buffer and returns disjoint transfer/drawing refs.
    ///
    /// The immutable transfer reference cannot alias the mutable drawing
    /// reference, so frame N may remain borrowed by a DMA future while frame
    /// N+1 is painted by another cooperatively scheduled task.
    pub fn swap(&mut self) -> (&SpriteBuffer<W, H>, &mut SpriteBuffer<W, H>) {
        self.display_index = 1 - self.display_index;
        if self.display_index == 0 {
            let (display, drawing) = self.buffers.split_at_mut(1);
            (&display[0], &mut drawing[0])
        } else {
            let (drawing, display) = self.buffers.split_at_mut(1);
            (&display[0], &mut drawing[0])
        }
    }
}

/// Fixed-capacity dirty-rectangle tracker for moving sprites.
pub struct DirtyRectTracker<const MAX_SPRITES: usize> {
    regions: [Option<Rectangle>; MAX_SPRITES],
    proximity: u32,
}

impl<const MAX_SPRITES: usize> DirtyRectTracker<MAX_SPRITES> {
    /// Default merge distance in pixels.
    pub const DEFAULT_PROXIMITY: u32 = 8;

    /// Creates a tracker using the default eight-pixel merge distance.
    #[must_use]
    pub const fn new() -> Self {
        Self::with_proximity(Self::DEFAULT_PROXIMITY)
    }

    /// Creates a tracker with a caller-selected merge distance.
    #[must_use]
    pub const fn with_proximity(proximity: u32) -> Self {
        Self {
            regions: [None; MAX_SPRITES],
            proximity,
        }
    }

    /// Records both the area to erase and the sprite's destination area.
    pub fn register_movement(
        &mut self,
        old_rect: Rectangle,
        new_rect: Rectangle,
    ) -> Result<(), SpriteError> {
        self.register(old_rect)?;
        self.register(new_rect)
    }

    /// Returns the consolidated dirty rectangles.
    pub fn iter(&self) -> impl Iterator<Item = &Rectangle> {
        self.regions.iter().filter_map(Option::as_ref)
    }

    /// Returns the number of consolidated rectangles.
    #[must_use]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Returns whether no region is dirty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    /// Clears all tracked movement.
    pub fn clear(&mut self) {
        self.regions.fill(None);
    }

    /// Clears each dirty area, invokes `redraw`, then resets the tracker.
    pub fn update_screen<D, F>(
        &mut self,
        target: &mut D,
        clear_color: Rgb565,
        mut redraw: F,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
        F: FnMut(&mut D, Rectangle) -> Result<(), D::Error>,
    {
        for region in self.iter().copied() {
            target.fill_solid(&region, clear_color)?;
            redraw(target, region)?;
        }
        self.clear();
        Ok(())
    }

    fn register(&mut self, region: Rectangle) -> Result<(), SpriteError> {
        if region.is_zero_sized() {
            return Ok(());
        }

        let mut merged = region;
        let mut index = 0;
        while index < MAX_SPRITES {
            if self.regions[index]
                .is_some_and(|current| rectangles_within(current, merged, self.proximity))
            {
                merged = bounding_union(self.regions[index].take().unwrap_or(merged), merged);
                index = 0;
            } else {
                index += 1;
            }
        }

        if let Some(slot) = self.regions.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(merged);
            Ok(())
        } else {
            Err(SpriteError::DirtyRegionCapacity)
        }
    }
}

impl<const MAX_SPRITES: usize> Default for DirtyRectTracker<MAX_SPRITES> {
    fn default() -> Self {
        Self::new()
    }
}

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

fn rectangles_within(a: Rectangle, b: Rectangle, proximity: u32) -> bool {
    let proximity = i32::try_from(proximity).unwrap_or(i32::MAX);
    let a_right = a.top_left.x.saturating_add_unsigned(a.size.width);
    let a_bottom = a.top_left.y.saturating_add_unsigned(a.size.height);
    let b_right = b.top_left.x.saturating_add_unsigned(b.size.width);
    let b_bottom = b.top_left.y.saturating_add_unsigned(b.size.height);

    a.top_left.x <= b_right.saturating_add(proximity)
        && b.top_left.x <= a_right.saturating_add(proximity)
        && a.top_left.y <= b_bottom.saturating_add(proximity)
        && b.top_left.y <= a_bottom.saturating_add(proximity)
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
