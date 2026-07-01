use bmi2::{
    Bmi2, I2cAddr, config,
    interface::I2cInterface,
    types::{
        AccBwp, AccConf, AccRange, Burst, Data, GyrBwp, GyrConf, GyrRange, GyrRangeVal, Odr,
        OisRange, PerfMode, PwrCtrl,
    },
};
use embedded_hal::delay::DelayNs;
use embedded_hal::i2c::I2c;

pub const BMI270_ADDRESS_LOW: u8 = 0x68;
pub const BMI270_ADDRESS_HIGH: u8 = 0x69;
pub const BMI270_CHIP_ID: u8 = 0x24;
// BMI270 register map, Bosch BMI270 datasheet section "Register map".
const REG_CHIP_ID: u8 = 0x00;
const REG_ACC_DATA_X_LSB: u8 = 0x0c;
const REG_GYR_DATA_X_LSB: u8 = 0x12;

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Acceleration {
    pub x_mg: i16,
    pub y_mg: i16,
    pub z_mg: i16,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Gyroscope {
    pub x_mdps: i32,
    pub y_mdps: i32,
    pub z_mdps: i32,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Orientation {
    pub pitch_degrees: f32,
    pub roll_degrees: f32,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImuError<E> {
    Bus(E),
    InvalidChipId(u8),
}

pub struct Imu<I2C> {
    i2c: I2C,
    address: u8,
}

pub struct Bmi270<I2C, DELAY> {
    inner: Bmi2<I2cInterface<I2C>, DELAY, 512>,
}

impl<I2C, DELAY> Bmi270<I2C, DELAY> {
    /// Creates a BMI270 driver using the default I2C address.
    #[must_use]
    pub fn new(i2c: I2C, delay: DELAY) -> Self {
        Self {
            inner: Bmi2::new_i2c(i2c, delay, I2cAddr::Default, Burst::new(255)),
        }
    }

    /// Releases the wrapped I2C bus.
    pub fn release(self) -> I2C {
        self.inner.release()
    }
}

impl<I2C, DELAY, E> Bmi270<I2C, DELAY>
where
    I2C: I2c<Error = E>,
    DELAY: DelayNs,
{
    /// Uploads BMI270 configuration and enables accelerometer and gyroscope output.
    pub fn init(&mut self) -> Result<(), bmi2::types::Error<E>> {
        self.inner.init(&config::BMI270_CONFIG_FILE)?;
        self.inner.set_acc_conf(AccConf {
            odr: Odr::Odr100,
            bwp: AccBwp::NormAvg4,
            filter_perf: PerfMode::Perf,
        })?;
        self.inner.set_acc_range(AccRange::Range4g)?;
        self.inner.set_gyr_conf(GyrConf {
            odr: Odr::Odr100,
            bwp: GyrBwp::Norm,
            noise_perf: PerfMode::Perf,
            filter_perf: PerfMode::Perf,
        })?;
        self.inner.set_gyr_range(GyrRange {
            range: GyrRangeVal::Range2000,
            ois_range: OisRange::Range250,
        })?;
        self.inner.set_pwr_ctrl(PwrCtrl {
            aux_en: false,
            gyr_en: true,
            acc_en: true,
            temp_en: false,
        })
    }

    /// Reads raw BMI270 accelerometer and gyroscope data.
    pub fn data(&mut self) -> Result<Data, bmi2::types::Error<E>> {
        self.inner.get_data()
    }

    /// Reads accelerometer data in milli-g.
    pub fn acceleration(&mut self) -> Result<Acceleration, bmi2::types::Error<E>> {
        let sensor_sample = self.data()?;
        Ok(Acceleration {
            x_mg: sensor_sample.acc.x,
            y_mg: sensor_sample.acc.y,
            z_mg: sensor_sample.acc.z,
        })
    }

    /// Reads gyroscope data in milli-degrees per second.
    pub fn gyroscope(&mut self) -> Result<Gyroscope, bmi2::types::Error<E>> {
        let sensor_sample = self.data()?;
        Ok(Gyroscope {
            x_mdps: i32::from(sensor_sample.gyr.x),
            y_mdps: i32::from(sensor_sample.gyr.y),
            z_mdps: i32::from(sensor_sample.gyr.z),
        })
    }
}

impl<I2C> Imu<I2C> {
    /// Creates a low-level BMI270 register reader at the provided I2C address.
    #[must_use]
    pub const fn new(i2c: I2C, address: u8) -> Self {
        Self { i2c, address }
    }

    /// Releases the wrapped I2C bus.
    pub fn release(self) -> I2C {
        self.i2c
    }
}

impl<I2C, E> Imu<I2C>
where
    I2C: I2c<Error = E>,
{
    /// Verifies the BMI270 chip identifier.
    pub fn verify(&mut self) -> Result<(), ImuError<E>> {
        let id = self.read_register(REG_CHIP_ID).map_err(ImuError::Bus)?;
        if id == BMI270_CHIP_ID {
            Ok(())
        } else {
            Err(ImuError::InvalidChipId(id))
        }
    }

    /// Reads raw accelerometer registers.
    pub fn accelerometer(&mut self) -> Result<Acceleration, E> {
        let mut accel_bytes = [0u8; 6];
        self.i2c
            .write_read(self.address, &[REG_ACC_DATA_X_LSB], &mut accel_bytes)?;
        Ok(Acceleration {
            x_mg: raw_i16(accel_bytes[0], accel_bytes[1]),
            y_mg: raw_i16(accel_bytes[2], accel_bytes[3]),
            z_mg: raw_i16(accel_bytes[4], accel_bytes[5]),
        })
    }

    /// Reads raw gyroscope registers.
    pub fn gyroscope(&mut self) -> Result<Gyroscope, E> {
        let mut gyro_bytes = [0u8; 6];
        self.i2c
            .write_read(self.address, &[REG_GYR_DATA_X_LSB], &mut gyro_bytes)?;
        Ok(Gyroscope {
            x_mdps: i32::from(raw_i16(gyro_bytes[0], gyro_bytes[1])),
            y_mdps: i32::from(raw_i16(gyro_bytes[2], gyro_bytes[3])),
            z_mdps: i32::from(raw_i16(gyro_bytes[4], gyro_bytes[5])),
        })
    }

    /// Estimates pitch and roll from the current accelerometer vector.
    pub fn orientation(&mut self) -> Result<Orientation, E> {
        let accel = self.accelerometer()?;
        let x = f32::from(accel.x_mg);
        let y = f32::from(accel.y_mg);
        let z = f32::from(accel.z_mg);
        Ok(Orientation {
            pitch_degrees: radians_to_degrees(atan2_approx(x, libm::sqrtf(y * y + z * z))),
            roll_degrees: radians_to_degrees(atan2_approx(y, z)),
        })
    }

    fn read_register(&mut self, register: u8) -> Result<u8, E> {
        let mut register_byte = [0u8; 1];
        self.i2c
            .write_read(self.address, &[register], &mut register_byte)?;
        Ok(register_byte[0])
    }
}

fn raw_i16(lsb: u8, msb: u8) -> i16 {
    i16::from_le_bytes([lsb, msb])
}

fn atan2_approx(y: f32, x: f32) -> f32 {
    libm::atan2f(y, x)
}

fn radians_to_degrees(radians: f32) -> f32 {
    radians * 180.0 / core::f32::consts::PI
}
