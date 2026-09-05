use optimization_core::{
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::SelectedInstructionPlanIdentity;
use target::NativeTarget;

use super::*;
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedViewCopyIdentity,
    LiteralFoldIdentity, LiveRangeIdentity, LivenessIdentity, PressureRematerializationIdentity,
    RegisterHomeIdentity,
};

type Mutation = fn(&mut PostAllocationOptimizationManifest);

fn record() -> PostAllocationOptimizationManifest {
    let mut record = PostAllocationOptimizationManifest {
        identity: PostAllocationOptimizationManifestIdentity::from_canonical_bytes(b"pending"),
        stage: PostAllocationManifestStage::ValidatedRegisterHomes,
        pre_physical: PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"pre"),
        target: NativeTarget::linux_x64(),
        selected: SelectedInstructionPlanIdentity::from_canonical_bytes(b"selected"),
        selected_lowering_completion: None,
        selected_transformations: Vec::new(),
        liveness: LivenessIdentity([1; 32]),
        ranges: LiveRangeIdentity::from_bytes([2; 32]),
        legality: AllocationLegalityIdentity::from_bytes([3; 32]),
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
        allocator_availability: AllocatorAvailabilityIdentity::from_bytes([6; 32]),
        homes: RegisterHomeIdentity::from_bytes([5; 32]),
        spills: PostAllocationSpillStatus::NotRequiredForValidatedHomePlan,
        frame: PostAllocationUnavailableData::Unavailable,
        emission: PostAllocationUnavailableData::Unavailable,
        publication: PostAllocationUnavailableData::Unavailable,
        statistics: PostAllocationStatistics {
            functions: 1,
            structural_unit_functions: 2,
            assignments: 2,
            distinct_physical_views: 2,
            virtual_interferences: 1,
            fixed_view_transitions: 0,
        },
    };
    record.identity = record.recomputed_identity();
    record
}

#[test]
fn identity_binds_every_post_allocation_domain() {
    let baseline = record();
    assert_eq!(baseline.identity, baseline.recomputed_identity());
    let mutations: Vec<Mutation> = vec![
        |record| {
            record.pre_physical =
                PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"other")
        },
        |record| record.target = NativeTarget::linux_arm64(),
        |record| record.selected = SelectedInstructionPlanIdentity::from_canonical_bytes(b"other"),
        |record| {
            record.selected_lowering_completion = Some(
                SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(b"completed"),
            )
        },
        |record| {
            record.selected_transformations.push(
                PostAllocationSelectedTransformation::FixedViewCopy(FixedViewCopyIdentity([6; 32])),
            )
        },
        |record| {
            record
                .selected_transformations
                .push(PostAllocationSelectedTransformation::LiteralFold(
                    LiteralFoldIdentity::from_bytes([13; 32]),
                ))
        },
        |record| {
            record.selected_transformations.push(
                PostAllocationSelectedTransformation::PressureRematerialization(
                    PressureRematerializationIdentity::from_bytes([14; 32]),
                ),
            )
        },
        |record| record.liveness = LivenessIdentity([7; 32]),
        |record| record.ranges = LiveRangeIdentity::from_bytes([8; 32]),
        |record| record.legality = AllocationLegalityIdentity::from_bytes([9; 32]),
        |record| {
            record.register_environment = TargetRegisterEnvironmentIdentity::from_bytes([10; 32])
        },
        |record| {
            record.allocator_availability = AllocatorAvailabilityIdentity::from_bytes([12; 32])
        },
        |record| record.homes = RegisterHomeIdentity::from_bytes([11; 32]),
        |record| record.statistics.functions += 1,
        |record| record.statistics.structural_unit_functions += 1,
        |record| record.statistics.assignments += 1,
        |record| record.statistics.distinct_physical_views += 1,
        |record| record.statistics.virtual_interferences += 1,
        |record| record.statistics.fixed_view_transitions += 1,
    ];
    for mutate in mutations {
        let mut changed = baseline.clone();
        mutate(&mut changed);
        assert_ne!(baseline.identity, changed.recomputed_identity());
    }
    let text = baseline.render_text();
    assert!(text.contains("spills: not required"));
    assert!(text.contains("publication: unavailable"));
    assert!(text.contains("structural Unit functions: 2"));
    let mut rematerialized = baseline.clone();
    rematerialized.selected_transformations = vec![
        PostAllocationSelectedTransformation::PressureRematerialization(
            PressureRematerializationIdentity::from_bytes([14; 32]),
        ),
    ];
    rematerialized.identity = rematerialized.recomputed_identity();
    assert!(
        rematerialized
            .render_text()
            .contains("pressure-rematerialization")
    );
}

#[test]
fn canonical_codec_round_trips_both_routes_and_rejects_corruption() {
    let direct = record();
    let encoded = direct.encode();
    assert_eq!(
        PostAllocationOptimizationManifest::decode(&encoded),
        Ok(direct)
    );

    let mut transformed = record();
    transformed.selected_lowering_completion =
        Some(SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(b"completed"));
    transformed.selected_transformations = vec![
        PostAllocationSelectedTransformation::FixedViewCopy(FixedViewCopyIdentity([12; 32])),
        PostAllocationSelectedTransformation::LiteralFold(LiteralFoldIdentity::from_bytes(
            [13; 32],
        )),
        PostAllocationSelectedTransformation::PressureRematerialization(
            PressureRematerializationIdentity::from_bytes([14; 32]),
        ),
    ];
    transformed.identity = transformed.recomputed_identity();
    assert_eq!(
        PostAllocationOptimizationManifest::decode(&transformed.encode()),
        Ok(transformed)
    );

    let mut identity_tamper = encoded.clone();
    identity_tamper[12] ^= 1;
    assert_eq!(
        PostAllocationOptimizationManifest::decode(&identity_tamper),
        Err(PostAllocationOptimizationManifestDecodeError::IdentityMismatch)
    );
    let mut structural_count_tamper = encoded.clone();
    let structural_count_offset = structural_count_tamper.len() - 5 * size_of::<u64>();
    structural_count_tamper[structural_count_offset] ^= 1;
    assert_eq!(
        PostAllocationOptimizationManifest::decode(&structural_count_tamper),
        Err(PostAllocationOptimizationManifestDecodeError::IdentityMismatch)
    );
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        PostAllocationOptimizationManifest::decode(&trailing),
        Err(PostAllocationOptimizationManifestDecodeError::TrailingBytes)
    );
    assert_eq!(
        PostAllocationOptimizationManifest::decode(&encoded[..encoded.len() - 1]),
        Err(PostAllocationOptimizationManifestDecodeError::Truncated)
    );
    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        PostAllocationOptimizationManifest::decode(&wrong_magic),
        Err(PostAllocationOptimizationManifestDecodeError::WrongMagic)
    );
    let mut wrong_version = encoded.clone();
    wrong_version[8..12].copy_from_slice(&5_u32.to_le_bytes());
    assert_eq!(
        PostAllocationOptimizationManifest::decode(&wrong_version),
        Err(PostAllocationOptimizationManifestDecodeError::UnsupportedVersion(5))
    );
    let content_offset = 8 + 4 + 32;
    let mut unknown_architecture = encoded.clone();
    unknown_architecture[content_offset + 1 + 32] = 9;
    assert_eq!(
        PostAllocationOptimizationManifest::decode(&unknown_architecture),
        Err(PostAllocationOptimizationManifestDecodeError::UnknownArchitecture(9))
    );
    let mut one_transformation = record();
    one_transformation.selected_transformations =
        vec![PostAllocationSelectedTransformation::FixedViewCopy(
            FixedViewCopyIdentity([12; 32]),
        )];
    one_transformation.identity = one_transformation.recomputed_identity();
    let mut unknown_transformation = one_transformation.encode();
    let transformation_tag_offset = content_offset + 1 + 32 + 18 + 32 + 1 + 8;
    unknown_transformation[transformation_tag_offset] = 9;
    assert_eq!(
        PostAllocationOptimizationManifest::decode(&unknown_transformation),
        Err(PostAllocationOptimizationManifestDecodeError::UnknownTransformationTag(9))
    );
}
