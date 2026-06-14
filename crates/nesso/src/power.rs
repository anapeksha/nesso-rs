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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChargeStatus {
    Unknown,
    Discharging,
    Charging,
    Full,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerState {
    Usb,
    Battery,
    Sleep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatteryStatus {
    pub voltage_mv: u16,
    pub current_ma: i16,
    pub percentage: u8,
    pub charge: ChargeStatus,
}

/// Battery under-voltage lockout threshold for AW32001.
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
        Ok(match (status >> 3) & 0b11 {
            0 => ChargeStatus::Discharging,
            1 => ChargeStatus::Charging,
            2 => ChargeStatus::Charging,
            3 => ChargeStatus::Full,
            _ => ChargeStatus::Unknown,
        })
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
        self.write_charger_bit(AW32001_POWER_ON_CFG, 3, !enable)
    }

    /// Sets the input voltage dynamic power-management limit.
    pub fn set_vin_dpm_voltage_mv(&mut self, voltage_mv: u16) -> Result<(), E> {
        let voltage_mv = voltage_mv.clamp(3880, 5080);
        let mut value = self.read_charger_register(AW32001_INPUT_SRC)?;
        value &= !0b0111_1000;
        value |= (((voltage_mv - 3880) / 80) as u8) << 4;
        self.write_charger_register(AW32001_INPUT_SRC, value)
    }

    /// Sets the input current limit.
    pub fn set_input_current_limit_ma(&mut self, current_ma: u16) -> Result<(), E> {
        let current_ma = current_ma.clamp(50, 500);
        let mut value = self.read_charger_register(AW32001_INPUT_SRC)?;
        value &= !0b0000_1111;
        value |= ((current_ma - 50) / 30) as u8 & 0b0000_1111;
        self.write_charger_register(AW32001_INPUT_SRC, value)
    }

    /// Sets the battery under-voltage lockout threshold.
    pub fn set_battery_uvlo(&mut self, uvlo: UnderVoltageLockout) -> Result<(), E> {
        let mut value = self.read_charger_register(AW32001_POWER_ON_CFG)?;
        value &= !0b0000_0111;
        value |= uvlo as u8 & 0b0000_0111;
        self.write_charger_register(AW32001_POWER_ON_CFG, value)
    }

    /// Sets the charge current.
    pub fn set_charge_current_ma(&mut self, current_ma: u16) -> Result<(), E> {
        let current_ma = current_ma.clamp(8, 456);
        let mut value = self.read_charger_register(AW32001_CHG_CURRENT)?;
        value &= !0b0011_1111;
        value |= ((current_ma - 8) / 8) as u8 & 0b0011_1111;
        self.write_charger_register(AW32001_CHG_CURRENT, value)
    }

    /// Sets the discharge current limit.
    pub fn set_discharge_current_ma(&mut self, current_ma: u16) -> Result<(), E> {
        let current_ma = current_ma.clamp(200, 3200);
        let mut value = self.read_charger_register(AW32001_TERM_CURRENT)?;
        value &= !0b1111_0000;
        value |= (((current_ma - 200) / 200) as u8 & 0b0000_1111) << 4;
        self.write_charger_register(AW32001_TERM_CURRENT, value)
    }

    /// Sets the battery charge voltage.
    pub fn set_charge_voltage_mv(&mut self, voltage_mv: u16) -> Result<(), E> {
        let voltage_mv = voltage_mv.clamp(3600, 4545);
        let mut value = self.read_charger_register(AW32001_CHG_VOLTAGE)?;
        value &= !0b1111_1100;
        value |= (((voltage_mv - 3600) / 15) as u8) << 2;
        self.write_charger_register(AW32001_CHG_VOLTAGE, value)
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
        let mut value = self.read_charger_register(AW32001_TIMER_WD)?;
        value &= !(0b11 << 5);
        value |= bits << 5;
        self.write_charger_register(AW32001_TIMER_WD, value)
    }

    /// Feeds the AW32001 watchdog.
    pub fn feed_watchdog(&mut self) -> Result<(), E> {
        self.write_charger_bit(AW32001_CHG_CURRENT, 6, true)
    }

    /// Enables or disables AW32001 ship mode.
    pub fn set_ship_mode(&mut self, enable: bool) -> Result<(), E> {
        self.write_charger_bit(AW32001_MAIN_CTRL, 5, enable)
    }

    /// Enables or disables AW32001 high-impedance input mode.
    pub fn set_hiz(&mut self, enable: bool) -> Result<(), E> {
        self.write_charger_bit(AW32001_POWER_ON_CFG, 4, enable)
    }

    fn read_control_word(&mut self, command: u8) -> Result<u16, E> {
        let low = self.read_fuel_gauge_register(command)?;
        let high = self.read_fuel_gauge_register(command + 1)?;
        Ok(u16::from_le_bytes([low, high]))
    }

    fn read_fuel_gauge_register(&mut self, command: u8) -> Result<u8, E> {
        let mut data = [0u8; 1];
        self.i2c
            .write_read(self.fuel_gauge_address, &[command], &mut data)?;
        Ok(data[0])
    }

    fn read_charger_register(&mut self, command: u8) -> Result<u8, E> {
        let mut data = [0u8; 1];
        self.i2c
            .write_read(self.charger_address, &[command], &mut data)?;
        Ok(data[0])
    }

    fn write_charger_register(&mut self, command: u8, value: u8) -> Result<(), E> {
        self.i2c.write(self.charger_address, &[command, value])
    }

    fn write_charger_bit(&mut self, command: u8, bit: u8, high: bool) -> Result<(), E> {
        let mut value = self.read_charger_register(command)?;
        if high {
            value |= 1u8 << bit;
        } else {
            value &= !(1u8 << bit);
        }
        self.write_charger_register(command, value)
    }
}
