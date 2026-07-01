//! Motion and orientation context helpers built from accelerometer samples.

use defmt::Format;

use crate::imu::Acceleration;

/// Dominant gravity direction inferred from an accelerometer sample.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub enum GravityDirection {
    /// Positive X axis has the largest magnitude.
    PositiveX,
    /// Negative X axis has the largest magnitude.
    NegativeX,
    /// Positive Y axis has the largest magnitude.
    PositiveY,
    /// Negative Y axis has the largest magnitude.
    NegativeY,
    /// Positive Z axis has the largest magnitude.
    PositiveZ,
    /// Negative Z axis has the largest magnitude.
    NegativeZ,
}

/// Coarse board pose inferred from gravity.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub enum Pose {
    /// Display is facing up.
    FaceUp,
    /// Display is facing down.
    FaceDown,
    /// Board is portrait with the top edge upward.
    PortraitUp,
    /// Board is portrait with the top edge downward.
    PortraitDown,
    /// Board is landscape with the left edge upward.
    LandscapeLeft,
    /// Board is landscape with the right edge upward.
    LandscapeRight,
}

/// Coarse motion state inferred from acceleration deltas.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub enum MotionState {
    /// Not enough samples have been observed.
    Unknown,
    /// Acceleration is stable across samples.
    Still,
    /// Acceleration changed enough to indicate movement.
    Moving,
}

/// Coarse orientation family inferred from pose.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub enum OrientationKind {
    /// Display plane is approximately horizontal.
    Flat,
    /// Board is closer to portrait than landscape.
    Portrait,
    /// Board is closer to landscape than portrait.
    Landscape,
}

/// Combined motion context returned by [`MotionDetector`].
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub struct MotionContext {
    /// Current coarse motion state.
    pub state: MotionState,
    /// Current coarse board pose.
    pub pose: Pose,
    /// Dominant gravity direction used to derive the pose.
    pub gravity: GravityDirection,
}

/// Tuning values for motion classification.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub struct MotionConfig {
    /// Delta threshold in milli-g below which a sample is considered stable.
    pub still_threshold_mg: i32,
    /// Number of consecutive stable samples required before reporting still.
    pub still_samples: u8,
}

impl Default for MotionConfig {
    fn default() -> Self {
        Self {
            still_threshold_mg: 80,
            still_samples: 4,
        }
    }
}

/// Stateful motion classifier for BMI270 acceleration samples.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub struct MotionDetector {
    config: MotionConfig,
    previous: Option<Acceleration>,
    stable_count: u8,
}

impl MotionDetector {
    /// Creates a detector with default tuning.
    #[must_use]
    pub const fn new() -> Self {
        Self::with_config(MotionConfig {
            still_threshold_mg: 80,
            still_samples: 4,
        })
    }

    /// Creates a detector with explicit tuning.
    #[must_use]
    pub const fn with_config(config: MotionConfig) -> Self {
        Self {
            config,
            previous: None,
            stable_count: 0,
        }
    }

    /// Updates the detector with one acceleration sample.
    #[must_use]
    pub fn update(&mut self, acceleration: Acceleration) -> MotionContext {
        let gravity = dominant_gravity(acceleration);
        let pose = pose_from_gravity(gravity);
        let state = self.motion_state(acceleration);
        self.previous = Some(acceleration);

        MotionContext {
            state,
            pose,
            gravity,
        }
    }

    fn motion_state(&mut self, acceleration: Acceleration) -> MotionState {
        let Some(previous) = self.previous else {
            self.stable_count = 0;
            return MotionState::Unknown;
        };

        let delta = (i32::from(acceleration.x_mg) - i32::from(previous.x_mg)).abs()
            + (i32::from(acceleration.y_mg) - i32::from(previous.y_mg)).abs()
            + (i32::from(acceleration.z_mg) - i32::from(previous.z_mg)).abs();

        if delta <= self.config.still_threshold_mg {
            self.stable_count = self.stable_count.saturating_add(1);
            if self.stable_count >= self.config.still_samples {
                MotionState::Still
            } else {
                MotionState::Unknown
            }
        } else {
            self.stable_count = 0;
            MotionState::Moving
        }
    }
}

impl Default for MotionDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the dominant gravity direction for one acceleration sample.
#[must_use]
pub fn dominant_gravity(acceleration: Acceleration) -> GravityDirection {
    let x = i32::from(acceleration.x_mg);
    let y = i32::from(acceleration.y_mg);
    let z = i32::from(acceleration.z_mg);
    let abs_x = x.abs();
    let abs_y = y.abs();
    let abs_z = z.abs();

    if abs_x >= abs_y && abs_x >= abs_z {
        if x >= 0 {
            GravityDirection::PositiveX
        } else {
            GravityDirection::NegativeX
        }
    } else if abs_y >= abs_z {
        if y >= 0 {
            GravityDirection::PositiveY
        } else {
            GravityDirection::NegativeY
        }
    } else if z >= 0 {
        GravityDirection::PositiveZ
    } else {
        GravityDirection::NegativeZ
    }
}

/// Converts a dominant gravity direction to a coarse board pose.
#[must_use]
pub const fn pose_from_gravity(direction: GravityDirection) -> Pose {
    match direction {
        GravityDirection::PositiveZ => Pose::FaceUp,
        GravityDirection::NegativeZ => Pose::FaceDown,
        GravityDirection::PositiveY => Pose::PortraitUp,
        GravityDirection::NegativeY => Pose::PortraitDown,
        GravityDirection::PositiveX => Pose::LandscapeRight,
        GravityDirection::NegativeX => Pose::LandscapeLeft,
    }
}

/// Converts a pose into a coarse orientation family.
#[must_use]
pub const fn orientation_kind(pose: Pose) -> OrientationKind {
    match pose {
        Pose::FaceUp | Pose::FaceDown => OrientationKind::Flat,
        Pose::PortraitUp | Pose::PortraitDown => OrientationKind::Portrait,
        Pose::LandscapeLeft | Pose::LandscapeRight => OrientationKind::Landscape,
    }
}
