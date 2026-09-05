use omega_optimization_core::PrePhysicalOptimizationManifestIdentity;
use omega_selected_instructions_to_register_homes::FixedViewCopyIdentity;

use super::fixture::{staged, validate};
use crate::tests::*;

#[test]
fn every_authoritative_reconstruction_input_is_independently_bound() {
    let source = staged(NativeTarget::linux_x64());
    let donor = staged(NativeTarget::linux_arm64());
    let candidate = source.post_allocation_manifest().record();
    assert_eq!(
        validate(&source, candidate).unwrap(),
        *source.post_allocation_manifest()
    );

    let legality = source.legality_stage();
    let ranges = legality.live_range_stage();
    assert_eq!(
        validate_post_allocation_optimization_manifest(
            candidate,
            PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"foreign root"),
            &[],
            ranges.ranges(),
            legality.legality(),
            source.homes(),
        ),
        Err(PostAllocationOptimizationManifestError::ContentMismatch)
    );
    assert_eq!(
        validate_post_allocation_optimization_manifest(
            candidate,
            source.custody().manifest(),
            &[PostAllocationSelectedTransformation::FixedViewCopy(
                FixedViewCopyIdentity::from_bytes([0x81; 32]),
            )],
            ranges.ranges(),
            legality.legality(),
            source.homes(),
        ),
        Err(PostAllocationOptimizationManifestError::ContentMismatch)
    );

    let donor_legality = donor.legality_stage();
    let donor_ranges = donor_legality.live_range_stage();
    for result in [
        validate_post_allocation_optimization_manifest(
            candidate,
            source.custody().manifest(),
            &[],
            donor_ranges.ranges(),
            legality.legality(),
            source.homes(),
        ),
        validate_post_allocation_optimization_manifest(
            candidate,
            source.custody().manifest(),
            &[],
            ranges.ranges(),
            donor_legality.legality(),
            source.homes(),
        ),
        validate_post_allocation_optimization_manifest(
            candidate,
            source.custody().manifest(),
            &[],
            ranges.ranges(),
            legality.legality(),
            donor.homes(),
        ),
    ] {
        assert_eq!(
            result,
            Err(PostAllocationOptimizationManifestError::RootMismatch)
        );
    }
}
