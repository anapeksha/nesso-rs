//! ESP32-C6 Wi-Fi support for the Arduino Nesso N1.
//!
//! The BSP owns the low-level radio peripherals and passes them into this
//! crate. Applications can use async station methods directly or the blocking
//! convenience wrappers for small examples. Applications that need TCP/IP can
//! take the SDK-created network interfaces and pass `interfaces.station` into
//! an `embassy-net` stack while keeping [`EspRadioWifi`] for station
//! scan/connect/disconnect control.

extern crate alloc;

#[cfg(not(any(test, nesso_host_tests)))]
use alloc::string::String as AllocString;
use alloc::vec::Vec as AllocVec;

#[cfg(not(any(test, nesso_host_tests)))]
use crate::{bsp::RadioRuntimeResources, runtime};
#[cfg(not(any(test, nesso_host_tests)))]
use embassy_futures::block_on;
#[cfg(not(any(test, nesso_host_tests)))]
use esp_radio::wifi::{
    AuthenticationMethod, Config, WifiController, ap::AccessPointInfo, scan::ScanConfig,
    sta::StationConfig,
};
use heapless::String;

/// ESP radio station network interface used by `embassy-net`.
#[cfg(not(any(test, nesso_host_tests)))]
pub type StationInterface = esp_radio::wifi::Interface<'static>;

/// ESP radio network interfaces created during Wi-Fi initialization.
#[cfg(not(any(test, nesso_host_tests)))]
pub type NetworkInterfaces = esp_radio::wifi::Interfaces<'static>;

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

/// Information about the current station association.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectedAccessPoint {
    /// Connected network SSID.
    pub ssid: String<32>,
    /// Access point BSSID.
    pub bssid: [u8; 6],
    /// Wi-Fi channel.
    pub channel: u8,
    /// Authentication method.
    pub auth: AuthMethod,
    /// Association identifier assigned by the access point.
    pub aid: u16,
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

impl Credentials {
    /// Creates station credentials for a protected network.
    pub fn new(ssid: &str, password: &str) -> Result<Self, CredentialsError> {
        let mut stored_ssid = String::new();
        let mut stored_password = String::new();
        stored_ssid
            .push_str(ssid)
            .map_err(|_| CredentialsError::SsidTooLong)?;
        stored_password
            .push_str(password)
            .map_err(|_| CredentialsError::PasswordTooLong)?;
        Ok(Self {
            ssid: stored_ssid,
            password: stored_password,
        })
    }

    /// Creates station credentials for an open network.
    pub fn open(ssid: &str) -> Result<Self, CredentialsError> {
        Self::new(ssid, "")
    }

    /// Returns true when the credentials target an open network.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.password.is_empty()
    }
}

/// Errors returned while building station credentials.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialsError {
    /// SSID exceeded the 32-byte IEEE 802.11 limit.
    SsidTooLong,
    /// Password exceeded the SDK's fixed 64-byte capacity.
    PasswordTooLong,
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
#[cfg(not(any(test, nesso_host_tests)))]
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

    /// Returns true when the station is connected.
    fn is_connected(&self) -> bool;
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
    /// Network interfaces were already handed to the application.
    InterfacesTaken,
}

/// Board-owned peripherals required to start ESP radio Wi-Fi.
#[cfg(not(any(test, nesso_host_tests)))]
pub struct RadioResources {
    /// ESP32-C6 Wi-Fi peripheral.
    pub wifi: esp_hal::peripherals::WIFI<'static>,
}

/// Stateful Wi-Fi station wrapper for Nesso N1.
#[cfg(not(any(test, nesso_host_tests)))]
pub struct EspRadioWifi {
    state: WifiState,
    resources: Option<RadioResources>,
    runtime: Option<RadioRuntimeResources>,
    runtime_started: bool,
    controller: Option<WifiController<'static>>,
    interfaces: Option<NetworkInterfaces>,
    connected: Option<ConnectedAccessPoint>,
}

#[cfg(not(any(test, nesso_host_tests)))]
impl EspRadioWifi {
    /// Creates a Wi-Fi wrapper from board-owned radio resources.
    #[must_use]
    pub const fn new(resources: RadioResources, runtime: RadioRuntimeResources) -> Self {
        Self {
            state: WifiState::Stopped,
            resources: Some(resources),
            runtime: Some(runtime),
            runtime_started: false,
            controller: None,
            interfaces: None,
            connected: None,
        }
    }

    /// Creates a Wi-Fi wrapper when the shared ESP radio runtime is already running.
    #[must_use]
    pub const fn new_started(resources: RadioResources) -> Self {
        Self {
            state: WifiState::Stopped,
            resources: Some(resources),
            runtime: None,
            runtime_started: true,
            controller: None,
            interfaces: None,
            connected: None,
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
        if !self.runtime_started {
            let runtime = self
                .runtime
                .take()
                .ok_or(EspRadioWifiError::ResourcesUnavailable)?;
            runtime::start_radio_runtime(runtime);
            self.runtime_started = true;
        }

        let (controller, interfaces) = esp_radio::wifi::new(resources.wifi, Default::default())
            .map_err(|_| EspRadioWifiError::Init)?;

        self.controller = Some(controller);
        self.interfaces = Some(interfaces);
        self.state = WifiState::Stopped;
        Ok(())
    }

    /// Returns the SDK-created ESP radio network interfaces exactly once.
    ///
    /// Applications can pass `interfaces.station` to `embassy-net` while
    /// continuing to use this [`EspRadioWifi`] value for station lifecycle
    /// control such as scan, connect, ensure-connected, and disconnect.
    ///
    /// This method starts Wi-Fi if needed. Calling it more than once returns
    /// [`EspRadioWifiError::InterfacesTaken`].
    pub fn take_interfaces(&mut self) -> Result<NetworkInterfaces, EspRadioWifiError> {
        self.start()?;
        self.interfaces
            .take()
            .ok_or(EspRadioWifiError::InterfacesTaken)
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
        let info = controller.connect_async().await.map_err(|_| {
            self.state = WifiState::Stopped;
            EspRadioWifiError::Connect
        })?;

        self.connected = Some(ConnectedAccessPoint {
            ssid: copy_ssid(info.ssid.as_str()),
            bssid: info.bssid,
            channel: info.channel,
            auth: convert_auth(info.authmode),
            aid: info.aid,
        });
        self.state = WifiState::Connected;
        Ok(())
    }

    /// Connects only when the station is not already connected.
    pub async fn ensure_connected_async(
        &mut self,
        credentials: &Credentials,
    ) -> Result<(), EspRadioWifiError> {
        if self.is_connected() {
            return Ok(());
        }
        self.connect_async(credentials).await
    }

    /// Blocking convenience wrapper around [`Self::ensure_connected_async`].
    pub fn ensure_connected(&mut self, credentials: &Credentials) -> Result<(), EspRadioWifiError> {
        block_on(self.ensure_connected_async(credentials))
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

        self.connected = None;
        self.state = WifiState::Stopped;
        Ok(())
    }

    #[must_use]
    /// Returns the current station state.
    pub const fn state(&self) -> WifiState {
        self.state
    }

    /// Returns true when the station reports an active connection.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.controller
            .as_ref()
            .is_some_and(WifiController::is_connected)
    }

    /// Returns information captured when the station connected.
    #[must_use]
    pub fn connection_info(&self) -> Option<&ConnectedAccessPoint> {
        self.connected.as_ref()
    }

    /// Reads the current station RSSI in dBm.
    pub fn rssi_dbm(&self) -> Result<i32, EspRadioWifiError> {
        self.controller
            .as_ref()
            .ok_or(EspRadioWifiError::NotStarted)?
            .rssi()
            .map_err(|_| EspRadioWifiError::NotStarted)
    }
}

#[cfg(not(any(test, nesso_host_tests)))]
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

    fn is_connected(&self) -> bool {
        self.is_connected()
    }
}

#[cfg(not(any(test, nesso_host_tests)))]
fn station_config(credentials: &Credentials) -> StationConfig {
    let mut config = StationConfig::default()
        .with_ssid(credentials.ssid.as_str())
        .with_password(AllocString::from(credentials.password.as_str()));

    if credentials.is_open() {
        config = config.with_auth_method(AuthenticationMethod::None);
    }

    config
}

#[cfg(not(any(test, nesso_host_tests)))]
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

#[cfg(not(any(test, nesso_host_tests)))]
fn copy_ssid(ssid: &str) -> String<32> {
    let mut out = String::new();
    let _ = out.push_str(ssid);
    out
}

#[cfg(not(any(test, nesso_host_tests)))]
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
