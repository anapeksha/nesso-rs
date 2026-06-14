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
- LoRa exposes the onboard SX1262 as an explicit opt-in driver. Driver
  construction does not power the transmitter or enter TX mode; applications
  must configure the radio and call transmit explicitly after attaching the
  external antenna.
- Storage uses fixed-capacity `heapless` data structures and an optional
  `esp-storage` flash adapter with a documented SDK settings partition.
- `nesso::runtime` centralizes ESP radio runtime startup so async applications
  can start Wi-Fi/BLE ordering explicitly without duplicating board wiring.

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

Applications using Wi-Fi or BLE from async tasks can call
`Nesso::start_async_runtime` once before creating radio controllers. The facade
then constructs Wi-Fi and BLE wrappers that reuse the already-started runtime.
Simple examples may continue to let Wi-Fi or BLE start the runtime lazily.

## Display Model

`nesso::display` targets ST7789P3 and exposes:

- bus and panel configuration separated along the same ownership boundary used
  by M5GFX,
- power-on command sequencing,
- reset and backlight ownership,
- backlight abstraction,
- clear/fill operations,
- centered text rendering,
- logical display orientation,
- orientation-aware fills and blits,
- `embedded-graphics` integration.

The command transport is generic over `embedded-hal` SPI and output-pin traits.
Panel offsets and color inversion are explicit configuration fields because the
Nesso N1 ST7789 visible area is offset inside display memory.

`nesso::sprite` adds caller-owned RGB565 framebuffers, coalescing dirty-region
lists, and region iterators so applications can render off-screen without a
global heap and copy only changed areas. `nesso::ui` adds small layout, text,
progress-bar, rounded shape, circle, line, arc, sector, and integer-transition
helpers that work with any `embedded-graphics` target. The SDK keeps these
primitives generic and avoids an application screen/router framework.

## Event Model

Buttons are modeled as an edge/state machine producing `Pressed`, `Released`,
`Held`, `Clicked`, and `DoubleClicked`. Touch state is modeled as sampled points
with a distinct event type for press, release, move, and idle.
`nesso::input` also provides generic button timing and touch gesture state
machines so applications can classify short press, long press, repeat, tap,
drag, and swipe without embedding navigation policy in the SDK.

## Async Model

Wi-Fi is the SDK layer that needs Embassy-style async behavior. `nesso::wifi`
is gated behind the `wifi` feature, owns the Nesso N1 radio resources, can
reuse an explicitly started radio runtime, and exposes `scan_async`,
`connect_async`,
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

## LoRa Model

The Nesso N1 SX1262 shares the documented SPI bus with the LCD. The BSP places
the raw ESP-HAL SPI bus behind an `embedded-hal-bus`
`CriticalSectionDevice` setup, giving the LCD and SX1262 separate SPI devices
with separate chip-select pins. This keeps synchronization in the board layer
and lets applications use `nesso.display` and `nesso.lora` at the same time.

`nesso::lora` is gated behind the `lora` feature and provides typed LoRa
configuration, standby/sleep, continuous or bounded receive, IRQ/status reads,
RSSI/packet metadata, and explicit packet transmit. `Nesso::new` and
`Sx1262::new_nesso` do not transmit. The transmit path is intentionally a named
method with antenna-safety documentation.

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
