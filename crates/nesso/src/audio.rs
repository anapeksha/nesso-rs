use embedded_hal::{delay::DelayNs, digital::OutputPin};
use heapless::Deque;

/// Fixed number of queued tones retained by [`Buzzer`].
pub const TONE_QUEUE_CAPACITY: usize = 8;
/// Maximum tone duration recommended for responsive event-loop applications.
pub const MAX_RECOMMENDED_TONE_DURATION_MS: u32 = 250;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tone {
    pub frequency_hz: u32,
    pub duration_ms: u32,
}

impl Tone {
    /// Default tone frequency for the Nesso N1 passive buzzer.
    pub const DEFAULT_BUZZER_HZ: u32 = 4_000;

    /// Creates a tone with the given frequency and duration.
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
    queue: Deque<Tone, TONE_QUEUE_CAPACITY>,
    active: Option<ActiveTone>,
    active_level: bool,
}

impl<PIN> Buzzer<PIN> {
    /// Creates a buzzer driver from an output pin.
    #[must_use]
    pub const fn new(pin: PIN) -> Self {
        Self {
            pin,
            queue: Deque::new(),
            active: None,
            active_level: false,
        }
    }

    /// Releases the wrapped output pin.
    pub fn release(self) -> PIN {
        self.pin
    }

    /// Returns the fixed number of tones the non-blocking queue can hold.
    #[must_use]
    pub const fn queue_capacity(&self) -> usize {
        TONE_QUEUE_CAPACITY
    }
}

impl<PIN, E> Buzzer<PIN>
where
    PIN: OutputPin<Error = E>,
{
    /// Drives the buzzer pin high.
    pub fn on(&mut self) -> Result<(), E> {
        self.pin.set_high()
    }

    /// Drives the buzzer pin low.
    pub fn off(&mut self) -> Result<(), E> {
        self.active_level = false;
        self.pin.set_low()
    }

    /// Enqueues a tone for non-blocking playback.
    ///
    /// Call [`Self::poll`] regularly with a monotonic microsecond timestamp to
    /// advance playback. The queue is fixed-capacity and does not allocate.
    pub fn enqueue(&mut self, tone: Tone) -> Result<(), AudioError> {
        if tone.frequency_hz == 0 || tone.duration_ms == 0 {
            return Ok(());
        }
        self.queue
            .push_back(tone)
            .map_err(|_| AudioError::QueueFull)
    }

    /// Clears queued and active tones, then drives the buzzer low.
    pub fn stop(&mut self) -> Result<(), E> {
        self.queue.clear();
        self.active = None;
        self.off()
    }

    /// Returns true while a tone is active or queued.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.active.is_some() || !self.queue.is_empty()
    }

    /// Returns the number of queued tones, excluding the active tone.
    #[must_use]
    pub fn queued_tones(&self) -> usize {
        self.queue.len()
    }

    /// Advances non-blocking tone playback.
    ///
    /// `now_us` must be a monotonic timestamp in microseconds. The method
    /// toggles the buzzer only when a half-period boundary has elapsed, so
    /// callers can poll it from an event loop without blocking UI or input.
    pub fn poll(&mut self, now_us: u64) -> Result<(), E> {
        loop {
            if self.active.is_none() {
                if let Some(tone) = self.queue.pop_front() {
                    self.active = Some(ActiveTone::new(tone, now_us));
                    self.active_level = false;
                } else {
                    return self.off();
                }
            }

            let Some(active) = self.active else {
                return self.off();
            };

            if now_us.saturating_sub(active.started_at_us)
                < u64::from(active.tone.duration_ms) * 1_000
            {
                break;
            }

            self.active = None;
            self.active_level = false;
            self.pin.set_low()?;
        }

        let Some(active) = self.active else {
            return self.off();
        };

        if now_us.saturating_sub(active.last_toggle_us) >= active.half_period_us {
            self.active_level = !self.active_level;
            if self.active_level {
                self.pin.set_high()?;
            } else {
                self.pin.set_low()?;
            }
            if let Some(active) = self.active.as_mut() {
                active.last_toggle_us = now_us;
            }
        }

        Ok(())
    }

    /// Plays a square-wave tone using a blocking delay provider.
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
struct ActiveTone {
    tone: Tone,
    started_at_us: u64,
    last_toggle_us: u64,
    half_period_us: u64,
}

impl ActiveTone {
    fn new(tone: Tone, now_us: u64) -> Self {
        Self {
            tone,
            started_at_us: now_us,
            last_toggle_us: now_us,
            half_period_us: u64::from(500_000_u32 / tone.frequency_hz.max(1)),
        }
    }
}

/// Errors returned by non-blocking audio helpers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioError {
    /// The fixed-capacity tone queue is full.
    QueueFull,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioCapability {
    PassiveBuzzer,
}
