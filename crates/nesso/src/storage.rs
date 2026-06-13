use heapless::{String, Vec};

/// Recommended partition label for applications that provide a partition table.
pub const NESSO_SETTINGS_PARTITION: &str = "nesso_settings";
/// Default settings offset for the factory 16 MiB Nesso N1 flash layout.
pub const NESSO_SETTINGS_OFFSET: u32 = 0x00FC_0000;
/// Reserved byte length for the SDK settings area.
pub const NESSO_SETTINGS_LEN: u32 = 4096;

/// Flash region used by a settings store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingsPartition {
    /// Human-readable partition label.
    pub label: &'static str,
    /// Absolute flash offset in bytes.
    pub offset: u32,
    /// Reserved byte length.
    pub len: u32,
}

impl SettingsPartition {
    /// Recommended SDK settings partition for the factory Nesso N1 flash map.
    pub const DEFAULT: Self = Self {
        label: NESSO_SETTINGS_PARTITION,
        offset: NESSO_SETTINGS_OFFSET,
        len: NESSO_SETTINGS_LEN,
    };
}

/// Errors returned by settings storage operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageError {
    /// The requested flash region is too small for the settings image.
    PartitionTooSmall,
    /// The requested flash region is not sector aligned.
    PartitionUnaligned,
    /// The fixed-capacity settings table is full.
    Full,
    /// A key exceeded the fixed key capacity.
    KeyTooLong,
    /// A value exceeded the fixed value capacity.
    ValueTooLong,
    /// The flash image did not match the SDK settings format.
    InvalidFormat,
    /// The flash image checksum did not match the stored checksum.
    ChecksumMismatch,
    /// The underlying storage backend returned an error.
    Backend,
}

/// One fixed-capacity settings entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    /// Entry key.
    pub key: String<24>,
    /// Entry value bytes.
    pub value: Vec<u8, 48>,
}

/// Heapless fixed-capacity key/value settings store.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SettingsStore {
    entries: Vec<Entry, 4>,
}

impl SettingsStore {
    /// Creates an empty fixed-capacity settings store.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Inserts or replaces one key/value pair.
    pub fn set(&mut self, key: &str, value: &[u8]) -> Result<(), StorageError> {
        if key.len() > 24 {
            return Err(StorageError::KeyTooLong);
        }
        if value.len() > 48 {
            return Err(StorageError::ValueTooLong);
        }
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
            entry.value.clear();
            entry
                .value
                .extend_from_slice(value)
                .map_err(|_| StorageError::ValueTooLong)?;
            return Ok(());
        }
        let mut stored_key = String::<24>::new();
        stored_key
            .push_str(key)
            .map_err(|_| StorageError::KeyTooLong)?;
        let mut stored_value = Vec::<u8, 48>::new();
        stored_value
            .extend_from_slice(value)
            .map_err(|_| StorageError::ValueTooLong)?;
        self.entries
            .push(Entry {
                key: stored_key,
                value: stored_value,
            })
            .map_err(|_| StorageError::Full)
    }

    #[must_use]
    /// Returns the stored value for a key.
    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.value.as_slice())
    }

    /// Removes a key/value pair and returns whether an entry was removed.
    pub fn remove(&mut self, key: &str) -> bool {
        if let Some(index) = self.entries.iter().position(|entry| entry.key == key) {
            let _removed = self.entries.remove(index);
            true
        } else {
            false
        }
    }

    /// Removes every setting.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns an iterator over stored entries.
    pub fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }

    #[must_use]
    /// Returns the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    /// Returns true when the store has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

pub trait KeyValueStore {
    /// Driver-specific error type.
    type Error;

    /// Loads settings into `settings`.
    fn load_into(&mut self, settings: &mut SettingsStore) -> Result<(), Self::Error>;

    /// Saves settings from `settings`.
    fn save(&mut self, settings: &SettingsStore) -> Result<(), Self::Error>;
}

impl KeyValueStore for SettingsStore {
    type Error = StorageError;

    fn load_into(&mut self, settings: &mut SettingsStore) -> Result<(), Self::Error> {
        *settings = self.clone();
        Ok(())
    }

    fn save(&mut self, settings: &SettingsStore) -> Result<(), Self::Error> {
        *self = settings.clone();
        Ok(())
    }
}

#[cfg(not(any(test, nesso_host_tests)))]
pub type EspFlashSettingsStore<'d> = FlashSettingsStore<esp_storage::FlashStorage<'d>>;

#[cfg(not(any(test, nesso_host_tests)))]
impl<'d> FlashSettingsStore<esp_storage::FlashStorage<'d>> {
    /// Creates a flash-backed settings store from the ESP-HAL flash peripheral.
    #[must_use]
    pub fn from_flash(flash: esp_hal::peripherals::FLASH<'d>, offset: u32) -> Self {
        Self::new(esp_storage::FlashStorage::new(flash), offset)
    }

    /// Creates a flash-backed settings store using a documented partition.
    pub fn from_partition(
        flash: esp_hal::peripherals::FLASH<'d>,
        partition: SettingsPartition,
    ) -> Result<Self, StorageError> {
        validate_partition(partition)?;
        Ok(Self::new(
            esp_storage::FlashStorage::new(flash),
            partition.offset,
        ))
    }
}

/// Flash-backed settings store over an embedded-storage backend.
pub struct FlashSettingsStore<STORAGE> {
    storage: STORAGE,
    offset: u32,
}

impl<STORAGE> FlashSettingsStore<STORAGE> {
    /// Creates a flash-backed settings store around an embedded-storage backend.
    #[must_use]
    pub const fn new(storage: STORAGE, offset: u32) -> Self {
        Self { storage, offset }
    }

    /// Creates a flash-backed settings store using a documented partition.
    pub fn from_storage_partition(
        storage: STORAGE,
        partition: SettingsPartition,
    ) -> Result<Self, StorageError> {
        validate_partition(partition)?;
        Ok(Self::new(storage, partition.offset))
    }

    /// Returns the absolute flash offset used by this store.
    #[must_use]
    pub const fn offset(&self) -> u32 {
        self.offset
    }

    /// Releases the wrapped storage backend.
    pub fn release(self) -> STORAGE {
        self.storage
    }
}

impl<STORAGE> KeyValueStore for FlashSettingsStore<STORAGE>
where
    STORAGE: embedded_storage::Storage,
{
    type Error = StorageError;

    fn load_into(&mut self, settings: &mut SettingsStore) -> Result<(), Self::Error> {
        let mut image = [0u8; SETTINGS_IMAGE_LEN];
        self.storage
            .read(self.offset, &mut image)
            .map_err(|_| StorageError::Backend)?;
        deserialize_settings_into(&image, settings)
    }

    fn save(&mut self, settings: &SettingsStore) -> Result<(), Self::Error> {
        let mut image = [0xff; SETTINGS_IMAGE_LEN];
        serialize_settings(settings, &mut image)?;
        self.storage
            .write(self.offset, &image)
            .map_err(|_| StorageError::Backend)
    }
}

const SETTINGS_MAGIC: &[u8; 4] = b"NSST";
const SETTINGS_VERSION_V1: u8 = 1;
const SETTINGS_VERSION: u8 = 2;
const SETTINGS_IMAGE_LEN: usize = 256;
const HEADER_LEN_V1: usize = 6;
const HEADER_LEN: usize = 10;

fn validate_partition(partition: SettingsPartition) -> Result<(), StorageError> {
    if partition.len < SETTINGS_IMAGE_LEN as u32 {
        return Err(StorageError::PartitionTooSmall);
    }
    if !partition.offset.is_multiple_of(NESSO_SETTINGS_LEN) {
        return Err(StorageError::PartitionUnaligned);
    }
    Ok(())
}

fn serialize_settings(
    settings: &SettingsStore,
    image: &mut [u8; SETTINGS_IMAGE_LEN],
) -> Result<(), StorageError> {
    image[..4].copy_from_slice(SETTINGS_MAGIC);
    image[4] = SETTINGS_VERSION;
    image[5] = settings.entries.len() as u8;
    let mut cursor = HEADER_LEN;

    for entry in &settings.entries {
        let key = entry.key.as_bytes();
        let value = entry.value.as_slice();
        let record_len = 2 + key.len() + value.len();
        if cursor + record_len > image.len() {
            return Err(StorageError::Full);
        }

        image[cursor] = key.len() as u8;
        image[cursor + 1] = value.len() as u8;
        cursor += 2;
        image[cursor..cursor + key.len()].copy_from_slice(key);
        cursor += key.len();
        image[cursor..cursor + value.len()].copy_from_slice(value);
        cursor += value.len();
    }

    let used_len = u16::try_from(cursor).map_err(|_| StorageError::Full)?;
    image[6..8].copy_from_slice(&used_len.to_le_bytes());
    image[8..10].fill(0);
    let checksum = settings_checksum(image, cursor);
    image[8..10].copy_from_slice(&checksum.to_le_bytes());

    Ok(())
}

fn deserialize_settings_into(
    image: &[u8; SETTINGS_IMAGE_LEN],
    settings: &mut SettingsStore,
) -> Result<(), StorageError> {
    *settings = SettingsStore::new();
    if &image[..4] == [0xff; 4].as_slice() {
        return Ok(());
    }
    if &image[..4] != SETTINGS_MAGIC {
        return Err(StorageError::InvalidFormat);
    }

    match image[4] {
        SETTINGS_VERSION_V1 => {
            deserialize_records(image, HEADER_LEN_V1, image[5] as usize, settings)
        }
        SETTINGS_VERSION => {
            let used_len = u16::from_le_bytes([image[6], image[7]]) as usize;
            if !(HEADER_LEN..=image.len()).contains(&used_len) {
                return Err(StorageError::InvalidFormat);
            }
            let expected = u16::from_le_bytes([image[8], image[9]]);
            let actual = settings_checksum(image, used_len);
            if expected != actual {
                return Err(StorageError::ChecksumMismatch);
            }
            deserialize_records(image, HEADER_LEN, image[5] as usize, settings)
        }
        _ => Err(StorageError::InvalidFormat),
    }
}

fn deserialize_records(
    image: &[u8; SETTINGS_IMAGE_LEN],
    mut cursor: usize,
    count: usize,
    settings: &mut SettingsStore,
) -> Result<(), StorageError> {
    for _ in 0..count {
        if cursor + 2 > image.len() {
            return Err(StorageError::InvalidFormat);
        }
        let key_len = image[cursor] as usize;
        let value_len = image[cursor + 1] as usize;
        cursor += 2;
        if cursor + key_len + value_len > image.len() {
            return Err(StorageError::InvalidFormat);
        }
        let key = core::str::from_utf8(&image[cursor..cursor + key_len])
            .map_err(|_| StorageError::InvalidFormat)?;
        cursor += key_len;
        let value = &image[cursor..cursor + value_len];
        cursor += value_len;
        settings.set(key, value)?;
    }

    Ok(())
}

fn checksum16(bytes: &[u8]) -> u16 {
    bytes.iter().fold(0u16, |checksum, byte| {
        checksum.wrapping_add(u16::from(*byte))
    })
}

fn settings_checksum(image: &[u8; SETTINGS_IMAGE_LEN], used_len: usize) -> u16 {
    checksum16(&image[..8]).wrapping_add(checksum16(&image[10..used_len]))
}
