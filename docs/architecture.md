# Architecture

## Research Summary

M5Unified provides a single high-level entry point (`begin`, `update`) and
aggregates display, input, IMU, speaker, microphone, and power services. M5GFX
separates display panel, bus, light, and touch configuration, then exposes a
graphics-oriented object. M5CoreS3 packages board defaults around those layers.

For Nesso N1, the Rust SDK keeps the useful shape but not the compatibility
surface:

- `nesso::bsp` owns verified board topology, pin/address constants, and Nesso
  display panel configuration.
- Public modules own one device family each.
- `nesso::Nesso` owns the public facade and constructs board services from
  ESP-HAL peripherals.
- Device drivers use `embedded-hal` and `embedded-hal-async` traits so esp-hal
  peripheral instances can be passed without global singletons.
- Graphics support is provided through `embedded-graphics::DrawTarget`.
- Wi-Fi exposes typed radio resources, a station lifecycle API, and a one-time
  station network-interface handoff using `esp-radio`.
- BLE exposes the ESP32-C6 HCI connector lifecycle using `esp-radio`, with
  fixed-capacity beacon scheduling types and Trouble-based examples for
  phone-visible GATT, passive scanning, and notification mirroring validation.
- Storage uses fixed-capacity `heapless` data structures and an optional
  `esp-storage` flash adapter with a documented SDK settings partition.

## Initialization Model

Applications initialize `esp-hal`, then hand the resulting peripherals to the
facade:

```rust
let peripherals = esp_hal::init(config);
let mut nesso = Nesso::new(peripherals)?;
```

`Nesso::new` owns fixed board wiring, display setup, LCD expander
reset/backlight setup, buzzer setup, and optional Wi-Fi radio resources. BMI270
configuration upload is explicit through `Nesso::init_imu` so applications that
only need display/audio do not fail on IMU bring-up. Touch, IMU, and power are
exposed through facade methods because they share the board I2C bus. This keeps
ownership visible and avoids global mutable state.

## Display Model

`nesso::display` targets ST7789P3 and exposes:

- bus and panel configuration separated along the same ownership boundary used
  by M5GFX,
- power-on command sequencing,
- reset and backlight ownership,
- backlight abstraction,
- clear/fill operations,
- centered text rendering,
- `embedded-graphics` integration.

The command transport is generic over `embedded-hal` SPI and output-pin traits.
Panel offsets and color inversion are explicit configuration fields because the
Nesso N1 ST7789 visible area is offset inside display memory.

`nesso::sprite` adds caller-owned RGB565 framebuffers so applications can render
off-screen without a global heap. `nesso::ui` adds small layout, text,
progress-bar, and integer-transition helpers that work with any
`embedded-graphics` target. The SDK keeps these primitives generic and avoids an
application screen/router framework.

## Event Model

Buttons are modeled as an edge/state machine producing `Pressed`, `Released`,
`Held`, `Clicked`, and `DoubleClicked`. Touch state is modeled as sampled points
with a distinct event type for press, release, move, and idle.

## Async Model

Wi-Fi is the SDK layer that needs Embassy-style async behavior. `nesso::wifi`
is gated behind the `wifi` feature, owns the Nesso N1 radio resources, starts
the ESP radio runtime, and exposes `scan_async`, `connect_async`,
`ensure_connected_async`, and `disconnect_async` for applications already
running an Embassy executor. The same module also provides blocking convenience
wrappers for small examples by using `embassy-futures::block_on` internally.
Applications that need TCP/IP call `EspRadioWifi::take_interfaces()` and pass
`interfaces.station` into their own `embassy-net` stack. The SDK keeps the
controller for scan/connect/disconnect and does not own HTTP, NTP, DNS, weather
clients, or other application protocols.

BLE is gated behind the `ble` feature. The current Rust ESP radio stack exposes
BLE as an HCI connector, so the SDK owns controller initialization and provides
typed advertising-data helpers and SDK service UUIDs. A BLE host stack should
build phone pairing, custom GATT services, notifications, and beacon scheduling
on top of that connector.

Display, touch, IMU, power, audio, storage, and ENV drivers remain
`embedded-hal` first. They do not force an executor onto consuming
applications.

## Storage Model

The SDK exposes `SettingsPartition::DEFAULT` for the factory Nesso N1 flash
layout and the label `nesso_settings` for applications that provide a partition
table. The explicit-offset constructor remains available for applications that
own their flash map. This keeps the SDK opinionated for Nesso N1 while avoiding
hidden writes into application-owned flash in custom layouts. Persisted settings
use a small versioned image format; v2 images include a checksum and the loader
keeps v1 compatibility for previously saved data.

## Motion Model

`nesso::motion` is intentionally a helper layer rather than a business logic
engine. It classifies coarse pose and still/moving context from acceleration
samples and leaves application decisions to consumers.
