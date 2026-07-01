use embedded_hal::i2c::I2c;

pub const BQ27220_ADDRESS: u8 = 0x55;
pub const AW32001_ADDRESS: u8 = 0x49;
pub const BQ27220_VOLTAGE: u8 = 0x08;
pub const BQ27220_CURRENT: u8 = 0x0c;
pub const BQ27220_REMAIN_CAPACITY: u8 = 0x10;
pub const BQ27220_FULL_CAPACITY: u8 = 0x12;
pub const AW32001_INPUT_SRC: u8 = 0x00;
pub const AW32001_POWER_ON_CFG: u8 = 0x01;
pub const AW32001_CHG_CURRENT: u8 = 0x02;
pub const AW32001_TERM_CURRENT: u8 = 0x03;
pub const AW32001_CHG_VOLTAGE: u8 = 0x04;
pub const AW32001_TIMER_WD: u8 = 0x05;
pub const AW32001_MAIN_CTRL: u8 = 0x06;
pub const AW32001_SYS_STATUS: u8 = 0x08;
// AW32001 charger register fields from the public register map used by Arduino's BSP.
const AW32001_CHARGE_DISABLE_BIT: u8 = 3;
const AW32001_HIZ_BIT: u8 = 4;
const AW32001_SHIP_MODE_BIT: u8 = 5;
const AW32001_WATCHDOG_RESET_BIT: u8 = 6;
const AW32001_VIN_DPM_MASK: u8 = 0b0111_1000;
const AW32001_VIN_DPM_SHIFT: u8 = 4;
const AW32001_VIN_DPM_MIN_MV: u16 = 3880;
const AW32001_VIN_DPM_MAX_MV: u16 = 5080;
const AW32001_VIN_DPM_STEP_MV: u16 = 80;
const AW32001_INPUT_CURRENT_MASK: u8 = 0b0000_1111;
const AW32001_INPUT_CURRENT_MIN_MA: u16 = 50;
const AW32001_INPUT_CURRENT_MAX_MA: u16 = 500;
const AW32001_INPUT_CURRENT_STEP_MA: u16 = 30;
const AW32001_UVLO_MASK: u8 = 0b0000_0111;
const AW32001_CHARGE_CURRENT_MASK: u8 = 0b0011_1111;
const AW32001_CHARGE_CURRENT_MIN_MA: u16 = 8;
const AW32001_CHARGE_CURRENT_MAX_MA: u16 = 456;
const AW32001_CHARGE_CURRENT_STEP_MA: u16 = 8;
const AW32001_DISCHARGE_CURRENT_MASK: u8 = 0b1111_0000;
const AW32001_DISCHARGE_CURRENT_SHIFT: u8 = 4;
const AW32001_DISCHARGE_CURRENT_MIN_MA: u16 = 200;
const AW32001_DISCHARGE_CURRENT_MAX_MA: u16 = 3200;
const AW32001_DISCHARGE_CURRENT_STEP_MA: u16 = 200;
const AW32001_CHARGE_VOLTAGE_MASK: u8 = 0b1111_1100;
const AW32001_CHARGE_VOLTAGE_SHIFT: u8 = 2;
const AW32001_CHARGE_VOLTAGE_MIN_MV: u16 = 3600;
const AW32001_CHARGE_VOLTAGE_MAX_MV: u16 = 4545;
const AW32001_CHARGE_VOLTAGE_STEP_MV: u16 = 15;
const AW32001_CHARGE_STATUS_SHIFT: u8 = 3;
const AW32001_CHARGE_STATUS_MASK: u8 = 0b11;
const AW32001_WATCHDOG_MASK: u8 = 0b11;
const AW32001_WATCHDOG_SHIFT: u8 = 5;

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChargeStatus {
    Unknown,
    Discharging,
    Charging,
    Full,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerState {
    Usb,
    Battery,
    Sleep,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatteryStatus {
    pub voltage_mv: u16,
    pub current_ma: i16,
    pub percentage: u8,
    pub charge: ChargeStatus,
}

/// Battery under-voltage lockout threshold for AW32001.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnderVoltageLockout {
    Mv2430 = 0,
    Mv2490 = 1,
    Mv2580 = 2,
    Mv2670 = 3,
    Mv2760 = 4,
    Mv2850 = 5,
    Mv2940 = 6,
    Mv3030 = 7,
}

/// AW32001 charging configuration.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChargingConfig {
    /// Charge current in mA, clamped to 8-456 mA in 8 mA steps.
    pub charge_current_ma: u16,
    /// Charge voltage in mV, clamped to 3600-4545 mV in 15 mV steps.
    pub charge_voltage_mv: u16,
    /// Battery under-voltage lockout threshold.
    pub uvlo: UnderVoltageLockout,
    /// Input voltage dynamic power-management limit in mV.
    ///
    /// Clamped to 3880-5080 mV in 80 mV steps.
    pub vin_dpm_mv: u16,
    /// Watchdog timeout in seconds: 0, 40, 80, or 160.
    pub watchdog_timeout_s: u8,
}

impl ChargingConfig {
    /// Default Nesso N1 charging configuration used by the official Arduino
    /// support library.
    pub const DEFAULT: Self = Self {
        charge_current_ma: 256,
        charge_voltage_mv: 4200,
        uvlo: UnderVoltageLockout::Mv2580,
        vin_dpm_mv: 4520,
        watchdog_timeout_s: 0,
    };
}

impl BatteryStatus {
    /// Returns true when battery percentage is at or below `threshold_percent`.
    #[must_use]
    pub const fn is_low(&self, threshold_percent: u8) -> bool {
        self.percentage <= threshold_percent
    }

    /// Returns true when the charger reports an active charging/full state.
    #[must_use]
    pub const fn has_external_power(&self) -> bool {
        matches!(self.charge, ChargeStatus::Charging | ChargeStatus::Full)
    }

    /// Returns a compact power state inferred from charger status.
    #[must_use]
    pub const fn power_state(&self) -> PowerState {
        match self.charge {
            ChargeStatus::Charging | ChargeStatus::Full => PowerState::Usb,
            ChargeStatus::Discharging | ChargeStatus::Unknown => PowerState::Battery,
        }
    }
}

pub struct Power<I2C> {
    i2c: I2C,
    fuel_gauge_address: u8,
    charger_address: u8,
}

impl<I2C> Power<I2C> {
    /// Creates a power-management reader on the shared Nesso N1 I2C bus.
    #[must_use]
    pub const fn new(i2c: I2C) -> Self {
        Self {
            i2c,
            fuel_gauge_address: BQ27220_ADDRESS,
            charger_address: AW32001_ADDRESS,
        }
    }

    /// Releases the wrapped I2C bus.
    pub fn release(self) -> I2C {
        self.i2c
    }
}

impl<I2C, E> Power<I2C>
where
    I2C: I2c<Error = E>,
{
    /// Reads battery voltage, current, percentage, and charge state.
    ///
    /// Fuel-gauge reads are required. If the charger status register cannot be
    /// read, the returned charge state is [`ChargeStatus::Unknown`].
    pub fn battery_status(&mut self) -> Result<BatteryStatus, E> {
        let voltage_mv = self.read_control_word(BQ27220_VOLTAGE)?;
        let current_ma = self.read_control_word(BQ27220_CURRENT)? as i16;
        let percentage = self.battery_percentage()?;
        let charge = match self.charge_status() {
            Ok(charge) => charge,
            Err(_) => ChargeStatus::Unknown,
        };
        Ok(BatteryStatus {
            voltage_mv,
            current_ma,
            percentage,
            charge,
        })
    }

    /// Reads battery voltage in millivolts.
    pub fn battery_voltage_mv(&mut self) -> Result<u16, E> {
        self.read_control_word(BQ27220_VOLTAGE)
    }

    /// Calculates battery percentage from remaining and full capacity registers.
    pub fn battery_percentage(&mut self) -> Result<u8, E> {
        let remaining = u32::from(self.read_control_word(BQ27220_REMAIN_CAPACITY)?);
        let full = u32::from(self.read_control_word(BQ27220_FULL_CAPACITY)?);
        if full == 0 {
            return Ok(0);
        }
        Ok(((remaining * 100) / full).min(100) as u8)
    }

    /// Returns true when battery percentage is at or below `threshold_percent`.
    pub fn is_low_battery(&mut self, threshold_percent: u8) -> Result<bool, E> {
        Ok(self.battery_percentage()? <= threshold_percent)
    }

    /// Reads charger state from the AW32001 status register.
    pub fn charge_status(&mut self) -> Result<ChargeStatus, E> {
        let status = self.read_charger_register(AW32001_SYS_STATUS)?;
        Ok(
            match (status >> AW32001_CHARGE_STATUS_SHIFT) & AW32001_CHARGE_STATUS_MASK {
                0 => ChargeStatus::Discharging,
                1 => ChargeStatus::Charging,
                2 => ChargeStatus::Charging,
                3 => ChargeStatus::Full,
                _ => ChargeStatus::Unknown,
            },
        )
    }

    /// Configures the charger with Nesso defaults and enables battery charging.
    pub fn begin_charging(&mut self) -> Result<(), E> {
        self.configure_charging(ChargingConfig::DEFAULT)
    }

    /// Configures the AW32001 charger and enables battery charging.
    pub fn configure_charging(&mut self, config: ChargingConfig) -> Result<(), E> {
        self.set_charge_current_ma(config.charge_current_ma)?;
        self.set_charge_voltage_mv(config.charge_voltage_mv)?;
        self.set_watchdog_timer_s(config.watchdog_timeout_s)?;
        self.set_battery_uvlo(config.uvlo)?;
        self.set_vin_dpm_voltage_mv(config.vin_dpm_mv)?;
        self.set_hiz(false)?;
        self.set_charge_enabled(true)
    }

    /// Enables battery charging on AW32001.
    pub fn enable_charge(&mut self) -> Result<(), E> {
        self.set_charge_enabled(true)
    }

    /// Enables or disables battery charging on AW32001.
    pub fn set_charge_enabled(&mut self, enable: bool) -> Result<(), E> {
        self.write_charger_bit(AW32001_POWER_ON_CFG, AW32001_CHARGE_DISABLE_BIT, !enable)
    }

    /// Sets the input voltage dynamic power-management limit.
    pub fn set_vin_dpm_voltage_mv(&mut self, voltage_mv: u16) -> Result<(), E> {
        let voltage_mv = voltage_mv.clamp(AW32001_VIN_DPM_MIN_MV, AW32001_VIN_DPM_MAX_MV);
        let mut input_src = self.read_charger_register(AW32001_INPUT_SRC)?;
        input_src &= !AW32001_VIN_DPM_MASK;
        input_src |= (((voltage_mv - AW32001_VIN_DPM_MIN_MV) / AW32001_VIN_DPM_STEP_MV) as u8)
            << AW32001_VIN_DPM_SHIFT;
        self.write_charger_register(AW32001_INPUT_SRC, input_src)
    }

    /// Sets the input current limit.
    pub fn set_input_current_limit_ma(&mut self, current_ma: u16) -> Result<(), E> {
        let current_ma =
            current_ma.clamp(AW32001_INPUT_CURRENT_MIN_MA, AW32001_INPUT_CURRENT_MAX_MA);
        let mut input_src = self.read_charger_register(AW32001_INPUT_SRC)?;
        input_src &= !AW32001_INPUT_CURRENT_MASK;
        input_src |= ((current_ma - AW32001_INPUT_CURRENT_MIN_MA) / AW32001_INPUT_CURRENT_STEP_MA)
            as u8
            & AW32001_INPUT_CURRENT_MASK;
        self.write_charger_register(AW32001_INPUT_SRC, input_src)
    }

    /// Sets the battery under-voltage lockout threshold.
    pub fn set_battery_uvlo(&mut self, uvlo: UnderVoltageLockout) -> Result<(), E> {
        let mut power_on_cfg = self.read_charger_register(AW32001_POWER_ON_CFG)?;
        power_on_cfg &= !AW32001_UVLO_MASK;
        power_on_cfg |= uvlo as u8 & AW32001_UVLO_MASK;
        self.write_charger_register(AW32001_POWER_ON_CFG, power_on_cfg)
    }

    /// Sets the charge current.
    pub fn set_charge_current_ma(&mut self, current_ma: u16) -> Result<(), E> {
        let current_ma =
            current_ma.clamp(AW32001_CHARGE_CURRENT_MIN_MA, AW32001_CHARGE_CURRENT_MAX_MA);
        let mut charge_current = self.read_charger_register(AW32001_CHG_CURRENT)?;
        charge_current &= !AW32001_CHARGE_CURRENT_MASK;
        charge_current |= ((current_ma - AW32001_CHARGE_CURRENT_MIN_MA)
            / AW32001_CHARGE_CURRENT_STEP_MA) as u8
            & AW32001_CHARGE_CURRENT_MASK;
        self.write_charger_register(AW32001_CHG_CURRENT, charge_current)
    }

    /// Sets the discharge current limit.
    pub fn set_discharge_current_ma(&mut self, current_ma: u16) -> Result<(), E> {
        let current_ma = current_ma.clamp(
            AW32001_DISCHARGE_CURRENT_MIN_MA,
            AW32001_DISCHARGE_CURRENT_MAX_MA,
        );
        let mut term_current = self.read_charger_register(AW32001_TERM_CURRENT)?;
        term_current &= !AW32001_DISCHARGE_CURRENT_MASK;
        term_current |= (((current_ma - AW32001_DISCHARGE_CURRENT_MIN_MA)
            / AW32001_DISCHARGE_CURRENT_STEP_MA) as u8
            & AW32001_INPUT_CURRENT_MASK)
            << AW32001_DISCHARGE_CURRENT_SHIFT;
        self.write_charger_register(AW32001_TERM_CURRENT, term_current)
    }

    /// Sets the battery charge voltage.
    pub fn set_charge_voltage_mv(&mut self, voltage_mv: u16) -> Result<(), E> {
        let voltage_mv =
            voltage_mv.clamp(AW32001_CHARGE_VOLTAGE_MIN_MV, AW32001_CHARGE_VOLTAGE_MAX_MV);
        let mut charge_voltage = self.read_charger_register(AW32001_CHG_VOLTAGE)?;
        charge_voltage &= !AW32001_CHARGE_VOLTAGE_MASK;
        charge_voltage |= (((voltage_mv - AW32001_CHARGE_VOLTAGE_MIN_MV)
            / AW32001_CHARGE_VOLTAGE_STEP_MV) as u8)
            << AW32001_CHARGE_VOLTAGE_SHIFT;
        self.write_charger_register(AW32001_CHG_VOLTAGE, charge_voltage)
    }

    /// Sets the charger watchdog timeout.
    ///
    /// Unsupported values select 160 seconds, matching the official Arduino
    /// support library behavior.
    pub fn set_watchdog_timer_s(&mut self, seconds: u8) -> Result<(), E> {
        let bits = match seconds {
            0 => 0b00,
            40 => 0b01,
            80 => 0b10,
            160 => 0b11,
            _ => 0b11,
        };
        let mut timer_watchdog = self.read_charger_register(AW32001_TIMER_WD)?;
        timer_watchdog &= !(AW32001_WATCHDOG_MASK << AW32001_WATCHDOG_SHIFT);
        timer_watchdog |= bits << AW32001_WATCHDOG_SHIFT;
        self.write_charger_register(AW32001_TIMER_WD, timer_watchdog)
    }

    /// Feeds the AW32001 watchdog.
    pub fn feed_watchdog(&mut self) -> Result<(), E> {
        self.write_charger_bit(AW32001_CHG_CURRENT, AW32001_WATCHDOG_RESET_BIT, true)
    }

    /// Enables or disables AW32001 ship mode.
    pub fn set_ship_mode(&mut self, enable: bool) -> Result<(), E> {
        self.write_charger_bit(AW32001_MAIN_CTRL, AW32001_SHIP_MODE_BIT, enable)
    }

    /// Enables or disables AW32001 high-impedance input mode.
    pub fn set_hiz(&mut self, enable: bool) -> Result<(), E> {
        self.write_charger_bit(AW32001_POWER_ON_CFG, AW32001_HIZ_BIT, enable)
    }

    fn read_control_word(&mut self, command: u8) -> Result<u16, E> {
        let low = self.read_fuel_gauge_register(command)?;
        let high = self.read_fuel_gauge_register(command + 1)?;
        Ok(u16::from_le_bytes([low, high]))
    }

    fn read_fuel_gauge_register(&mut self, command: u8) -> Result<u8, E> {
        let mut gauge_byte = [0u8; 1];
        self.i2c
            .write_read(self.fuel_gauge_address, &[command], &mut gauge_byte)?;
        Ok(gauge_byte[0])
    }

    fn read_charger_register(&mut self, command: u8) -> Result<u8, E> {
        let mut charger_byte = [0u8; 1];
        self.i2c
            .write_read(self.charger_address, &[command], &mut charger_byte)?;
        Ok(charger_byte[0])
    }

    fn write_charger_register(&mut self, command: u8, value: u8) -> Result<(), E> {
        self.i2c.write(self.charger_address, &[command, value])
    }

    fn write_charger_bit(&mut self, command: u8, bit: u8, high: bool) -> Result<(), E> {
        let mut register_byte = self.read_charger_register(command)?;
        if high {
            register_byte |= 1u8 << bit;
        } else {
            register_byte &= !(1u8 << bit);
        }
        self.write_charger_register(command, register_byte)
    }
}
