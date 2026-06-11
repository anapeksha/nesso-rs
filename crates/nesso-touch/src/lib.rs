#![no_std]

use embedded_hal::i2c::I2c;
use heapless::Vec;

pub const FT6336U_ADDRESS: u8 = 0x38;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TouchPoint {
    pub id: u8,
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TouchState {
    pub points: Vec<TouchPoint, 2>,
}

impl TouchState {
    #[must_use]
    pub fn is_pressed(&self) -> bool {
        !self.points.is_empty()
    }

    #[must_use]
    pub fn primary(&self) -> Option<TouchPoint> {
        self.points.first().copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    #[must_use]
    pub const fn new(i2c: I2C) -> Self {
        Self {
            i2c,
            address: FT6336U_ADDRESS,
            previous: TouchState { points: Vec::new() },
        }
    }

    pub fn release(self) -> I2C {
        self.i2c
    }
}

impl<I2C, E> Touch<I2C>
where
    I2C: I2c<Error = E>,
{
    pub fn read_state(&mut self) -> Result<TouchState, E> {
        let mut count = [0u8; 1];
        self.i2c.write_read(self.address, &[0x02], &mut count)?;
        let touches = count[0] & 0x0f;
        let mut state = TouchState::default();
        if touches > 0 {
            let mut data = [0u8; 4];
            self.i2c.write_read(self.address, &[0x03], &mut data)?;
            let x = (u16::from(data[0] & 0x0f) << 8) | u16::from(data[1]);
            let y = (u16::from(data[2] & 0x0f) << 8) | u16::from(data[3]);
            let _ = state.points.push(TouchPoint { id: 0, x, y });
        }
        Ok(state)
    }

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
