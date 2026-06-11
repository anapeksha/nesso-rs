# M5GFX Analysis

Sources inspected: M5GFX `M5GFX.cpp`, `LGFXBase.hpp`, `Panel_Device.hpp`,
ESP32 `Bus_SPI.hpp`, `Touch_FT5x06.hpp`, `LGFX_Sprite.hpp`, and
`SpriteBuffer.{hpp,cpp}`.

## Architecture

M5GFX separates display support into four main ownership boundaries:

- Device: `LGFX_Device` owns a selected panel and exposes drawing APIs.
- Bus: `Bus_SPI`, parallel, I2C, DSI, or host-specific buses own transport
  configuration and transaction behavior.
- Panel: `Panel_Device` owns panel geometry, offsets, inversion, CS/reset/busy,
  bus pointer, optional light, optional touch, DMA hooks, and address-window
  writes.
- Touch/light: touch and backlight are attached to the panel but configured as
  separate devices.

Runtime initialization builds concrete bus, panel, touch, and light objects,
configures each with a typed config object, attaches them, then calls the device
initializer. This makes board defaults explicit while allowing runtime panel
detection for boards that support more than one display.

## Bus Abstraction

`Bus_SPI::config_t` includes write/read frequency, pins, SPI mode, locking,
host selection, 3-wire/quad flags, and DMA channel. The bus owns low-level
transactions and exposes `writeCommand`, `writeData`, byte writes, reads, and
DMA queue operations.

For Nesso N1 the useful lesson is not dynamic bus polymorphism; the board has
one known ST7789 display. The useful lesson is explicit bus configuration and
treating DMA as a bus property. `nesso-display` now exposes `BusConfig` with
write frequency and DMA intent.

## Panel Abstraction

`Panel_Device::config_t` includes CS, reset, busy, memory dimensions, visible
panel dimensions, X/Y offsets, rotation offset, read dummy bits, inversion,
RGB/BGR order, 16-bit transfer alignment, and bus-sharing status. The panel owns
address-window logic and optional touch/light attachment.

For Nesso N1 this directly maps to verified ST7789 settings from the official
Arduino_Nesso_N1 library: 135 x 240, offset X 52, offset Y 40, invert enabled,
CS GPIO17, DC GPIO16, reset via expander, backlight via expander.
`nesso-display` now has a `PanelConfig` instead of only width and height.

## Touch Abstraction

M5GFX provides `Touch_FT5x06` with address `0x38`, 400 kHz I2C default, raw
point reads, sleep/wake, and coordinate calibration through panel conversion.
Nesso N1 uses FT6336U, which is FT6x36-family compatible for the basic status
and point registers used by the SDK.

`nesso-touch` remains a separate crate because Rust ownership is clearer when
the I2C device is not hidden behind the display object. The verified interrupt
line is now documented in the BSP as GPIO3.

## DMA and Sprites

M5GFX treats DMA as a bus capability and uses `SpriteBuffer` to allocate DMA
capable buffers when possible, falling back to normal or preallocated memory.
Sprites are off-screen drawing targets that can be pushed to a panel.

This pass does not add sprite functionality because the request explicitly says
not to create new functionality. The architectural implication is captured in
`BusConfig::use_dma`, leaving room for a future explicit sprite crate or module
without changing the basic display ownership model.

## Applied Improvements

- `nesso-display`: added bus/panel config separation, reset ownership, panel
  offsets, inversion, and verified Nesso display defaults in examples.
- `nesso-touch`: kept independent I2C ownership; BSP now documents touch INT.
- `nesso`: no structural change required beyond relying on the improved
  display/touch types. The existing parts-bundle constructor is consistent with
  Rust ownership and avoids hidden global device state.
