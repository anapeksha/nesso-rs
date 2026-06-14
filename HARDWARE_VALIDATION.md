# Nesso N1 Hardware Validation

This is the release hardware-validation checklist for `nesso`.

Run this document before publishing a new crates.io/GitHub release. CI validates
formatting, linting, host-testable logic, docs, and embedded builds; this file
covers the board-run checks that require a physical Arduino Nesso N1.

Build examples in release mode and flash them one at a time to a connected
board. Record pass/fail notes in the release PR or release checklist.

Set the serial port for your machine:

```bash
export NESSO_PORT=/dev/cu.usbmodem1101
```

For Wi-Fi validation, set:

```bash
export NESSO_WIFI_SSID="$WIFI_SSID"
export NESSO_WIFI_PASSWORD="$WIFI_PASSWORD"
```

## Display

```bash
cargo build -p display_test --release
espflash flash --chip esp32c6 -p "$NESSO_PORT" \
  target/riscv32imac-unknown-none-elf/release/display_test
```

Pass criteria:

- Backlight turns on.
- Screen clears without continuous flicker.
- Text and color blocks are visible and stable.

## Display Orientation

```bash
cargo build -p display_orientation --release
espflash flash --chip esp32c6 -p "$NESSO_PORT" \
  target/riscv32imac-unknown-none-elf/release/display_orientation
```

Pass criteria:

- Landscape content is readable.
- Direction markers match the selected orientation.
- No clipping occurs at the logical display edges.

## Dirty Regions

```bash
cargo build -p dirty_regions --release
espflash flash --chip esp32c6 -p "$NESSO_PORT" \
  target/riscv32imac-unknown-none-elf/release/dirty_regions
```

Pass criteria:

- Partial updates are visible.
- The display does not perform full-screen flashing during updates.

## Touch

```bash
cargo build -p touch_test --release
espflash flash --chip esp32c6 -p "$NESSO_PORT" \
  target/riscv32imac-unknown-none-elf/release/touch_test
```

Pass criteria:

- Touch coordinates update while dragging.
- Press and release states are visible.
- Coordinates remain within display bounds.

## Input Events

```bash
cargo build -p input_events --release
espflash flash --chip esp32c6 -p "$NESSO_PORT" \
  target/riscv32imac-unknown-none-elf/release/input_events
```

Pass criteria:

- KEY1/KEY2 press state changes are displayed.
- Touch tap, drag, and release feedback appears.

## IMU And Motion

```bash
cargo build -p motion_status --release
espflash flash --chip esp32c6 -p "$NESSO_PORT" \
  target/riscv32imac-unknown-none-elf/release/motion_status
```

Pass criteria:

- Acceleration-derived pose changes when the board is tilted.
- Motion state changes after moving and then resting the board.

## Audio

```bash
cargo build -p queued_audio --release
espflash flash --chip esp32c6 -p "$NESSO_PORT" \
  target/riscv32imac-unknown-none-elf/release/queued_audio
```

Pass criteria:

- A short repeating buzzer tone is audible.
- Display/input responsiveness is not blocked by tone playback.

## Power

```bash
cargo build -p power_status --release
espflash flash --chip esp32c6 -p "$NESSO_PORT" \
  target/riscv32imac-unknown-none-elf/release/power_status
```

Pass criteria:

- Battery percentage and voltage render on screen.
- Charging state changes appropriately when USB/external power changes.

## Storage

```bash
cargo build -p storage_settings --release
espflash flash --chip esp32c6 -p "$NESSO_PORT" \
  target/riscv32imac-unknown-none-elf/release/storage_settings
```

Pass criteria:

- Settings can be written, read back, removed, and shown on screen.
- Reflashing does not corrupt unrelated examples.

## Wi-Fi

```bash
NESSO_WIFI_SSID="$NESSO_WIFI_SSID" NESSO_WIFI_PASSWORD="$NESSO_WIFI_PASSWORD" \
  cargo build -p wifi_net_stack --release
espflash flash --chip esp32c6 -p "$NESSO_PORT" \
  target/riscv32imac-unknown-none-elf/release/wifi_net_stack
```

Pass criteria:

- Board connects to the configured AP.
- Display shows the `embassy-net ready` state.
- No application-level HTTP/NTP behavior is required for SDK validation.

## BLE

```bash
cargo build -p ble_notifications --release
espflash flash --chip esp32c6 -p "$NESSO_PORT" \
  target/riscv32imac-unknown-none-elf/release/ble_notifications
```

Pass criteria:

- Device advertises and can be inspected from nRF Connect.
- Writing `app|title|body` to the mirror characteristic updates the display.

## LoRa Info

```bash
cargo build -p lora_info --release
espflash flash --chip esp32c6 -p "$NESSO_PORT" \
  target/riscv32imac-unknown-none-elf/release/lora_info
```

Pass criteria:

- Display shows the SX1262 info screen before the SPI bus is handed to LoRa.
- The board does not reset after the SX1262 no-TX bring-up path runs.
- No transmit path is exercised.

## LoRa Receive

```bash
cargo build -p lora_receive --release
espflash flash --chip esp32c6 -p "$NESSO_PORT" \
  target/riscv32imac-unknown-none-elf/release/lora_receive
```

Pass criteria:

- Display shows the RX continuous screen before the SPI bus is handed to LoRa.
- The board enters receive mode and remains stable.
- No transmit path is exercised.

## LoRa Send

Attach the external LoRa antenna before running this validation.

```bash
NESSO_LORA_ALLOW_TX=1 cargo build -p lora_send --release
espflash flash --chip esp32c6 -p "$NESSO_PORT" \
  target/riscv32imac-unknown-none-elf/release/lora_send
```

Pass criteria:

- Display shows the antenna warning before the SPI bus is handed to LoRa.
- Packets transmit only when built with `NESSO_LORA_ALLOW_TX=1`.
- A second LoRa receiver configured for the same frequency can receive packets.

## Board Info

```bash
cargo build -p board_info --release
espflash flash --chip esp32c6 -p "$NESSO_PORT" \
  target/riscv32imac-unknown-none-elf/release/board_info
```

Pass criteria:

- Board name, MCU, display, touch, IMU, power, and radio capability information
  render correctly.
