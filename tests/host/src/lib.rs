#[path = "../../../crates/nesso/src/audio.rs"]
pub mod audio;

#[path = "../../../crates/nesso/src/input.rs"]
pub mod input;

#[path = "../../../crates/nesso/src/display.rs"]
pub mod display;

#[path = "../../../crates/nesso/src/touch.rs"]
pub mod touch;

#[path = "../../../crates/nesso/src/sprite.rs"]
pub mod sprite;

#[path = "../../../crates/nesso/src/ui.rs"]
pub mod ui;

pub mod imu {
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct Acceleration {
        pub x_mg: i16,
        pub y_mg: i16,
        pub z_mg: i16,
    }
}

#[path = "../../../crates/nesso/src/motion.rs"]
pub mod motion;

#[path = "../../../crates/nesso/src/power.rs"]
pub mod power;

#[path = "../../../crates/nesso/src/ble.rs"]
pub mod ble;

#[path = "../../../crates/nesso/src/wifi.rs"]
pub mod wifi;

#[path = "../../../crates/nesso/src/storage.rs"]
pub mod storage;
