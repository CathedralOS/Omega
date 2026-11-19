use super::*;

// Older review files use raw fixed-size digests, two-byte target tags and
// eight-byte identities and text lengths. Build these bytes without the codec.
fn previous_review(condition: u8, profile: u16) -> Vec<u8> {
    let mut bytes = b"OMGINST1".to_vec();
    bytes.extend(1u64.to_le_bytes());
    bytes.push(condition);
    bytes.extend(VocabularyMarker::CURRENT.get().to_le_bytes());
    bytes.extend([3; 32]);
    bytes.extend(profile.to_le_bytes());
    bytes.extend(9u64.to_le_bytes());
    bytes.extend(17u64.to_le_bytes());
    bytes.push(0); // Interval.
    if condition == 1 {
        bytes.push(0); // Architectural exceptions and external IRQs.
    }
    bytes.extend([8; 32]);
    for text in ["published machine", "caller review"] {
        bytes.extend((text.len() as u64).to_le_bytes());
        bytes.extend(text.bytes());
    }
    bytes
}

#[test]
fn old_review_rows_reencode_exactly_and_embed_without_the_file_header() {
    for (condition, tag, profile) in [
        (0, 5, TargetProfile::UefiX64),
        (1, 1, TargetProfile::LinuxX64),
    ] {
        let bytes = previous_review(condition, tag);
        let rows = decode_installation_assumptions(&bytes).unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.condition_tag(), condition);
        assert_eq!(row.profile(), profile);
        assert_eq!(row.entry().get(), 9);
        assert_eq!(row.installation().get(), 17);
        assert_eq!(row.psi().program_fingerprint.as_bytes(), &[3; 32]);
        assert_eq!(row.environment_description_sha256(), &[8; 32]);
        assert_eq!(row.source_provenance(), "published machine");
        assert_eq!(row.caller_provenance(), "caller review");
        assert_eq!(encode_installation_assumptions(&rows).unwrap(), bytes);
        let mut native = RecordWriter::new(RecordLimits::default());
        row.write(&mut native).unwrap();
        assert_eq!(native.finish(), bytes[16..]);
    }
}

#[test]
fn reject_foreign_incomplete_and_invalid_review_records() {
    let mut bytes = previous_review(0, 5);
    bytes[0] = 0;
    assert!(decode_installation_assumptions(&bytes).is_err());
    let mut bytes = previous_review(0, 5);
    bytes.push(0);
    assert!(decode_installation_assumptions(&bytes).is_err());
    bytes.truncate(bytes.len() - 2);
    assert!(decode_installation_assumptions(&bytes).is_err());
    assert!(decode_installation_assumptions(&previous_review(2, 1)).is_err());
    assert!(decode_installation_assumptions(&previous_review(1, 0)).is_err()); // AArch64 interrupt delivery.
}
