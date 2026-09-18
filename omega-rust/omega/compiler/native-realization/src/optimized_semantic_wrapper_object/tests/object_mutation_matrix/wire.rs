//! ProgramStorage wrapper object wire envelope and closed wire-axis mutations.
//!
//! The container payload is bound by the embedded object identity, so any
//! representable-content byte substitution fails the identity check and any
//! canonical-axis byte (magic, version, vocabulary, target contract, option
//! and role tags, symbol/machine ids, the resolution-state marker, or string
//! length) fails decoding outright. Fields that `encode_*` refuses to emit are
//! additionally replayed through a hand-crafted envelope carrying a consistent
//! recomputed identity, proving decode-level canonical enforcement
//! (`InvalidObject`) independently of the encoding-time check.
use super::super::super::{
    OptimizedProgramStorageSemanticWrapperObjectDecodeError,
    OptimizedProgramStorageSemanticWrapperObjectSymbolRole,
};
use super::super::{
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    decode_optimized_program_storage_semantic_wrapper_object,
};
use super::fixture::staged_parts;
use super::wire_offsets::wire_offsets;
use crate::optimized_semantic_wrapper_object::codec::encode_plan_content;
use crate::optimized_semantic_wrapper_object::model::{CODEC_VERSION, CONTAINER_MAGIC};

fn assert_decode_error(
    baseline: &[u8],
    mutate: impl FnOnce(&mut Vec<u8>),
    expected: OptimizedProgramStorageSemanticWrapperObjectDecodeError,
) {
    let mut encoded = baseline.to_vec();
    mutate(&mut encoded);
    assert_eq!(
        decode_optimized_program_storage_semantic_wrapper_object(&encoded),
        Err(expected),
    );
}

/// Encodes a plan envelope by hand so the embedded identity stays consistent
/// with substituted content; only `validate_object` inside the decoder can
/// still reject it.
fn craft_envelope(object: &OptimizedProgramStorageSemanticWrapperObjectPlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(CONTAINER_MAGIC);
    bytes.extend_from_slice(&CODEC_VERSION.to_le_bytes());
    bytes.extend_from_slice(&object.identity.bytes());
    bytes.extend_from_slice(&encode_plan_content(object).unwrap());
    bytes
}

#[test]
fn wrapper_object_wire_rejects_every_closed_axis_and_envelope_mutation() {
    let (object, container, _, _) = staged_parts();
    let encoded = &container.bytes;
    let offsets = wire_offsets(encoded, &object);

    assert_decode_error(
        encoded,
        |bytes| bytes[0] ^= 1,
        OptimizedProgramStorageSemanticWrapperObjectDecodeError::WrongMagic,
    );
    assert_decode_error(
        encoded,
        |bytes| bytes[8..12].copy_from_slice(&99_u32.to_le_bytes()),
        OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnsupportedVersion(99),
    );
    assert_decode_error(
        encoded,
        |bytes| bytes[offsets.identity] ^= 1,
        OptimizedProgramStorageSemanticWrapperObjectDecodeError::IdentityMismatch,
    );
    assert_decode_error(
        encoded,
        |bytes| bytes[offsets.vocabulary..offsets.vocabulary + 2].fill(0),
        OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidVocabulary,
    );
    for offset in [offsets.architecture, offsets.object_format] {
        assert_decode_error(
            encoded,
            |bytes| bytes[offset] = 99,
            OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidTarget,
        );
    }
    for offset in [offsets.pointer_size, offsets.pointer_alignment] {
        assert_decode_error(
            encoded,
            |bytes| bytes[offset..offset + 8].copy_from_slice(&4_u64.to_le_bytes()),
            OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidTarget,
        );
    }

    // Symbol rows: zero ids, unknown option/role tags, a zero machine, and
    // non-UTF-8 names are all non-canonical on the wire.
    for row in offsets.symbol_rows {
        assert_decode_error(
            encoded,
            |bytes| bytes[row.symbol..row.symbol + 8].fill(0),
            OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidSymbol,
        );
        assert_decode_error(
            encoded,
            |bytes| bytes[row.source_function_index_tag] = 2,
            OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag,
        );
        assert_decode_error(
            encoded,
            |bytes| bytes[row.machine_tag] = 2,
            OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag,
        );
        assert_decode_error(
            encoded,
            |bytes| bytes[row.name] = 0xff,
            OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidUtf8,
        );
        assert_decode_error(
            encoded,
            |bytes| bytes[row.role] = 9,
            OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag,
        );
    }
    let continuation_machine = offsets.symbol_rows[1].machine.unwrap();
    assert_decode_error(
        encoded,
        |bytes| bytes[continuation_machine..continuation_machine + 8].fill(0),
        OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidMachine,
    );

    // A representable tag substitution (still a known role) survives decoding
    // of the field but fails the embedded identity join.
    assert_decode_error(
        encoded,
        |bytes| bytes[offsets.symbol_rows[0].role] = 2,
        OptimizedProgramStorageSemanticWrapperObjectDecodeError::IdentityMismatch,
    );
    assert_decode_error(
        encoded,
        |bytes| bytes[offsets.symbol_rows[1].role] = 1,
        OptimizedProgramStorageSemanticWrapperObjectDecodeError::IdentityMismatch,
    );

    for offset in [offsets.wrapper_symbol, offsets.continuation_symbol] {
        assert_decode_error(
            encoded,
            |bytes| bytes[offset..offset + 8].fill(0),
            OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidSymbol,
        );
    }
    assert_decode_error(
        encoded,
        |bytes| bytes[offsets.call_state] = 2,
        OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag,
    );

    // Any payload-byte substitution — even one a schema-only reader would
    // accept — fails the embedded object identity.
    assert_decode_error(
        encoded,
        |bytes| bytes[offsets.displacement] ^= 1,
        OptimizedProgramStorageSemanticWrapperObjectDecodeError::IdentityMismatch,
    );
    assert_decode_error(
        encoded,
        |bytes| bytes[offsets.text_bytes] ^= 1,
        OptimizedProgramStorageSemanticWrapperObjectDecodeError::IdentityMismatch,
    );

    // A string length that runs past the envelope is non-canonical.
    assert_decode_error(
        encoded,
        |bytes| {
            bytes[offsets.text_section_name_len..offsets.text_section_name_len + 8]
                .copy_from_slice(&u64::MAX.to_le_bytes())
        },
        OptimizedProgramStorageSemanticWrapperObjectDecodeError::Truncated,
    );

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        decode_optimized_program_storage_semantic_wrapper_object(&trailing),
        Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::TrailingBytes),
    );
    assert_eq!(
        decode_optimized_program_storage_semantic_wrapper_object(&encoded[..encoded.len() - 1]),
        Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::Truncated),
    );
}

#[test]
fn wrapper_object_wire_rejects_noncanonical_plans_with_consistent_identity() {
    let (object, _, _, _) = staged_parts();

    // A closed-field substitution that `encode_*` rejects still cannot pass
    // decoding when its recomputed identity is embedded consistently.
    let mut renamed = object.clone();
    renamed.text_section_name = ".omega_bad".into();
    renamed.identity = renamed.recomputed_identity().unwrap();
    assert_eq!(
        decode_optimized_program_storage_semantic_wrapper_object(&craft_envelope(&renamed)),
        Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidObject),
    );

    let mut displaced = object.clone();
    displaced.call_resolution.displacement = 6;
    displaced.identity = displaced.recomputed_identity().unwrap();
    assert_eq!(
        decode_optimized_program_storage_semantic_wrapper_object(&craft_envelope(&displaced)),
        Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidObject),
    );

    let mut reclassified = object.clone();
    reclassified.symbols[1].role =
        OptimizedProgramStorageSemanticWrapperObjectSymbolRole::SemanticWrapperV1;
    reclassified.identity = reclassified.recomputed_identity().unwrap();
    assert_eq!(
        decode_optimized_program_storage_semantic_wrapper_object(&craft_envelope(&reclassified)),
        Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::InvalidObject),
    );
}
