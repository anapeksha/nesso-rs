use nesso_host_tests::{
    imu::Acceleration,
    motion::{
        GravityDirection, MotionConfig, MotionDetector, MotionState, OrientationKind, Pose,
        dominant_gravity, orientation_kind, pose_from_gravity,
    },
    power::{BatteryStatus, ChargeStatus, PowerState},
};

#[test]
fn motion_maps_dominant_gravity_to_pose() {
    assert_eq!(
        dominant_gravity(Acceleration {
            x_mg: 980,
            y_mg: 10,
            z_mg: 20
        }),
        GravityDirection::PositiveX
    );
    assert_eq!(
        pose_from_gravity(GravityDirection::NegativeZ),
        Pose::FaceDown
    );
    assert_eq!(
        orientation_kind(Pose::LandscapeLeft),
        OrientationKind::Landscape
    );
    assert_eq!(orientation_kind(Pose::PortraitUp), OrientationKind::Portrait);
    assert_eq!(orientation_kind(Pose::FaceUp), OrientationKind::Flat);
}

#[test]
fn motion_detector_reports_still_after_stable_samples() {
    let mut detector = MotionDetector::with_config(MotionConfig {
        still_threshold_mg: 20,
        still_samples: 2,
    });
    let sample = Acceleration {
        x_mg: 0,
        y_mg: 0,
        z_mg: 1000,
    };

    assert_eq!(detector.update(sample).state, MotionState::Unknown);
    assert_eq!(detector.update(sample).state, MotionState::Unknown);
    assert_eq!(detector.update(sample).state, MotionState::Still);
    assert_eq!(
        detector
            .update(Acceleration {
                x_mg: 200,
                y_mg: 0,
                z_mg: 900
            })
            .state,
        MotionState::Moving
    );
}

#[test]
fn battery_status_helpers_are_threshold_based() {
    let charging = BatteryStatus {
        voltage_mv: 4100,
        current_ma: 120,
        percentage: 15,
        charge: ChargeStatus::Charging,
    };
    assert!(charging.is_low(20));
    assert!(charging.has_external_power());
    assert_eq!(charging.power_state(), PowerState::Usb);

    let discharging = BatteryStatus {
        voltage_mv: 3700,
        current_ma: -80,
        percentage: 80,
        charge: ChargeStatus::Discharging,
    };
    assert!(!discharging.is_low(20));
    assert!(!discharging.has_external_power());
    assert_eq!(discharging.power_state(), PowerState::Battery);
}
