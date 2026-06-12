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
- Wi-Fi exposes typed radio resources and a scan-first station API using
  `esp-radio`.
- Storage uses fixed-capacity `heapless` data structures and an optional
  `esp-storage` flash adapter.

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

## Event Model

Buttons are modeled as an edge/state machine producing `Pressed`, `Released`,
`Held`, `Clicked`, and `DoubleClicked`. Touch state is modeled as sampled points
with a distinct event type for press, release, move, and idle.

## Async Model

Wi-Fi is the SDK layer that needs Embassy-style async behavior. `nesso::wifi`
is gated behind the `wifi` feature, owns the Nesso N1 radio resources, starts
the ESP radio runtime, and exposes `scan_async`, `connect_async`, and
`disconnect_async` for applications already running an Embassy executor. The
same module also provides blocking convenience wrappers for small examples by
using `embassy-futures::block_on` internally.

Display, touch, IMU, power, audio, storage, and ENV drivers remain
`embedded-hal` first. They do not force an executor onto consuming
applications.
