use core::convert::Infallible;
use std::{cell::RefCell, rc::Rc};

use embedded_graphics::{
    mock_display::MockDisplay,
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use embedded_hal::spi::{ErrorType, Operation, SpiDevice};
use nesso_host_tests::{
    display::{
        BusConfig, Display, DisplayGeometry, DisplayOrientation, NullOutputPin, PanelConfig,
    },
    sprite::{
        DirtyRectTracker, DirtyRegions, DoubleBufferedCanvas, MaskedSprite, Sprite, SpriteBuffer,
        SpriteError, clipped_region, union_region,
    },
    touch::{TouchPoint, TouchState},
    ui::{
        DitherPattern, GraphSeriesBuffer, GraphViewport, Insets, ScreenLayout, draw_arc,
        draw_black_dither_veil, draw_filled_sector, draw_graph_series, draw_line,
        draw_outlined_circle, draw_three_series_layered_fills,
    },
};

#[derive(Clone)]
struct RecordingSpi(RecordedWrites);

type RecordedWrites = Rc<RefCell<std::vec::Vec<std::vec::Vec<u8>>>>;
type RecordingDisplay = Display<RecordingSpi, NullOutputPin, NullOutputPin, NullOutputPin>;

impl ErrorType for RecordingSpi {
    type Error = Infallible;
}

impl SpiDevice for RecordingSpi {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        for operation in operations {
            if let Operation::Write(bytes) = operation {
                self.0.borrow_mut().push(bytes.to_vec());
            }
        }
        Ok(())
    }
}

fn recording_display() -> (RecordingDisplay, RecordedWrites) {
    let writes = Rc::new(RefCell::new(std::vec::Vec::new()));
    let display = Display::new(
        RecordingSpi(writes.clone()),
        NullOutputPin,
        NullOutputPin,
        NullOutputPin,
        BusConfig {
            write_hz: 40_000_000,
            use_dma: false,
        },
        PanelConfig {
            geometry: DisplayGeometry {
                width: 20,
                height: 20,
                offset_x: 0,
                offset_y: 0,
            },
            invert_colors: false,
        },
    );
    (display, writes)
}

#[test]
fn touch_coordinates_follow_display_orientation() {
    let geometry = DisplayGeometry {
        width: 135,
        height: 240,
        offset_x: 52,
        offset_y: 40,
    };
    let point = TouchPoint {
        id: 7,
        x: 10,
        y: 20,
    };

    assert_eq!(
        point.oriented(geometry, DisplayOrientation::Portrait),
        TouchPoint {
            id: 7,
            x: 10,
            y: 20
        }
    );
    assert_eq!(
        point.oriented(geometry, DisplayOrientation::PortraitInverted),
        TouchPoint {
            id: 7,
            x: 124,
            y: 219
        }
    );
    assert_eq!(
        point.oriented(geometry, DisplayOrientation::LandscapeClockwise),
        TouchPoint {
            id: 7,
            x: 219,
            y: 10
        }
    );
    assert_eq!(
        point.oriented(geometry, DisplayOrientation::LandscapeCounterClockwise),
        TouchPoint {
            id: 7,
            x: 20,
            y: 124
        }
    );
}

#[test]
fn touch_state_maps_all_points_without_allocating() {
    let geometry = DisplayGeometry {
        width: 135,
        height: 240,
        offset_x: 0,
        offset_y: 0,
    };
    let mut state = TouchState::default();
    assert!(state.points.push(TouchPoint { id: 0, x: 1, y: 2 }).is_ok());
    assert!(state.points.push(TouchPoint { id: 1, x: 3, y: 4 }).is_ok());

    let oriented = state.oriented(geometry, DisplayOrientation::PortraitInverted);
    assert_eq!(oriented.points[0].x, 133);
    assert_eq!(oriented.points[0].y, 237);
    assert_eq!(oriented.points[1].x, 131);
    assert_eq!(oriented.points[1].y, 235);
}

#[test]
fn dirty_regions_clip_and_coalesce() -> Result<(), String> {
    let bounds = Rectangle::new(Point::zero(), Size::new(100, 80));
    let mut dirty = DirtyRegions::<4>::new();

    dirty
        .mark(
            Rectangle::new(Point::new(10, 10), Size::new(10, 10)),
            bounds,
        )
        .map_err(|error| format!("{error:?}"))?;
    dirty
        .mark(
            Rectangle::new(Point::new(18, 12), Size::new(10, 10)),
            bounds,
        )
        .map_err(|error| format!("{error:?}"))?;
    dirty
        .mark(
            Rectangle::new(Point::new(90, 70), Size::new(20, 20)),
            bounds,
        )
        .map_err(|error| format!("{error:?}"))?;

    let regions: heapless::Vec<Rectangle, 4> = dirty.iter().copied().collect();
    assert_eq!(regions.len(), 2);
    assert_eq!(
        regions[0],
        Rectangle::new(Point::new(10, 10), Size::new(18, 12))
    );
    assert_eq!(
        regions[1],
        Rectangle::new(Point::new(90, 70), Size::new(10, 10))
    );
    Ok(())
}

#[test]
fn dirty_regions_report_capacity() -> Result<(), String> {
    let bounds = Rectangle::new(Point::zero(), Size::new(100, 100));
    let mut dirty = DirtyRegions::<1>::new();
    dirty
        .mark(Rectangle::new(Point::new(0, 0), Size::new(4, 4)), bounds)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        dirty.mark(Rectangle::new(Point::new(10, 10), Size::new(4, 4)), bounds),
        Err(SpriteError::DirtyRegionCapacity)
    );
    Ok(())
}

#[test]
fn sprite_region_iterator_is_row_major_and_clipped() -> Result<(), String> {
    let mut pixels = [
        Rgb565::RED,
        Rgb565::GREEN,
        Rgb565::BLUE,
        Rgb565::WHITE,
        Rgb565::BLACK,
        Rgb565::YELLOW,
    ];
    let sprite = Sprite::new(3, 2, &mut pixels).map_err(|error| format!("{error:?}"))?;
    let region = Rectangle::new(Point::new(1, 0), Size::new(4, 2));
    let colors: heapless::Vec<Rgb565, 6> = sprite.region_pixels(&region).collect();

    assert_eq!(
        colors.as_slice(),
        [Rgb565::GREEN, Rgb565::BLUE, Rgb565::BLACK, Rgb565::YELLOW]
    );
    Ok(())
}

#[test]
fn dirty_regions_flush_sprite_at_origin() -> Result<(), String> {
    let mut display = MockDisplay::<Rgb565>::new();
    let mut pixels = [Rgb565::BLACK; 4];
    let mut sprite = Sprite::new(2, 2, &mut pixels).map_err(|error| format!("{error:?}"))?;
    sprite
        .fill_solid(
            &Rectangle::new(Point::zero(), Size::new(2, 2)),
            Rgb565::GREEN,
        )
        .map_err(|error| format!("{error:?}"))?;
    let mut dirty = DirtyRegions::<2>::new();
    dirty
        .mark_clipped(
            Rectangle::new(Point::zero(), Size::new(2, 2)),
            sprite.bounds(),
        )
        .map_err(|error| format!("{error:?}"))?;
    dirty
        .flush_sprite_at(&sprite, &mut display, Point::new(3, 4))
        .map_err(|error| format!("{error:?}"))?;

    assert_eq!(
        clipped_region(
            Rectangle::new(Point::new(-1, -1), Size::new(4, 4)),
            sprite.bounds()
        ),
        sprite.bounds()
    );
    assert_eq!(
        union_region(
            Rectangle::new(Point::new(0, 0), Size::new(2, 2)),
            Rectangle::new(Point::new(2, 1), Size::new(2, 2))
        ),
        Rectangle::new(Point::new(0, 0), Size::new(4, 3))
    );
    Ok(())
}

#[test]
fn fixed_sprite_buffers_draw_without_allocation() {
    let mut sprite = SpriteBuffer::<4, 2>::new(Rgb565::BLACK);
    sprite
        .draw_iter([
            Pixel(Point::new(1, 0), Rgb565::RED),
            Pixel(Point::new(2, 1), Rgb565::GREEN),
        ])
        .expect("fixed sprite drawing should be infallible");

    assert_eq!(sprite.row(0).expect("row zero exists")[1], Rgb565::RED);
    assert_eq!(sprite.row(1).expect("row one exists")[2], Rgb565::GREEN);
    assert_eq!(sprite.bounds().size, Size::new(4, 2));
}

#[test]
fn masked_sprite_packs_bits_and_reports_opaque_runs() {
    let mut sprite = MaskedSprite::<10, 2>::new(Rgb565::BLACK);
    sprite
        .draw_iter([
            Pixel(Point::new(1, 0), Rgb565::RED),
            Pixel(Point::new(2, 0), Rgb565::GREEN),
            Pixel(Point::new(5, 0), Rgb565::BLUE),
            Pixel(Point::new(9, 0), Rgb565::WHITE),
        ])
        .expect("masked sprite drawing should be infallible");

    assert_eq!(sprite.mask_rows()[0][0], 0b0010_0110);
    assert_eq!(sprite.mask_rows()[0][1], 0b0000_0010);
    assert_eq!(sprite.next_opaque_run(0, 0), Some(1..3));
    assert_eq!(sprite.next_opaque_run(0, 3), Some(5..6));
    assert_eq!(sprite.next_opaque_run(0, 6), Some(9..10));
    assert_eq!(sprite.next_opaque_run(0, 10), None);

    sprite.set_opaque(Point::new(5, 0), false);
    assert!(!sprite.is_opaque(Point::new(5, 0)));
    assert_eq!(sprite.next_opaque_run(0, 3), Some(9..10));
}

#[test]
fn masked_sprite_uses_one_memory_window_per_opaque_run() {
    let (mut display, writes) = recording_display();
    let mut sprite = MaskedSprite::<6, 1>::new(Rgb565::BLACK);
    sprite
        .draw_iter([
            Pixel(Point::new(1, 0), Rgb565::RED),
            Pixel(Point::new(2, 0), Rgb565::GREEN),
            Pixel(Point::new(5, 0), Rgb565::BLUE),
        ])
        .expect("masked sprite drawing should be infallible");

    display
        .draw_masked_sprite(Point::new(3, 4), &sprite)
        .expect("recording display should be infallible");
    let writes = writes.borrow();
    assert_eq!(
        writes
            .iter()
            .filter(|write| write.as_slice() == [0x2c])
            .count(),
        2
    );
    assert!(writes.iter().any(|write| write.len() == 4));
    assert!(writes.iter().any(|write| write.len() == 2));
}

#[test]
fn display_draw_iter_bursts_mixed_color_horizontal_rows() {
    let (mut display, writes) = recording_display();
    display
        .draw_iter([
            Pixel(Point::new(1, 2), Rgb565::RED),
            Pixel(Point::new(2, 2), Rgb565::GREEN),
            Pixel(Point::new(3, 2), Rgb565::BLUE),
        ])
        .expect("recording display should be infallible");

    let writes = writes.borrow();
    assert_eq!(
        writes
            .iter()
            .filter(|write| write.as_slice() == [0x2c])
            .count(),
        1
    );
    assert!(writes.iter().any(|write| write.len() == 6));
}

#[test]
fn double_buffer_swap_returns_disjoint_frames() {
    let mut canvas = DoubleBufferedCanvas::<2, 1>::new(Rgb565::BLACK);
    canvas.drawing_buffer().rows_mut()[0][0] = Rgb565::RED;

    let (display, drawing) = canvas.swap();
    assert_eq!(display.row(0).expect("display row exists")[0], Rgb565::RED);
    drawing.rows_mut()[0][0] = Rgb565::GREEN;
    assert_eq!(display.row(0).expect("display row exists")[0], Rgb565::RED);
}

#[test]
fn dirty_tracker_merges_nearby_movement_and_updates_once() -> Result<(), String> {
    let mut tracker = DirtyRectTracker::<4>::new();
    tracker
        .register_movement(
            Rectangle::new(Point::new(0, 0), Size::new(4, 4)),
            Rectangle::new(Point::new(10, 0), Size::new(4, 4)),
        )
        .map_err(|error| format!("{error:?}"))?;

    assert_eq!(tracker.len(), 1);
    assert_eq!(
        tracker.iter().next().copied(),
        Some(Rectangle::new(Point::new(0, 0), Size::new(14, 4)))
    );

    let mut display = MockDisplay::<Rgb565>::new();
    display.set_allow_overdraw(true);
    let mut redraw_count = 0;
    tracker
        .update_screen(&mut display, Rgb565::BLACK, |target, region| {
            redraw_count += 1;
            target.fill_solid(&region, Rgb565::BLUE)
        })
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(redraw_count, 1);
    assert!(tracker.is_empty());
    Ok(())
}

#[test]
fn dither_patterns_cover_expected_points() {
    assert!(DitherPattern::Checker50.covers(Point::new(0, 0)));
    assert!(!DitherPattern::Checker50.covers(Point::new(1, 0)));
    assert!(DitherPattern::Vertical50.covers(Point::new(2, 3)));
    assert!(!DitherPattern::Vertical50.covers(Point::new(3, 3)));
}

#[test]
fn dither_and_graph_helpers_draw_into_mock_target() -> Result<(), String> {
    let mut display = MockDisplay::<Rgb565>::new();
    display.set_allow_overdraw(true);
    draw_black_dither_veil(
        &mut display,
        Rectangle::new(Point::zero(), Size::new(8, 8)),
        DitherPattern::Checker50,
    )
    .map_err(|error| format!("{error:?}"))?;

    let viewport = GraphViewport::new(Rectangle::new(Point::new(0, 0), Size::new(8, 8)), 0.0, 1.0);
    assert_eq!(viewport.x_for_index(1, 3), 3);
    assert_eq!(viewport.y_for_value(0.0), 7);
    assert_eq!(viewport.y_for_value(1.0), 0);
    draw_graph_series(&mut display, viewport, &[0.0, 0.5, 1.0], Rgb565::GREEN)
        .map_err(|error| format!("{error:?}"))?;
    draw_three_series_layered_fills(
        &mut display,
        viewport,
        [
            (&[0.8, 0.2], Rgb565::RED, DitherPattern::Vertical50),
            (&[0.5, 0.5], Rgb565::GREEN, DitherPattern::Checker50),
            (&[0.2, 0.8], Rgb565::BLUE, DitherPattern::Sparse25),
        ],
    )
    .map_err(|error| format!("{error:?}"))?;
    Ok(())
}

#[test]
fn graph_series_buffer_wraps_without_allocating() {
    let mut storage = [0.0; 3];
    let mut series = GraphSeriesBuffer::new(&mut storage);
    series.push(1.0);
    series.push(2.0);
    series.push(3.0);
    series.push(4.0);

    let values: heapless::Vec<f32, 3> = series.iter().collect();
    assert_eq!(values.as_slice(), [2.0, 3.0, 4.0]);
}

#[test]
fn ui_geometry_helpers_draw_into_mock_target() -> Result<(), String> {
    let mut display = MockDisplay::<Rgb565>::new();
    display.set_allow_overdraw(true);
    draw_line(
        &mut display,
        Point::new(0, 0),
        Point::new(5, 0),
        Rgb565::WHITE,
        1,
    )
    .map_err(|error| format!("{error:?}"))?;
    draw_outlined_circle(&mut display, Point::new(2, 2), 8, Rgb565::RED, 1)
        .map_err(|error| format!("{error:?}"))?;
    draw_arc(&mut display, Point::new(16, 16), 8, 0, 90, Rgb565::GREEN)
        .map_err(|error| format!("{error:?}"))?;
    draw_filled_sector(&mut display, Point::new(32, 16), 8, 0, 45, Rgb565::BLUE)
        .map_err(|error| format!("{error:?}"))?;

    Rectangle::new(Point::zero(), Size::new(1, 1))
        .into_styled(PrimitiveStyle::with_fill(Rgb565::WHITE))
        .draw(&mut display)
        .map_err(|error| format!("{error:?}"))?;
    Ok(())
}

#[test]
fn layout_insets_saturate() {
    let layout = ScreenLayout::new(Size::new(20, 10));
    assert_eq!(
        layout.content(Insets::symmetric(2, 1)),
        Rectangle::new(Point::new(2, 1), Size::new(16, 8))
    );
    assert_eq!(
        layout.content(Insets::symmetric(40, 40)).size,
        Size::new(0, 0)
    );
}
