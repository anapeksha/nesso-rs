//! Button and touch input state machines.
//!
//! These helpers are pure `no_std` logic. Feed them sampled board state and
//! they return semantic events.
//!
//! ```rust,ignore
//! let mut buttons = nesso.init_button_events()?;
//! let levels = nesso.button_levels()?;
//! if let Some(event) = buttons.update(levels.key1_pressed, levels.key2_pressed, now_ms) {
//!     match event {
//!         nesso::input::BoardButtonEvent::Key1(nesso::input::ButtonEvent::ShortPressed) => {}
//!         nesso::input::BoardButtonEvent::Key2(nesso::input::ButtonEvent::Held) => {}
//!         _ => {}
//!     }
//! }
//! ```

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Button {
    pressed: bool,
    pressed_at_ms: u32,
    last_release_ms: Option<u32>,
    last_repeat_ms: u32,
    held_reported: bool,
    timing: ButtonTiming,
}

/// Board-level button event for the Arduino Nesso N1 physical keys.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardButtonEvent {
    /// Event from physical KEY1.
    Key1(ButtonEvent),
    /// Event from physical KEY2.
    Key2(ButtonEvent),
}

/// Events produced by one board-button sampling tick.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BoardButtonEvents {
    /// Event from KEY1, if any.
    pub key1: Option<ButtonEvent>,
    /// Event from KEY2, if any.
    pub key2: Option<ButtonEvent>,
}

impl BoardButtonEvents {
    /// Returns true when neither key produced an event.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.key1.is_none() && self.key2.is_none()
    }

    /// Returns an iterator over events in KEY1 then KEY2 order.
    pub const fn iter(self) -> BoardButtonEventsIter {
        BoardButtonEventsIter {
            events: self,
            index: 0,
        }
    }
}

impl IntoIterator for BoardButtonEvents {
    type IntoIter = BoardButtonEventsIter;
    type Item = BoardButtonEvent;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over board button events from one sampling tick.
#[derive(Clone, Copy, Debug)]
pub struct BoardButtonEventsIter {
    events: BoardButtonEvents,
    index: u8,
}

impl Iterator for BoardButtonEventsIter {
    type Item = BoardButtonEvent;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < 2 {
            let event = match self.index {
                0 => self.events.key1.map(BoardButtonEvent::Key1),
                _ => self.events.key2.map(BoardButtonEvent::Key2),
            };
            self.index += 1;
            if event.is_some() {
                return event;
            }
        }
        None
    }
}

/// Two-button state machine for Nesso N1 KEY1/KEY2.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardButtons {
    key1: Button,
    key2: Button,
}

impl BoardButtons {
    /// Creates a board-button helper with shared timing thresholds.
    #[must_use]
    pub const fn new(timing: ButtonTiming) -> Self {
        Self {
            key1: Button::new(timing),
            key2: Button::new(timing),
        }
    }

    /// Updates both keys and returns the first event observed.
    pub fn update(
        &mut self,
        key1_pressed: bool,
        key2_pressed: bool,
        now_ms: u32,
    ) -> Option<BoardButtonEvent> {
        self.update_all(key1_pressed, key2_pressed, now_ms)
            .into_iter()
            .next()
    }

    /// Updates both keys and returns up to one event per key.
    #[must_use]
    pub fn update_all(
        &mut self,
        key1_pressed: bool,
        key2_pressed: bool,
        now_ms: u32,
    ) -> BoardButtonEvents {
        BoardButtonEvents {
            key1: self.key1.update(key1_pressed, now_ms),
            key2: self.key2.update(key2_pressed, now_ms),
        }
    }
}

impl Default for BoardButtons {
    fn default() -> Self {
        Self::new(ButtonTiming::default())
    }
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
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
