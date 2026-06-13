#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonEvent {
    Pressed,
    Released,
    Held,
    Clicked,
    DoubleClicked,
    ShortPressed,
    LongPressed,
    Repeat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ButtonTiming {
    pub debounce_ms: u32,
    pub hold_ms: u32,
    pub repeat_ms: u32,
    pub double_click_ms: u32,
}

impl Default for ButtonTiming {
    fn default() -> Self {
        Self {
            debounce_ms: 25,
            hold_ms: 600,
            repeat_ms: 250,
            double_click_ms: 350,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Button {
    pressed: bool,
    pressed_at_ms: u32,
    last_release_ms: Option<u32>,
    last_repeat_ms: u32,
    held_reported: bool,
    timing: ButtonTiming,
}

impl Button {
    /// Creates a button state machine with custom timing thresholds.
    #[must_use]
    pub const fn new(timing: ButtonTiming) -> Self {
        Self {
            pressed: false,
            pressed_at_ms: 0,
            last_release_ms: None,
            last_repeat_ms: 0,
            held_reported: false,
            timing,
        }
    }

    /// Updates the button state and returns an event when one is produced.
    pub fn update(&mut self, is_pressed: bool, now_ms: u32) -> Option<ButtonEvent> {
        match (self.pressed, is_pressed) {
            (false, true) => {
                self.pressed = true;
                self.pressed_at_ms = now_ms;
                self.last_repeat_ms = now_ms;
                self.held_reported = false;
                Some(ButtonEvent::Pressed)
            }
            (true, false) => {
                self.pressed = false;
                let held_ms = now_ms.saturating_sub(self.pressed_at_ms);
                let event = if self.held_reported || held_ms >= self.timing.hold_ms {
                    ButtonEvent::LongPressed
                } else if self
                    .last_release_ms
                    .is_some_and(|last| now_ms.saturating_sub(last) <= self.timing.double_click_ms)
                {
                    ButtonEvent::DoubleClicked
                } else {
                    ButtonEvent::ShortPressed
                };
                self.last_release_ms = Some(now_ms);
                Some(event)
            }
            (true, true)
                if !self.held_reported
                    && now_ms.saturating_sub(self.pressed_at_ms) >= self.timing.hold_ms =>
            {
                self.held_reported = true;
                Some(ButtonEvent::Held)
            }
            (true, true)
                if self.held_reported
                    && self.timing.repeat_ms > 0
                    && now_ms.saturating_sub(self.last_repeat_ms) >= self.timing.repeat_ms =>
            {
                self.last_repeat_ms = now_ms;
                Some(ButtonEvent::Repeat)
            }
            _ => None,
        }
    }
}

/// Generic touch gesture emitted by a touch event tracker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gesture {
    /// Touch went down.
    Down,
    /// Touch went up without enough movement to be a drag.
    Tap,
    /// Touch moved while pressed.
    Drag,
    /// Touch released after a larger horizontal or vertical movement.
    Swipe,
    /// Touch went up after a drag below the swipe threshold.
    Up,
}

/// Configurable touch gesture thresholds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TouchTiming {
    /// Maximum movement in pixels still considered a tap.
    pub tap_slop_px: u16,
    /// Movement in pixels required before a release becomes a swipe.
    pub swipe_threshold_px: u16,
}

impl Default for TouchTiming {
    fn default() -> Self {
        Self {
            tap_slop_px: 8,
            swipe_threshold_px: 36,
        }
    }
}

/// Stateful generic touch gesture helper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TouchGesture {
    timing: TouchTiming,
    start: Option<(u16, u16)>,
    previous: Option<(u16, u16)>,
}

impl TouchGesture {
    /// Creates a gesture helper with explicit thresholds.
    #[must_use]
    pub const fn new(timing: TouchTiming) -> Self {
        Self {
            timing,
            start: None,
            previous: None,
        }
    }

    /// Updates the helper with the current primary touch point.
    pub fn update(&mut self, point: Option<(u16, u16)>) -> Option<Gesture> {
        match (self.previous, point) {
            (None, Some(current)) => {
                self.start = Some(current);
                self.previous = Some(current);
                Some(Gesture::Down)
            }
            (Some(_), Some(current)) => {
                self.previous = Some(current);
                Some(Gesture::Drag)
            }
            (Some(previous), None) => {
                let start = match self.start {
                    Some(start) => start,
                    None => previous,
                };
                self.start = None;
                self.previous = None;
                let dx = previous.0.abs_diff(start.0);
                let dy = previous.1.abs_diff(start.1);
                if dx.max(dy) <= self.timing.tap_slop_px {
                    Some(Gesture::Tap)
                } else if dx.max(dy) >= self.timing.swipe_threshold_px {
                    Some(Gesture::Swipe)
                } else {
                    Some(Gesture::Up)
                }
            }
            (None, None) => None,
        }
    }
}

impl Default for TouchGesture {
    fn default() -> Self {
        Self::new(TouchTiming::default())
    }
}

impl Default for Button {
    fn default() -> Self {
        Self::new(ButtonTiming::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
