//! Fixed-view-copy proposal assembly and exact policy application loop.

mod apply;
mod preflight;
mod site;
mod source_exit;

use std::collections::BTreeSet;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use selected_instructions::{
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedOperand, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerSign, ScalarType};
use target_operations_to_selected_instructions::ValidatedSelectedInstructions;

use crate::{
    FixedViewCopy, FixedViewCopyDestination, FixedViewCopyError, FixedViewCopyPlan,
    FixedViewCopyPolicy, FixedViewCopySourceEvidence, ValidatedAllocationLegality,
    ValidatedFixedPrecoloredIntervals, ValidatedFixedPrecoloredSegmentHomes,
    ValidatedFixedPrecoloredSplitRequirements, ValidatedLiveRanges, VirtualFixedConstraintSite,
};

use preflight::{copy_row, next_instruction_id, next_register_id, validate_roots, work_usage};
use site::build_site_copies;
use source_exit::build_source_exit_copies;

pub(crate) fn compute_terminal_fixed_view_copies(
    selected: &ValidatedSelectedInstructions,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
    requirements: &ValidatedFixedPrecoloredSplitRequirements,
    homes: &ValidatedFixedPrecoloredSegmentHomes,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: FixedViewCopyPolicy,
    budget: OptimizationWorkBudget,
) -> Result<FixedViewCopyPlan, FixedViewCopyError> {
    validate_roots(
        selected,
        ranges,
        legality,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    let evidence =
        super::evidence::derive_positionally(ranges, legality, fixed, requirements, homes)?;
    let copy_row = copy_row(constraints, selected_keys)?;
    let usage = super::work::combined_usage(
        evidence.usage,
        work_usage(selected, &evidence.boundaries, policy)?,
    )?;
    if !usage.within(budget) {
        return Err(FixedViewCopyError::BudgetExceeded {
            required: usage,
            budget,
        });
    }

    let mut transformed = selected.plan().clone();
    let mut copies = Vec::new();
    for (function_index, source_function) in selected.plan().functions.iter().enumerate() {
        let boundaries = evidence
            .boundaries
            .iter()
            .filter(|boundary| boundary.function == function_index)
            .collect::<Vec<_>>();
        let next_instruction = next_instruction_id(function_index, source_function)?;
        let next_register = next_register_id(function_index, source_function)?;
        if matches!(
            policy,
            FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1
                | FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1
        ) {
            copies.extend(build_source_exit_copies(
                function_index,
                source_function,
                &boundaries,
                &mut transformed.functions[function_index],
                copy_row,
                selected_keys.copy_i64,
                policy,
                next_instruction,
                next_register,
            )?);
            continue;
        }
        let leaf_local = policy == FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1;
        copies.extend(build_site_copies(
            function_index,
            source_function,
            &boundaries,
            &mut transformed.functions[function_index],
            copy_row,
            selected_keys.copy_i64,
            leaf_local,
            next_instruction,
            next_register,
        )?);
    }

    Ok(FixedViewCopyPlan {
        source_selected: selected.receipt().identity(),
        source_ranges: ranges.receipt().identity(),
        source_legality: legality.receipt().identity(),
        register_environment,
        allocator_availability: legality.receipt().allocator_availability(),
        source_evidence: FixedViewCopySourceEvidence::FixedPrecoloredSegmentHomesV1 {
            fixed_intervals: fixed.receipt().identity(),
            split_requirements: requirements.receipt().identity(),
            segment_homes: homes.receipt().identity(),
        },
        policy,
        budget,
        usage,
        copies,
        transformed: transformed.into(),
    })
}

#[cfg(test)]
pub(crate) mod tests;
