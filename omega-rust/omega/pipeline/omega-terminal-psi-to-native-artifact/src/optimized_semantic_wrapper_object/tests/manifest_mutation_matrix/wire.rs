//! ProgramStorage wrapper manifest envelope and closed wire-axis mutations.

use super::super::*;
use super::fixture::manifest_fixture;
use super::wire_offsets::wire_offsets;

fn assert_decode_error(
    baseline: &[u8],
    mutate: impl FnOnce(&mut Vec<u8>),
    expected: OptimizedProgramStorageSemanticWrapperObjectDecodeError,
) {
    let mut encoded = baseline.to_vec();
    mutate(&mut encoded);
    assert_eq!(
        OptimizedProgramStorageSemanticWrapperObjectManifest::decode(&encoded),
        Err(expected),
    );
}

#[test]
fn manifest_wire_rejects_every_closed_axis_and_envelope_mutation() {
    let (_, _, manifest) = manifest_fixture();
    let encoded = manifest.encode();
    let offsets = wire_offsets(&encoded);

    assert_decode_error(
        &encoded,
        |bytes| bytes[0] ^= 1,
        OptimizedProgramStorageSemanticWrapperObjectDecodeError::WrongMagic,
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[8..12].copy_from_slice(&99_u32.to_le_bytes()),
        OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnsupportedVersion(99),
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[12] ^= 1,
        OptimizedProgramStorageSemanticWrapperObjectDecodeError::IdentityMismatch,
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.stage] = 99,
        OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag,
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.vocabulary..offsets.vocabulary + 2].fill(0),
        OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidVocabulary,
    );
    for offset in [offsets.architecture, offsets.object_format] {
        assert_decode_error(
            &encoded,
            |bytes| bytes[offset] = 99,
            OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidTarget,
        );
    }
    for offset in [offsets.pointer_size, offsets.pointer_alignment] {
        assert_decode_error(
            &encoded,
            |bytes| bytes[offset..offset + 8].copy_from_slice(&4_u64.to_le_bytes()),
            OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidTarget,
        );
    }
    for offset in [offsets.wrapper_symbol, offsets.continuation_symbol] {
        assert_decode_error(
            &encoded,
            |bytes| bytes[offset..offset + 8].fill(0),
            OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidSymbol,
        );
    }
    for offset in offsets.unavailable {
        assert_decode_error(
            &encoded,
            |bytes| bytes[offset] = 99,
            OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag,
        );
    }

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        OptimizedProgramStorageSemanticWrapperObjectManifest::decode(&trailing),
        Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::TrailingBytes),
    );
    assert_eq!(
        OptimizedProgramStorageSemanticWrapperObjectManifest::decode(&encoded[..encoded.len() - 1]),
        Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::Truncated),
    );
}
