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

Implemented: fixed-size key/value settings store and an optional
`esp-storage`-backed flash adapter.

Gap: Partition-table offsets are application-specific and not specified by the
Nesso datasheet. The SDK therefore requires applications to provide an explicit
flash offset instead of hard-coding one.

## Wi-Fi

Implemented: typed ESP32-C6 radio resources, `esp-radio` scan/connect/disconnect
support, facade-owned Wi-Fi state, and async station APIs with blocking
convenience wrappers for simple examples.

Remaining gap: full TCP/IP socket lifecycle is not wrapped by the SDK yet.
Applications that need non-blocking Wi-Fi should run an Embassy executor and
use the async station methods exposed by `nesso::wifi`.

## Audio

Implemented: passive buzzer tones through a generic output abstraction.

Remaining gap: the investigated sources list only a passive buzzer for audio.
No source lists a microphone, so microphone support remains intentionally absent.

## Environmental Units

Implemented: `nesso::env` supports M5Stack Unit ENV Pro U169 raw BME688
measurements over I2C address `0x77`: temperature, humidity, pressure, and gas
resistance.

Remaining gap: IAQ, VOC, and eCO2 estimates are not implemented because they are
not direct BME688 register outputs. The SDK exposes raw sensor values instead of
inventing derived air-quality values.

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
