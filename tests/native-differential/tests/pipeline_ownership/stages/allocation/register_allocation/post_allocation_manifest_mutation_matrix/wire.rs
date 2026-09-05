use omega_selected_instructions_to_register_homes::{
    FixedViewCopyIdentity, PostAllocationOptimizationManifestDecodeError,
};

use super::fixture::staged;
use super::wire_offsets::locate;
use crate::tests::*;

#[test]
fn every_closed_wire_tag_and_envelope_fails_with_its_exact_error() {
    let encoded = staged(NativeTarget::linux_x64())
        .post_allocation_manifest()
        .record()
        .encode();
    let offsets = locate(&encoded);

    corrupt(
        &encoded,
        0,
        0xff,
        PostAllocationOptimizationManifestDecodeError::WrongMagic,
    );
    let mut version = encoded.clone();
    version[8..12].copy_from_slice(&99_u32.to_le_bytes());
    assert_decode_error(
        version,
        PostAllocationOptimizationManifestDecodeError::UnsupportedVersion(99),
    );
    corrupt(
        &encoded,
        12,
        encoded[12] ^ 1,
        PostAllocationOptimizationManifestDecodeError::IdentityMismatch,
    );
    corrupt(
        &encoded,
        offsets.stage,
        99,
        PostAllocationOptimizationManifestDecodeError::UnknownStage(99),
    );
    corrupt(
        &encoded,
        offsets.architecture,
        99,
        PostAllocationOptimizationManifestDecodeError::UnknownArchitecture(99),
    );
    corrupt(
        &encoded,
        offsets.object_format,
        99,
        PostAllocationOptimizationManifestDecodeError::UnknownObjectFormat(99),
    );
    corrupt(
        &encoded,
        offsets.completion,
        99,
        PostAllocationOptimizationManifestDecodeError::UnknownCompletionStatus(99),
    );
    corrupt(
        &encoded,
        offsets.spills,
        99,
        PostAllocationOptimizationManifestDecodeError::UnknownSpillStatus(99),
    );
    for offset in [offsets.frame, offsets.emission, offsets.publication] {
        corrupt(
            &encoded,
            offset,
            99,
            PostAllocationOptimizationManifestDecodeError::UnknownUnavailableStatus(99),
        );
    }

    let mut transformed = staged(NativeTarget::linux_x64())
        .post_allocation_manifest()
        .record()
        .clone();
    transformed.selected_transformations =
        vec![PostAllocationSelectedTransformation::FixedViewCopy(
            FixedViewCopyIdentity::from_bytes([0x71; 32]),
        )];
    transformed.identity = transformed.recomputed_identity();
    let transformed = transformed.encode();
    let transformation = locate(&transformed)
        .first_transformation
        .expect("transformed fixture has one row");
    corrupt(
        &transformed,
        transformation,
        99,
        PostAllocationOptimizationManifestDecodeError::UnknownTransformationTag(99),
    );
}

#[test]
fn framing_rejects_truncation_and_trailing_bytes() {
    let encoded = staged(NativeTarget::linux_x64())
        .post_allocation_manifest()
        .record()
        .encode();
    assert_decode_error(
        encoded[..encoded.len() - 1].to_vec(),
        PostAllocationOptimizationManifestDecodeError::Truncated,
    );
    let mut trailing = encoded;
    trailing.push(0);
    assert_decode_error(
        trailing,
        PostAllocationOptimizationManifestDecodeError::TrailingBytes,
    );
}

fn corrupt(
    encoded: &[u8],
    offset: usize,
    value: u8,
    expected: PostAllocationOptimizationManifestDecodeError,
) {
    let mut corrupted = encoded.to_vec();
    corrupted[offset] = value;
    assert_decode_error(corrupted, expected);
}

fn assert_decode_error(encoded: Vec<u8>, expected: PostAllocationOptimizationManifestDecodeError) {
    assert_eq!(
        PostAllocationOptimizationManifest::decode(&encoded),
        Err(expected)
    );
}
