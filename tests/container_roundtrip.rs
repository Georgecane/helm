use helm::{inspect, pack, unpack, unpack_with_limit, HelmError};

#[test]
fn arbitrary_binary_round_trip_is_exact() {
    let input: Vec<u8> = (0..=255).cycle().take(100_003).map(|n| n as u8).collect();
    assert_eq!(unpack(&pack(&input).unwrap()).unwrap(), input);
}

#[test]
fn header_checksum_is_stable_for_same_input() {
    let input = b"same input, same digest";
    let first = inspect(&pack(input).unwrap()).unwrap();
    let second = inspect(&pack(input).unwrap()).unwrap();
    assert_eq!(first.checksum_blake3, second.checksum_blake3);
}

#[test]
fn oversized_declared_output_is_rejected() {
    let packed = pack(b"123456789").unwrap();
    assert!(matches!(
        unpack_with_limit(&packed, 8),
        Err(HelmError::OutputLimitExceeded { .. })
    ));
}
