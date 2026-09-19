//! Independent per-boundary fixed-site copy reconstruction for the leaf-local
//! and immediate-site policies.
use super::apply::replay_apply;
use super::leaf_destination::{replay_is_u64, replay_leaf_block, replay_site_block};
use std::collections::BTreeSet;

use register_model::{RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{SelectedInstructionId, VirtualRegisterId, VirtualRegisterOrigin};

use crate::{
    FixedViewCopy, FixedViewCopyDestination, FixedViewCopyError, VirtualFixedConstraintSite,
};

#[cfg(test)]
mod tests;

/// Rebuild the copy sequence the producer admits for the selected site
/// policy and apply it to `output_function`; every premise is re-derived from
/// the source function and the authenticated boundary rows.
#[allow(clippy::too_many_arguments)]
pub(super) fn replay_site_copies(
    function_index: usize,
    source_function: &selected_instructions::SelectedFunction,
    function_boundaries: &[&super::super::evidence::AuthenticatedFixedViewBoundary],
    output_function: &mut selected_instructions::SelectedFunction,
    row: &RegisterInstructionConstraint,
    copy_key: RegisterConstraintKey,
    leaf_local: bool,
    mut next_instruction: u32,
    mut next_register: u32,
) -> Result<Vec<FixedViewCopy>, FixedViewCopyError> {
    let mut expected = Vec::new();
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
        // V1 admits only live-in entry parameters; the immediate form
        // admits any origin that still names a scalar source value.
        let source_value = match (leaf_local, source.origin) {
            (_, VirtualRegisterOrigin::EntryParameter { source_value, .. })
            | (
                false,
                VirtualRegisterOrigin::InstructionResult { source_value, .. }
                | VirtualRegisterOrigin::BlockParameter { source_value, .. }
                | VirtualRegisterOrigin::ScalarAbiAddress { source_value, .. },
            ) => source_value,
            _ => {
                return Err(FixedViewCopyError::UnsupportedSourceRegister {
                    function: function_index,
                    register: boundary.virtual_register.0,
                });
            }
        };
        if source.class != boundary.class
            || (leaf_local && source.entry_fixed_view != Some(boundary.from_view))
            || !replay_is_u64(source.scalar_type)
            || row.operands[0].class != source.class
        {
            return Err(FixedViewCopyError::UnsupportedSourceRegister {
                function: function_index,
                register: boundary.virtual_register.0,
            });
        }
        let block = if leaf_local {
            replay_leaf_block(
                function_index,
                source_function,
                instruction,
                operand,
                source.id,
                boundary.to_view,
            )?
        } else {
            replay_site_block(
                function_index,
                source_function,
                instruction,
                operand,
                source.id,
                boundary.to_view,
            )?
        };
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
            source_definition_site: source.definition_site.ok_or(
                FixedViewCopyError::UnsupportedSourceRegister {
                    function: function_index,
                    register: source.id.0,
                },
            )?,
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
            copy_constraint: copy_key,
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
    Ok(expected)
}
