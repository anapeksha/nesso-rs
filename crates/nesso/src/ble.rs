//! Bluetooth Low Energy controller support for the ESP32-C6 radio.
//!
//! This module owns the board BLE controller lifecycle and exposes the HCI
//! connector provided by `esp-radio`. A BLE host stack can build GATT,
//! notification, and advertising behavior on top of that connector.

#[cfg(not(any(test, nesso_host_tests)))]
use crate::{bsp::RadioRuntimeResources, runtime};
use defmt::Format;
#[cfg(not(any(test, nesso_host_tests)))]
use esp_radio::ble::{Config, controller::BleConnector};
use heapless::{String, Vec};

// Bluetooth Core Assigned Numbers: GAP advertising data types.
const AD_TYPE_FLAGS: u8 = 0x01;
const AD_TYPE_COMPLETE_UUID16_LIST: u8 = 0x03;
const AD_TYPE_COMPLETE_LOCAL_NAME: u8 = 0x09;
const AD_TYPE_SERVICE_DATA_UUID16: u8 = 0x16;
const AD_TYPE_MANUFACTURER_SPECIFIC: u8 = 0xFF;
const LE_GENERAL_DISCOVERABLE_NO_BR_EDR: u8 = 0x06;

/// Board-owned peripherals required to start the BLE controller.
#[cfg(not(any(test, nesso_host_tests)))]
pub struct BluetoothResources {
    /// ESP32-C6 Bluetooth controller peripheral.
    pub bluetooth: esp_hal::peripherals::BT<'static>,
}

/// High-level state tracked by the SDK BLE wrapper.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub enum BleState {
    /// The controller has not been initialized.
    Stopped,
    /// The HCI connector is available.
    HciReady,
}

/// Errors returned by the Nesso BLE wrapper.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub enum BleError {
    /// Board Bluetooth resources were already consumed.
    ResourcesUnavailable,
    /// Controller initialization failed.
    Init,
    /// The HCI connector has not been started.
    NotStarted,
    /// A BLE advertising payload exceeded its fixed capacity.
    AdvertisementTooLong,
    /// A BLE device name exceeded its fixed capacity.
    NameTooLong,
    /// HCI read or write failed.
    Hci,
    /// A mirrored notification field exceeded its fixed capacity.
    NotificationTooLong,
    /// A mirrored notification payload was not valid.
    InvalidNotification,
}

/// Nesso-owned BLE controller lifecycle wrapper.
#[cfg(not(any(test, nesso_host_tests)))]
pub struct Ble {
    state: BleState,
    resources: Option<BluetoothResources>,
    runtime: Option<RadioRuntimeResources>,
    runtime_started: bool,
    connector: Option<BleConnector<'static>>,
}

#[cfg(not(any(test, nesso_host_tests)))]
impl Ble {
    /// Creates a stopped BLE wrapper from board-owned resources.
    #[must_use]
    pub const fn new(resources: BluetoothResources, runtime: RadioRuntimeResources) -> Self {
        Self {
            state: BleState::Stopped,
            resources: Some(resources),
            runtime: Some(runtime),
            runtime_started: false,
            connector: None,
        }
    }

    /// Creates a BLE wrapper when the shared ESP radio runtime is already running.
    #[must_use]
    pub const fn new_started(resources: BluetoothResources) -> Self {
        Self {
            state: BleState::Stopped,
            resources: Some(resources),
            runtime: None,
            runtime_started: true,
            connector: None,
        }
    }

    /// Starts the BLE HCI connector using the default ESP controller config.
    pub fn start_hci(&mut self) -> Result<&mut BleConnector<'static>, BleError> {
        self.start_hci_with_config(Config::default())
    }

    /// Starts BLE if needed and returns the owned HCI connector.
    ///
    /// Use this when handing the ESP32-C6 controller to a BLE host stack such
    /// as Trouble. After this call the `Ble` wrapper is consumed because the
    /// host stack owns the controller transport.
    pub fn into_hci_connector(mut self) -> Result<BleConnector<'static>, BleError> {
        if self.connector.is_none() {
            let _connector = self.start_hci()?;
        }

        self.connector.take().ok_or(BleError::NotStarted)
    }

    /// Starts the BLE HCI connector with an explicit ESP controller config.
    pub fn start_hci_with_config(
        &mut self,
        config: Config,
    ) -> Result<&mut BleConnector<'static>, BleError> {
        if self.connector.is_none() {
            let resources = self
                .resources
                .take()
                .ok_or(BleError::ResourcesUnavailable)?;
            if !self.runtime_started {
                let runtime = self.runtime.take().ok_or(BleError::ResourcesUnavailable)?;
                runtime::start_radio_runtime(runtime);
                self.runtime_started = true;
            }

            let connector =
                BleConnector::new(resources.bluetooth, config).map_err(|_| BleError::Init)?;
            self.connector = Some(connector);
            self.state = BleState::HciReady;
        }

        self.connector.as_mut().ok_or(BleError::NotStarted)
    }

    /// Returns the current BLE lifecycle state.
    #[must_use]
    pub const fn state(&self) -> BleState {
        self.state
    }

    /// Reads one HCI packet or fragment into `buffer`.
    pub fn read_hci(&mut self, buffer: &mut [u8]) -> Result<usize, BleError> {
        self.start_hci()?.read(buffer).map_err(|_| BleError::Hci)
    }

    /// Writes one HCI packet or fragment from `buffer`.
    pub fn write_hci(&mut self, buffer: &[u8]) -> Result<usize, BleError> {
        self.start_hci()?.write(buffer).map_err(|_| BleError::Hci)
    }

    /// Asynchronously reads one HCI packet or fragment into `buffer`.
    pub async fn read_hci_async(&mut self, buffer: &mut [u8]) -> Result<usize, BleError> {
        self.start_hci()?
            .read_async(buffer)
            .await
            .map_err(|_| BleError::Hci)
    }
}

/// BLE peripheral identity used by advertising and GATT host stacks.
#[derive(Clone, Debug, Format, Eq, PartialEq)]
pub struct DeviceIdentity {
    name: String<32>,
}

impl DeviceIdentity {
    /// Creates a fixed-capacity BLE device identity.
    pub fn new(name: &str) -> Result<Self, BleError> {
        let mut stored = String::new();
        stored.push_str(name).map_err(|_| BleError::NameTooLong)?;
        Ok(Self { name: stored })
    }

    /// Returns the advertised device name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
}

/// Builder for legacy BLE advertising data.
#[derive(Clone, Debug, Format, Eq, PartialEq)]
pub struct Advertisement<const N: usize> {
    bytes: Vec<u8, N>,
}

impl<const N: usize> Advertisement<N> {
    /// Creates an empty advertising payload.
    #[must_use]
    pub const fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    /// Appends the standard BLE discoverable/no-BR-EDR flags field.
    pub fn push_flags(&mut self) -> Result<(), BleError> {
        self.push_field(AD_TYPE_FLAGS, &[LE_GENERAL_DISCOVERABLE_NO_BR_EDR])
    }

    /// Appends a complete local name field.
    pub fn push_complete_name(&mut self, name: &str) -> Result<(), BleError> {
        self.push_field(AD_TYPE_COMPLETE_LOCAL_NAME, name.as_bytes())
    }

    /// Appends one complete 16-bit service UUID.
    pub fn push_service_uuid16(&mut self, uuid: u16) -> Result<(), BleError> {
        self.push_field(AD_TYPE_COMPLETE_UUID16_LIST, &uuid.to_le_bytes())
    }

    /// Appends service data associated with one 16-bit service UUID.
    pub fn push_service_data16(&mut self, uuid: u16, data: &[u8]) -> Result<(), BleError> {
        let mut payload: Vec<u8, N> = Vec::new();
        payload
            .extend_from_slice(&uuid.to_le_bytes())
            .map_err(|_| BleError::AdvertisementTooLong)?;
        payload
            .extend_from_slice(data)
            .map_err(|_| BleError::AdvertisementTooLong)?;
        self.push_field(AD_TYPE_SERVICE_DATA_UUID16, payload.as_slice())
    }

    /// Appends manufacturer-specific advertising data.
    pub fn push_manufacturer_data(
        &mut self,
        company_identifier: u16,
        data: &[u8],
    ) -> Result<(), BleError> {
        let mut payload: Vec<u8, N> = Vec::new();
        payload
            .extend_from_slice(&company_identifier.to_le_bytes())
            .map_err(|_| BleError::AdvertisementTooLong)?;
        payload
            .extend_from_slice(data)
            .map_err(|_| BleError::AdvertisementTooLong)?;
        self.push_field(AD_TYPE_MANUFACTURER_SPECIFIC, payload.as_slice())
    }

    /// Returns the encoded advertising payload.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    fn push_field(&mut self, field_type: u8, payload: &[u8]) -> Result<(), BleError> {
        let length = payload
            .len()
            .checked_add(1)
            .and_then(|value| u8::try_from(value).ok())
            .ok_or(BleError::AdvertisementTooLong)?;
        self.bytes
            .push(length)
            .map_err(|_| BleError::AdvertisementTooLong)?;
        self.bytes
            .push(field_type)
            .map_err(|_| BleError::AdvertisementTooLong)?;
        self.bytes
            .extend_from_slice(payload)
            .map_err(|_| BleError::AdvertisementTooLong)
    }
}

impl<const N: usize> Default for Advertisement<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// One scheduled BLE beacon payload.
#[derive(Clone, Debug, Format, Eq, PartialEq)]
pub struct BeaconFrame<const N: usize> {
    advertisement: Advertisement<N>,
    scan_response: Advertisement<N>,
    duration_ms: u32,
}

impl<const N: usize> BeaconFrame<N> {
    /// Creates a non-connectable beacon frame.
    #[must_use]
    pub const fn new(
        advertisement: Advertisement<N>,
        scan_response: Advertisement<N>,
        duration_ms: u32,
    ) -> Self {
        Self {
            advertisement,
            scan_response,
            duration_ms,
        }
    }

    /// Returns the advertising payload for this frame.
    #[must_use]
    pub fn advertisement(&self) -> &[u8] {
        self.advertisement.as_slice()
    }

    /// Returns the scan-response payload for this frame.
    #[must_use]
    pub fn scan_response(&self) -> &[u8] {
        self.scan_response.as_slice()
    }

    /// Returns how long this frame should be advertised before advancing.
    #[must_use]
    pub const fn duration_ms(&self) -> u32 {
        self.duration_ms
    }
}

/// Fixed-capacity BLE beacon rotation schedule.
#[derive(Clone, Debug, Format, Eq, PartialEq)]
pub struct BeaconSchedule<const FRAMES: usize, const N: usize> {
    frames: Vec<BeaconFrame<N>, FRAMES>,
    cursor: usize,
}

impl<const FRAMES: usize, const N: usize> BeaconSchedule<FRAMES, N> {
    /// Creates an empty beacon schedule.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            frames: Vec::new(),
            cursor: 0,
        }
    }

    /// Appends one frame to the schedule.
    pub fn push(&mut self, frame: BeaconFrame<N>) -> Result<(), BleError> {
        self.frames
            .push(frame)
            .map_err(|_| BleError::AdvertisementTooLong)
    }

    /// Returns the number of configured frames.
    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Returns true when the schedule has no frames.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Returns the current frame without advancing.
    #[must_use]
    pub fn current(&self) -> Option<&BeaconFrame<N>> {
        self.frames.get(self.cursor)
    }

    /// Advances to the next frame and returns it.
    pub fn advance(&mut self) -> Option<&BeaconFrame<N>> {
        if self.frames.is_empty() {
            return None;
        }
        self.cursor = (self.cursor + 1) % self.frames.len();
        self.current()
    }

    /// Resets the schedule to its first frame.
    pub fn reset(&mut self) {
        self.cursor = 0;
    }
}

impl<const FRAMES: usize, const N: usize> Default for BeaconSchedule<FRAMES, N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Coarse importance level for a mirrored phone notification.
#[derive(Clone, Copy, Debug, Format, Eq, PartialEq)]
pub enum NotificationPriority {
    /// Normal notification.
    Normal,
    /// Higher-priority notification that the application may want to emphasize.
    High,
}

/// Fixed-capacity phone notification mirrored over BLE.
#[derive(Clone, Debug, Format, Eq, PartialEq)]
pub struct MirroredNotification {
    app: String<32>,
    title: String<48>,
    body: String<96>,
    priority: NotificationPriority,
}

impl MirroredNotification {
    /// Creates a mirrored notification from app, title, and body fields.
    pub fn new(
        app: &str,
        title: &str,
        body: &str,
        priority: NotificationPriority,
    ) -> Result<Self, BleError> {
        let mut stored_app = String::new();
        let mut stored_title = String::new();
        let mut stored_body = String::new();
        stored_app
            .push_str(app)
            .map_err(|_| BleError::NotificationTooLong)?;
        stored_title
            .push_str(title)
            .map_err(|_| BleError::NotificationTooLong)?;
        stored_body
            .push_str(body)
            .map_err(|_| BleError::NotificationTooLong)?;
        Ok(Self {
            app: stored_app,
            title: stored_title,
            body: stored_body,
            priority,
        })
    }

    /// Parses `app|title|body` UTF-8 bytes sent by a phone/app GATT client.
    pub fn from_wire(bytes: &[u8]) -> Result<Self, BleError> {
        let bytes = trim_trailing_nul(bytes);
        let text = core::str::from_utf8(bytes).map_err(|_| BleError::InvalidNotification)?;
        let mut parts = text.splitn(3, '|');
        let app = parts.next().ok_or(BleError::InvalidNotification)?;
        let title = parts.next().ok_or(BleError::InvalidNotification)?;
        let body = parts.next().ok_or(BleError::InvalidNotification)?;
        if app.is_empty() || title.is_empty() {
            return Err(BleError::InvalidNotification);
        }
        Self::new(app, title, body, NotificationPriority::Normal)
    }

    /// Writes this notification as `app|title|body` UTF-8 bytes.
    pub fn write_wire(&self, output: &mut [u8]) -> Result<usize, BleError> {
        let fields = [
            self.app.as_bytes(),
            self.title.as_bytes(),
            self.body.as_bytes(),
        ];
        let required_len = fields
            .iter()
            .map(|field| field.len())
            .sum::<usize>()
            .saturating_add(2);
        if output.len() < required_len {
            return Err(BleError::NotificationTooLong);
        }

        let mut cursor = 0;
        for (index, field) in fields.iter().enumerate() {
            output[cursor..cursor + field.len()].copy_from_slice(field);
            cursor += field.len();
            if index < fields.len() - 1 {
                output[cursor] = b'|';
                cursor += 1;
            }
        }
        Ok(cursor)
    }

    /// Returns the source application name.
    #[must_use]
    pub fn app(&self) -> &str {
        self.app.as_str()
    }

    /// Returns the notification title.
    #[must_use]
    pub fn title(&self) -> &str {
        self.title.as_str()
    }

    /// Returns the notification body.
    #[must_use]
    pub fn body(&self) -> &str {
        self.body.as_str()
    }

    /// Returns the notification priority.
    #[must_use]
    pub const fn priority(&self) -> NotificationPriority {
        self.priority
    }
}

/// Fixed-capacity inbox for phone notifications mirrored over BLE.
#[derive(Clone, Debug, Format, Eq, PartialEq)]
pub struct NotificationMirror<const N: usize> {
    notifications: Vec<MirroredNotification, N>,
}

impl<const N: usize> NotificationMirror<N> {
    /// Creates an empty notification mirror inbox.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            notifications: Vec::new(),
        }
    }

    /// Pushes a new notification, dropping the oldest when the inbox is full.
    pub fn push(&mut self, notification: MirroredNotification) -> Result<(), BleError> {
        if self.notifications.is_full() {
            let _dropped = self.notifications.remove(0);
        }
        self.notifications
            .push(notification)
            .map_err(|_| BleError::NotificationTooLong)
    }

    /// Parses and pushes a notification encoded as `app|title|body`.
    pub fn push_wire(&mut self, bytes: &[u8]) -> Result<&MirroredNotification, BleError> {
        let notification = MirroredNotification::from_wire(bytes)?;
        self.push(notification)?;
        self.latest().ok_or(BleError::InvalidNotification)
    }

    /// Returns the latest mirrored notification.
    #[must_use]
    pub fn latest(&self) -> Option<&MirroredNotification> {
        self.notifications.last()
    }

    /// Returns an iterator over stored notifications, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &MirroredNotification> {
        self.notifications.iter()
    }

    /// Returns the fixed inbox capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Returns the number of stored notifications.
    #[must_use]
    pub fn len(&self) -> usize {
        self.notifications.len()
    }

    /// Returns true when no notifications are stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
    }

    /// Clears all stored notifications.
    pub fn clear(&mut self) {
        self.notifications.clear();
    }
}

impl<const N: usize> Default for NotificationMirror<N> {
    fn default() -> Self {
        Self::new()
    }
}

fn trim_trailing_nul(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .rposition(|byte| *byte != 0)
        .map_or(0, |index| index + 1);
    &bytes[..end]
}

/// SDK-owned UUIDs for the default Nesso GATT surface.
pub mod service {
    /// Nesso SDK primary service UUID.
    pub const NESSO_SERVICE_UUID16: u16 = 0xF0A0;
    /// Characteristic intended for short app-to-device commands.
    pub const COMMAND_CHARACTERISTIC_UUID16: u16 = 0xF0A1;
    /// Characteristic intended for device-to-app notifications.
    pub const NOTIFY_CHARACTERISTIC_UUID16: u16 = 0xF0A2;
    /// Characteristic intended for phone/app-to-device notification mirroring.
    pub const MIRROR_CHARACTERISTIC_UUID16: u16 = 0xF0A3;
}
