//! Fixed-view-copy proposal assembly and exact policy application loop.

mod apply;
mod preflight;
mod shared_entry;

use std::collections::BTreeSet;

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use omega_selected_instructions::{
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedOperand, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use omega_target_operations_to_selected_instructions::ValidatedSelectedInstructions;
use psi_core::{IntegerSign, ScalarType};

use crate::{
    FixedViewCopy, FixedViewCopyDestination, FixedViewCopyError, FixedViewCopyPlan,
    FixedViewCopyPolicy, FunctionAllocationLegality, ValidatedAllocationLegality,
    ValidatedLiveRanges, VirtualFixedConstraintSite,
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
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
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
    let copy_row = copy_row(constraints, selected_keys)?;
    let usage = work_usage(selected, legality, policy)?;
    if !usage.within(budget) {
        return Err(FixedViewCopyError::BudgetExceeded {
            required: usage,
            budget,
        });
    }

    let mut transformed = selected.plan().clone();
    let mut copies = Vec::new();
    for (function_index, (source_function, legality_function)) in selected
        .plan()
        .functions
        .iter()
        .zip(&legality.plan().functions)
        .enumerate()
    {
        if source_function.machine != legality_function.machine {
            return Err(FixedViewCopyError::FunctionMismatch {
                function: function_index,
            });
        }
        let mut next_instruction = next_instruction_id(function_index, source_function)?;
        let mut next_register = next_register_id(function_index, source_function)?;
        if policy == FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 {
            if let Some(copy) = build_shared_entry_copy(
                function_index,
                source_function,
                legality_function,
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
        for legality_register in &legality_function.virtual_registers {
            for transition in &legality_register.entry_transitions {
                let VirtualFixedConstraintSite::Operand {
                    instruction,
                    operand,
                    access: RegisterOperandAccess::Use,
                    ..
                } = transition.to_site
                else {
                    return Err(FixedViewCopyError::UnsupportedTransitionSite {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    });
                };
                if !destinations.insert((instruction, operand)) {
                    return Err(FixedViewCopyError::NonCanonicalCopies);
                }
                let source_register = source_function
                    .virtual_registers
                    .get(
                        usize::try_from(legality_register.virtual_register.0).map_err(|_| {
                            FixedViewCopyError::UnsupportedSourceRegister {
                                function: function_index,
                                register: legality_register.virtual_register.0,
                            }
                        })?,
                    )
                    .filter(|register| {
                        register.id == legality_register.virtual_register
                            && register.class == legality_register.class
                            && register.entry_fixed_view == Some(transition.from_view)
                    })
                    .ok_or(FixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    })?;
                let VirtualRegisterOrigin::EntryParameter { source_value, .. } =
                    source_register.origin
                else {
                    return Err(FixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    });
                };
                if !is_u64(source_register.scalar_type)
                    || copy_row.operands[0].class != source_register.class
                    || copy_row.operands[1].class != source_register.class
                {
                    return Err(FixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: legality_register.virtual_register.0,
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
                    from_view: transition.from_view,
                    to_view: transition.to_view,
                    insertion_block: find_leaf_block(
                        function_index,
                        source_function,
                        instruction,
                        operand,
                        source_register.id,
                        transition.to_view,
                    )?,
                    before_instruction: instruction,
                    destinations: vec![FixedViewCopyDestination {
                        site: transition.to_site,
                        block: find_leaf_block(
                            function_index,
                            source_function,
                            instruction,
                            operand,
                            source_register.id,
                            transition.to_view,
                        )?,
                        view: transition.to_view,
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
                next_instruction = next_instruction.checked_add(1).ok_or(
                    FixedViewCopyError::IdentifierOverflow {
                        function: function_index,
                    },
                )?;
                next_register =
                    next_register
                        .checked_add(1)
                        .ok_or(FixedViewCopyError::IdentifierOverflow {
                            function: function_index,
                        })?;
            }
        }
    }

    Ok(FixedViewCopyPlan {
        source_selected: selected.receipt().identity(),
        source_ranges: ranges.receipt().identity(),
        source_legality: legality.receipt().identity(),
        register_environment,
        allocator_availability: legality.receipt().allocator_availability(),
        policy,
        budget,
        usage,
        copies,
        transformed,
    })
}

#[cfg(test)]
pub(crate) mod tests;
