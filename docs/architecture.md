# Architecture

## Research Summary

M5Unified provides a single high-level entry point (`begin`, `update`) and
aggregates display, input, IMU, speaker, microphone, and power services. M5GFX
separates display panel, bus, light, and touch configuration, then exposes a
graphics-oriented object. M5CoreS3 packages board defaults around those layers.

For Nesso N1, the Rust SDK keeps the useful shape but not the compatibility
surface:

- `nesso-n1` owns verified board topology, pin/address constants, and Nesso
  display panel configuration.
- Peripheral crates own one device family each.
- `nesso` aggregates concrete Nesso services for application code.
- Device drivers use `embedded-hal` and `embedded-hal-async` traits so esp-hal
  peripheral instances can be passed without global singletons.
- Graphics support is provided through `embedded-graphics::DrawTarget`.
- Wi-Fi is represented with async station operations compatible with Embassy.
- Storage uses fixed-capacity `heapless` data structures to avoid allocator
  requirements in the public API.

## Initialization Model

Applications should initialize `esp-hal`, split peripherals into board resources
with `NessoN1::new`, and then build the services needed by the application. The
unified facade is intentionally small:

```rust
let board = NessoN1::new(BoardResources::new());
let mut nesso = Nesso::from_parts(board, display, touch, imu, power, storage, wifi);
```

This keeps ownership visible and avoids global mutable state.

## Display Model

`nesso-display` targets ST7789P3 and exposes:

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

Wi-Fi station operations are async trait methods. This keeps the SDK compatible
with Embassy while letting esp-wifi integration live behind a small adapter.
