#![no_std]
#![no_main]

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{Point, RgbColor, Size},
    primitives::Rectangle,
};
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use nesso::{
    Nesso,
    ui::{
        DitherPattern, GraphViewport, Insets, LabelStyle, ScreenLayout, TextBlockStyle,
        draw_black_dither_veil, draw_graph_fill_to_axis, draw_graph_series, draw_label,
        draw_progress_bar, draw_wrapped_text,
    },
};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut delay = Delay::new();
    let mut nesso = match Nesso::new(peripherals) {
        Ok(nesso) => nesso,
        Err(_) => abort(),
    };

    let layout = ScreenLayout::new(Size::new(135, 240));
    if nesso.display.clear(Rgb565::BLACK).is_err()
        || draw_label(
            &mut nesso.display,
            layout.row(44, 24, Insets::symmetric(8, 0)),
            "UI Test",
            LabelStyle::centered(Rgb565::CYAN),
        )
        .is_err()
        || draw_label(
            &mut nesso.display,
            layout.row(84, 24, Insets::symmetric(8, 0)),
            "Layout + text",
            LabelStyle::centered(Rgb565::WHITE),
        )
        .is_err()
        || draw_progress_bar(
            &mut nesso.display,
            layout.row(126, 12, Insets::symmetric(18, 0)),
            72,
            Rgb565::GREEN,
            Rgb565::new(2, 2, 2),
        )
        .is_err()
        || draw_black_dither_veil(
            &mut nesso.display,
            Rectangle::new(Point::new(18, 116), Size::new(99, 34)),
            DitherPattern::Checker50,
        )
        .is_err()
        || draw_graph_fill_to_axis(
            &mut nesso.display,
            GraphViewport::new(
                Rectangle::new(Point::new(18, 152), Size::new(99, 40)),
                0.0,
                1.0,
            ),
            &[0.2, 0.5, 0.35, 0.8, 0.65],
            Rgb565::new(0, 12, 6),
            DitherPattern::Vertical50,
        )
        .is_err()
        || draw_graph_series(
            &mut nesso.display,
            GraphViewport::new(
                Rectangle::new(Point::new(18, 152), Size::new(99, 40)),
                0.0,
                1.0,
            ),
            &[0.2, 0.5, 0.35, 0.8, 0.65],
            Rgb565::GREEN,
        )
        .is_err()
        || draw_wrapped_text(
            &mut nesso.display,
            layout.row(204, 28, Insets::symmetric(12, 0)),
            "Wrapped text stays inside its view.",
            TextBlockStyle::new(Rgb565::YELLOW),
        )
        .is_err()
    {
        abort()
    }

    loop {
        delay.delay_ms(1000);
    }
}

fn abort() -> ! {
    esp_hal::system::software_reset()
}
