# nesso-rs

`nesso-rs` is a Rust-native SDK for the Arduino Nesso N1, an ESP32-C6 based
device with display, touch, IMU, Wi-Fi, audio, and battery/power-management
hardware.

The public crate is [`nesso`](https://crates.io/crates/nesso). The repository is
a Cargo workspace so lower-level hardware crates can also be used directly when
an application needs finer control.

## Status

This project is an early hardware-validated SDK foundation.

Version `0.0.1` targets only the Arduino Nesso N1. It intentionally does not
provide a generic board abstraction layer, an M5Stack compatibility layer, or
support for other ESP32-C6 boards.

Validated examples currently cover:

- ST7789P3 display initialization and text rendering
- FT6336U touch reads
- BMI270 IMU live axis reads
- Passive buzzer tone output
- BQ27220/AW32001 battery and charger status reads
- ESP32-C6 Wi-Fi scan using `esp-radio`
- Board information display

## Installation

Add the public facade crate:

```toml
[dependencies]
nesso = "0.0.1"
```

Advanced users may depend on subsystem crates directly:

```toml
[dependencies]
nesso-display = "0.0.1"
nesso-imu = "0.0.1"
nesso-wifi = "0.0.1"
```

## Workspace Crates

- `nesso`: public facade crate and re-exports for the SDK.
- `nesso-n1`: Nesso N1 board constants, GPIOs, I2C addresses, and
  board-specific setup helpers.
- `nesso-display`: ST7789P3 display driver with `embedded-graphics`
  integration.
- `nesso-touch`: FT6336U touch controller support.
- `nesso-input`: button event state machine helpers.
- `nesso-imu`: BMI270 initialization, config upload, and sensor reads.
- `nesso-audio`: passive buzzer output and blocking tone generation.
- `nesso-power`: BQ27220 fuel gauge and AW32001 charger status support.
- `nesso-wifi`: ESP32-C6 Wi-Fi scan support using `esp-radio` and `esp-rtos`.
- `nesso-storage`: heapless settings storage primitives.

## Example

The BSP owns the fixed Nesso N1 wiring. Applications initialize the ESP-HAL
peripherals once, then ask the BSP for ready-to-use board resources.

```rust,ignore
#![no_std]
#![no_main]

use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
use embedded_hal::delay::DelayNs;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use nesso::bsp::NessoN1Board;

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut delay = Delay::new();
    let mut display = NessoN1Board::new(peripherals).into_display().unwrap();

    display.clear(Rgb565::BLACK).unwrap();
    display
        .print_centered("Hello from nesso-rs", 120, Rgb565::WHITE)
        .unwrap();

    loop {
        delay.delay_ms(1000);
    }
}
```

See `examples/` for hardware-focused examples.

## Build

Install Rust with the target specified in `rust-toolchain.toml`, then run:

```bash
cargo metadata --no-deps --format-version 1
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo build --workspace
cargo build --workspace --release
```

## Flashing Examples

Build and flash an example with `espflash`:

```bash
cargo build -p hello_world --release
espflash flash --chip esp32c6 -p /dev/cu.usbmodem1101 \
  target/riscv32imac-unknown-none-elf/release/hello_world
```

Change the serial port for your host.

## Documentation

Hardware and architecture notes are kept in `docs/`:

- `docs/hardware.md`
- `docs/architecture.md`
- `docs/m5gfx-analysis.md`
- `docs/gap-analysis.md`
- `docs/roadmap.md`

## Release Process

Releases are published from GitHub Releases.

1. Update the workspace version in `Cargo.toml`.
2. Merge through a pull request to `main`.
3. Create and push a tag matching the version, for example `v0.0.1`.
4. Publish a GitHub Release for that tag.

The release workflow validates the workspace, publishes internal crates first,
then publishes the public `nesso` crate to crates.io.

The workflow requires a repository secret named `CARGO_REGISTRY_TOKEN`.

## License

Licensed under the MIT License. See the `LICENSE` file.
