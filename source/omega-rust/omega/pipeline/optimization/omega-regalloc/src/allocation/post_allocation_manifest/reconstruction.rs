//! Independent reconstruction of the expected manifest record.

use std::collections::BTreeSet;

use omega_optimization_core::{
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};

use crate::{ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedRegisterHomes};

use super::{
    PostAllocationManifestStage, PostAllocationOptimizationManifest,
    PostAllocationOptimizationManifestError, PostAllocationSelectedTransformation,
    PostAllocationSpillStatus, PostAllocationStatistics, PostAllocationUnavailableData,
};

pub(super) fn expected_record(
    pre_physical: PrePhysicalOptimizationManifestIdentity,
    selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
    selected_transformations: &[PostAllocationSelectedTransformation],
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
) -> Result<PostAllocationOptimizationManifest, PostAllocationOptimizationManifestError> {
    let mut unique_transformations = BTreeSet::new();
    if selected_transformations.iter().any(|transformation| {
        let key = match transformation {
            PostAllocationSelectedTransformation::FixedViewCopy(identity) => {
                (1_u8, identity.bytes())
            }
            PostAllocationSelectedTransformation::LiteralFold(identity) => (2_u8, identity.bytes()),
            PostAllocationSelectedTransformation::PressureRematerialization(identity) => {
                (3_u8, identity.bytes())
            }
        };
        !unique_transformations.insert(key)
    }) {
        return Err(PostAllocationOptimizationManifestError::NonCanonicalTransformationLedger);
    }
    if legality.receipt().ranges() != ranges.receipt().identity()
        || homes.receipt().legality() != legality.receipt().identity()
        || homes.receipt().ranges() != ranges.receipt().identity()
        || homes.receipt().register_environment() != legality.receipt().register_environment()
        || homes.receipt().allocator_availability() != legality.receipt().allocator_availability()
        || homes.plan().functions.len() != ranges.plan().functions.len()
        || homes.plan().functions.len() != legality.plan().functions.len()
        || homes.plan().structural_unit_functions.len()
            != ranges.plan().structural_unit_functions.len()
        || homes.plan().structural_unit_functions.len()
            != legality.plan().structural_unit_functions.len()
        || ranges.receipt().structural_unit_function_count()
            != ranges.plan().structural_unit_functions.len()
        || legality.receipt().structural_unit_function_count()
            != legality.plan().structural_unit_functions.len()
        || homes.receipt().structural_unit_function_count()
            != homes.plan().structural_unit_functions.len()
    {
        return Err(PostAllocationOptimizationManifestError::RootMismatch);
    }
    let transition_count = legality
        .plan()
        .functions
        .iter()
        .flat_map(|function| &function.virtual_registers)
        .map(|register| register.entry_transitions.len())
        .sum::<usize>();
    if transition_count != 0 {
        return Err(PostAllocationOptimizationManifestError::UnresolvedFixedViewTransitions);
    }
    let distinct_views = homes
        .plan()
        .functions
        .iter()
        .flat_map(|function| &function.assignments)
        .map(|assignment| assignment.view)
        .collect::<BTreeSet<_>>()
        .len();
    let interference_count = ranges
        .plan()
        .functions
        .iter()
        .map(|function| function.interference.len())
        .sum::<usize>();
    let statistics = PostAllocationStatistics {
        functions: count(homes.plan().functions.len())?,
        structural_unit_functions: count(homes.plan().structural_unit_functions.len())?,
        assignments: count(homes.receipt().assignment_count())?,
        distinct_physical_views: count(distinct_views)?,
        virtual_interferences: count(interference_count)?,
        fixed_view_transitions: 0,
    };
    let mut record = PostAllocationOptimizationManifest {
        identity: PostAllocationOptimizationManifestIdentity::from_canonical_bytes(b"pending"),
        stage: PostAllocationManifestStage::ValidatedRegisterHomes,
        pre_physical,
        target: ranges.plan().target,
        selected: ranges.plan().selected,
        selected_lowering_completion,
        selected_transformations: selected_transformations.to_vec(),
        liveness: ranges.receipt().liveness(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        register_environment: legality.receipt().register_environment(),
        allocator_availability: legality.receipt().allocator_availability(),
        homes: homes.receipt().identity(),
        spills: PostAllocationSpillStatus::NotRequiredForValidatedHomePlan,
        frame: PostAllocationUnavailableData::Unavailable,
        emission: PostAllocationUnavailableData::Unavailable,
        publication: PostAllocationUnavailableData::Unavailable,
        statistics,
    };
    record.identity = record.recomputed_identity();
    Ok(record)
}

fn count(value: usize) -> Result<u64, PostAllocationOptimizationManifestError> {
    u64::try_from(value).map_err(|_| PostAllocationOptimizationManifestError::StatisticsOverflow)
}
