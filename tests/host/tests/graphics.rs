use embedded_graphics::{
    mock_display::MockDisplay,
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use nesso_host_tests::{
    display::{DisplayGeometry, DisplayOrientation},
    sprite::{DirtyRegions, Sprite, SpriteError, clipped_region, union_region},
    touch::{TouchPoint, TouchState},
    ui::{
        DitherPattern, GraphSeriesBuffer, GraphViewport, Insets, ScreenLayout,
        draw_black_dither_veil, draw_arc, draw_filled_sector, draw_graph_series,
        draw_line, draw_outlined_circle, draw_three_series_layered_fills,
    },
};

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
        .mark(Rectangle::new(Point::new(10, 10), Size::new(10, 10)), bounds)
        .map_err(|error| format!("{error:?}"))?;
    dirty
        .mark(Rectangle::new(Point::new(18, 12), Size::new(10, 10)), bounds)
        .map_err(|error| format!("{error:?}"))?;
    dirty
        .mark(Rectangle::new(Point::new(90, 70), Size::new(20, 20)), bounds)
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
