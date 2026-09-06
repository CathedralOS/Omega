use super::apply::replay_apply;
use super::leaf_destination::{replay_is_u64, replay_leaf_block, terminator};
use super::shared_entry::replay_shared_entry_copy;
use std::collections::BTreeSet;

use register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
};
use selected_instructions::{SelectedInstructionId, VirtualRegisterId, VirtualRegisterOrigin};
use target_operations_to_selected_instructions::ValidatedSelectedInstructions;

use crate::{
    FixedViewCopy, FixedViewCopyDestination, FixedViewCopyError, FixedViewCopyPolicy,
    VirtualFixedConstraintSite,
};

pub(super) fn replay_transformation(
    selected: &ValidatedSelectedInstructions,
    boundaries: &[super::super::evidence::AuthenticatedFixedViewBoundary],
    keys: &TargetRegisterEnvironmentConstraintKeys,
    row: &RegisterInstructionConstraint,
    policy: FixedViewCopyPolicy,
) -> Result<
    (
        Vec<FixedViewCopy>,
        selected_instructions::SelectedInstructionPlan,
    ),
    FixedViewCopyError,
> {
    let mut output = selected.plan().clone();
    let mut expected = Vec::new();
    for (function_index, (source_function, output_function)) in selected
        .plan()
        .functions
        .iter()
        .zip(&mut output.functions)
        .enumerate()
    {
        let function_boundaries = boundaries
            .iter()
            .filter(|boundary| boundary.function == function_index)
            .collect::<Vec<_>>();
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
                &function_boundaries,
                row,
                keys.copy_i64,
                next_instruction,
                next_register,
            )? {
                replay_apply(function_index, output_function, &copy, row)?;
                expected.push(copy);
            }
            continue;
        }
        let mut seen = BTreeSet::new();
        for boundary in function_boundaries {
            let VirtualFixedConstraintSite::Operand {
                instruction,
                operand,
                access,
                ..
            } = boundary.site
            else {
                return Err(FixedViewCopyError::UnsupportedTransitionSite {
                    function: function_index,
                    register: boundary.virtual_register.0,
                });
            };
            if access != RegisterOperandAccess::Use || !seen.insert((instruction, operand)) {
                return Err(FixedViewCopyError::NonCanonicalCopies);
            }
            let source = source_function
                .virtual_registers
                .iter()
                .find(|register| register.id == boundary.virtual_register)
                .ok_or(FixedViewCopyError::UnsupportedSourceRegister {
                    function: function_index,
                    register: boundary.virtual_register.0,
                })?;
            let VirtualRegisterOrigin::EntryParameter { source_value, .. } = source.origin else {
                return Err(FixedViewCopyError::UnsupportedSourceRegister {
                    function: function_index,
                    register: boundary.virtual_register.0,
                });
            };
            if source.class != boundary.class
                || source.entry_fixed_view != Some(boundary.from_view)
                || !replay_is_u64(source.scalar_type)
                || row.operands[0].class != source.class
            {
                return Err(FixedViewCopyError::UnsupportedSourceRegister {
                    function: function_index,
                    register: boundary.virtual_register.0,
                });
            }
            let block = replay_leaf_block(
                function_index,
                source_function,
                instruction,
                operand,
                source.id,
                boundary.to_view,
            )?;
            if block != boundary.block {
                return Err(FixedViewCopyError::SegmentEvidenceMismatch);
            }
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
                from_view: boundary.from_view,
                to_view: boundary.to_view,
                insertion_block: block,
                before_instruction: instruction,
                destinations: vec![FixedViewCopyDestination {
                    site: boundary.site,
                    block,
                    view: boundary.to_view,
                }],
                copy_instruction: SelectedInstructionId(next_instruction),
                result_virtual_register: VirtualRegisterId(next_register),
                copy_constraint: keys.copy_i64,
            };
            replay_apply(function_index, output_function, &copy, row)?;
            expected.push(copy);
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
    Ok((expected, output))
}
