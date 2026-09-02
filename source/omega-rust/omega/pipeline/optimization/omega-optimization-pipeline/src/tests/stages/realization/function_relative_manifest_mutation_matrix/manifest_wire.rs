//! Function-relative V9 envelope and closed wire-axis mutations.

use crate::tests::*;

use super::fixture::{direct_rel8_realization, post_allocation_realization};
use super::wire_offsets::wire_offsets;

fn assert_decode_error(
    baseline: &[u8],
    mutate: impl FnOnce(&mut Vec<u8>),
    expected: FunctionRelativeOptimizationRealizationManifestDecodeError,
) {
    let mut encoded = baseline.to_vec();
    mutate(&mut encoded);
    assert_eq!(
        FunctionRelativeOptimizationRealizationManifest::decode(&encoded),
        Err(expected),
    );
}

#[test]
fn function_relative_v10_wire_rejects_every_closed_axis_and_envelope_mutation() {
    let staged = direct_rel8_realization();
    let encoded = staged.manifest().record().encode();
    let offsets = wire_offsets(&encoded);

    assert_decode_error(
        &encoded,
        |bytes| bytes[0] ^= 1,
        FunctionRelativeOptimizationRealizationManifestDecodeError::WrongMagic,
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[8..12].copy_from_slice(&99_u32.to_le_bytes()),
        FunctionRelativeOptimizationRealizationManifestDecodeError::UnsupportedVersion(99),
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[12] ^= 1,
        FunctionRelativeOptimizationRealizationManifestDecodeError::IdentityMismatch,
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.stage] = 99,
        FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownStage(99),
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.selected_completion_status] = 99,
        FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownSelectedLoweringCompletionStatus(99),
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.x86_relaxation_status] = 99,
        FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownX86BranchRelaxationStatus(99),
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.post_allocation_status] = 99,
        FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownPostAllocationMachineOptimizationStatus(99),
    );
    for (offset, expected) in [
        (
            offsets.architecture,
            FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownArchitecture(99),
        ),
        (
            offsets.object_format,
            FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownObjectFormat(99),
        ),
    ] {
        assert_decode_error(&encoded, |bytes| bytes[offset] = 99, expected);
    }
    for offset in [offsets.pointer_size, offsets.pointer_alignment] {
        assert_decode_error(
            &encoded,
            |bytes| bytes[offset..offset + 8].copy_from_slice(&4_u64.to_le_bytes()),
            FunctionRelativeOptimizationRealizationManifestDecodeError::IdentityMismatch,
        );
    }
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.layout_policy] = 99,
        FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownLayoutPolicy(99),
    );
    assert_decode_error(
        &encoded,
        |bytes| bytes[offsets.scope] = 99,
        FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownScope(99),
    );
    for offset in offsets.unavailable {
        assert_decode_error(
            &encoded,
            |bytes| bytes[offset] = 99,
            FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownUnavailableStatus(
                99,
            ),
        );
    }

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        FunctionRelativeOptimizationRealizationManifest::decode(&trailing),
        Err(FunctionRelativeOptimizationRealizationManifestDecodeError::TrailingBytes),
    );
    assert_eq!(
        FunctionRelativeOptimizationRealizationManifest::decode(&encoded[..encoded.len() - 1]),
        Err(FunctionRelativeOptimizationRealizationManifestDecodeError::Truncated),
    );
}

#[test]
fn post_allocation_wire_rejects_unknown_rule_and_conflicting_transformations() {
    let staged = post_allocation_realization();
    let encoded = staged.manifest().record().encode();
    let offsets = wire_offsets(&encoded);
    let optimization = offsets
        .post_allocation_optimization
        .expect("post-allocation fixture status");
    assert_decode_error(
        &encoded,
        |bytes| bytes[optimization] = 99,
        FunctionRelativeOptimizationRealizationManifestDecodeError::UnknownPostAllocationMachineOptimization(99),
    );

    let mut conflicting = encoded.clone();
    conflicting[offsets.x86_relaxation_status] = 1;
    conflicting.splice(
        offsets.x86_relaxation_status + 1..offsets.x86_relaxation_status + 1,
        [0xee; 32],
    );
    assert_eq!(
        FunctionRelativeOptimizationRealizationManifest::decode(&conflicting),
        Err(
            FunctionRelativeOptimizationRealizationManifestDecodeError::ConflictingPhysicalTransformations,
        ),
    );
}
