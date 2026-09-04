//! Canonical callable-record envelope and closed wire-axis mutations.

use crate::tests::*;

use super::fixture::staged_callable;
use super::wire_offsets::record_wire_offsets;

fn assert_decode_error(
    baseline: &[u8],
    mutate: impl FnOnce(&mut Vec<u8>),
    expected: OptimizedOrdinaryCallableEntryDecodeError,
) {
    let mut encoded = baseline.to_vec();
    mutate(&mut encoded);
    assert_eq!(
        OptimizedOrdinaryCallableEntryRecord::decode(&encoded),
        Err(expected),
    );
}

#[test]
fn ordinary_callable_record_wire_rejects_every_closed_axis_and_envelope_mutation() {
    let staged = staged_callable();
    let encoded = staged.entry().encode().unwrap();
    let offsets = record_wire_offsets(&encoded);

    assert_decode_error(
        &encoded,
        |bytes| bytes[0] ^= 1,
        OptimizedOrdinaryCallableEntryDecodeError::WrongMagic,
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[8..12].copy_from_slice(&99_u32.to_le_bytes()),
        OptimizedOrdinaryCallableEntryDecodeError::UnsupportedVersion(99),
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[12] ^= 1,
        OptimizedOrdinaryCallableEntryDecodeError::IdentityMismatch,
    );
    assert_decode_error(
        &encoded,
        |bytes| {
            bytes[offsets.vocabulary..offsets.vocabulary + 2].copy_from_slice(&59_u16.to_le_bytes())
        },
        OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.architecture] = 99,
        OptimizedOrdinaryCallableEntryDecodeError::UnknownArchitecture(99),
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.object_format] = 99,
        OptimizedOrdinaryCallableEntryDecodeError::UnknownObjectFormat(99),
    );
    for offset in [offsets.pointer_size, offsets.pointer_alignment] {
        assert_decode_error(
            &encoded,
            |bytes| bytes[offset..offset + 8].copy_from_slice(&4_u64.to_le_bytes()),
            OptimizedOrdinaryCallableEntryDecodeError::InvalidTarget,
        );
    }
    assert_decode_error(
        &encoded,
        |bytes| {
            bytes[offsets.architecture] = 2;
            bytes[offsets.object_format] = 3;
        },
        OptimizedOrdinaryCallableEntryDecodeError::InvalidTarget,
    );
    for offset in [
        offsets.semantic_entry,
        offsets.semantic_entry_symbol,
        offsets.parameter_value,
    ] {
        assert_decode_error(
            &encoded,
            |bytes| bytes[offset..offset + 8].copy_from_slice(&0_u64.to_le_bytes()),
            OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
        );
    }
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.symbol_name_byte] = 0xff,
        OptimizedOrdinaryCallableEntryDecodeError::InvalidUtf8,
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.calling_policy] = 99,
        OptimizedOrdinaryCallableEntryDecodeError::UnknownCallingPolicy(99),
    );
    assert_decode_error(
        &encoded,
        |bytes| {
            bytes[offsets.parameter_ordinal..offsets.parameter_ordinal + 8]
                .copy_from_slice(&1_u64.to_le_bytes())
        },
        OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.scalar_tag] = 99,
        OptimizedOrdinaryCallableEntryDecodeError::InvalidScalarType,
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.integer_carrier] = 99,
        OptimizedOrdinaryCallableEntryDecodeError::InvalidIntegerType,
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.integer_sign] = 99,
        OptimizedOrdinaryCallableEntryDecodeError::InvalidIntegerType,
    );
    assert_decode_error(
        &encoded,
        |bytes| {
            bytes[offsets.integer_bits..offsets.integer_bits + 2]
                .copy_from_slice(&0_u16.to_le_bytes())
        },
        OptimizedOrdinaryCallableEntryDecodeError::InvalidIntegerType,
    );
    assert_decode_error(
        &encoded,
        |bytes| {
            bytes[offsets.integer_scalar_tag] = 3;
            bytes[offsets.integer_carrier] = 99;
        },
        OptimizedOrdinaryCallableEntryDecodeError::InvalidScalarType,
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.shape_tag] = 99,
        OptimizedOrdinaryCallableEntryDecodeError::InvalidScalarType,
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.register_tag] = 99,
        OptimizedOrdinaryCallableEntryDecodeError::UnknownRegister(99),
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.register_index] = 1,
        OptimizedOrdinaryCallableEntryDecodeError::UnknownRegister(encoded[offsets.register_tag]),
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.exit_policy] = 99,
        OptimizedOrdinaryCallableEntryDecodeError::UnknownExitPolicy(99),
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.hardening] = 99,
        OptimizedOrdinaryCallableEntryDecodeError::UnknownHardening(99),
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.entry_assumption] = 99,
        OptimizedOrdinaryCallableEntryDecodeError::UnknownEntryAssumption(99),
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.disposition] = 99,
        OptimizedOrdinaryCallableEntryDecodeError::UnknownDisposition(99),
    );

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        OptimizedOrdinaryCallableEntryRecord::decode(&trailing),
        Err(OptimizedOrdinaryCallableEntryDecodeError::TrailingBytes),
    );
    assert_eq!(
        OptimizedOrdinaryCallableEntryRecord::decode(&encoded[..encoded.len() - 1]),
        Err(OptimizedOrdinaryCallableEntryDecodeError::Truncated),
    );
}
