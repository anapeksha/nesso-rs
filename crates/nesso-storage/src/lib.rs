#![no_std]

use heapless::{String, Vec};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageError {
    Full,
    KeyTooLong,
    ValueTooLong,
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
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

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
    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.value.as_slice())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
