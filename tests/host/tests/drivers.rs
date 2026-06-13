use core::convert::Infallible;

use embedded_hal::{
    digital::{ErrorType as DigitalErrorType, OutputPin},
    i2c::{ErrorType as I2cErrorType, I2c, Operation},
};
use nesso_host_tests::{
    audio::{AudioError, Buzzer, Tone},
    power::{ChargeStatus, Power},
    touch::{Touch, TouchEvent, TouchPoint},
};

#[derive(Clone, Copy, Debug, Default)]
struct FakePin {
    high: bool,
    transitions: u16,
}

impl DigitalErrorType for FakePin {
    type Error = Infallible;
}

impl OutputPin for FakePin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        if self.high {
            self.transitions = self.transitions.saturating_add(1);
        }
        self.high = false;
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        if !self.high {
            self.transitions = self.transitions.saturating_add(1);
        }
        self.high = true;
        Ok(())
    }
}

#[test]
fn queued_buzzer_is_non_blocking_and_reports_capacity() -> Result<(), String> {
    let mut buzzer = Buzzer::new(FakePin::default());
    assert!(!buzzer.is_busy());

    buzzer
        .enqueue(Tone::new(1_000, 2))
        .map_err(|error| format!("{error:?}"))?;
    assert!(buzzer.is_busy());
    assert_eq!(buzzer.queued_tones(), 1);

    buzzer.poll(0).map_err(|error| format!("{error:?}"))?;
    buzzer.poll(500).map_err(|error| format!("{error:?}"))?;
    buzzer.poll(1_000).map_err(|error| format!("{error:?}"))?;
    buzzer.poll(2_000).map_err(|error| format!("{error:?}"))?;
    assert!(!buzzer.is_busy());

    for _ in 0..8 {
        buzzer
            .enqueue(Tone::new(2_000, 1))
            .map_err(|error| format!("{error:?}"))?;
    }
    assert_eq!(buzzer.enqueue(Tone::new(2_000, 1)), Err(AudioError::QueueFull));
    Ok(())
}

#[derive(Clone, Debug)]
struct FakeI2c {
    touch_count: u8,
    touch_data: [u8; 4],
    voltage_mv: u16,
    current_ma: i16,
    remaining_capacity: u16,
    full_capacity: u16,
    charger_status: u8,
}

impl Default for FakeI2c {
    fn default() -> Self {
        Self {
            touch_count: 0,
            touch_data: [0; 4],
            voltage_mv: 0,
            current_ma: 0,
            remaining_capacity: 0,
            full_capacity: 1,
            charger_status: 0,
        }
    }
}

impl I2cErrorType for FakeI2c {
    type Error = Infallible;
}

impl I2c for FakeI2c {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        let mut command = 0;
        for operation in operations {
            match operation {
                Operation::Write(bytes) => {
                    if let Some(first) = bytes.first() {
                        command = *first;
                    }
                }
                Operation::Read(bytes) => match (address, command, bytes.len()) {
                    (0x38, 0x02, 1) => bytes[0] = self.touch_count,
                    (0x38, 0x03, 4) => bytes.copy_from_slice(&self.touch_data),
                    (0x55, 0x08, 2) => bytes.copy_from_slice(&self.voltage_mv.to_le_bytes()),
                    (0x55, 0x0c, 2) => bytes.copy_from_slice(&self.current_ma.to_le_bytes()),
                    (0x55, 0x10, 2) => {
                        bytes.copy_from_slice(&self.remaining_capacity.to_le_bytes());
                    }
                    (0x55, 0x12, 2) => {
                        bytes.copy_from_slice(&self.full_capacity.to_le_bytes());
                    }
                    (0x49, 0x08, 1) => bytes[0] = self.charger_status,
                    _ => bytes.fill(0),
                },
            }
        }
        Ok(())
    }
}

#[test]
fn touch_driver_parses_coordinates_and_events() -> Result<(), String> {
    let i2c = FakeI2c {
        touch_count: 1,
        touch_data: [0x01, 0x23, 0x04, 0x56],
        ..FakeI2c::default()
    };
    let mut touch = Touch::new(i2c);

    let state = touch.read_state().map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        state.primary(),
        Some(TouchPoint {
            id: 0,
            x: 0x123,
            y: 0x456
        })
    );
    assert_eq!(
        touch.poll_event().map_err(|error| format!("{error:?}"))?,
        TouchEvent::Pressed(TouchPoint {
            id: 0,
            x: 0x123,
            y: 0x456
        })
    );
    Ok(())
}

#[test]
fn power_driver_reads_battery_and_charge_status() -> Result<(), String> {
    let i2c = FakeI2c {
        voltage_mv: 3_900,
        current_ma: -42,
        remaining_capacity: 400,
        full_capacity: 800,
        charger_status: 0b0001_0000,
        ..FakeI2c::default()
    };
    let mut power = Power::new(i2c);
    let status = power
        .battery_status()
        .map_err(|error| format!("{error:?}"))?;

    assert_eq!(status.voltage_mv, 3_900);
    assert_eq!(status.current_ma, -42);
    assert_eq!(status.percentage, 50);
    assert_eq!(status.charge, ChargeStatus::Charging);
    Ok(())
}
