use heapless::{String, Vec};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageError {
    Full,
    KeyTooLong,
    ValueTooLong,
    InvalidFormat,
    Backend,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub key: String<24>,
    pub value: Vec<u8, 48>,
}

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
    type Error;

    fn load_into(&mut self, settings: &mut SettingsStore) -> Result<(), Self::Error>;

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

pub type EspFlashSettingsStore<'d> = FlashSettingsStore<esp_storage::FlashStorage<'d>>;

impl<'d> FlashSettingsStore<esp_storage::FlashStorage<'d>> {
    /// Creates a flash-backed settings store from the ESP-HAL flash peripheral.
    #[must_use]
    pub fn from_flash(flash: esp_hal::peripherals::FLASH<'d>, offset: u32) -> Self {
        Self::new(esp_storage::FlashStorage::new(flash), offset)
    }
}

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
const SETTINGS_VERSION: u8 = 1;
const SETTINGS_IMAGE_LEN: usize = 256;
const HEADER_LEN: usize = 6;

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
    if &image[..4] != SETTINGS_MAGIC || image[4] != SETTINGS_VERSION {
        return Err(StorageError::InvalidFormat);
    }

    let count = image[5] as usize;
    let mut cursor = HEADER_LEN;

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
