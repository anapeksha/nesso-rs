# Roadmap

## Completed Foundation

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
7. Add SDK-owned BLE controller lifecycle, advertising payload helpers,
   connectable GATT peripheral validation, passive beacon scheduling, and
   notification mirror helpers.
8. Add motion/context helpers for coarse still/moving and pose classification.
9. Add UI/layout/text/progress/transition helpers on top of `embedded-graphics`.
10. Define a default SDK settings partition strategy while retaining explicit
    offset support for custom flash maps.
11. Add wrapped text rendering, BLE notification wire helpers, and checksummed
    settings persistence.

## Later

1. Promote more BLE host helpers into the SDK API after the Trouble/ESP32-C6 host
   integration surface is stable enough to commit to semver.
2. Add higher-level Wi-Fi network/socket lifecycle helpers only if they can stay
   board-lifecycle focused and executor-neutral.
3. Add partition-table lookup when the ESP Rust storage stack exposes a stable
   named-partition API.
4. Add LoRa support for SX1262 after the core SDK stabilizes.
5. Add optional 802.15.4 services.
6. Add hardware-in-the-loop CI scripts for Nesso N1 examples.
7. Keep ENV Pro support raw-only unless an MIT-compatible open algorithm becomes available.
