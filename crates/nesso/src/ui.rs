//! Lightweight graphics and UI helpers for Nesso display applications.
//!
//! The helpers operate on any `embedded-graphics` draw target and do not own
//! application state. They are intended for small embedded screens where layout
//! and dirty-region rendering should stay predictable.
//!
//! ```rust,ignore
//! nesso::ui::draw_black_dither_veil(&mut display, area, nesso::ui::DitherPattern::Checker50)?;
//! ```

use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyleBuilder, ascii::FONT_6X10},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
    text::{Alignment, Text},
};

/// Insets applied around a rectangle.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Insets {
    /// Left inset in pixels.
    pub left: u32,
    /// Top inset in pixels.
    pub top: u32,
    /// Right inset in pixels.
    pub right: u32,
    /// Bottom inset in pixels.
    pub bottom: u32,
}

impl Insets {
    /// Creates symmetric horizontal and vertical insets.
    #[must_use]
    pub const fn symmetric(horizontal: u32, vertical: u32) -> Self {
        Self {
            left: horizontal,
            top: vertical,
            right: horizontal,
            bottom: vertical,
        }
    }

    /// Applies the insets to `area`.
    #[must_use]
    pub fn apply(self, area: Rectangle) -> Rectangle {
        let width = area
            .size
            .width
            .saturating_sub(self.left.saturating_add(self.right));
        let height = area
            .size
            .height
            .saturating_sub(self.top.saturating_add(self.bottom));
        Rectangle::new(
            area.top_left + Point::new(self.left as i32, self.top as i32),
            Size::new(width, height),
        )
    }
}

/// Horizontal text alignment.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextAlign {
    /// Align text to the left edge.
    Left,
    /// Align text around the horizontal center.
    Center,
    /// Align text to the right edge.
    Right,
}

/// Text drawing style for compact embedded UI labels.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LabelStyle {
    /// Text color.
    pub color: Rgb565,
    /// Optional background color used to clear the label area first.
    pub background: Option<Rgb565>,
    /// Horizontal alignment.
    pub align: TextAlign,
}

/// Multi-line text drawing style.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextBlockStyle {
    /// Text color.
    pub color: Rgb565,
    /// Optional background color used to clear the text block first.
    pub background: Option<Rgb565>,
    /// Space between text baselines in pixels.
    pub line_height: u32,
}

/// Ordered dither pattern used for faux translucency on RGB565 targets.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DitherPattern {
    /// Draw one quarter of pixels.
    Sparse25,
    /// Draw half the pixels in a checkerboard pattern.
    Checker50,
    /// Draw roughly three quarters of pixels.
    Dense75,
    /// Draw every fourth vertical column.
    Vertical25,
    /// Draw alternating vertical columns.
    Vertical50,
}

impl DitherPattern {
    /// Returns true when a pixel at `point` should be filled.
    #[must_use]
    pub const fn covers(self, point: Point) -> bool {
        match self {
            Self::Sparse25 => ((point.x + point.y) & 0b11) == 0,
            Self::Checker50 => ((point.x ^ point.y) & 1) == 0,
            Self::Dense75 => ((point.x + point.y) & 0b11) != 0,
            Self::Vertical25 => (point.x & 0b11) == 0,
            Self::Vertical50 => (point.x & 1) == 0,
        }
    }
}

/// Caller-owned circular graph series buffer.
pub struct GraphSeriesBuffer<'a> {
    values: &'a mut [f32],
    start: usize,
    len: usize,
}

impl<'a> GraphSeriesBuffer<'a> {
    /// Creates an empty graph series backed by caller-owned storage.
    pub fn new(values: &'a mut [f32]) -> Self {
        Self {
            values,
            start: 0,
            len: 0,
        }
    }

    /// Appends a sample, overwriting the oldest sample when full.
    pub fn push(&mut self, value: f32) {
        if self.values.is_empty() {
            return;
        }
        if self.len < self.values.len() {
            let index = (self.start + self.len) % self.values.len();
            self.values[index] = value;
            self.len += 1;
        } else {
            self.values[self.start] = value;
            self.start = (self.start + 1) % self.values.len();
        }
    }

    /// Clears the logical series.
    pub fn clear(&mut self) {
        self.start = 0;
        self.len = 0;
    }

    /// Returns the number of samples.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns true when no samples are stored.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the sample at chronological `index`.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<f32> {
        if index >= self.len || self.values.is_empty() {
            return None;
        }
        Some(self.values[(self.start + index) % self.values.len()])
    }

    /// Iterates samples from oldest to newest.
    pub fn iter(&self) -> GraphSeriesIter<'_> {
        GraphSeriesIter {
            values: self.values,
            start: self.start,
            len: self.len,
            index: 0,
        }
    }
}

/// Iterator over a graph series buffer.
pub struct GraphSeriesIter<'a> {
    values: &'a [f32],
    start: usize,
    len: usize,
    index: usize,
}

impl Iterator for GraphSeriesIter<'_> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len || self.values.is_empty() {
            return None;
        }
        let sample = self.values[(self.start + self.index) % self.values.len()];
        self.index += 1;
        Some(sample)
    }
}

/// Mapping between graph data values and display pixels.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphViewport {
    /// Pixel area used for the graph.
    pub area: Rectangle,
    /// Data value drawn at the bottom of `area`.
    pub min: f32,
    /// Data value drawn at the top of `area`.
    pub max: f32,
}

impl GraphViewport {
    /// Creates a viewport for values in `min..=max`.
    #[must_use]
    pub const fn new(area: Rectangle, min: f32, max: f32) -> Self {
        Self { area, min, max }
    }

    /// Maps a sample index and count to an X coordinate.
    #[must_use]
    pub fn x_for_index(self, index: usize, count: usize) -> i32 {
        if count <= 1 || self.area.size.width <= 1 {
            return self.area.top_left.x;
        }
        self.area.top_left.x
            + (index as i32 * (self.area.size.width as i32 - 1) / (count as i32 - 1))
    }

    /// Maps a data value to a clipped Y coordinate.
    #[must_use]
    pub fn y_for_value(self, value: f32) -> i32 {
        let span = self.max - self.min;
        if span <= f32::EPSILON || self.area.size.height <= 1 {
            return self.area.top_left.y + self.area.size.height as i32 - 1;
        }
        let normalized = ((value - self.min) / span).clamp(0.0, 1.0);
        self.area.top_left.y + ((1.0 - normalized) * (self.area.size.height as f32 - 1.0)) as i32
    }

    /// Maps a sample to a point inside the viewport.
    #[must_use]
    pub fn point_for(self, index: usize, count: usize, value: f32) -> Point {
        Point::new(self.x_for_index(index, count), self.y_for_value(value))
    }

    /// Returns the graph x-axis Y coordinate.
    #[must_use]
    pub fn x_axis_y(self) -> i32 {
        self.area.top_left.y + self.area.size.height as i32 - 1
    }
}

impl TextBlockStyle {
    /// Creates a text block style with the built-in mono font.
    #[must_use]
    pub const fn new(color: Rgb565) -> Self {
        Self {
            color,
            background: None,
            line_height: 12,
        }
    }

    /// Returns this style with a background clear color.
    #[must_use]
    pub const fn with_background(mut self, background: Rgb565) -> Self {
        self.background = Some(background);
        self
    }
}

impl LabelStyle {
    /// Creates a centered label style with no background clear.
    #[must_use]
    pub const fn centered(color: Rgb565) -> Self {
        Self {
            color,
            background: None,
            align: TextAlign::Center,
        }
    }

    /// Returns this style with a background clear color.
    #[must_use]
    pub const fn with_background(mut self, background: Rgb565) -> Self {
        self.background = Some(background);
        self
    }
}

/// Layout helper for a fixed-size screen.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScreenLayout {
    bounds: Rectangle,
}

impl ScreenLayout {
    /// Creates a layout helper from a screen size.
    #[must_use]
    pub fn new(size: Size) -> Self {
        Self {
            bounds: Rectangle::new(Point::zero(), size),
        }
    }

    /// Returns the full screen bounds.
    #[must_use]
    pub const fn bounds(&self) -> Rectangle {
        self.bounds
    }

    /// Returns the content bounds after applying insets.
    #[must_use]
    pub fn content(&self, insets: Insets) -> Rectangle {
        insets.apply(self.bounds)
    }

    /// Returns a horizontal row inside the screen.
    #[must_use]
    pub fn row(&self, y: i32, height: u32, insets: Insets) -> Rectangle {
        let content = self.content(insets);
        Rectangle::new(
            Point::new(content.top_left.x, y),
            Size::new(content.size.width, height),
        )
    }
}

/// Trait for stateful screens that render into an embedded-graphics target.
pub trait View<D>
where
    D: DrawTarget<Color = Rgb565>,
{
    /// Draws the view into the provided target.
    fn render(&mut self, target: &mut D) -> Result<(), D::Error>;
}

/// Draws one text label inside `area`.
pub fn draw_label<D>(
    target: &mut D,
    area: Rectangle,
    text: &str,
    style: LabelStyle,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    if let Some(background) = style.background {
        area.into_styled(PrimitiveStyle::with_fill(background))
            .draw(target)?;
    }

    let (x, alignment) = match style.align {
        TextAlign::Left => (area.top_left.x, Alignment::Left),
        TextAlign::Center => (
            area.top_left.x + (area.size.width / 2) as i32,
            Alignment::Center,
        ),
        TextAlign::Right => (area.top_left.x + area.size.width as i32, Alignment::Right),
    };
    let y = area.top_left.y + (area.size.height / 2) as i32 + 4;
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(style.color)
        .build();

    Text::with_alignment(text, Point::new(x, y), text_style, alignment)
        .draw(target)
        .map(|_| ())
}

/// Draws a horizontal progress bar.
pub fn draw_progress_bar<D>(
    target: &mut D,
    area: Rectangle,
    value: u8,
    foreground: Rgb565,
    background: Rgb565,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    area.into_styled(PrimitiveStyle::with_fill(background))
        .draw(target)?;

    let clamped = value.min(100);
    let filled_width = area.size.width.saturating_mul(u32::from(clamped)) / 100;
    if filled_width == 0 {
        return Ok(());
    }

    Rectangle::new(area.top_left, Size::new(filled_width, area.size.height))
        .into_styled(PrimitiveStyle::with_fill(foreground))
        .draw(target)
}

/// Draws a dithered rectangle fill.
pub fn draw_dithered_rect<D>(
    target: &mut D,
    area: Rectangle,
    color: Rgb565,
    pattern: DitherPattern,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    if area.is_zero_sized() {
        return Ok(());
    }
    let x1 = area.top_left.x + area.size.width as i32;
    let y1 = area.top_left.y + area.size.height as i32;
    for y in area.top_left.y..y1 {
        for x in area.top_left.x..x1 {
            let point = Point::new(x, y);
            if pattern.covers(point) {
                Pixel(point, color).draw(target)?;
            }
        }
    }
    Ok(())
}

/// Draws a black dither veil for dimming an underlying area.
pub fn draw_black_dither_veil<D>(
    target: &mut D,
    area: Rectangle,
    pattern: DitherPattern,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    draw_dithered_rect(target, area, Rgb565::BLACK, pattern)
}

/// Draws a sparse vertical fill between two Y coordinates.
pub fn draw_sparse_vertical_fill<D>(
    target: &mut D,
    x: i32,
    top_y: i32,
    bottom_y: i32,
    color: Rgb565,
    pattern: DitherPattern,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    for y in top_y.min(bottom_y)..=top_y.max(bottom_y) {
        let point = Point::new(x, y);
        if pattern.covers(point) {
            Pixel(point, color).draw(target)?;
        }
    }
    Ok(())
}

/// Draws one graph series as connected line segments.
pub fn draw_graph_series<D>(
    target: &mut D,
    viewport: GraphViewport,
    values: &[f32],
    color: Rgb565,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    if values.len() < 2 {
        return Ok(());
    }
    let mut previous = viewport.point_for(0, values.len(), values[0]);
    for (index, value) in values.iter().copied().enumerate().skip(1) {
        let current = viewport.point_for(index, values.len(), value);
        draw_line(target, previous, current, color, 1)?;
        previous = current;
    }
    Ok(())
}

/// Draws dithered fill from one series down to the x-axis.
pub fn draw_graph_fill_to_axis<D>(
    target: &mut D,
    viewport: GraphViewport,
    values: &[f32],
    color: Rgb565,
    pattern: DitherPattern,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    for (index, value) in values.iter().copied().enumerate() {
        let point = viewport.point_for(index, values.len(), value);
        draw_sparse_vertical_fill(
            target,
            point.x,
            point.y,
            viewport.x_axis_y(),
            color,
            pattern,
        )?;
    }
    Ok(())
}

/// Draws layered, non-overlapping dither fills for three graph series.
///
/// Each sample column is sorted by scaled Y coordinate so fill ownership
/// changes correctly when lines cross.
pub fn draw_three_series_layered_fills<D>(
    target: &mut D,
    viewport: GraphViewport,
    series: [(&[f32], Rgb565, DitherPattern); 3],
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    let count = series
        .iter()
        .map(|(values, _, _)| values.len())
        .min()
        .unwrap_or(0);
    for index in 0..count {
        let x = viewport.x_for_index(index, count);
        let mut bands = [
            (
                viewport.y_for_value(series[0].0[index]),
                series[0].1,
                series[0].2,
            ),
            (
                viewport.y_for_value(series[1].0[index]),
                series[1].1,
                series[1].2,
            ),
            (
                viewport.y_for_value(series[2].0[index]),
                series[2].1,
                series[2].2,
            ),
        ];
        sort_graph_bands(&mut bands);
        draw_sparse_vertical_fill(target, x, bands[0].0, bands[1].0, bands[0].1, bands[0].2)?;
        draw_sparse_vertical_fill(target, x, bands[1].0, bands[2].0, bands[1].1, bands[1].2)?;
        draw_sparse_vertical_fill(
            target,
            x,
            bands[2].0,
            viewport.x_axis_y(),
            bands[2].1,
            bands[2].2,
        )?;
    }
    Ok(())
}

/// Draws y-axis labels at normalized positions from 0.0 bottom to 1.0 top.
pub fn draw_graph_y_labels<D>(
    target: &mut D,
    viewport: GraphViewport,
    labels: &[(&str, f32)],
    color: Rgb565,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(color)
        .build();
    for (text, normalized) in labels.iter().copied() {
        let y = viewport.area.top_left.y
            + ((1.0 - normalized.clamp(0.0, 1.0)) * viewport.area.size.height as f32) as i32;
        Text::new(text, Point::new(viewport.area.top_left.x, y), text_style).draw(target)?;
    }
    Ok(())
}

/// Draws x-axis labels at normalized positions from 0.0 left to 1.0 right.
pub fn draw_graph_x_labels<D>(
    target: &mut D,
    viewport: GraphViewport,
    labels: &[(&str, f32)],
    color: Rgb565,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(color)
        .build();
    for (text, normalized) in labels.iter().copied() {
        let x = viewport.area.top_left.x
            + (normalized.clamp(0.0, 1.0) * viewport.area.size.width as f32) as i32;
        Text::new(text, Point::new(x, viewport.x_axis_y() + 10), text_style).draw(target)?;
    }
    Ok(())
}

/// Draws a filled pill shape inside `area`.
///
/// The helper clips naturally through the target. Very narrow areas fall back
/// to a filled rectangle.
pub fn draw_filled_pill<D>(target: &mut D, area: Rectangle, color: Rgb565) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    if area.size.width <= area.size.height {
        return area
            .into_styled(PrimitiveStyle::with_fill(color))
            .draw(target);
    }

    let radius = area.size.height / 2;
    let diameter = radius * 2;
    let style = PrimitiveStyle::with_fill(color);
    Circle::new(area.top_left, diameter)
        .into_styled(style)
        .draw(target)?;
    Circle::new(
        Point::new(
            area.top_left.x + area.size.width as i32 - diameter as i32,
            area.top_left.y,
        ),
        diameter,
    )
    .into_styled(style)
    .draw(target)?;
    Rectangle::new(
        Point::new(area.top_left.x + radius as i32, area.top_left.y),
        Size::new(area.size.width - diameter, area.size.height),
    )
    .into_styled(style)
    .draw(target)
}

/// Draws a filled rounded rectangle with radius derived from the area height.
///
/// This is equivalent to [`draw_filled_pill`] and is intended for compact
/// embedded cards, chips, and progress indicators.
pub fn draw_filled_rounded_rect<D>(
    target: &mut D,
    area: Rectangle,
    color: Rgb565,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    draw_filled_pill(target, area, color)
}

/// Draws an outlined circle.
pub fn draw_outlined_circle<D>(
    target: &mut D,
    top_left: Point,
    diameter: u32,
    color: Rgb565,
    stroke_width: u32,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    Circle::new(top_left, diameter)
        .into_styled(
            PrimitiveStyleBuilder::new()
                .stroke_color(color)
                .stroke_width(stroke_width)
                .build(),
        )
        .draw(target)
}

/// Draws a straight line with a fixed stroke width.
pub fn draw_line<D>(
    target: &mut D,
    start: Point,
    end: Point,
    color: Rgb565,
    stroke_width: u32,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    Line::new(start, end)
        .into_styled(
            PrimitiveStyleBuilder::new()
                .stroke_color(color)
                .stroke_width(stroke_width)
                .build(),
        )
        .draw(target)
}

/// Draws an approximated circular arc in degrees.
pub fn draw_arc<D>(
    target: &mut D,
    center: Point,
    radius: i32,
    start_degrees: i32,
    sweep_degrees: i32,
    color: Rgb565,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    if radius <= 0 || sweep_degrees == 0 {
        return Ok(());
    }
    let sweep = sweep_degrees.clamp(-360, 360);
    let step = if sweep > 0 { 4 } else { -4 };
    let mut angle = start_degrees;
    let end = start_degrees + sweep;
    let mut previous = point_on_circle(center, radius, angle);

    while angle != end {
        let next_angle = if step > 0 {
            (angle + step).min(end)
        } else {
            (angle + step).max(end)
        };
        let next = point_on_circle(center, radius, next_angle);
        draw_line(target, previous, next, color, 1)?;
        previous = next;
        angle = next_angle;
    }
    Ok(())
}

/// Draws a filled circular sector in degrees.
///
/// `start_degrees` is measured clockwise from the positive X axis and
/// `sweep_degrees` is clamped to `[-360, 360]`. The implementation is intended
/// for compact embedded gauges and indicators, not sub-pixel antialiasing.
pub fn draw_filled_sector<D>(
    target: &mut D,
    center: Point,
    radius: i32,
    start_degrees: i32,
    sweep_degrees: i32,
    color: Rgb565,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    if radius <= 0 || sweep_degrees == 0 {
        return Ok(());
    }

    let radius_squared = radius * radius;
    let sweep = sweep_degrees.clamp(-360, 360);
    let start = normalize_degrees(start_degrees);
    let end = normalize_degrees(start_degrees + sweep);

    for y in -radius..=radius {
        for x in -radius..=radius {
            if x * x + y * y > radius_squared {
                continue;
            }

            let angle = point_degrees(x, y);
            if angle_in_sweep(angle, start, end, sweep) {
                Pixel(center + Point::new(x, y), color).draw(target)?;
            }
        }
    }

    Ok(())
}

/// Draws wrapped text inside `area` and returns the number of lines drawn.
///
/// Wrapping uses the built-in 6x10 mono font metrics. Text is clipped at the
/// bottom of `area`; words longer than the available width are split.
pub fn draw_wrapped_text<D>(
    target: &mut D,
    area: Rectangle,
    text: &str,
    style: TextBlockStyle,
) -> Result<usize, D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    if let Some(background) = style.background {
        area.into_styled(PrimitiveStyle::with_fill(background))
            .draw(target)?;
    }

    let max_chars = (area.size.width / 6).max(1) as usize;
    let max_lines = (area.size.height / style.line_height).max(1) as usize;
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(style.color)
        .build();
    let mut lines_drawn = 0usize;
    let mut remaining = text.trim();

    while !remaining.is_empty() && lines_drawn < max_lines {
        let (line, rest) = split_line(remaining, max_chars);
        let baseline = area.top_left.y + 10 + (lines_drawn as i32 * style.line_height as i32);
        Text::new(line, Point::new(area.top_left.x, baseline), text_style).draw(target)?;
        lines_drawn += 1;
        remaining = rest.trim_start();
    }

    Ok(lines_drawn)
}

fn split_line(text: &str, max_chars: usize) -> (&str, &str) {
    if text.chars().count() <= max_chars {
        return (text, "");
    }

    let mut split_byte = 0;
    let mut last_space = None;
    for (char_count, (byte_index, ch)) in text.char_indices().enumerate() {
        if char_count == max_chars {
            break;
        }
        split_byte = byte_index + ch.len_utf8();
        if ch.is_whitespace() {
            last_space = Some(byte_index);
        }
    }

    let split_at = match last_space {
        Some(space) if space > 0 => space,
        _ => split_byte,
    };
    (&text[..split_at], &text[split_at..])
}

fn sort_graph_bands(bands: &mut [(i32, Rgb565, DitherPattern); 3]) {
    if bands[0].0 > bands[1].0 {
        bands.swap(0, 1);
    }
    if bands[1].0 > bands[2].0 {
        bands.swap(1, 2);
    }
    if bands[0].0 > bands[1].0 {
        bands.swap(0, 1);
    }
}

fn normalize_degrees(degrees: i32) -> i32 {
    let normalized = degrees % 360;
    if normalized < 0 {
        normalized + 360
    } else {
        normalized
    }
}

fn point_degrees(x: i32, y: i32) -> i32 {
    let radians = libm::atan2f(y as f32, x as f32);
    normalize_degrees((radians * 180.0 / core::f32::consts::PI) as i32)
}

fn point_on_circle(center: Point, radius: i32, degrees: i32) -> Point {
    let radians = degrees as f32 * core::f32::consts::PI / 180.0;
    Point::new(
        center.x + (libm::cosf(radians) * radius as f32) as i32,
        center.y + (libm::sinf(radians) * radius as f32) as i32,
    )
}

fn angle_in_sweep(angle: i32, start: i32, end: i32, sweep: i32) -> bool {
    if sweep >= 360 || sweep <= -360 {
        return true;
    }

    if sweep > 0 {
        if start <= end {
            angle >= start && angle <= end
        } else {
            angle >= start || angle <= end
        }
    } else if end <= start {
        angle <= start && angle >= end
    } else {
        angle <= start || angle >= end
    }
}

/// Easing curve for simple frame-based transitions.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Easing {
    /// Linear interpolation.
    Linear,
    /// Smooth ease-in/ease-out interpolation.
    SmoothStep,
}

/// Integer transition helper for simple embedded animations.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Transition {
    from: i32,
    to: i32,
    frames: u16,
    current: u16,
    easing: Easing,
}

impl Transition {
    /// Creates a new integer transition.
    #[must_use]
    pub const fn new(from: i32, to: i32, frames: u16, easing: Easing) -> Self {
        Self {
            from,
            to,
            frames,
            current: 0,
            easing,
        }
    }

    /// Advances the transition by one frame and returns the current value.
    #[must_use]
    pub fn step(&mut self) -> i32 {
        if self.current < self.frames {
            self.current += 1;
        }
        self.value()
    }

    /// Returns the current interpolated value.
    #[must_use]
    pub fn value(&self) -> i32 {
        if self.frames == 0 {
            return self.to;
        }
        let progress = self.current.min(self.frames) as f32 / self.frames as f32;
        let t = match self.easing {
            Easing::Linear => progress,
            Easing::SmoothStep => progress * progress * (3.0 - 2.0 * progress),
        };
        self.from + ((self.to - self.from) as f32 * t) as i32
    }

    /// Returns true when the transition has reached its final frame.
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.current >= self.frames
    }
}
