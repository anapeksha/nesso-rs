#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonEvent {
    Pressed,
    Released,
    Held,
    Clicked,
    DoubleClicked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ButtonTiming {
    pub hold_ms: u32,
    pub double_click_ms: u32,
}

impl Default for ButtonTiming {
    fn default() -> Self {
        Self {
            hold_ms: 600,
            double_click_ms: 350,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Button {
    pressed: bool,
    pressed_at_ms: u32,
    last_release_ms: Option<u32>,
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
                self.held_reported = false;
                Some(ButtonEvent::Pressed)
            }
            (true, false) => {
                self.pressed = false;
                let event = if self
                    .last_release_ms
                    .is_some_and(|last| now_ms.saturating_sub(last) <= self.timing.double_click_ms)
                {
                    ButtonEvent::DoubleClicked
                } else {
                    ButtonEvent::Clicked
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
            _ => None,
        }
    }
}

impl Default for Button {
    fn default() -> Self {
        Self::new(ButtonTiming::default())
    }
}
