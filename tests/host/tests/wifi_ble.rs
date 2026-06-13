use nesso_host_tests::{
    ble::{
        Advertisement, BeaconFrame, BeaconSchedule, BleError, MirroredNotification,
        NotificationMirror, NotificationPriority,
    },
    wifi::{Credentials, CredentialsError},
};

#[test]
fn credentials_validate_fixed_capacities() -> Result<(), String> {
    let open = Credentials::open("env").map_err(|error| format!("{error:?}"))?;
    assert!(open.is_open());
    assert_eq!(open.ssid.as_str(), "env");

    let protected = Credentials::new("env", "password").map_err(|error| format!("{error:?}"))?;
    assert!(!protected.is_open());
    assert_eq!(protected.password.as_str(), "password");

    assert_eq!(
        Credentials::new("123456789012345678901234567890123", "password"),
        Err(CredentialsError::SsidTooLong)
    );
    assert_eq!(
        Credentials::new("env", "12345678901234567890123456789012345678901234567890123456789012345"),
        Err(CredentialsError::PasswordTooLong)
    );
    Ok(())
}

#[test]
fn mirrored_notification_round_trips_wire_payload() -> Result<(), String> {
    let notification = MirroredNotification::new(
        "Phone",
        "Title",
        "Body text",
        NotificationPriority::Normal,
    )
    .map_err(|error| format!("{error:?}"))?;
    let mut wire = [0u8; 64];
    let len = notification
        .write_wire(&mut wire)
        .map_err(|error| format!("{error:?}"))?;

    let parsed = MirroredNotification::from_wire(&wire[..len])
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(parsed.app(), "Phone");
    assert_eq!(parsed.title(), "Title");
    assert_eq!(parsed.body(), "Body text");
    Ok(())
}

#[test]
fn mirrored_notification_rejects_malformed_payloads() {
    assert_eq!(
        MirroredNotification::from_wire(b"app|title"),
        Err(BleError::InvalidNotification)
    );
    assert_eq!(
        MirroredNotification::from_wire(b"|title|body"),
        Err(BleError::InvalidNotification)
    );
    assert_eq!(
        MirroredNotification::from_wire(&[0xff, b'|', b't', b'|', b'b']),
        Err(BleError::InvalidNotification)
    );
}

#[test]
fn notification_mirror_drops_oldest_when_full() -> Result<(), String> {
    let mut mirror = NotificationMirror::<2>::new();
    let first = MirroredNotification::new("A", "1", "body", NotificationPriority::Normal)
        .map_err(|error| format!("{error:?}"))?;
    let second = MirroredNotification::new("B", "2", "body", NotificationPriority::Normal)
        .map_err(|error| format!("{error:?}"))?;
    let third = MirroredNotification::new("C", "3", "body", NotificationPriority::High)
        .map_err(|error| format!("{error:?}"))?;

    assert!(mirror.push(first).is_ok());
    assert!(mirror.push(second).is_ok());
    assert!(mirror.push(third).is_ok());

    let titles: heapless::Vec<&str, 2> = mirror.iter().map(MirroredNotification::title).collect();
    assert_eq!(titles.as_slice(), ["2", "3"]);
    assert_eq!(mirror.capacity(), 2);
    Ok(())
}

#[test]
fn advertisement_and_beacon_schedule_are_bounded() {
    let mut advertisement = Advertisement::<16>::new();
    assert!(advertisement.push_flags().is_ok());
    assert!(advertisement.push_service_uuid16(0xF0A0).is_ok());
    assert_eq!(advertisement.as_slice(), &[2, 1, 6, 3, 3, 0xA0, 0xF0]);

    let too_long = Advertisement::<4>::new().push_complete_name("Nesso");
    assert_eq!(too_long, Err(BleError::AdvertisementTooLong));

    let frame = BeaconFrame::new(advertisement, Advertisement::new(), 250);
    let mut schedule = BeaconSchedule::<2, 16>::new();
    assert!(schedule.push(frame.clone()).is_ok());
    assert!(schedule.push(frame).is_ok());
    assert_eq!(schedule.len(), 2);
    assert_eq!(schedule.current().map(BeaconFrame::duration_ms), Some(250));
    assert_eq!(schedule.advance().map(BeaconFrame::duration_ms), Some(250));
}
