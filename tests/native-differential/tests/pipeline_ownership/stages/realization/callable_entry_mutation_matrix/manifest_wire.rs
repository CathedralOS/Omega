//! Canonical ordinary-callable manifest envelope and closed wire-axis mutations.

use crate::tests::*;

use super::fixture::staged_callable;

fn record_error(
    error: OptimizedOrdinaryCallableEntryDecodeError,
) -> OptimizedOrdinaryCallableEntryManifestDecodeError {
    OptimizedOrdinaryCallableEntryManifestDecodeError::Record(error)
}

#[test]
fn ordinary_callable_manifest_wire_rejects_every_closed_axis_and_envelope_mutation() {
    let staged = staged_callable();
    let encoded = staged.manifest().record().encode();
    assert_eq!(
        encoded.len(),
        295,
        "ordinary-callable manifest V3 is pinned"
    );

    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&wrong_magic),
        Err(OptimizedOrdinaryCallableEntryManifestDecodeError::WrongMagic),
    );

    let mut wrong_version = encoded.clone();
    wrong_version[8..12].copy_from_slice(&99_u32.to_le_bytes());
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&wrong_version),
        Err(OptimizedOrdinaryCallableEntryManifestDecodeError::UnsupportedVersion(99)),
    );

    let mut wrong_identity = encoded.clone();
    wrong_identity[12] ^= 1;
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&wrong_identity),
        Err(OptimizedOrdinaryCallableEntryManifestDecodeError::IdentityMismatch),
    );

    let mut unknown_stage = encoded.clone();
    unknown_stage[44] = 99;
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&unknown_stage),
        Err(OptimizedOrdinaryCallableEntryManifestDecodeError::UnknownStage(99)),
    );

    let mut invalid_vocabulary = encoded.clone();
    invalid_vocabulary[141..143].copy_from_slice(&59_u16.to_le_bytes());
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&invalid_vocabulary),
        Err(record_error(
            OptimizedOrdinaryCallableEntryDecodeError::InvalidId
        )),
    );

    let mut unknown_architecture = encoded.clone();
    unknown_architecture[207] = 99;
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&unknown_architecture),
        Err(record_error(
            OptimizedOrdinaryCallableEntryDecodeError::UnknownArchitecture(99),
        )),
    );

    let mut unknown_object_format = encoded.clone();
    unknown_object_format[208] = 99;
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&unknown_object_format),
        Err(record_error(
            OptimizedOrdinaryCallableEntryDecodeError::UnknownObjectFormat(99),
        )),
    );

    for offset in [209, 217] {
        let mut invalid_target = encoded.clone();
        invalid_target[offset..offset + 8].copy_from_slice(&4_u64.to_le_bytes());
        assert_eq!(
            OptimizedOrdinaryCallableEntryManifest::decode(&invalid_target),
            Err(record_error(
                OptimizedOrdinaryCallableEntryDecodeError::InvalidTarget
            )),
        );
    }
    let mut incompatible_target = encoded.clone();
    incompatible_target[207] = 2;
    incompatible_target[208] = 3;
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&incompatible_target),
        Err(record_error(
            OptimizedOrdinaryCallableEntryDecodeError::InvalidTarget,
        )),
    );

    for offset in [225, 233] {
        let mut invalid_id = encoded.clone();
        invalid_id[offset..offset + 8].copy_from_slice(&0_u64.to_le_bytes());
        assert_eq!(
            OptimizedOrdinaryCallableEntryManifest::decode(&invalid_id),
            Err(record_error(
                OptimizedOrdinaryCallableEntryDecodeError::InvalidId
            )),
        );
    }

    let mut unknown_disposition = encoded.clone();
    unknown_disposition[289] = 99;
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&unknown_disposition),
        Err(record_error(
            OptimizedOrdinaryCallableEntryDecodeError::UnknownDisposition(99),
        )),
    );

    for offset in 290..295 {
        let mut unavailable = encoded.clone();
        unavailable[offset] = 99;
        assert_eq!(
            OptimizedOrdinaryCallableEntryManifest::decode(&unavailable),
            Err(OptimizedOrdinaryCallableEntryManifestDecodeError::UnknownUnavailableStatus),
            "unavailable field at wire offset {offset} must fail closed",
        );
    }

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&trailing),
        Err(OptimizedOrdinaryCallableEntryManifestDecodeError::TrailingBytes),
    );
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&encoded[..encoded.len() - 1]),
        Err(OptimizedOrdinaryCallableEntryManifestDecodeError::Truncated),
    );
}
