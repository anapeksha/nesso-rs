use nesso_host_tests::lora::{
    Bandwidth, CodingRate, HeaderMode, LoraConfig, LoraConfigError, MAX_FREQUENCY_HZ,
    MAX_LORA_PAYLOAD_LEN, MIN_FREQUENCY_HZ, ReceiveTimeout,
};

#[test]
fn lora_default_config_is_valid_in_documented_range() {
    let config = LoraConfig::new(915_000_000);

    assert_eq!(config.frequency_hz, 915_000_000);
    assert_eq!(config.spreading_factor, 7);
    assert_eq!(config.bandwidth, Bandwidth::Bw125);
    assert_eq!(config.coding_rate, CodingRate::Cr45);
    assert_eq!(config.header_mode, HeaderMode::Explicit);
    assert!(config.crc);
    assert!(!config.invert_iq);
    assert_eq!(config.output_power_dbm, 14);
    assert_eq!(config.validate(), Ok(()));
}

#[test]
fn lora_frequency_limits_match_nesso_rf_range() {
    assert_eq!(LoraConfig::new(MIN_FREQUENCY_HZ).validate(), Ok(()));
    assert_eq!(LoraConfig::new(MAX_FREQUENCY_HZ).validate(), Ok(()));
    assert_eq!(
        LoraConfig::new(MIN_FREQUENCY_HZ - 1).validate(),
        Err(LoraConfigError::InvalidFrequency)
    );
    assert_eq!(
        LoraConfig::new(MAX_FREQUENCY_HZ + 1).validate(),
        Err(LoraConfigError::InvalidFrequency)
    );
}

#[test]
fn lora_rejects_invalid_spreading_factor() {
    let mut too_low = LoraConfig::new(915_000_000);
    too_low.spreading_factor = 4;
    assert_eq!(
        too_low.validate(),
        Err(LoraConfigError::InvalidSpreadingFactor)
    );

    let mut too_high = LoraConfig::new(915_000_000);
    too_high.spreading_factor = 13;
    assert_eq!(
        too_high.validate(),
        Err(LoraConfigError::InvalidSpreadingFactor)
    );
}

#[test]
fn lora_rejects_invalid_output_power() {
    let mut too_low = LoraConfig::new(915_000_000);
    too_low.output_power_dbm = -10;
    assert_eq!(too_low.validate(), Err(LoraConfigError::InvalidOutputPower));

    let mut too_high = LoraConfig::new(915_000_000);
    too_high.output_power_dbm = 23;
    assert_eq!(too_high.validate(), Err(LoraConfigError::InvalidOutputPower));
}

#[test]
fn lora_payload_capacity_matches_sx1262_buffer_mode() {
    assert_eq!(MAX_LORA_PAYLOAD_LEN, 255);
}

#[test]
fn receive_timeout_tick_field_is_bounded_to_24_bits() {
    assert_eq!(ReceiveTimeout::Continuous.command_bytes(), [0xFF, 0xFF, 0xFF]);
    assert_eq!(ReceiveTimeout::Ticks(0x12_34_56).command_bytes(), [0x12, 0x34, 0x56]);
    assert_eq!(ReceiveTimeout::Ticks(0x01_23_45_67).command_bytes(), [0x23, 0x45, 0x67]);
}
