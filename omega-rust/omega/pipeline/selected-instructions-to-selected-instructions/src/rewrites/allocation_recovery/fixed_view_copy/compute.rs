//! Fixed-view-copy proposal assembly and exact policy application loop.

mod apply;
mod preflight;
mod shared_entry;

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

use apply::{apply_copy, is_u64};
use preflight::{
    copy_row, find_leaf_block, next_instruction_id, next_register_id, validate_roots, work_usage,
};
use shared_entry::build_shared_entry_copy;

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
        let mut next_instruction = next_instruction_id(function_index, source_function)?;
        let mut next_register = next_register_id(function_index, source_function)?;
        if policy == FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 {
            if let Some(copy) = build_shared_entry_copy(
                function_index,
                source_function,
                &boundaries,
                copy_row,
                selected_keys.copy_i64,
                next_instruction,
                next_register,
            )? {
                apply_copy(
                    function_index,
                    &mut transformed.functions[function_index],
                    &copy,
                    copy_row,
                )?;
                copies.push(copy);
            }
            continue;
        }
        let mut destinations = BTreeSet::new();
        for boundary in boundaries {
            let VirtualFixedConstraintSite::Operand {
                instruction,
                operand,
                access: RegisterOperandAccess::Use,
                ..
            } = boundary.site
            else {
                return Err(FixedViewCopyError::UnsupportedTransitionSite {
                    function: function_index,
                    register: boundary.virtual_register.0,
                });
            };
            if !destinations.insert((instruction, operand)) {
                return Err(FixedViewCopyError::NonCanonicalCopies);
            }
            let source_register = source_function
                .virtual_registers
                .get(usize::try_from(boundary.virtual_register.0).map_err(|_| {
                    FixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: boundary.virtual_register.0,
                    }
                })?)
                .filter(|register| {
                    register.id == boundary.virtual_register
                        && register.class == boundary.class
                        && register.entry_fixed_view == Some(boundary.from_view)
                })
                .ok_or(FixedViewCopyError::UnsupportedSourceRegister {
                    function: function_index,
                    register: boundary.virtual_register.0,
                })?;
            let VirtualRegisterOrigin::EntryParameter { source_value, .. } = source_register.origin
            else {
                return Err(FixedViewCopyError::UnsupportedSourceRegister {
                    function: function_index,
                    register: boundary.virtual_register.0,
                });
            };
            if !is_u64(source_register.scalar_type)
                || copy_row.operands[0].class != source_register.class
                || copy_row.operands[1].class != source_register.class
            {
                return Err(FixedViewCopyError::UnsupportedSourceRegister {
                    function: function_index,
                    register: boundary.virtual_register.0,
                });
            }
            let function_u32 = u32::try_from(function_index).map_err(|_| {
                FixedViewCopyError::IdentifierOverflow {
                    function: function_index,
                }
            })?;
            let copy = FixedViewCopy {
                function: function_u32,
                machine: source_function.machine,
                source_virtual_register: source_register.id,
                source_value,
                source_definition_site: source_register.definition_site,
                from_view: boundary.from_view,
                to_view: boundary.to_view,
                insertion_block: {
                    let block = find_leaf_block(
                        function_index,
                        source_function,
                        instruction,
                        operand,
                        source_register.id,
                        boundary.to_view,
                    )?;
                    if block != boundary.block {
                        return Err(FixedViewCopyError::SegmentEvidenceMismatch);
                    }
                    block
                },
                before_instruction: instruction,
                destinations: vec![FixedViewCopyDestination {
                    site: boundary.site,
                    block: boundary.block,
                    view: boundary.to_view,
                }],
                copy_instruction: SelectedInstructionId(next_instruction),
                result_virtual_register: VirtualRegisterId(next_register),
                copy_constraint: selected_keys.copy_i64,
            };
            apply_copy(
                function_index,
                &mut transformed.functions[function_index],
                &copy,
                copy_row,
            )?;
            copies.push(copy);
            next_instruction =
                next_instruction
                    .checked_add(1)
                    .ok_or(FixedViewCopyError::IdentifierOverflow {
                        function: function_index,
                    })?;
            next_register =
                next_register
                    .checked_add(1)
                    .ok_or(FixedViewCopyError::IdentifierOverflow {
                        function: function_index,
                    })?;
        }
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
