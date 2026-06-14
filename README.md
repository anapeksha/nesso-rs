# nesso

`nesso` is a Rust-native SDK for the Arduino Nesso N1, an ESP32-C6 based
device with display, touch, IMU, Wi-Fi, BLE, LoRa, audio, and battery/power-management
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

- ST7789P3 display initialization, orientation, text rendering, and partial
  sprite updates
- orientation-aware touch mapping and generic input event helpers
- FT6336U touch reads
- BMI270 IMU live axis reads
- KEY1/KEY2 button events
- Passive buzzer tone output with blocking and non-blocking queued playback
- BQ27220/AW32001 battery and charger status reads
- Heapless settings storage
- ESP32-C6 Wi-Fi scan/connect/disconnect lifecycle and `embassy-net`
  interface handoff using `esp-radio`
- ESP32-C6 BLE controller lifecycle with a connectable GATT peripheral example
  and notification-mirroring GATT surface
- SX1262 LoRa construction, safe no-TX bring-up, receive mode, and explicitly
  gated transmit example
- Motion/context helpers derived from BMI270 acceleration samples
- Lightweight layout, text, progress, transition, shape, and sprite helpers
- M5Stack Unit ENV Pro BME688 environmental reads over I2C/Qwiic
- Board information display

## Installation

Add the public facade crate:

```bash
cargo add nesso
```

Enable Wi-Fi only for applications that use the ESP32-C6 radio:

```bash
cargo add nesso --features wifi
cargo add esp-alloc
```

Enable ENV Pro support only for applications that use the external unit:

```bash
cargo add nesso --features env
```

Enable BLE only for applications that use the ESP32-C6 Bluetooth controller:

```bash
cargo add nesso --features ble
cargo add esp-alloc
```

Enable onboard SX1262 LoRa support only for applications that use the LoRa
transceiver:

```bash
cargo add nesso --features lora
```

## Public Modules

- `nesso::Nesso`: public facade and shared board ownership.
- `nesso::bsp`: Nesso N1 board constants, GPIOs, I2C addresses, and
  board-specific setup helpers.
- `nesso::display`: ST7789P3 display driver with `embedded-graphics`
  integration.
- `nesso::env`: external environmental unit support, gated behind the `env`
  feature.
- `nesso::ble`: ESP32-C6 BLE controller lifecycle and HCI handoff, gated
  behind the `ble` feature.
- `nesso::lora`: onboard SX1262 LoRa transceiver support, gated behind the
  `lora` feature.
- `nesso::runtime`: explicit ESP radio runtime startup helpers for Wi-Fi/BLE
  async applications.
- `nesso::touch`: FT6336U touch controller support.
- `nesso::input`: button event and touch gesture state machine helpers.
- `nesso::imu`: BMI270 initialization, config upload, and sensor reads.
- `nesso::motion`: coarse motion and pose helpers built from accelerometer
  samples.
- `nesso::audio`: passive buzzer output with blocking and queued non-blocking
  tone generation.
- `nesso::power`: BQ27220 fuel gauge and AW32001 charger status support.
- `nesso::wifi`: ESP32-C6 Wi-Fi station lifecycle support and network interface
  handoff for application-owned network stacks, gated behind the `wifi`
  feature.
- `nesso::storage`: heapless settings storage primitives and
  `esp-storage` flash-backed persistence.
- `nesso::sprite`: caller-owned RGB565 sprite/framebuffer support, sprite
  region iterators, and dirty-region flushing.
- `nesso::ui`: small `embedded-graphics` layout, label, progress, and
  transition helpers plus generic shape primitives.

## Examples

Public modules have focused hardware or module-validation examples:

| Module | Example |
| --- | --- |
| `nesso::Nesso` | `hello_world` |
| `nesso::bsp` | `board_info` |
| `nesso::display` | `display_test`, `display_orientation` |
| `nesso::touch` | `touch_test` |
| `nesso::input` | `input_test`, `input_events` |
| `nesso::imu` | `imu_test` |
| `nesso::motion` | `motion_test`, `motion_status` |
| `nesso::audio` | `audio_test`, `queued_audio` |
| `nesso::power` | `battery_test`, `power_status` |
| `nesso::wifi` | `wifi_scan`, `wifi_net_stack` |
| `nesso::ble` | `ble_peripheral`, `ble_beacon`, `ble_notifications`, `ble_notification_mirror` |
| `nesso::lora` | `lora_info`, `lora_receive`, `lora_send` |
| `nesso::storage` | `storage_test`, `storage_settings` |
| `nesso::ui` | `ui_test` |
| `nesso::sprite` | `dirty_regions` |
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
Async applications that use Wi-Fi, BLE, or both can call
`nesso.start_async_runtime()` before creating radio controllers so task startup
ordering is explicit.
Applications that need TCP/IP create their own `embassy-net` stack by calling
`wifi.take_interfaces()` and passing `interfaces.station` to `embassy-net`.
The SDK continues to own station control through the same `EspRadioWifi` value.
HTTP, NTP, DNS, weather APIs, and other protocols belong in application crates.

BLE is behind the optional `ble` feature and is initialized lazily with
`nesso.init_ble()`. The SDK owns board/controller bring-up and can hand the HCI
connector to a host stack. The `ble_peripheral` example uses Trouble to
advertise as `Nesso N1` and exposes a small custom GATT service that can be
inspected with nRF Connect. The `ble_beacon` example rotates passive
non-connectable advertising payloads using `nesso::ble::BeaconSchedule`. The
`ble_notifications` example accepts `app|title|body` writes on the Nesso mirror
characteristic and displays the latest mirrored phone/app notification.

LoRa is behind the optional `lora` feature. `Nesso::new` and `Nesso::into_lora`
do not start transmission or place the SX1262 in TX mode. Attach the external
LoRa antenna before using `Sx1262::transmit`. The `lora_send` example is
compile-time gated and will not transmit unless built with
`NESSO_LORA_ALLOW_TX=1`.

## Build

Install Rust with the target specified in `rust-toolchain.toml`, then run:

```bash
cargo metadata --no-deps --format-version 1
cargo fmt --check --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-features
HOST_TARGET="$(rustc -vV | sed -n 's/^host: //p')"
RUSTFLAGS="--cfg nesso_host_tests" \
  cargo test --manifest-path tests/host/Cargo.toml --target "${HOST_TARGET}"
RUSTFLAGS="--cfg nesso_host_tests" \
  cargo clippy --manifest-path tests/host/Cargo.toml \
    --target "${HOST_TARGET}" --all-targets -- -D warnings
cargo build --workspace
cargo build --workspace --release
```

The host test harness exercises reusable logic that does not require hardware:
notification parsing, Wi-Fi credential validation, settings serialization and
checksum handling, dirty-region coalescing, touch orientation mapping, input
events, touch and power I2C parsing, queued audio behavior, motion
classification, power helpers, and generic graphics helpers.

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
- `HARDWARE_VALIDATION.md`

Release maintainers should also use `HARDWARE_VALIDATION.md` before publishing
a new release. It is the board-run checklist for examples and peripherals that
cannot be fully validated by CI.

## Release Process

Releases are published from GitHub Releases.

1. Merge through a pull request to `main`.
2. Run the software validation commands from the build section.
3. Run the board checklist in `HARDWARE_VALIDATION.md` on a connected Nesso N1.
4. Create and push the release tag.
5. Publish a GitHub Release for that tag.

The release workflow validates the workspace and publishes only the public
`nesso` crate to crates.io.

The workflow uses crates.io trusted publishing and does not require a long-lived
Cargo registry token.

## License

Licensed under the MIT License. See the `LICENSE` file.
