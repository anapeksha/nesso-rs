use embedded_hal::i2c::I2c;

pub const BQ27220_ADDRESS: u8 = 0x55;
pub const AW32001_ADDRESS: u8 = 0x49;
pub const BQ27220_VOLTAGE: u8 = 0x08;
pub const BQ27220_CURRENT: u8 = 0x0c;
pub const BQ27220_REMAIN_CAPACITY: u8 = 0x10;
pub const BQ27220_FULL_CAPACITY: u8 = 0x12;
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
    pub fn battery_status(&mut self) -> Result<BatteryStatus, E> {
        let voltage_mv = self.read_control_word(0x08)?;
        let current_ma = self.read_control_word(BQ27220_CURRENT)? as i16;
        let percentage = self.battery_percentage()?;
        Ok(BatteryStatus {
            voltage_mv,
            current_ma,
            percentage,
            charge: self.charge_status()?,
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
            1 => ChargeStatus::Discharging,
            2 => ChargeStatus::Charging,
            3 => ChargeStatus::Full,
            _ => ChargeStatus::Unknown,
        })
    }

    fn read_control_word(&mut self, command: u8) -> Result<u16, E> {
        let mut data = [0u8; 2];
        self.i2c
            .write_read(self.fuel_gauge_address, &[command], &mut data)?;
        Ok(u16::from_le_bytes(data))
    }

    fn read_charger_register(&mut self, command: u8) -> Result<u8, E> {
        let mut data = [0u8; 1];
        self.i2c
            .write_read(self.charger_address, &[command], &mut data)?;
        Ok(data[0])
    }
}
