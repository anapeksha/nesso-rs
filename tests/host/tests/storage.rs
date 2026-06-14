use embedded_storage::{ReadStorage, Storage};
use nesso_host_tests::storage::{
    FlashSettingsStore, KeyValueStore, SETTINGS_CAPACITY, SETTINGS_FORMAT_VERSION,
    SETTINGS_IMAGE_LEN, SETTINGS_KEY_MAX_LEN, SETTINGS_VALUE_MAX_LEN, SettingsPartition,
    SettingsStore, StorageError,
};

#[derive(Clone)]
struct MemoryStorage {
    bytes: [u8; 512],
}

impl MemoryStorage {
    fn erased() -> Self {
        Self { bytes: [0xff; 512] }
    }
}

impl ReadStorage for MemoryStorage {
    type Error = core::convert::Infallible;

    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        let offset = offset as usize;
        bytes.copy_from_slice(&self.bytes[offset..offset + bytes.len()]);
        Ok(())
    }

    fn capacity(&self) -> usize {
        self.bytes.len()
    }
}

impl Storage for MemoryStorage {
    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        let offset = offset as usize;
        self.bytes[offset..offset + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }
}

#[test]
fn settings_store_enforces_capacity_and_replacement() -> Result<(), String> {
    let mut settings = SettingsStore::new();
    settings
        .set("mode", b"clock")
        .map_err(|error| format!("{error:?}"))?;
    settings
        .set("mode", b"timer")
        .map_err(|error| format!("{error:?}"))?;

    assert_eq!(settings.len(), 1);
    assert_eq!(settings.capacity(), SETTINGS_CAPACITY);
    assert_eq!(settings.max_key_len(), SETTINGS_KEY_MAX_LEN);
    assert_eq!(settings.max_value_len(), SETTINGS_VALUE_MAX_LEN);
    assert_eq!(settings.get("mode"), Some(&b"timer"[..]));
    assert!(settings.remove("mode"));
    assert!(settings.is_empty());

    assert_eq!(
        settings.set("1234567890123456789012345", b"x"),
        Err(StorageError::KeyTooLong)
    );
    assert_eq!(settings.set("k", &[0; 49]), Err(StorageError::ValueTooLong));
    Ok(())
}

#[test]
fn settings_public_format_constants_match_store_behavior() {
    let settings = SettingsStore::new();
    assert_eq!(settings.capacity(), 4);
    assert_eq!(settings.max_key_len(), 24);
    assert_eq!(settings.max_value_len(), 48);
    assert_eq!(SETTINGS_IMAGE_LEN, 256);
    assert_eq!(SETTINGS_FORMAT_VERSION, 2);
}

#[test]
fn settings_flash_store_round_trips_and_detects_checksum_corruption() -> Result<(), String> {
    let mut settings = SettingsStore::new();
    settings
        .set("brightness", &[80])
        .map_err(|error| format!("{error:?}"))?;
    settings
        .set("name", b"nesso")
        .map_err(|error| format!("{error:?}"))?;

    let storage = MemoryStorage::erased();
    let mut flash = FlashSettingsStore::new(storage, 0);
    flash
        .save(&settings)
        .map_err(|error| format!("{error:?}"))?;

    let mut loaded = SettingsStore::new();
    flash
        .load_into(&mut loaded)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(loaded, settings);

    let mut storage = flash.release();
    storage.bytes[20] ^= 0x55;
    let mut corrupted = FlashSettingsStore::new(storage, 0);
    let mut destination = SettingsStore::new();
    assert_eq!(
        corrupted.load_into(&mut destination),
        Err(StorageError::ChecksumMismatch)
    );
    Ok(())
}

#[test]
fn erased_flash_loads_as_empty_settings() -> Result<(), String> {
    let mut flash = FlashSettingsStore::new(MemoryStorage::erased(), 0);
    let mut settings = SettingsStore::new();
    flash
        .load_into(&mut settings)
        .map_err(|error| format!("{error:?}"))?;
    assert!(settings.is_empty());
    Ok(())
}

#[test]
fn partition_validation_rejects_small_or_unaligned_regions() {
    assert_eq!(
        FlashSettingsStore::from_storage_partition(
            MemoryStorage::erased(),
            SettingsPartition {
                label: "small",
                offset: 0,
                len: 128
            }
        )
        .map(|_| ()),
        Err(StorageError::PartitionTooSmall)
    );
    assert_eq!(
        FlashSettingsStore::from_storage_partition(
            MemoryStorage::erased(),
            SettingsPartition {
                label: "bad",
                offset: 1,
                len: 4096
            }
        )
        .map(|_| ()),
        Err(StorageError::PartitionUnaligned)
    );
}
