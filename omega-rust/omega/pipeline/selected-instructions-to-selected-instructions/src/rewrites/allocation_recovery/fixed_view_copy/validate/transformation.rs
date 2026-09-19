use super::apply::replay_apply;
use super::leaf_destination::terminator;
use super::shared_entry::replay_shared_entry_copy;
use super::site::replay_site_copies;

use register_model::{RegisterInstructionConstraint, TargetRegisterEnvironmentConstraintKeys};
use target_operations_to_selected_instructions::ValidatedSelectedInstructions;

use crate::{FixedViewCopy, FixedViewCopyError, FixedViewCopyPolicy};

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
        let next_instruction = instruction_count;
        let next_register =
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
        let leaf_local = policy == FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1;
        expected.extend(replay_site_copies(
            function_index,
            source_function,
            &function_boundaries,
            output_function,
            row,
            keys.copy_i64,
            leaf_local,
            next_instruction,
            next_register,
        )?);
    }
    Ok((expected, output))
}
