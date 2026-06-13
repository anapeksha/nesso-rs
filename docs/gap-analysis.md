# Gap Analysis

## Hardware Discovery Evidence

Investigated sources:

- Arduino TPX00227 public datasheet.
- Arduino TPX00227 full pinout PDF.
- Espressif Arduino ESP32 variant:
  `variants/arduino_nesso_n1/pins_arduino.h`.
- Official Arduino_Nesso_N1 support library:
  `src/Arduino_Nesso_N1.h` and `src/expander.cpp`.
- Official Arduino_Nesso_N1 example `BatteryDisplay`.
- M5Stack Arduino Nesso N1 documentation.

The combination of those sources resolves all requested GPIO and bus mappings:
`LCD_CS=GPIO17`, `LCD_DC/LCD_RS=GPIO16`, `LCD_RST=E1.P1`, `LCD_BL=E1.P6`,
touch and IMU interrupt line `GPIO3`, I2C `SDA=GPIO10`, `SCL=GPIO8`, SPI
`MOSI=GPIO21`, `MISO=GPIO22`, `SCK=GPIO20`.

## Display

Implemented: ST7789P3 command abstraction, M5GFX-inspired bus/panel separation,
verified SPI pins, verified CS/DC/reset/backlight ownership, 135 x 240 geometry,
offsets 52/40, inversion, clear/fill support, centered text helpers, and
`embedded-graphics::DrawTarget`.

Remaining gap: no gap in the requested GPIO mappings.

## Touch

Implemented: FT6336U I2C reads for touch count and first point, plus event state
conversion.

Remaining gap: no gap in the requested GPIO/I2C mapping. Touch polling is
implemented; an interrupt-driven async wrapper is not added in this pass because
the request was refactoring and verification, not new functionality.

## IMU

Implemented: BMI270 config upload through the `bmi2` crate, explicit
facade-level initialization through `Nesso::init_imu`, raw accelerometer reads,
raw gyroscope reads, chip-id read helper, and orientation data types.

Remaining gap: interrupt-driven motion events are not implemented. The shared
interrupt line is documented, but this pass keeps IMU reads polling-based.

## Power

Implemented: BQ27220 and AW32001 addresses, verified fuel-gauge register reads,
capacity-derived percentage, and AW32001 charge-state read.

Remaining gap: no requested bus or register-access gap. Full charger control
methods remain out of scope for this refactor.

## Storage

Implemented: fixed-size key/value settings store, removal/iteration helpers,
and an optional `esp-storage`-backed flash adapter. The SDK now defines the
default `nesso_settings` partition strategy through `SettingsPartition::DEFAULT`,
using offset `0x00FC_0000` and a 4 KiB reserved area for the factory Nesso N1
flash layout. Applications with custom flash maps can still pass an explicit
partition or offset. Flash persistence writes a v2 settings image with a
checksum while retaining v1 read compatibility.

Remaining gap: stable named-partition lookup is not exposed by `esp-storage`
0.9.0. The SDK documents the partition label and offset, but does not parse a
runtime partition table.

## Wi-Fi

Implemented: typed ESP32-C6 radio resources, explicit shared radio runtime
startup through `Nesso::start_async_runtime`, `esp-radio`
scan/connect/disconnect support, validated credentials, facade-owned Wi-Fi
state, typed connection snapshots, RSSI reads, async station APIs with blocking
convenience wrappers for simple examples, and a one-time `NetworkInterfaces`
handoff for application-owned `embassy-net` stacks. The `wifi_scan` example can optionally connect when
`NESSO_WIFI_SSID` and `NESSO_WIFI_PASSWORD` are provided at compile time. The
`wifi_net_stack` example starts the runtime explicitly, connects, takes the
SDK-created interfaces, and creates an `embassy-net` stack from
`interfaces.station`.

Remaining gap: TCP/IP sockets and application protocols are intentionally not
wrapped by the SDK. The SDK boundary is board Wi-Fi lifecycle.

## BLE

Implemented: optional `nesso::ble` feature, board-owned ESP32-C6 Bluetooth
resource handoff, explicit or lazy shared radio runtime startup, BLE HCI
controller initialization through `esp-radio`, HCI read/write helpers, owned HCI
connector handoff, fixed-capacity
advertising-data builder, SDK service UUID constants, and a Trouble-based
connectable peripheral example for nRF Connect validation. Fixed-capacity
beacon scheduling primitives and a passive rotating beacon example are also
implemented. Fixed-capacity notification mirror primitives and a GATT write
example are implemented for phone/app notification mirroring.

Remaining gap: native OS notification capture still requires a companion phone
app or platform-specific client permission. The SDK exposes the board-side BLE
transport and mirror data model, not a mobile application.

## Audio

Implemented: passive buzzer tones through a generic output abstraction,
blocking tone generation, and a fixed-capacity non-blocking tone queue for
event-loop polling.

Remaining gap: the investigated sources list only a passive buzzer for audio.
No source lists a microphone, so microphone support remains intentionally absent.

## Environmental Units

Implemented: `nesso::env` supports M5Stack Unit ENV Pro U169 raw BME688
measurements over I2C address `0x77`: temperature, humidity, pressure, and gas
resistance.

Remaining gap: IAQ, VOC, and eCO2 estimates are not implemented because they are
not direct BME688 register outputs. The SDK exposes raw sensor values instead of
inventing derived air-quality values.

## Motion and UI

Implemented: motion helper module for coarse pose, orientation kind, dominant
gravity, and still/moving context classification; caller-owned RGB565 sprites;
display orientation; orientation-aware touch mapping; coalescing dirty-region
primitives; sprite region iterators; and draw-target-agnostic UI helpers for
labels, progress bars, layout rows, filled rounded rectangles, circles, lines,
arcs, filled sectors, and integer transitions.

Remaining gap: no app-level view router is included. The SDK intentionally
provides reusable primitives rather than application/business workflow screens.

## BSP

Implemented: verified component list, addresses, geometry, display offsets, bus
pins, native GPIOs, expander GPIOs, and concrete facade-ready board parts.

Remaining gap: no requested BSP mapping gap remains.

## Facade

Implemented: `nesso::Nesso::new(peripherals)` initializes board-owned display,
I2C, LCD expander reset/backlight, passive buzzer, Wi-Fi radio resources, and
optional flash settings ownership. BMI270 configuration is explicit through
`Nesso::init_imu`.

Remaining gap: I2C-backed subsystems are exposed through facade methods rather
than public fields because they share one physical I2C bus.
