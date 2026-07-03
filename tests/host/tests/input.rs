use nesso_host_tests::input::{
    BoardButtonEvent, BoardButtons, Button, ButtonEvent, ButtonTiming, Gesture, TouchGesture,
    TouchTiming,
};

#[test]
fn button_reports_short_press() {
    let mut button = Button::default();

    assert_eq!(button.update(true, 100), Some(ButtonEvent::Pressed));
    assert_eq!(button.update(false, 180), Some(ButtonEvent::ShortPressed));
}

#[test]
fn button_reports_double_click() {
    let mut button = Button::default();

    assert_eq!(button.update(true, 0), Some(ButtonEvent::Pressed));
    assert_eq!(button.update(false, 50), Some(ButtonEvent::ShortPressed));
    assert_eq!(button.update(true, 200), Some(ButtonEvent::Pressed));
    assert_eq!(button.update(false, 240), Some(ButtonEvent::DoubleClicked));
}

#[test]
fn button_reports_hold_long_press_and_repeat() {
    let mut button = Button::new(ButtonTiming {
        debounce_ms: 25,
        hold_ms: 100,
        repeat_ms: 50,
        double_click_ms: 350,
    });

    assert_eq!(button.update(true, 0), Some(ButtonEvent::Pressed));
    assert_eq!(button.update(true, 99), None);
    assert_eq!(button.update(true, 100), Some(ButtonEvent::Held));
    assert_eq!(button.update(true, 149), Some(ButtonEvent::Repeat));
    assert_eq!(button.update(false, 180), Some(ButtonEvent::LongPressed));
}

#[test]
fn board_buttons_report_key_events() {
    let mut buttons = BoardButtons::default();

    assert_eq!(
        buttons.update(true, false, 10),
        Some(BoardButtonEvent::Key1(ButtonEvent::Pressed))
    );
    assert_eq!(
        buttons.update(false, false, 80),
        Some(BoardButtonEvent::Key1(ButtonEvent::ShortPressed))
    );
    assert_eq!(
        buttons.update(false, true, 100),
        Some(BoardButtonEvent::Key2(ButtonEvent::Pressed))
    );
}

#[test]
fn board_buttons_collect_simultaneous_events() {
    let mut buttons = BoardButtons::default();

    let events = buttons.update_all(true, true, 10);
    assert_eq!(events.key1, Some(ButtonEvent::Pressed));
    assert_eq!(events.key2, Some(ButtonEvent::Pressed));
    assert_eq!(
        events.into_iter().collect::<heapless::Vec<_, 2>>().as_slice(),
        [
            BoardButtonEvent::Key1(ButtonEvent::Pressed),
            BoardButtonEvent::Key2(ButtonEvent::Pressed)
        ]
    );
}

#[test]
fn touch_gesture_reports_tap_drag_swipe() {
    let mut gesture = TouchGesture::new(TouchTiming {
        tap_slop_px: 4,
        swipe_threshold_px: 20,
    });

    assert_eq!(gesture.update(Some((10, 10))), Some(Gesture::Down));
    assert_eq!(gesture.update(Some((12, 12))), Some(Gesture::Drag));
    assert_eq!(gesture.update(None), Some(Gesture::Tap));

    assert_eq!(gesture.update(Some((10, 10))), Some(Gesture::Down));
    assert_eq!(gesture.update(Some((24, 10))), Some(Gesture::Drag));
    assert_eq!(gesture.update(None), Some(Gesture::Up));

    assert_eq!(gesture.update(Some((10, 10))), Some(Gesture::Down));
    assert_eq!(gesture.update(Some((40, 10))), Some(Gesture::Drag));
    assert_eq!(gesture.update(None), Some(Gesture::Swipe));
}
