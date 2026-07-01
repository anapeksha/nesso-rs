use defmt::Format;
use embedded_hal::i2c::I2c;
use heapless::Vec;

use crate::display::{DisplayGeometry, DisplayOrientation};

pub const FT6336U_ADDRESS: u8 = 0x38;
// FT6336U register map: TD_STATUS reports active touches, P1_XH starts point 1.
const REG_TD_STATUS: u8 = 0x02;
const REG_P1_XH: u8 = 0x03;
const TOUCH_COUNT_MASK: u8 = 0x0f;
const COORDINATE_HIGH_MASK: u8 = 0x0f;

#[derive(Clone, Copy, Debug, Format, Default, Eq, PartialEq)]
pub struct TouchPoint {
    pub id: u8,
    pub x: u16,
    pub y: u16,
}

impl TouchPoint {
    /// Maps this raw touch point into the display's logical orientation.
    #[must_use]
    pub fn oriented(self, geometry: DisplayGeometry, orientation: DisplayOrientation) -> Self {
        let native_width = geometry.width.saturating_sub(1);
        let native_height = geometry.height.saturating_sub(1);
        let (x, y) = match orientation {
            DisplayOrientation::Portrait => (self.x, self.y),
            DisplayOrientation::PortraitInverted => (
                native_width.saturating_sub(self.x),
                native_height.saturating_sub(self.y),
            ),
            DisplayOrientation::LandscapeClockwise => {
                (native_height.saturating_sub(self.y), self.x)
            }
            DisplayOrientation::LandscapeCounterClockwise => {
                (self.y, native_width.saturating_sub(self.x))
            }
        };
        Self { id: self.id, x, y }
    }
}

#[derive(Clone, Debug, Format, Default, Eq, PartialEq)]
pub struct TouchState {
    pub points: Vec<TouchPoint, 2>,
}

impl TouchState {
    /// Returns true when at least one touch point is active.
    #[must_use]
    pub fn is_pressed(&self) -> bool {
        !self.points.is_empty()
    }

    #[must_use]
    /// Returns the first active touch point.
    pub fn primary(&self) -> Option<TouchPoint> {
        self.points.first().copied()
    }

    /// Returns a copy of this state mapped into the display's logical orientation.
    #[must_use]
    pub fn oriented(&self, geometry: DisplayGeometry, orientation: DisplayOrientation) -> Self {
        let mut points = Vec::new();
        for point in self.points.iter().copied() {
            let _ = points.push(point.oriented(geometry, orientation));
        }
        Self { points }
    }
}

#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub enum TouchEvent {
    Pressed(TouchPoint),
    Released,
    Moved(TouchPoint),
    Idle,
}

pub struct Touch<I2C> {
    i2c: I2C,
    address: u8,
    previous: TouchState,
}

impl<I2C> Touch<I2C> {
    /// Creates an FT6336U touch driver on the default Nesso N1 I2C address.
    #[must_use]
    pub const fn new(i2c: I2C) -> Self {
        Self {
            i2c,
            address: FT6336U_ADDRESS,
            previous: TouchState { points: Vec::new() },
        }
    }

    /// Releases the wrapped I2C bus.
    pub fn release(self) -> I2C {
        self.i2c
    }
}

impl<I2C, E> Touch<I2C>
where
    I2C: I2c<Error = E>,
{
    /// Reads the current touch points from the controller.
    pub fn read_state(&mut self) -> Result<TouchState, E> {
        let mut count = [0u8; 1];
        self.i2c
            .write_read(self.address, &[REG_TD_STATUS], &mut count)?;
        let touches = count[0] & TOUCH_COUNT_MASK;
        let mut state = TouchState::default();
        if touches > 0 {
            let mut point_bytes = [0u8; 4];
            self.i2c
                .write_read(self.address, &[REG_P1_XH], &mut point_bytes)?;
            let x =
                (u16::from(point_bytes[0] & COORDINATE_HIGH_MASK) << 8) | u16::from(point_bytes[1]);
            let y =
                (u16::from(point_bytes[2] & COORDINATE_HIGH_MASK) << 8) | u16::from(point_bytes[3]);
            let _ = state.points.push(TouchPoint { id: 0, x, y });
        }
        Ok(state)
    }

    /// Polls the controller and returns the state transition since the last poll.
    pub fn poll_event(&mut self) -> Result<TouchEvent, E> {
        let current = self.read_state()?;
        let event = match (self.previous.primary(), current.primary()) {
            (None, Some(point)) => TouchEvent::Pressed(point),
            (Some(_), None) => TouchEvent::Released,
            (Some(previous), Some(point)) if previous != point => TouchEvent::Moved(point),
            _ => TouchEvent::Idle,
        };
        self.previous = current;
        Ok(event)
    }
}
