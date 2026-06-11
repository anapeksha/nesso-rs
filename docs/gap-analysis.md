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

Implemented: BMI270 chip-id read, acceleration, gyroscope, and orientation data
types.

Remaining gap: Bosch BMI270 feature configuration upload is not encoded. The
official Arduino library delegates this to Arduino_BMI270_BMM150 and configures
accelerometer plus gyroscope at 25 Hz. Porting that vendor configuration blob
would be new functionality rather than a refactor.

## Power

Implemented: BQ27220 and AW32001 addresses, verified fuel-gauge register reads,
capacity-derived percentage, and AW32001 charge-state read.

Remaining gap: no requested bus or register-access gap. Full charger control
methods remain out of scope for this refactor.

## Storage

Implemented: fixed-size key/value settings store suitable for flash page
serialization by an esp-storage adapter.

Gap: Partition-table offsets are application-specific and not specified by the
Nesso datasheet.

## Wi-Fi

Implemented: async station trait covering scan, connect, disconnect, and state.

Remaining gap: concrete esp-wifi adapter needs application-owned Embassy
executor, timers, RNG, and network stack wiring. This is not a GPIO discovery
gap.

## Audio

Implemented: passive buzzer tones through a generic output abstraction.

Remaining gap: the investigated sources list only a passive buzzer for audio.
No source lists a microphone, so microphone support remains intentionally absent.

## BSP

Implemented: verified component list, addresses, geometry, display offsets, bus
pins, native GPIOs, and expander GPIOs.

Remaining gap: no requested BSP mapping gap remains.
