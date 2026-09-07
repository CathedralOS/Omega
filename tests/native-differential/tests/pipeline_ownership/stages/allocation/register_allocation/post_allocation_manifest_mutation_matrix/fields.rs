use optimization_core::{
    PrePhysicalOptimizationManifestIdentity, SelectedLoweringOptimizationCompletionIdentity,
};
use register_homes::{AllocationLegalityIdentity, AllocatorAvailabilityIdentity};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::SelectedInstructionPlanIdentity;
use selected_instructions::{LiveRangeIdentity, LivenessIdentity};
use selected_instructions_to_register_homes::{
    FixedViewCopyIdentity, LiteralFoldIdentity, PressureRematerializationIdentity,
    RegisterHomeIdentity,
};
use target::{Architecture, ObjectFormat};

use super::fixture::{staged, validate};
use crate::tests::*;

type Mutation = fn(&mut PostAllocationOptimizationManifest);

#[test]
fn every_mutable_logical_field_is_bound_by_independent_reconstruction() {
    let source = staged(NativeTarget::linux_x64());
    let baseline = source.post_allocation_manifest().record();
    assert_eq!(
        MUTATIONS.len(),
        22,
        "complete representable mutation-case matrix"
    );

    for (name, mutate) in MUTATIONS {
        let mut candidate = baseline.clone();
        mutate(&mut candidate);
        candidate.identity = candidate.recomputed_identity();
        assert_eq!(
            PostAllocationOptimizationManifest::decode(&candidate.encode()),
            Ok(candidate.clone()),
            "mutation `{name}` must remain a canonical V6 record"
        );
        assert_eq!(
            validate(&source, &candidate),
            Err(PostAllocationOptimizationManifestError::ContentMismatch),
            "independent reconstruction must reject mutation `{name}`"
        );
    }
}

#[test]
fn stale_outer_identity_fails_both_codec_and_independent_admission() {
    let source = staged(NativeTarget::linux_x64());
    let mut candidate = source.post_allocation_manifest().record().clone();
    candidate.statistics.assignments += 1;

    assert_eq!(
        PostAllocationOptimizationManifest::decode(&candidate.encode()),
        Err(selected_instructions_to_register_homes::PostAllocationOptimizationManifestDecodeError::IdentityMismatch)
    );
    assert_eq!(
        validate(&source, &candidate),
        Err(PostAllocationOptimizationManifestError::IdentityMismatch)
    );
}

const MUTATIONS: &[(&str, Mutation)] = &[
    ("pre_physical", |record| {
        record.pre_physical =
            PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"mutated prephysical")
    }),
    ("target.architecture", |record| {
        record.target.architecture = Architecture::Aarch64
    }),
    ("target.object_format", |record| {
        record.target.object_format = ObjectFormat::MachO
    }),
    ("target.pointer_size", |record| {
        record.target.pointer_size += 1
    }),
    ("target.pointer_alignment", |record| {
        record.target.pointer_alignment += 1
    }),
    ("selected", |record| {
        record.selected = SelectedInstructionPlanIdentity::from_canonical_bytes(b"mutated selected")
    }),
    ("selected_lowering_completion", |record| {
        record.selected_lowering_completion = Some(
            SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(
                b"mutated completion",
            ),
        )
    }),
    ("selected_transformations.fixed_view_copy", |record| {
        record.selected_transformations = vec![PostAllocationSelectedTransformation::FixedViewCopy(
            FixedViewCopyIdentity::from_bytes([0x41; 32]),
        )]
    }),
    ("selected_transformations.literal_fold", |record| {
        record.selected_transformations = vec![PostAllocationSelectedTransformation::LiteralFold(
            LiteralFoldIdentity::from_bytes([0x42; 32]),
        )]
    }),
    ("selected_transformations.rematerialization", |record| {
        record.selected_transformations = vec![
            PostAllocationSelectedTransformation::PressureRematerialization(
                PressureRematerializationIdentity::from_bytes([0x43; 32]),
            ),
        ]
    }),
    ("liveness", |record| {
        record.liveness = LivenessIdentity::from_bytes([0x44; 32])
    }),
    ("ranges", |record| {
        record.ranges = LiveRangeIdentity::from_bytes([0x45; 32])
    }),
    ("legality", |record| {
        record.legality = AllocationLegalityIdentity::from_bytes([0x46; 32])
    }),
    ("register_environment", |record| {
        record.register_environment = TargetRegisterEnvironmentIdentity::from_bytes([0x47; 32])
    }),
    ("allocator_availability", |record| {
        record.allocator_availability = AllocatorAvailabilityIdentity::from_bytes([0x48; 32])
    }),
    ("homes", |record| {
        record.homes = RegisterHomeIdentity::from_bytes([0x49; 32])
    }),
    ("statistics.functions", |record| {
        record.statistics.functions += 1
    }),
    ("statistics.structural_unit_functions", |record| {
        record.statistics.structural_unit_functions += 1
    }),
    ("statistics.assignments", |record| {
        record.statistics.assignments += 1
    }),
    ("statistics.distinct_physical_views", |record| {
        record.statistics.distinct_physical_views += 1
    }),
    ("statistics.virtual_interferences", |record| {
        record.statistics.virtual_interferences += 1
    }),
    ("statistics.fixed_view_transitions", |record| {
        record.statistics.fixed_view_transitions += 1
    }),
];
