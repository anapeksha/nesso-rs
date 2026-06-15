# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses semantic versioning.

## [0.2.0] - 2026-06-15

### Added

- Public facade hardening release for the single-crate `nesso` SDK.
- Shared SPI bus ownership for display and onboard SX1262 LoRa using
  `embedded-hal-bus`.
- Simultaneous `nesso.display` and `nesso.lora` access when the `lora` feature
  is enabled.
- `dashboard` example combining landscape display output, touch, battery
  status, and IMU reads through the unified facade.
- Release hardware-validation documentation linked from the README.

### Changed

- `nesso.lora` is the non-consuming LoRa access path. The deprecated
  `Nesso::into_lora()` compatibility method was removed before a future stable
  API freeze.
- README installation, examples, feature documentation, and release validation
  commands now use the embedded ESP32-C6 target explicitly.

## [0.1.0] - 2026-06-14

### Added

- Shared SPI bus ownership for display and onboard SX1262 LoRa using
  `embedded-hal-bus`.
- Simultaneous `nesso.display` and `nesso.lora` access when the `lora` feature
  is enabled.
- `dashboard` example combining landscape display output, touch, battery
  status, and IMU reads through the unified facade.
- Release hardware-validation documentation linked from the README.

### Changed

- `Nesso::new` now constructs the LoRa driver as a facade field when
  `features = ["lora"]` is enabled.
- `Nesso::into_lora()` was deprecated. Use `nesso.lora` instead.
- LoRa examples use the non-consuming shared-bus API.
- README installation, examples, and feature documentation now reflect the
  single-crate facade model.

## [0.0.9] - 2026-06-14

### Added

- Optional onboard SX1262 LoRa support behind `features = ["lora"]`.
- Typed LoRa configuration, receive, transmit, status, IRQ, RSSI, standby, and
  sleep helpers.
- `lora_info`, `lora_receive`, and gated `lora_send` examples.
- AW32001 charger control APIs and facade charging helpers.

### Fixed

- Battery charging is now explicitly enabled by firmware when requested.
- BQ27220 fuel-gauge reads match the official Arduino byte-read behavior.

## [0.0.8] - 2026-06-14

### Added

- Public audio queue and storage capacity constants.
- Additional host tests for touch orientation, queued audio edge cases, and
  storage format constants.
- Local release hardware-validation checklist.

### Changed

- Queued buzzer polling became iterative instead of recursive.

## [0.0.7] - 2026-06-14

### Added

- Display orientation, dirty-region helpers, richer graphics primitives, and
  touch orientation mapping.
- Generic input gesture helpers and non-blocking queued audio.
- BLE notification mirror, beacon scheduling, and validation examples.
- Power and motion helper APIs.

## [0.0.6] - 2026-06-14

### Added

- Explicit shared ESP radio runtime startup through `Nesso::start_async_runtime`.
- Wi-Fi/BLE startup ordering guidance and examples.
- Display, audio, storage, power, and motion ergonomic improvements.

## [0.0.5] - 2026-06-14

### Added

- Wi-Fi station network-interface handoff for application-owned `embassy-net`
  stacks.
- `NetworkInterfaces`, `StationInterface`, and `EspRadioWifi::take_interfaces`.
- Minimal `wifi_net_stack` example.

## [0.0.4] - 2026-06-14

### Added

- BLE notification wire helpers and notification inbox iteration.
- Wrapped text rendering for compact embedded UI.
- Settings removal, clearing, iteration, and checksummed v2 persistence.

## [0.0.3] - 2026-06-14

### Added

- Wi-Fi connect/disconnect lifecycle improvements.
- BLE controller startup and basic GATT/beacon examples.
- Motion/context helpers and richer UI primitives.
- Default SDK settings partition strategy.

## [0.0.2] - 2026-06-14

### Changed

- Collapsed publishing to the single public `nesso` crate with public modules.
- Kept Wi-Fi and ENV support behind feature flags.
- Improved display streaming, sprite, and partial-update performance.
- Updated CI and release automation to publish only `nesso`.

## [0.0.1] - 2026-06-14

### Added

- Initial Nesso N1 Rust SDK foundation.
- Board facade, display, touch, input, IMU, audio, power, Wi-Fi, storage, and UI
  modules.
- Hardware notes, architecture docs, examples, and initial release workflow.
