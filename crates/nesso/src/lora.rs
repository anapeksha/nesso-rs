//! SX1262 LoRa support for the onboard Nesso N1 radio.
//!
//! Constructing the driver does not transmit. Applications must attach the
//! external LoRa antenna before calling `Sx1262::transmit`.

#[cfg(not(nesso_host_tests))]
use crate::bsp::{NessoI2c, NessoLoraSpiDevice};
use embedded_hal::{
    digital::InputPin,
    i2c::I2c,
    spi::{Operation, SpiDevice},
};
#[cfg(not(nesso_host_tests))]
use esp_hal::gpio::Input;

/// Concrete onboard Nesso N1 SX1262 driver.
#[cfg(not(nesso_host_tests))]
pub type NessoLora = Sx1262<NessoLoraSpiDevice, NessoI2c, Input<'static>, Input<'static>>;

/// Maximum LoRa payload size supported by SX1262 buffer mode.
pub const MAX_LORA_PAYLOAD_LEN: usize = 255;
/// Lowest RF frequency documented for the onboard Nesso N1 SX1262 path.
pub const MIN_FREQUENCY_HZ: u32 = 850_000_000;
/// Highest RF frequency documented for the onboard Nesso N1 SX1262 path.
pub const MAX_FREQUENCY_HZ: u32 = 960_000_000;

const EXPANDER_0_ADDRESS: u8 = 0x43;
const EXPANDER_OUTPUT_ENABLE: u8 = 0x03;
const EXPANDER_OUTPUT_STATE: u8 = 0x05;
const EXPANDER_HIGH_IMPEDANCE: u8 = 0x07;
const EXPANDER_DEFAULT_OUTPUT: u8 = 0x09;
const EXPANDER_INTERRUPT_MASK: u8 = 0x11;
const EXPANDER_GLOBAL_CONTROL: u8 = 0x01;
const EXPANDER_GLOBAL_CONTROL_ENABLE: u8 = 0x01;
const LORA_LNA_ENABLE_PIN: u8 = 5;
const LORA_ANTENNA_SWITCH_PIN: u8 = 6;
const LORA_ENABLE_PIN: u8 = 7;
const SX126X_XTAL_HZ: u64 = 32_000_000;
const RF_FREQUENCY_SCALE: u64 = 1 << 25;
const BUSY_WAIT_POLLS: usize = 200_000;

const CMD_GET_STATUS: u8 = 0xC0;
const CMD_SET_SLEEP: u8 = 0x84;
const CMD_SET_STANDBY: u8 = 0x80;
const CMD_SET_PACKET_TYPE: u8 = 0x8A;
const CMD_SET_RF_FREQUENCY: u8 = 0x86;
const CMD_SET_PA_CONFIG: u8 = 0x95;
const CMD_SET_TX_PARAMS: u8 = 0x8E;
const CMD_SET_BUFFER_BASE_ADDRESS: u8 = 0x8F;
const CMD_WRITE_BUFFER: u8 = 0x0E;
const CMD_READ_BUFFER: u8 = 0x1E;
const CMD_SET_MODULATION_PARAMS: u8 = 0x8B;
const CMD_SET_PACKET_PARAMS: u8 = 0x8C;
const CMD_SET_DIO_IRQ_PARAMS: u8 = 0x08;
const CMD_GET_IRQ_STATUS: u8 = 0x12;
const CMD_CLEAR_IRQ_STATUS: u8 = 0x02;
const CMD_SET_DIO2_AS_RF_SWITCH: u8 = 0x9D;
const CMD_SET_TX: u8 = 0x83;
const CMD_SET_RX: u8 = 0x82;
const CMD_GET_RX_BUFFER_STATUS: u8 = 0x13;
const CMD_GET_PACKET_STATUS: u8 = 0x14;
const CMD_GET_RSSI_INST: u8 = 0x15;

const PACKET_TYPE_LORA: u8 = 0x01;
const STANDBY_RC: u8 = 0x00;
const HEADER_EXPLICIT: u8 = 0x00;
const HEADER_IMPLICIT: u8 = 0x01;
const CRC_OFF: u8 = 0x00;
const CRC_ON: u8 = 0x01;
const IQ_STANDARD: u8 = 0x00;
const IQ_INVERTED: u8 = 0x01;
const IRQ_TX_DONE: u16 = 1 << 0;
const IRQ_RX_DONE: u16 = 1 << 1;
const IRQ_TIMEOUT: u16 = 1 << 9;
const TX_BASE: u8 = 0;
const RX_BASE: u8 = 128;
const RX_CONTINUOUS_TIMEOUT: [u8; 3] = [0xFF, 0xFF, 0xFF];
const SX126X_SPI_DUMMY: u8 = 0x00;
const SLEEP_COLD_START: u8 = 0x00;
const SLEEP_WARM_START: u8 = 0x04;
const DIO2_RF_SWITCH_ENABLE: u8 = 0x01;
// SX1262 high-power PA settings, datasheet SetPaConfig command.
const PA_DUTY_CYCLE_HIGH_POWER: u8 = 0x04;
const PA_HP_MAX_SX1262: u8 = 0x07;
const PA_DEVICE_SEL_SX1262: u8 = 0x00;
const PA_LUT_RESERVED: u8 = 0x01;
const TX_RAMP_200_US: u8 = 0x04;
const IRQ_TX_RX_MASK: u16 = IRQ_TX_DONE | IRQ_RX_DONE;
const IRQ_DIO_DISABLED: u16 = 0;
const LOW_DATA_RATE_OPTIMIZE_ON: u8 = 0x01;
const LOW_DATA_RATE_OPTIMIZE_OFF: u8 = 0x00;

/// Errors returned by the SX1262 driver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoraError<SpiError, I2cError, PinError> {
    /// SPI transaction failed.
    Spi(SpiError),
    /// I2C transaction failed while controlling Nesso RF frontend pins.
    I2c(I2cError),
    /// BUSY or IRQ pin read failed.
    Pin(PinError),
    /// SX1262 BUSY did not clear within the bounded wait.
    BusyTimeout,
    /// Transmit or receive was requested before [`Sx1262::configure`] succeeded.
    NotConfigured,
    /// Frequency is outside the documented Nesso N1 850-960 MHz RF range.
    InvalidFrequency,
    /// Spreading factor must be in the SX1262 LoRa range 5 through 12.
    InvalidSpreadingFactor,
    /// Payload exceeded the SX1262 buffer capacity.
    PayloadTooLong,
    /// Requested output power is outside the supported SX1262 high-power range.
    InvalidOutputPower,
}

/// LoRa signal bandwidth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bandwidth {
    Bw7,
    Bw10,
    Bw15,
    Bw20,
    Bw31,
    Bw41,
    Bw62,
    Bw125,
    Bw250,
    Bw500,
}

impl Bandwidth {
    const fn register_value(self) -> u8 {
        match self {
            Self::Bw7 => 0x00,
            Self::Bw10 => 0x08,
            Self::Bw15 => 0x01,
            Self::Bw20 => 0x09,
            Self::Bw31 => 0x02,
            Self::Bw41 => 0x0A,
            Self::Bw62 => 0x03,
            Self::Bw125 => 0x04,
            Self::Bw250 => 0x05,
            Self::Bw500 => 0x06,
        }
    }
}

/// LoRa coding rate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodingRate {
    Cr45,
    Cr46,
    Cr47,
    Cr48,
}

impl CodingRate {
    const fn register_value(self) -> u8 {
        match self {
            Self::Cr45 => 0x01,
            Self::Cr46 => 0x02,
            Self::Cr47 => 0x03,
            Self::Cr48 => 0x04,
        }
    }
}

/// SX1262 packet header mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeaderMode {
    Explicit,
    Implicit,
}

impl HeaderMode {
    const fn register_value(self) -> u8 {
        match self {
            Self::Explicit => HEADER_EXPLICIT,
            Self::Implicit => HEADER_IMPLICIT,
        }
    }
}

/// LoRa PHY configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoraConfig {
    /// RF frequency in hertz. Must be in the Nesso N1 documented 850-960 MHz range.
    pub frequency_hz: u32,
    /// Spreading factor, 5 through 12.
    pub spreading_factor: u8,
    /// Signal bandwidth.
    pub bandwidth: Bandwidth,
    /// Coding rate.
    pub coding_rate: CodingRate,
    /// Preamble length in symbols.
    pub preamble_len: u16,
    /// Packet header mode.
    pub header_mode: HeaderMode,
    /// CRC enabled.
    pub crc: bool,
    /// Invert IQ.
    pub invert_iq: bool,
    /// TX output power in dBm. SX1262 high-power path supports up to +22 dBm.
    pub output_power_dbm: i8,
}

impl LoraConfig {
    /// Creates a conservative LoRa config with explicit frequency selection.
    #[must_use]
    pub const fn new(frequency_hz: u32) -> Self {
        Self {
            frequency_hz,
            spreading_factor: 7,
            bandwidth: Bandwidth::Bw125,
            coding_rate: CodingRate::Cr45,
            preamble_len: 8,
            header_mode: HeaderMode::Explicit,
            crc: true,
            invert_iq: false,
            output_power_dbm: 14,
        }
    }

    /// Validates frequency, spreading factor, and output power ranges.
    pub const fn validate(&self) -> Result<(), LoraConfigError> {
        if self.frequency_hz < MIN_FREQUENCY_HZ || self.frequency_hz > MAX_FREQUENCY_HZ {
            return Err(LoraConfigError::InvalidFrequency);
        }
        if self.spreading_factor < 5 || self.spreading_factor > 12 {
            return Err(LoraConfigError::InvalidSpreadingFactor);
        }
        if self.output_power_dbm < -9 || self.output_power_dbm > 22 {
            return Err(LoraConfigError::InvalidOutputPower);
        }
        Ok(())
    }
}

/// Errors returned while validating LoRa configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoraConfigError {
    InvalidFrequency,
    InvalidSpreadingFactor,
    InvalidOutputPower,
}

/// Packet metadata returned after receive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketStatus {
    /// Packet RSSI in dBm.
    pub rssi_dbm: i16,
    /// Packet SNR in dB.
    pub snr_db: i8,
    /// Signal RSSI in dBm.
    pub signal_rssi_dbm: i16,
}

/// Receive result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReceivedPacket {
    /// Number of bytes copied into the caller buffer.
    pub len: usize,
    /// Packet signal metadata.
    pub status: PacketStatus,
}

/// SX1262 receive timeout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiveTimeout {
    /// Continuous receive mode.
    Continuous,
    /// Single receive window in SX1262 timeout ticks.
    ///
    /// The SX1262 command uses a 24-bit timeout field. One tick is 15.625 us.
    Ticks(u32),
}

impl ReceiveTimeout {
    /// Returns the three SX1262 command bytes for this timeout.
    #[must_use]
    pub const fn command_bytes(self) -> [u8; 3] {
        match self {
            Self::Continuous => RX_CONTINUOUS_TIMEOUT,
            Self::Ticks(ticks) => {
                let bounded = ticks & 0x00FF_FFFF;
                [(bounded >> 16) as u8, (bounded >> 8) as u8, bounded as u8]
            }
        }
    }
}

/// Onboard SX1262 driver.
pub struct Sx1262<SPI, I2C, BUSY, IRQ> {
    spi: SPI,
    i2c: I2C,
    busy: BUSY,
    irq: IRQ,
    config: Option<LoraConfig>,
}

impl<SPI, I2C, BUSY, IRQ> Sx1262<SPI, I2C, BUSY, IRQ> {
    /// Creates a driver without touching or transmitting from the chip.
    #[must_use]
    pub const fn new_nesso(spi: SPI, i2c: I2C, busy: BUSY, irq: IRQ) -> Self {
        Self {
            spi,
            i2c,
            busy,
            irq,
            config: None,
        }
    }

    /// Releases the wrapped SPI, I2C, BUSY, and IRQ resources.
    pub fn release(self) -> (SPI, I2C, BUSY, IRQ) {
        (self.spi, self.i2c, self.busy, self.irq)
    }

    /// Returns true after [`Self::configure`] succeeds.
    #[must_use]
    pub const fn is_configured(&self) -> bool {
        self.config.is_some()
    }
}

impl<SPI, I2C, BUSY, IRQ, SpiError, I2cError, PinError> Sx1262<SPI, I2C, BUSY, IRQ>
where
    SPI: SpiDevice<u8, Error = SpiError>,
    I2C: I2c<Error = I2cError>,
    BUSY: InputPin<Error = PinError>,
    IRQ: InputPin<Error = PinError>,
{
    /// Powers the RF frontend and resets the SX1262 without transmitting.
    pub fn begin(&mut self) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        self.configure_frontend_outputs()?;
        self.set_reset(false)?;
        self.set_reset(true)?;
        self.set_antenna_switch(true)?;
        self.set_lna(true)?;
        self.wait_ready()?;
        self.set_standby()
    }

    /// Places the SX1262 in standby after it has been powered/reset.
    pub fn standby(&mut self) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        self.set_standby()
    }

    /// Places the SX1262 in sleep mode.
    ///
    /// This does not disable the Nesso RF frontend expander pins. Recreate the
    /// driver or reset the chip before reconfiguring after cold sleep.
    pub fn sleep(
        &mut self,
        warm_start: bool,
    ) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        let config = if warm_start {
            SLEEP_WARM_START
        } else {
            SLEEP_COLD_START
        };
        self.write_command(&[CMD_SET_SLEEP, config])
    }

    /// Reads the SX1262 status byte.
    pub fn status(&mut self) -> Result<u8, LoraError<SpiError, I2cError, PinError>> {
        self.wait_ready()?;
        let command = [CMD_GET_STATUS];
        let mut status = [0];
        self.spi
            .transaction(&mut [Operation::Write(&command), Operation::Read(&mut status)])
            .map_err(LoraError::Spi)?;
        Ok(status[0])
    }

    /// Configures LoRa modulation and packet parameters without transmitting.
    pub fn configure(
        &mut self,
        config: LoraConfig,
    ) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        config.validate().map_err(|error| match error {
            LoraConfigError::InvalidFrequency => LoraError::InvalidFrequency,
            LoraConfigError::InvalidSpreadingFactor => LoraError::InvalidSpreadingFactor,
            LoraConfigError::InvalidOutputPower => LoraError::InvalidOutputPower,
        })?;
        self.begin()?;
        self.write_command(&[CMD_SET_PACKET_TYPE, PACKET_TYPE_LORA])?;
        self.write_command(&[CMD_SET_DIO2_AS_RF_SWITCH, DIO2_RF_SWITCH_ENABLE])?;
        self.set_frequency(config.frequency_hz)?;
        self.write_command(&[
            CMD_SET_PA_CONFIG,
            PA_DUTY_CYCLE_HIGH_POWER,
            PA_HP_MAX_SX1262,
            PA_DEVICE_SEL_SX1262,
            PA_LUT_RESERVED,
        ])?;
        self.write_command(&[
            CMD_SET_TX_PARAMS,
            config.output_power_dbm as u8,
            TX_RAMP_200_US,
        ])?;
        self.write_command(&[CMD_SET_BUFFER_BASE_ADDRESS, TX_BASE, RX_BASE])?;
        self.write_command(&[
            CMD_SET_MODULATION_PARAMS,
            config.spreading_factor,
            config.bandwidth.register_value(),
            config.coding_rate.register_value(),
            low_data_rate_optimize(config),
        ])?;
        self.set_packet_params(config, MAX_LORA_PAYLOAD_LEN as u8)?;
        let irq_mask = IRQ_TX_RX_MASK.to_be_bytes();
        let disabled_irq = IRQ_DIO_DISABLED.to_be_bytes();
        self.write_command(&[
            CMD_SET_DIO_IRQ_PARAMS,
            irq_mask[0],
            irq_mask[1],
            irq_mask[0],
            irq_mask[1],
            disabled_irq[0],
            disabled_irq[1],
            disabled_irq[0],
            disabled_irq[1],
        ])?;
        self.clear_irq(u16::MAX)?;
        self.config = Some(config);
        Ok(())
    }

    /// Transmits one LoRa packet.
    ///
    /// Attach the external LoRa antenna before calling this method.
    pub fn transmit(
        &mut self,
        payload: &[u8],
    ) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        if payload.len() > MAX_LORA_PAYLOAD_LEN {
            return Err(LoraError::PayloadTooLong);
        }
        let config = self.config.ok_or(LoraError::NotConfigured)?;
        self.set_lna(false)?;
        self.clear_irq(u16::MAX)?;
        self.write_buffer(TX_BASE, payload)?;
        self.set_packet_params(config, payload.len() as u8)?;
        self.write_command(&[
            CMD_SET_TX,
            SX126X_SPI_DUMMY,
            SX126X_SPI_DUMMY,
            SX126X_SPI_DUMMY,
        ])
    }

    /// Starts continuous receive mode.
    pub fn start_receive(&mut self) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        self.start_receive_with_timeout(ReceiveTimeout::Continuous)
    }

    /// Starts receive mode with a bounded or continuous SX1262 timeout.
    pub fn start_receive_with_timeout(
        &mut self,
        timeout: ReceiveTimeout,
    ) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        if self.config.is_none() {
            return Err(LoraError::NotConfigured);
        }
        self.set_lna(true)?;
        self.clear_irq(u16::MAX)?;
        let timeout = timeout.command_bytes();
        self.write_command(&[CMD_SET_RX, timeout[0], timeout[1], timeout[2]])
    }

    /// Returns true when SX1262 DIO1 is asserted.
    pub fn irq_asserted(&mut self) -> Result<bool, LoraError<SpiError, I2cError, PinError>> {
        self.irq.is_high().map_err(LoraError::Pin)
    }

    /// Reads and clears IRQ status.
    pub fn irq_status(&mut self) -> Result<u16, LoraError<SpiError, I2cError, PinError>> {
        self.wait_ready()?;
        let command = [CMD_GET_IRQ_STATUS, SX126X_SPI_DUMMY, SX126X_SPI_DUMMY];
        let mut out = [0; 3];
        self.spi
            .transaction(&mut [Operation::Write(&command), Operation::Read(&mut out)])
            .map_err(LoraError::Spi)?;
        Ok(u16::from_be_bytes([out[1], out[2]]))
    }

    /// Reads one received packet when RX_DONE is set.
    pub fn read_packet(
        &mut self,
        output: &mut [u8],
    ) -> Result<Option<ReceivedPacket>, LoraError<SpiError, I2cError, PinError>> {
        let irq = self.irq_status()?;
        if irq & IRQ_TIMEOUT != 0 {
            self.clear_irq(irq)?;
            return Ok(None);
        }
        if irq & IRQ_RX_DONE == 0 {
            return Ok(None);
        }

        let (len, offset) = self.rx_buffer_status()?;
        let copy_len = output.len().min(usize::from(len));
        self.read_buffer(offset, &mut output[..copy_len])?;
        let status = self.packet_status()?;
        self.clear_irq(irq | IRQ_RX_DONE)?;
        Ok(Some(ReceivedPacket {
            len: copy_len,
            status,
        }))
    }

    /// Returns instantaneous RSSI in dBm.
    pub fn rssi_dbm(&mut self) -> Result<i16, LoraError<SpiError, I2cError, PinError>> {
        self.wait_ready()?;
        let command = [CMD_GET_RSSI_INST, SX126X_SPI_DUMMY];
        let mut out = [0; 2];
        self.spi
            .transaction(&mut [Operation::Write(&command), Operation::Read(&mut out)])
            .map_err(LoraError::Spi)?;
        Ok(-(i16::from(out[1]) / 2))
    }

    /// Returns true when the TX_DONE IRQ bit is set.
    pub fn tx_done(&mut self) -> Result<bool, LoraError<SpiError, I2cError, PinError>> {
        Ok(self.irq_status()? & IRQ_TX_DONE != 0)
    }

    fn set_standby(&mut self) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        self.write_command(&[CMD_SET_STANDBY, STANDBY_RC])
    }

    fn set_frequency(
        &mut self,
        frequency_hz: u32,
    ) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        if !(MIN_FREQUENCY_HZ..=MAX_FREQUENCY_HZ).contains(&frequency_hz) {
            return Err(LoraError::InvalidFrequency);
        }
        let register = ((u64::from(frequency_hz) * RF_FREQUENCY_SCALE) / SX126X_XTAL_HZ) as u32;
        let bytes = register.to_be_bytes();
        self.write_command(&[CMD_SET_RF_FREQUENCY, bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    fn set_packet_params(
        &mut self,
        config: LoraConfig,
        payload_len: u8,
    ) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        self.write_command(&[
            CMD_SET_PACKET_PARAMS,
            (config.preamble_len >> 8) as u8,
            config.preamble_len as u8,
            config.header_mode.register_value(),
            payload_len,
            if config.crc { CRC_ON } else { CRC_OFF },
            if config.invert_iq {
                IQ_INVERTED
            } else {
                IQ_STANDARD
            },
        ])
    }

    fn write_buffer(
        &mut self,
        offset: u8,
        payload: &[u8],
    ) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        if payload.len() > MAX_LORA_PAYLOAD_LEN {
            return Err(LoraError::PayloadTooLong);
        }
        self.wait_ready()?;
        let command = [CMD_WRITE_BUFFER, offset];
        self.spi
            .transaction(&mut [Operation::Write(&command), Operation::Write(payload)])
            .map_err(LoraError::Spi)
    }

    fn read_buffer(
        &mut self,
        offset: u8,
        output: &mut [u8],
    ) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        self.wait_ready()?;
        let command = [CMD_READ_BUFFER, offset, SX126X_SPI_DUMMY];
        self.spi
            .transaction(&mut [Operation::Write(&command), Operation::Read(output)])
            .map_err(LoraError::Spi)
    }

    fn rx_buffer_status(&mut self) -> Result<(u8, u8), LoraError<SpiError, I2cError, PinError>> {
        self.wait_ready()?;
        let command = [CMD_GET_RX_BUFFER_STATUS, SX126X_SPI_DUMMY, SX126X_SPI_DUMMY];
        let mut out = [0; 3];
        self.spi
            .transaction(&mut [Operation::Write(&command), Operation::Read(&mut out)])
            .map_err(LoraError::Spi)?;
        Ok((out[1], out[2]))
    }

    fn packet_status(&mut self) -> Result<PacketStatus, LoraError<SpiError, I2cError, PinError>> {
        self.wait_ready()?;
        let command = [
            CMD_GET_PACKET_STATUS,
            SX126X_SPI_DUMMY,
            SX126X_SPI_DUMMY,
            SX126X_SPI_DUMMY,
        ];
        let mut out = [0; 4];
        self.spi
            .transaction(&mut [Operation::Write(&command), Operation::Read(&mut out)])
            .map_err(LoraError::Spi)?;
        Ok(PacketStatus {
            rssi_dbm: -(i16::from(out[1]) / 2),
            snr_db: (out[2] as i8) / 4,
            signal_rssi_dbm: -(i16::from(out[3]) / 2),
        })
    }

    fn clear_irq(&mut self, mask: u16) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        let bytes = mask.to_be_bytes();
        self.write_command(&[CMD_CLEAR_IRQ_STATUS, bytes[0], bytes[1]])
    }

    fn write_command(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        self.wait_ready()?;
        self.spi
            .transaction(&mut [Operation::Write(bytes)])
            .map_err(LoraError::Spi)
    }

    fn wait_ready(&mut self) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        for _ in 0..BUSY_WAIT_POLLS {
            if !self.busy.is_high().map_err(LoraError::Pin)? {
                return Ok(());
            }
        }
        Err(LoraError::BusyTimeout)
    }

    fn configure_frontend_outputs(
        &mut self,
    ) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        let address = EXPANDER_0_ADDRESS;
        let _discarded = self.read_register(address, EXPANDER_GLOBAL_CONTROL)?;
        self.write_register(
            address,
            EXPANDER_GLOBAL_CONTROL,
            EXPANDER_GLOBAL_CONTROL_ENABLE,
        )?;
        self.write_register(address, EXPANDER_DEFAULT_OUTPUT, 0xFF)?;
        self.write_register(address, EXPANDER_INTERRUPT_MASK, 0xFF)?;
        self.configure_expander_output(address, LORA_ENABLE_PIN)?;
        self.configure_expander_output(address, LORA_ANTENNA_SWITCH_PIN)?;
        self.configure_expander_output(address, LORA_LNA_ENABLE_PIN)
    }

    fn set_reset(&mut self, high: bool) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        self.write_expander_bit(EXPANDER_0_ADDRESS, LORA_ENABLE_PIN, high)
    }

    fn set_antenna_switch(
        &mut self,
        high: bool,
    ) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        self.write_expander_bit(EXPANDER_0_ADDRESS, LORA_ANTENNA_SWITCH_PIN, high)
    }

    fn set_lna(&mut self, high: bool) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        self.write_expander_bit(EXPANDER_0_ADDRESS, LORA_LNA_ENABLE_PIN, high)
    }

    fn configure_expander_output(
        &mut self,
        address: u8,
        pin: u8,
    ) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        let mut output_enable = self.read_register(address, EXPANDER_OUTPUT_ENABLE)?;
        output_enable |= 1u8 << pin;
        self.write_register(address, EXPANDER_OUTPUT_ENABLE, output_enable)?;

        let mut high_impedance = self.read_register(address, EXPANDER_HIGH_IMPEDANCE)?;
        high_impedance &= !(1u8 << pin);
        self.write_register(address, EXPANDER_HIGH_IMPEDANCE, high_impedance)
    }

    fn write_expander_bit(
        &mut self,
        address: u8,
        pin: u8,
        high: bool,
    ) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        let mut state = self.read_register(address, EXPANDER_OUTPUT_STATE)?;
        if high {
            state |= 1u8 << pin;
        } else {
            state &= !(1u8 << pin);
        }
        self.write_register(address, EXPANDER_OUTPUT_STATE, state)
    }

    fn read_register(
        &mut self,
        address: u8,
        register: u8,
    ) -> Result<u8, LoraError<SpiError, I2cError, PinError>> {
        let mut register_byte = [0];
        self.i2c
            .write_read(address, &[register], &mut register_byte)
            .map_err(LoraError::I2c)?;
        Ok(register_byte[0])
    }

    fn write_register(
        &mut self,
        address: u8,
        register: u8,
        value: u8,
    ) -> Result<(), LoraError<SpiError, I2cError, PinError>> {
        self.i2c
            .write(address, &[register, value])
            .map_err(LoraError::I2c)
    }
}

const fn low_data_rate_optimize(config: LoraConfig) -> u8 {
    match (config.spreading_factor, config.bandwidth) {
        (
            11..=12,
            Bandwidth::Bw125
            | Bandwidth::Bw62
            | Bandwidth::Bw41
            | Bandwidth::Bw31
            | Bandwidth::Bw20
            | Bandwidth::Bw15
            | Bandwidth::Bw10
            | Bandwidth::Bw7,
        ) => LOW_DATA_RATE_OPTIMIZE_ON,
        (
            10,
            Bandwidth::Bw62
            | Bandwidth::Bw41
            | Bandwidth::Bw31
            | Bandwidth::Bw20
            | Bandwidth::Bw15
            | Bandwidth::Bw10
            | Bandwidth::Bw7,
        ) => LOW_DATA_RATE_OPTIMIZE_ON,
        _ => LOW_DATA_RATE_OPTIMIZE_OFF,
    }
}
