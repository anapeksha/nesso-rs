#![no_std]

use embedded_hal::{delay::DelayNs, digital::OutputPin};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tone {
    pub frequency_hz: u32,
    pub duration_ms: u32,
}

impl Tone {
    pub const DEFAULT_BUZZER_HZ: u32 = 4_000;

    #[must_use]
    pub const fn new(frequency_hz: u32, duration_ms: u32) -> Self {
        Self {
            frequency_hz,
            duration_ms,
        }
    }
}

pub struct Buzzer<PIN> {
    pin: PIN,
}

impl<PIN> Buzzer<PIN> {
    #[must_use]
    pub const fn new(pin: PIN) -> Self {
        Self { pin }
    }

    pub fn release(self) -> PIN {
        self.pin
    }
}

impl<PIN, E> Buzzer<PIN>
where
    PIN: OutputPin<Error = E>,
{
    pub fn on(&mut self) -> Result<(), E> {
        self.pin.set_high()
    }

    pub fn off(&mut self) -> Result<(), E> {
        self.pin.set_low()
    }

    pub fn play_blocking<Delay>(&mut self, tone: Tone, delay: &mut Delay) -> Result<(), E>
    where
        Delay: DelayNs,
    {
        if tone.frequency_hz == 0 || tone.duration_ms == 0 {
            return self.off();
        }

        let half_period_us = 500_000_u32 / tone.frequency_hz;
        let cycles = tone.duration_ms.saturating_mul(tone.frequency_hz) / 1_000;
        for _ in 0..cycles {
            self.on()?;
            delay.delay_us(half_period_us);
            self.off()?;
            delay.delay_us(half_period_us);
        }
        self.off()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioCapability {
    PassiveBuzzer,
}
