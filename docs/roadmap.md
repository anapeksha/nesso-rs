# Roadmap

## v0.0.2

1. Add the public `nesso::Nesso::new(peripherals)` facade entry point.
2. Route display, touch, IMU, power, audio, and Wi-Fi scan examples through the
   facade.
3. Publish only the single public `nesso` crate.
4. Add an optional `esp-storage` backed settings adapter that requires an
   application-selected flash offset.
5. Harden the Wi-Fi API around typed radio resources, scan/connect/disconnect
   station operations, and blocking wrappers for simple examples.
6. Add `nesso::env` support with M5Stack Unit ENV Pro / BME688 raw
   environmental measurements over Qwiic/I2C.

## Later

1. Add higher-level Wi-Fi network/socket lifecycle helpers on top of the current
   station connect/disconnect API.
2. Add partition-table driven persistent settings offsets instead of requiring
   callers to pass a raw flash offset.
3. Add LoRa support for SX1262 after the core SDK stabilizes.
4. Add optional BLE and 802.15.4 services.
5. Add hardware-in-the-loop CI scripts for Nesso N1 examples.
6. Keep ENV Pro support raw-only unless an MIT-compatible open algorithm becomes available.
