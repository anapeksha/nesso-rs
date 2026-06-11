#![no_std]

extern crate alloc;

use embassy_futures::block_on;
use esp_hal::{interrupt::software::SoftwareInterruptControl, timer::timg::TimerGroup};
use esp_radio::wifi::{AuthenticationMethod, ap::AccessPointInfo, scan::ScanConfig};
use heapless::{String, Vec};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthMethod {
    Open,
    Wpa,
    Wpa2,
    Wpa3,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessPoint {
    pub ssid: String<32>,
    pub rssi_dbm: i8,
    pub channel: u8,
    pub auth: AuthMethod,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Credentials {
    pub ssid: String<32>,
    pub password: String<64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WifiState {
    Stopped,
    Scanning,
    Connecting,
    Connected,
}

pub trait WifiStation {
    type Error;

    fn scan(
        &mut self,
    ) -> impl core::future::Future<Output = Result<Vec<AccessPoint, 16>, Self::Error>> + Send;

    fn connect(
        &mut self,
        credentials: &Credentials,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send;

    fn disconnect(&mut self) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send;

    fn state(&self) -> WifiState;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EspRadioWifiError {
    Init,
    Scan,
}

pub struct EspRadioWifi {
    state: WifiState,
}

impl EspRadioWifi {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: WifiState::Stopped,
        }
    }

    pub fn scan_once(
        &mut self,
        wifi: esp_hal::peripherals::WIFI<'static>,
        timg0_peripheral: esp_hal::peripherals::TIMG0<'static>,
        software_interrupt: esp_hal::peripherals::SW_INTERRUPT<'static>,
    ) -> Result<Vec<AccessPoint, 4>, EspRadioWifiError> {
        let timg0 = TimerGroup::new(timg0_peripheral);
        let sw_interrupt = SoftwareInterruptControl::new(software_interrupt);
        esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

        let (mut controller, _interfaces) =
            esp_radio::wifi::new(wifi, Default::default()).map_err(|_| EspRadioWifiError::Init)?;

        self.state = WifiState::Scanning;
        let scan_config = ScanConfig::default().with_max(4);
        let aps = block_on(controller.scan_async(&scan_config)).map_err(|_| {
            self.state = WifiState::Stopped;
            EspRadioWifiError::Scan
        })?;

        self.state = WifiState::Stopped;
        Ok(convert_access_points(&aps))
    }

    #[must_use]
    pub const fn state(&self) -> WifiState {
        self.state
    }
}

impl Default for EspRadioWifi {
    fn default() -> Self {
        Self::new()
    }
}

fn convert_access_points(aps: &[AccessPointInfo]) -> Vec<AccessPoint, 4> {
    let mut out = Vec::new();
    for ap in aps {
        let mut ssid = String::<32>::new();
        let _ = ssid.push_str(ap.ssid.as_str());
        let _ = out.push(AccessPoint {
            ssid,
            rssi_dbm: ap.signal_strength,
            channel: ap.channel,
            auth: ap.auth_method.map_or(AuthMethod::Unknown, convert_auth),
        });
    }
    out
}

fn convert_auth(auth: AuthenticationMethod) -> AuthMethod {
    match auth {
        AuthenticationMethod::None => AuthMethod::Open,
        AuthenticationMethod::Wpa => AuthMethod::Wpa,
        AuthenticationMethod::Wpa2Personal
        | AuthenticationMethod::WpaWpa2Personal
        | AuthenticationMethod::Wpa2Enterprise => AuthMethod::Wpa2,
        AuthenticationMethod::Wpa3Personal
        | AuthenticationMethod::Wpa2Wpa3Personal
        | AuthenticationMethod::Wpa3EntSuiteB192Bit
        | AuthenticationMethod::Wpa3ExtPsk
        | AuthenticationMethod::Wpa3ExtPskMixed
        | AuthenticationMethod::Wpa3Enterprise
        | AuthenticationMethod::Wpa2Wpa3Enterprise => AuthMethod::Wpa3,
        _ => AuthMethod::Unknown,
    }
}
