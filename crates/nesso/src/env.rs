//! Environmental sensor drivers for Nesso applications.
//!
//! The first supported unit is the M5Stack Unit ENV Pro, SKU U169. It uses a
//! Bosch BME688 at I2C address `0x77`.
//!
//! The BME688 register flow and compensation formulas are ported from Bosch
//! Sensortec's official BME68x SensorAPI. This crate keeps the implementation
//! Rust-native and depends only on `embedded-hal`.

use embedded_hal::{delay::DelayNs, i2c::I2c};

/// Default I2C address for M5Stack Unit ENV Pro.
pub const ENV_PRO_I2C_ADDRESS: u8 = 0x77;

/// Bosch BME688 chip identifier.
pub const BME688_CHIP_ID: u8 = 0x61;

const REG_COEFF3: u8 = 0x00;
const REG_FIELD0: u8 = 0x1d;
const REG_RES_HEAT0: u8 = 0x5a;
const REG_GAS_WAIT0: u8 = 0x64;
const REG_CTRL_GAS_0: u8 = 0x70;
const REG_CTRL_GAS_1: u8 = 0x71;
const REG_CTRL_HUM: u8 = 0x72;
const REG_CTRL_MEAS: u8 = 0x74;
const REG_CONFIG: u8 = 0x75;
const REG_COEFF1: u8 = 0x8a;
const REG_CHIP_ID: u8 = 0xd0;
const REG_SOFT_RESET: u8 = 0xe0;
const REG_COEFF2: u8 = 0xe1;
const REG_VARIANT_ID: u8 = 0xf0;

const SOFT_RESET_CMD: u8 = 0xb6;
const PERIOD_RESET_US: u32 = 10_000;
const PERIOD_POLL_US: u32 = 10_000;
const FIELD_LEN: usize = 17;

const OS_TEMP_X2: u8 = 2;
const OS_PRESS_X1: u8 = 1;
const OS_HUM_X16: u8 = 5;
const FILTER_OFF: u8 = 0;
const ODR_NONE: u8 = 8;
const FORCED_MODE: u8 = 1;
const SLEEP_MODE: u8 = 0;

const NEW_DATA_MSK: u8 = 0x80;
const GAS_VALID_MSK: u8 = 0x20;
const HEAT_STAB_MSK: u8 = 0x10;
const HCTRL_MSK: u8 = 0x08;
const RUN_GAS_MSK: u8 = 0x30;
const DEFAULT_HEATER_TEMP_C: u16 = 300;
const DEFAULT_HEATER_DURATION_MS: u16 = 100;
const DEFAULT_AMBIENT_TEMP_C: i16 = 25;

const GAS_LOOKUP_K1: [f32; 16] = [
    0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, -0.8, 0.0, 0.0, -0.2, -0.5, 0.0, -1.0, 0.0, 0.0,
];
const GAS_LOOKUP_K2: [f32; 16] = [
    0.0, 0.0, 0.0, 0.0, 0.1, 0.7, 0.0, -0.8, -0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
];

/// Error type returned by the ENV Pro driver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvError<E> {
    /// I2C bus operation failed.
    Bus(E),
    /// The device at `0x77` did not report the BME688 chip id.
    InvalidChipId(u8),
    /// Forced-mode measurement completed without a new field.
    NoNewData,
    /// The sensor reported an unsupported BME68x variant id.
    UnsupportedVariant(u8),
}

/// BME688 heater and ambient compensation configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvConfig {
    /// Gas heater target in degrees Celsius. Values above 400 are capped by the sensor formula.
    pub heater_temp_c: u16,
    /// Gas heater duration in milliseconds.
    pub heater_duration_ms: u16,
    /// Ambient temperature estimate used by the gas heater formula.
    pub ambient_temp_c: i16,
}

impl Default for EnvConfig {
    fn default() -> Self {
        Self {
            heater_temp_c: DEFAULT_HEATER_TEMP_C,
            heater_duration_ms: DEFAULT_HEATER_DURATION_MS,
            ambient_temp_c: DEFAULT_AMBIENT_TEMP_C,
        }
    }
}

/// M5Stack Unit ENV Pro environmental sensor.
///
/// This wrapper exposes raw BME688 environmental readings: temperature,
/// humidity, pressure, and gas resistance. IAQ, VOC, and eCO2 values are not
/// direct BME688 register outputs and are intentionally not synthesized by this
/// MIT-licensed crate.
pub struct EnvPro<I2C, DELAY> {
    i2c: I2C,
    delay: DELAY,
    address: u8,
    config: EnvConfig,
    variant: Variant,
    calib: Calibration,
}

/// Measurement returned by [`EnvPro`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvMeasurement {
    /// Temperature in degrees Celsius.
    pub temperature_c: f32,
    /// Relative humidity in percent.
    pub humidity_percent: f32,
    /// Pressure in hectopascals.
    pub pressure_hpa: f32,
    /// Gas resistance in Ohms. `None` means the gas field was not valid or heat-stable.
    pub gas_resistance_ohm: Option<f32>,
}

impl<I2C, DELAY> EnvPro<I2C, DELAY>
where
    I2C: I2c,
    DELAY: DelayNs,
{
    /// Initializes the Unit ENV Pro with the default BME688 configuration.
    pub fn new(i2c: I2C, delay: DELAY) -> Result<Self, EnvError<I2C::Error>> {
        Self::with_config(i2c, delay, EnvConfig::default())
    }

    /// Initializes the Unit ENV Pro with custom BME688 heater configuration.
    pub fn with_config(
        i2c: I2C,
        delay: DELAY,
        config: EnvConfig,
    ) -> Result<Self, EnvError<I2C::Error>> {
        let mut sensor = Self {
            i2c,
            delay,
            address: ENV_PRO_I2C_ADDRESS,
            config,
            variant: Variant::GasLow,
            calib: Calibration::default(),
        };
        sensor.init()?;
        Ok(sensor)
    }

    /// Takes one forced-mode environmental measurement.
    pub fn measure(&mut self) -> Result<EnvMeasurement, EnvError<I2C::Error>> {
        self.set_op_mode(SLEEP_MODE)?;
        self.write_reg(REG_CTRL_HUM, OS_HUM_X16)?;
        self.write_reg(REG_CTRL_MEAS, ctrl_meas(FORCED_MODE))?;

        self.delay
            .delay_us(measurement_delay_us(self.config.heater_duration_ms));
        let raw = self.read_field()?;
        let mut calib = self.calib;
        let temperature_c = calc_temperature(raw.temperature_adc, &mut calib);
        let pressure_hpa = calc_pressure(raw.pressure_adc, &calib) / 100.0;
        let humidity_percent = calc_humidity(raw.humidity_adc, &calib);
        let gas_resistance_ohm = raw.gas.map(|gas| match self.variant {
            Variant::GasLow => calc_gas_resistance_low(gas.adc, gas.range, &calib),
            Variant::GasHigh => calc_gas_resistance_high(gas.adc, gas.range),
        });
        self.calib.t_fine = calib.t_fine;

        Ok(EnvMeasurement {
            temperature_c,
            humidity_percent,
            pressure_hpa,
            gas_resistance_ohm,
        })
    }

    /// Releases the wrapped I2C bus and delay provider.
    pub fn release(self) -> (I2C, DELAY) {
        (self.i2c, self.delay)
    }

    fn init(&mut self) -> Result<(), EnvError<I2C::Error>> {
        self.write_reg(REG_SOFT_RESET, SOFT_RESET_CMD)?;
        self.delay.delay_us(PERIOD_RESET_US);

        let chip_id = self.read_reg(REG_CHIP_ID)?;
        if chip_id != BME688_CHIP_ID {
            return Err(EnvError::InvalidChipId(chip_id));
        }

        self.variant = Variant::from_id(self.read_reg(REG_VARIANT_ID)?)?;
        self.calib = self.read_calibration()?;
        self.configure_forced_mode()
    }

    fn configure_forced_mode(&mut self) -> Result<(), EnvError<I2C::Error>> {
        self.set_op_mode(SLEEP_MODE)?;
        self.write_reg(REG_CTRL_HUM, OS_HUM_X16)?;
        self.write_reg(REG_CTRL_MEAS, ctrl_meas(SLEEP_MODE))?;
        self.write_reg(REG_CONFIG, config_reg())?;

        let res_heat = calc_res_heat(
            self.config.heater_temp_c,
            self.config.ambient_temp_c,
            &self.calib,
        );
        self.write_reg(REG_RES_HEAT0, res_heat)?;
        self.write_reg(REG_GAS_WAIT0, calc_gas_wait(self.config.heater_duration_ms))?;

        let mut ctrl_gas = [0; 2];
        self.read_regs(REG_CTRL_GAS_0, &mut ctrl_gas)?;
        ctrl_gas[0] = set_bits(ctrl_gas[0], HCTRL_MSK, 3, 0);
        ctrl_gas[1] = set_bits(ctrl_gas[1], 0x0f, 0, 0);
        ctrl_gas[1] = set_bits(ctrl_gas[1], RUN_GAS_MSK, 4, self.variant.run_gas_value());
        self.write_reg(REG_CTRL_GAS_0, ctrl_gas[0])?;
        self.write_reg(REG_CTRL_GAS_1, ctrl_gas[1])
    }

    fn set_op_mode(&mut self, mode: u8) -> Result<(), EnvError<I2C::Error>> {
        loop {
            let ctrl_meas = self.read_reg(REG_CTRL_MEAS)?;
            if ctrl_meas & 0x03 == SLEEP_MODE {
                self.write_reg(REG_CTRL_MEAS, (ctrl_meas & !0x03) | mode)?;
                return Ok(());
            }

            self.write_reg(REG_CTRL_MEAS, ctrl_meas & !0x03)?;
            self.delay.delay_us(PERIOD_POLL_US);
        }
    }

    fn read_field(&mut self) -> Result<RawMeasurement, EnvError<I2C::Error>> {
        let mut field = [0; FIELD_LEN];
        for _ in 0..5 {
            self.read_regs(REG_FIELD0, &mut field)?;
            if field[0] & NEW_DATA_MSK != 0 {
                return Ok(RawMeasurement::from_field(&field, self.variant));
            }
            self.delay.delay_us(PERIOD_POLL_US);
        }
        Err(EnvError::NoNewData)
    }

    fn read_calibration(&mut self) -> Result<Calibration, EnvError<I2C::Error>> {
        let mut coeff = [0; 42];
        self.read_regs(REG_COEFF1, &mut coeff[0..23])?;
        self.read_regs(REG_COEFF2, &mut coeff[23..37])?;
        self.read_regs(REG_COEFF3, &mut coeff[37..42])?;
        Ok(Calibration::from_coefficients(&coeff))
    }

    fn read_reg(&mut self, reg: u8) -> Result<u8, EnvError<I2C::Error>> {
        let mut byte = [0];
        self.read_regs(reg, &mut byte)?;
        Ok(byte[0])
    }

    fn read_regs(&mut self, reg: u8, bytes: &mut [u8]) -> Result<(), EnvError<I2C::Error>> {
        self.i2c
            .write_read(self.address, &[reg], bytes)
            .map_err(EnvError::Bus)
    }

    fn write_reg(&mut self, reg: u8, value: u8) -> Result<(), EnvError<I2C::Error>> {
        self.i2c
            .write(self.address, &[reg, value])
            .map_err(EnvError::Bus)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Variant {
    GasLow,
    GasHigh,
}

impl Variant {
    fn from_id<E>(id: u8) -> Result<Self, EnvError<E>> {
        match id {
            0 => Ok(Self::GasLow),
            1 => Ok(Self::GasHigh),
            other => Err(EnvError::UnsupportedVariant(other)),
        }
    }

    const fn run_gas_value(self) -> u8 {
        match self {
            Self::GasLow => 1,
            Self::GasHigh => 2,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Calibration {
    par_t1: u16,
    par_t2: i16,
    par_t3: i8,
    par_p1: u16,
    par_p2: i16,
    par_p3: i8,
    par_p4: i16,
    par_p5: i16,
    par_p6: i8,
    par_p7: i8,
    par_p8: i16,
    par_p9: i16,
    par_p10: u8,
    par_h1: u16,
    par_h2: u16,
    par_h3: i8,
    par_h4: i8,
    par_h5: i8,
    par_h6: u8,
    par_h7: i8,
    par_gh1: i8,
    par_gh2: i16,
    par_gh3: i8,
    res_heat_range: u8,
    res_heat_val: i8,
    range_sw_err: i8,
    t_fine: f32,
}

impl Calibration {
    fn from_coefficients(c: &[u8; 42]) -> Self {
        Self {
            par_t1: u16::from_be_bytes([c[32], c[31]]),
            par_t2: i16::from_be_bytes([c[1], c[0]]),
            par_t3: c[2] as i8,
            par_p1: u16::from_be_bytes([c[5], c[4]]),
            par_p2: i16::from_be_bytes([c[7], c[6]]),
            par_p3: c[8] as i8,
            par_p4: i16::from_be_bytes([c[11], c[10]]),
            par_p5: i16::from_be_bytes([c[13], c[12]]),
            par_p6: c[15] as i8,
            par_p7: c[14] as i8,
            par_p8: i16::from_be_bytes([c[19], c[18]]),
            par_p9: i16::from_be_bytes([c[21], c[20]]),
            par_p10: c[22],
            par_h1: ((u16::from(c[25])) << 4) | (u16::from(c[24]) & 0x0f),
            par_h2: ((u16::from(c[23])) << 4) | (u16::from(c[24]) >> 4),
            par_h3: c[26] as i8,
            par_h4: c[27] as i8,
            par_h5: c[28] as i8,
            par_h6: c[29],
            par_h7: c[30] as i8,
            par_gh1: c[35] as i8,
            par_gh2: i16::from_be_bytes([c[34], c[33]]),
            par_gh3: c[36] as i8,
            res_heat_range: (c[39] & 0x30) / 16,
            res_heat_val: c[37] as i8,
            range_sw_err: ((c[41] & 0xf0) as i8) / 16,
            t_fine: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
struct RawGas {
    adc: u16,
    range: u8,
}

#[derive(Clone, Copy)]
struct RawMeasurement {
    temperature_adc: u32,
    pressure_adc: u32,
    humidity_adc: u16,
    gas: Option<RawGas>,
}

impl RawMeasurement {
    fn from_field(field: &[u8; FIELD_LEN], variant: Variant) -> Self {
        let pressure_adc =
            (u32::from(field[2]) * 4096) | (u32::from(field[3]) * 16) | (u32::from(field[4]) / 16);
        let temperature_adc =
            (u32::from(field[5]) * 4096) | (u32::from(field[6]) * 16) | (u32::from(field[7]) / 16);
        let humidity_adc = ((u16::from(field[8])) << 8) | u16::from(field[9]);
        let gas = match variant {
            Variant::GasLow => gas_from_bytes(field[13], field[14]),
            Variant::GasHigh => gas_from_bytes(field[15], field[16]),
        };

        Self {
            temperature_adc,
            pressure_adc,
            humidity_adc,
            gas,
        }
    }
}

fn gas_from_bytes(msb: u8, lsb: u8) -> Option<RawGas> {
    if lsb & (GAS_VALID_MSK | HEAT_STAB_MSK) != (GAS_VALID_MSK | HEAT_STAB_MSK) {
        return None;
    }

    Some(RawGas {
        adc: (u16::from(msb) * 4) | (u16::from(lsb) / 64),
        range: lsb & 0x0f,
    })
}

fn ctrl_meas(mode: u8) -> u8 {
    (OS_TEMP_X2 << 5) | (OS_PRESS_X1 << 2) | mode
}

fn config_reg() -> u8 {
    let odr20 = if ODR_NONE == 8 { 0 } else { ODR_NONE };
    let odr3 = if ODR_NONE == 8 { 1 } else { 0 };
    (odr20 << 5) | (FILTER_OFF << 2) | (odr3 << 7)
}

fn set_bits(register: u8, mask: u8, shift: u8, value: u8) -> u8 {
    (register & !mask) | ((value << shift) & mask)
}

fn measurement_delay_us(heater_duration_ms: u16) -> u32 {
    let oversample_cycles = 2 + 1 + 16;
    let tph_duration_us = oversample_cycles * 1_963 + 477 * 9 + 1_000;
    tph_duration_us + u32::from(heater_duration_ms) * 1_000
}

fn calc_temperature(temp_adc: u32, calib: &mut Calibration) -> f32 {
    let adc = temp_adc as f32;
    let var1 = ((adc / 16384.0) - (calib.par_t1 as f32 / 1024.0)) * calib.par_t2 as f32;
    let var2_base = (adc / 131072.0) - (calib.par_t1 as f32 / 8192.0);
    let var2 = var2_base * var2_base * (calib.par_t3 as f32 * 16.0);
    calib.t_fine = var1 + var2;
    calib.t_fine / 5120.0
}

fn calc_pressure(pressure_adc: u32, calib: &Calibration) -> f32 {
    let mut var1 = (calib.t_fine / 2.0) - 64000.0;
    let mut var2 = var1 * var1 * (calib.par_p6 as f32 / 131072.0);
    var2 += var1 * calib.par_p5 as f32 * 2.0;
    var2 = (var2 / 4.0) + (calib.par_p4 as f32 * 65536.0);
    var1 =
        (((calib.par_p3 as f32 * var1 * var1) / 16384.0) + (calib.par_p2 as f32 * var1)) / 524288.0;
    var1 = (1.0 + (var1 / 32768.0)) * calib.par_p1 as f32;
    if var1 == 0.0 {
        return 0.0;
    }

    let mut pressure = 1048576.0 - pressure_adc as f32;
    pressure = ((pressure - (var2 / 4096.0)) * 6250.0) / var1;
    var1 = (calib.par_p9 as f32 * pressure * pressure) / 2147483648.0;
    var2 = pressure * (calib.par_p8 as f32 / 32768.0);
    let var3 = (pressure / 256.0)
        * (pressure / 256.0)
        * (pressure / 256.0)
        * (calib.par_p10 as f32 / 131072.0);
    pressure + (var1 + var2 + var3 + (calib.par_p7 as f32 * 128.0)) / 16.0
}

fn calc_humidity(humidity_adc: u16, calib: &Calibration) -> f32 {
    let temp_comp = calib.t_fine / 5120.0;
    let var1 = humidity_adc as f32
        - ((calib.par_h1 as f32 * 16.0) + ((calib.par_h3 as f32 / 2.0) * temp_comp));
    let var2 = var1
        * (calib.par_h2 as f32 / 262144.0)
        * (1.0
            + ((calib.par_h4 as f32 / 16384.0) * temp_comp)
            + ((calib.par_h5 as f32 / 1048576.0) * temp_comp * temp_comp));
    let var3 = calib.par_h6 as f32 / 16384.0;
    let var4 = calib.par_h7 as f32 / 2097152.0;
    (var2 + ((var3 + (var4 * temp_comp)) * var2 * var2)).clamp(0.0, 100.0)
}

fn calc_gas_resistance_low(gas_adc: u16, gas_range: u8, calib: &Calibration) -> f32 {
    let gas_range = gas_range as usize;
    let var1 = 1340.0 + (5.0 * calib.range_sw_err as f32);
    let var2 = var1 * (1.0 + GAS_LOOKUP_K1[gas_range] / 100.0);
    let var3 = 1.0 + (GAS_LOOKUP_K2[gas_range] / 100.0);
    let gas_range_factor = (1_u32 << gas_range) as f32;
    1.0 / (var3 * 0.000000125 * gas_range_factor * (((gas_adc as f32 - 512.0) / var2) + 1.0))
}

fn calc_gas_resistance_high(gas_adc: u16, gas_range: u8) -> f32 {
    let var1 = 262_144_u32 >> gas_range;
    let var2 = 4096 + ((i32::from(gas_adc) - 512) * 3);
    1_000_000.0 * var1 as f32 / var2 as f32
}

fn calc_res_heat(temp: u16, ambient_temp_c: i16, calib: &Calibration) -> u8 {
    let target = temp.min(400) as f32;
    let var1 = (calib.par_gh1 as f32 / 16.0) + 49.0;
    let var2 = ((calib.par_gh2 as f32 / 32768.0) * 0.0005) + 0.00235;
    let var3 = calib.par_gh3 as f32 / 1024.0;
    let var4 = var1 * (1.0 + (var2 * target));
    let var5 = var4 + (var3 * ambient_temp_c as f32);
    let heater = 3.4
        * ((var5
            * (4.0 / (4.0 + calib.res_heat_range as f32))
            * (1.0 / (1.0 + (calib.res_heat_val as f32 * 0.002))))
            - 25.0);
    heater as u8
}

fn calc_gas_wait(mut duration_ms: u16) -> u8 {
    let mut factor = 0;
    if duration_ms >= 0x0fc0 {
        return 0xff;
    }

    while duration_ms > 0x3f {
        duration_ms /= 4;
        factor += 1;
    }

    duration_ms as u8 + factor * 64
}
