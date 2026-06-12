# nesso-rs

`nesso-rs` is a Rust-native SDK for the Arduino Nesso N1, an ESP32-C6 based
device with display, touch, IMU, Wi-Fi, audio, and battery/power-management
hardware and optional external unit support for Nesso-compatible expansion
sensors such as M5Stack Unit ENV Pro.

The public crate is [`nesso`](https://crates.io/crates/nesso). The repository is
a Cargo workspace for examples and validation, but only the `nesso` crate is
published to crates.io.

## Status

This project is an early hardware-validated SDK foundation.

This SDK targets only the Arduino Nesso N1. It intentionally does not provide a
generic board abstraction layer, an M5Stack compatibility layer, or support for
other ESP32-C6 boards.

Validated examples currently cover:

- ST7789P3 display initialization and text rendering
- FT6336U touch reads
- BMI270 IMU live axis reads
- KEY1/KEY2 button events
- Passive buzzer tone output
- BQ27220/AW32001 battery and charger status reads
- Heapless settings storage
- ESP32-C6 Wi-Fi scan using `esp-radio`
- M5Stack Unit ENV Pro BME688 environmental reads over I2C/Qwiic
- Board information display

## Installation

Add the public facade crate:

```bash
cargo add nesso
```

Enable Wi-Fi only for applications that use the ESP32-C6 radio:

```toml
[dependencies]
nesso = { path = "crates/nesso", features = ["wifi"] }
esp-alloc = "0.10"
```

Enable ENV Pro support only for applications that use the external unit:

```toml
[dependencies]
nesso = { path = "crates/nesso", features = ["env"] }
```

## Public Modules

- `nesso::Nesso`: public facade and shared board ownership.
- `nesso::bsp`: Nesso N1 board constants, GPIOs, I2C addresses, and
  board-specific setup helpers.
- `nesso::display`: ST7789P3 display driver with `embedded-graphics`
  integration.
- `nesso::env`: external environmental unit support, gated behind the `env`
  feature.
- `nesso::touch`: FT6336U touch controller support.
- `nesso::input`: button event state machine helpers.
- `nesso::imu`: BMI270 initialization, config upload, and sensor reads.
- `nesso::audio`: passive buzzer output and blocking tone generation.
- `nesso::power`: BQ27220 fuel gauge and AW32001 charger status support.
- `nesso::wifi`: ESP32-C6 Wi-Fi scan support, gated behind the `wifi` feature.
- `nesso::storage`: heapless settings storage primitives and
  `esp-storage` flash-backed persistence.
- `nesso::sprite`: caller-owned RGB565 sprite/framebuffer support for
  flicker-free dirty-region rendering.

## Examples

Each public module has one focused hardware or module-validation example:

| Module | Example |
| --- | --- |
| `nesso::Nesso` | `hello_world` |
| `nesso::bsp` | `board_info` |
| `nesso::display` | `display_test` |
| `nesso::touch` | `touch_test` |
| `nesso::input` | `input_test` |
| `nesso::imu` | `imu_test` |
| `nesso::audio` | `audio_test` |
| `nesso::power` | `battery_test` |
| `nesso::wifi` | `wifi_scan` |
| `nesso::storage` | `storage_test` |
| `nesso::env` | `env_pro_test` |

## Example

The facade owns the fixed Nesso N1 wiring. Applications initialize ESP-HAL once,
then hand the peripherals to `Nesso::new`.

```rust,ignore
#![no_std]
#![no_main]

use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
use embedded_hal::delay::DelayNs;
use esp_hal::{clock::CpuClock, delay::Delay, main};
use nesso::Nesso;

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut delay = Delay::new();
    let mut nesso = match Nesso::new(peripherals) {
        Ok(nesso) => nesso,
        Err(_) => esp_hal::system::software_reset(),
    };

    if nesso.display.clear(Rgb565::BLACK).is_err()
        || nesso
            .display
            .print_centered("Hello from nesso-rs", 120, Rgb565::WHITE)
            .is_err()
    {
        esp_hal::system::software_reset()
    }

    loop {
        delay.delay_ms(1000);
    }
}
```

See `examples/` for hardware-focused examples.

Wi-Fi is behind the optional `wifi` feature and is initialized lazily with
`nesso.init_wifi()`. Only applications that enable Wi-Fi need to compile
`esp-radio`/`esp-rtos` and provide an `esp_alloc` heap for the ESP radio stack.

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

1. Merge through a pull request to `main`.
2. Create and push the release tag.
3. Publish a GitHub Release for that tag.

The release workflow validates the workspace and publishes only the public
`nesso` crate to crates.io.

The workflow uses crates.io trusted publishing and does not require a long-lived
Cargo registry token.

## License

Licensed under the MIT License. See the `LICENSE` file.
