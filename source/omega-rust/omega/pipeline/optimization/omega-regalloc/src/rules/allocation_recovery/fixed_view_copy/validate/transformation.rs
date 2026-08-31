use super::apply::replay_apply;
use super::leaf_destination::{replay_is_u64, replay_leaf_block, terminator};
use super::shared_entry::replay_shared_entry_copy;
use std::collections::BTreeSet;

use omega_register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
};
use omega_selected_instructions::{
    SelectedInstructionId, VirtualRegisterId, VirtualRegisterOrigin,
};
use omega_target_operations_to_selected_instructions::ValidatedSelectedInstructions;

use crate::{
    FixedViewCopy, FixedViewCopyDestination, FixedViewCopyError, FixedViewCopyPolicy,
    ValidatedAllocationLegality, VirtualFixedConstraintSite,
};

pub(super) fn replay_transformation(
    selected: &ValidatedSelectedInstructions,
    legality: &ValidatedAllocationLegality,
    keys: TargetRegisterEnvironmentConstraintKeys,
    row: &RegisterInstructionConstraint,
    policy: FixedViewCopyPolicy,
) -> Result<
    (
        Vec<FixedViewCopy>,
        omega_selected_instructions::SelectedInstructionPlan,
    ),
    FixedViewCopyError,
> {
    let mut output = selected.plan().clone();
    let mut expected = Vec::new();
    for function_index in 0..selected.plan().functions.len() {
        let source_function = &selected.plan().functions[function_index];
        let legality_function = &legality.plan().functions[function_index];
        if source_function.machine != legality_function.machine {
            return Err(FixedViewCopyError::FunctionMismatch {
                function: function_index,
            });
        }
        let mut instruction_ids = source_function
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .instructions
                    .iter()
                    .map(|instruction| instruction.id.0)
                    .chain(std::iter::once(terminator(&block.terminator).id.0))
            })
            .collect::<Vec<_>>();
        instruction_ids.sort_unstable();
        let instruction_count = u32::try_from(instruction_ids.len()).map_err(|_| {
            FixedViewCopyError::IdentifierOverflow {
                function: function_index,
            }
        })?;
        if instruction_ids != (0..instruction_count).collect::<Vec<_>>()
            || source_function
                .virtual_registers
                .iter()
                .enumerate()
                .any(|(index, register)| usize::try_from(register.id.0) != Ok(index))
        {
            return Err(FixedViewCopyError::FunctionMismatch {
                function: function_index,
            });
        }
        let mut next_instruction = instruction_count;
        let mut next_register =
            u32::try_from(source_function.virtual_registers.len()).map_err(|_| {
                FixedViewCopyError::IdentifierOverflow {
                    function: function_index,
                }
            })?;
        if policy == FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 {
            if let Some(copy) = replay_shared_entry_copy(
                function_index,
                source_function,
                legality_function,
                row,
                keys.copy_i64,
                next_instruction,
                next_register,
            )? {
                replay_apply(
                    function_index,
                    &mut output.functions[function_index],
                    &copy,
                    row,
                )?;
                expected.push(copy);
            }
            continue;
        }
        let mut seen = BTreeSet::new();
        for legality_register in &legality_function.virtual_registers {
            for transition in &legality_register.entry_transitions {
                let VirtualFixedConstraintSite::Operand {
                    instruction,
                    operand,
                    access,
                    ..
                } = transition.to_site
                else {
                    return Err(FixedViewCopyError::UnsupportedTransitionSite {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    });
                };
                if access != RegisterOperandAccess::Use || !seen.insert((instruction, operand)) {
                    return Err(FixedViewCopyError::NonCanonicalCopies);
                }
                let source = source_function
                    .virtual_registers
                    .iter()
                    .find(|register| register.id == legality_register.virtual_register)
                    .ok_or(FixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    })?;
                let VirtualRegisterOrigin::EntryParameter { source_value, .. } = source.origin
                else {
                    return Err(FixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    });
                };
                if source.class != legality_register.class
                    || source.entry_fixed_view != Some(transition.from_view)
                    || !replay_is_u64(source.scalar_type)
                    || row.operands[0].class != source.class
                {
                    return Err(FixedViewCopyError::UnsupportedSourceRegister {
                        function: function_index,
                        register: legality_register.virtual_register.0,
                    });
                }
                let block = replay_leaf_block(
                    function_index,
                    source_function,
                    instruction,
                    operand,
                    source.id,
                    transition.to_view,
                )?;
                let copy = FixedViewCopy {
                    function: u32::try_from(function_index).map_err(|_| {
                        FixedViewCopyError::IdentifierOverflow {
                            function: function_index,
                        }
                    })?,
                    machine: source_function.machine,
                    source_virtual_register: source.id,
                    source_value,
                    source_definition_site: source.definition_site,
                    from_view: transition.from_view,
                    to_view: transition.to_view,
                    insertion_block: block,
                    before_instruction: instruction,
                    destinations: vec![FixedViewCopyDestination {
                        site: transition.to_site,
                        block,
                        view: transition.to_view,
                    }],
                    copy_instruction: SelectedInstructionId(next_instruction),
                    result_virtual_register: VirtualRegisterId(next_register),
                    copy_constraint: keys.copy_i64,
                };
                replay_apply(
                    function_index,
                    &mut output.functions[function_index],
                    &copy,
                    row,
                )?;
                expected.push(copy);
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
    Ok((expected, output))
}
