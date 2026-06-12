//! ESP32-C6 Wi-Fi support for the Arduino Nesso N1.
//!
//! The BSP owns the low-level radio peripherals and passes them into this
//! crate. Applications can use async station methods directly or the blocking
//! convenience wrappers for small examples.

extern crate alloc;

use alloc::{string::String as AllocString, vec::Vec as AllocVec};

use embassy_futures::block_on;
use esp_hal::{interrupt::software::SoftwareInterruptControl, timer::timg::TimerGroup};
use esp_radio::wifi::{
    AuthenticationMethod, Config, WifiController, ap::AccessPointInfo, scan::ScanConfig,
    sta::StationConfig,
};
use heapless::String;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthMethod {
    /// Open network.
    Open,
    /// WPA network.
    Wpa,
    /// WPA2 network.
    Wpa2,
    /// WPA3 network.
    Wpa3,
    /// Authentication method not mapped by the SDK.
    Unknown,
}

/// Access point summary returned by a station scan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessPoint {
    /// Network SSID.
    pub ssid: String<32>,
    /// Received signal strength in dBm.
    pub rssi_dbm: i8,
    /// Wi-Fi channel.
    pub channel: u8,
    /// Advertised authentication method.
    pub auth: AuthMethod,
}

/// Heap-backed scan result list returned by ESP radio station scans.
pub type AccessPoints = AllocVec<AccessPoint>;

/// Station credentials for connecting to an access point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Credentials {
    /// Network SSID.
    pub ssid: String<32>,
    /// Network password. Leave empty for open networks.
    pub password: String<64>,
}

/// High-level station state tracked by the SDK wrapper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WifiState {
    /// Radio is initialized or available but not running a station operation.
    Stopped,
    /// Station scan is in progress.
    Scanning,
    /// Station connection or disconnection is in progress.
    Connecting,
    /// Station is connected to an access point.
    Connected,
}

/// Async station interface exposed by Wi-Fi implementations.
pub trait WifiStation {
    /// Driver-specific error type.
    type Error;

    /// Scans for nearby access points.
    fn scan_async(
        &mut self,
    ) -> impl core::future::Future<Output = Result<AccessPoints, Self::Error>> + '_;

    /// Connects to an access point.
    fn connect_async<'a>(
        &'a mut self,
        credentials: &'a Credentials,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + 'a;

    /// Disconnects the station if it is connected.
    fn disconnect_async(
        &mut self,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + '_;

    /// Returns the current station state.
    fn state(&self) -> WifiState;
}

/// Errors returned by the ESP radio Wi-Fi wrapper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EspRadioWifiError {
    /// ESP radio controller initialization failed.
    Init,
    /// Board radio resources were already consumed.
    ResourcesUnavailable,
    /// Controller was expected to be started but was unavailable.
    NotStarted,
    /// Station configuration failed.
    Configure,
    /// Access point scan failed.
    Scan,
    /// Station connect failed.
    Connect,
    /// Station disconnect failed.
    Disconnect,
}

/// Board-owned peripherals required to start ESP radio Wi-Fi.
pub struct RadioResources {
    /// ESP32-C6 Wi-Fi peripheral.
    pub wifi: esp_hal::peripherals::WIFI<'static>,
    /// Timer group used by the ESP radio runtime.
    pub timer_group0: esp_hal::peripherals::TIMG0<'static>,
    /// Software interrupt peripheral used by the ESP radio runtime.
    pub software_interrupt: esp_hal::peripherals::SW_INTERRUPT<'static>,
}

/// Stateful Wi-Fi station wrapper for Nesso N1.
pub struct EspRadioWifi {
    state: WifiState,
    resources: Option<RadioResources>,
    controller: Option<WifiController<'static>>,
}

impl EspRadioWifi {
    /// Creates a Wi-Fi wrapper from board-owned radio resources.
    #[must_use]
    pub const fn new(resources: RadioResources) -> Self {
        Self {
            state: WifiState::Stopped,
            resources: Some(resources),
            controller: None,
        }
    }

    /// Starts the ESP radio runtime and Wi-Fi controller.
    ///
    /// This consumes the board-owned radio resources exactly once. Applications
    /// using an Embassy executor may call this before awaiting station methods;
    /// blocking examples can use [`Self::scan`] directly.
    pub fn start(&mut self) -> Result<(), EspRadioWifiError> {
        if self.controller.is_some() {
            return Ok(());
        }

        let resources = self
            .resources
            .take()
            .ok_or(EspRadioWifiError::ResourcesUnavailable)?;

        let timg0 = TimerGroup::new(resources.timer_group0);
        let sw_interrupt = SoftwareInterruptControl::new(resources.software_interrupt);
        esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

        let (controller, _interfaces) = esp_radio::wifi::new(resources.wifi, Default::default())
            .map_err(|_| EspRadioWifiError::Init)?;

        self.controller = Some(controller);
        self.state = WifiState::Stopped;
        Ok(())
    }

    /// Blocking convenience wrapper around [`Self::scan_async`].
    ///
    /// This is intended for simple examples. Applications already running an
    /// Embassy executor should call [`Self::scan_async`] instead.
    pub fn scan(&mut self) -> Result<AccessPoints, EspRadioWifiError> {
        block_on(self.scan_async())
    }

    /// Runs an async station scan using the board-owned ESP32-C6 radio.
    pub async fn scan_async(&mut self) -> Result<AccessPoints, EspRadioWifiError> {
        self.start()?;
        let controller = self
            .controller
            .as_mut()
            .ok_or(EspRadioWifiError::NotStarted)?;

        self.state = WifiState::Scanning;
        let scan_config = ScanConfig::default().with_max(16);
        let aps = controller.scan_async(&scan_config).await.map_err(|_| {
            self.state = WifiState::Stopped;
            EspRadioWifiError::Scan
        })?;

        self.state = WifiState::Stopped;
        Ok(convert_access_points(&aps))
    }

    /// Blocking convenience wrapper around [`Self::connect_async`].
    pub fn connect(&mut self, credentials: &Credentials) -> Result<(), EspRadioWifiError> {
        block_on(self.connect_async(credentials))
    }

    /// Connects to a Wi-Fi access point using station mode.
    pub async fn connect_async(
        &mut self,
        credentials: &Credentials,
    ) -> Result<(), EspRadioWifiError> {
        self.start()?;
        let controller = self
            .controller
            .as_mut()
            .ok_or(EspRadioWifiError::NotStarted)?;

        let config = Config::Station(station_config(credentials));
        controller
            .set_config(&config)
            .map_err(|_| EspRadioWifiError::Configure)?;

        self.state = WifiState::Connecting;
        controller.connect_async().await.map_err(|_| {
            self.state = WifiState::Stopped;
            EspRadioWifiError::Connect
        })?;

        self.state = WifiState::Connected;
        Ok(())
    }

    /// Blocking convenience wrapper around [`Self::disconnect_async`].
    pub fn disconnect(&mut self) -> Result<(), EspRadioWifiError> {
        block_on(self.disconnect_async())
    }

    /// Disconnects the station if it is currently connected.
    pub async fn disconnect_async(&mut self) -> Result<(), EspRadioWifiError> {
        self.start()?;
        let controller = self
            .controller
            .as_mut()
            .ok_or(EspRadioWifiError::NotStarted)?;

        if controller.is_connected() {
            self.state = WifiState::Connecting;
            controller
                .disconnect_async()
                .await
                .map_err(|_| EspRadioWifiError::Disconnect)?;
        }

        self.state = WifiState::Stopped;
        Ok(())
    }

    #[must_use]
    /// Returns the current station state.
    pub const fn state(&self) -> WifiState {
        self.state
    }
}

impl WifiStation for EspRadioWifi {
    type Error = EspRadioWifiError;

    fn scan_async(
        &mut self,
    ) -> impl core::future::Future<Output = Result<AccessPoints, Self::Error>> + '_ {
        Self::scan_async(self)
    }

    fn connect_async<'a>(
        &'a mut self,
        credentials: &'a Credentials,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + 'a {
        Self::connect_async(self, credentials)
    }

    fn disconnect_async(
        &mut self,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + '_ {
        Self::disconnect_async(self)
    }

    fn state(&self) -> WifiState {
        self.state()
    }
}

fn station_config(credentials: &Credentials) -> StationConfig {
    let mut config = StationConfig::default()
        .with_ssid(credentials.ssid.as_str())
        .with_password(AllocString::from(credentials.password.as_str()));

    if credentials.password.is_empty() {
        config = config.with_auth_method(AuthenticationMethod::None);
    }

    config
}

fn convert_access_points(aps: &[AccessPointInfo]) -> AccessPoints {
    let mut out = AllocVec::new();
    for ap in aps {
        let mut ssid = String::<32>::new();
        let _ = ssid.push_str(ap.ssid.as_str());
        out.push(AccessPoint {
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
