//! Independent shared-source-exit copy reconstruction: replay re-derives the
//! same boundary partition and rebuilds each emitted copy from current facts
//! alone — the shared dominating point from connector evidence, member site
//! blocks from the source function, and the admission gate from the policy.
use super::apply::replay_apply;
use super::leaf_destination::{replay_is_u64, replay_site_block};
use std::collections::BTreeSet;

use register_model::{RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{SelectedInstructionId, VirtualRegisterId, VirtualRegisterOrigin};

use crate::{
    FixedViewCopy, FixedViewCopyDestination, FixedViewCopyError, FixedViewCopyPolicy,
    VirtualFixedConstraintSite,
};

#[cfg(test)]
mod tests;

/// Rebuild the copy sequence the producer emits under a shared-exit policy
/// and apply it to `output_function`. The declared shared-entry gate admits
/// live-in entry parameters still carrying their entry fixed view and
/// refuses any boundary the partition could not share; the default-path leg
/// admits every origin naming a scalar source value and falls back to a
/// site copy.
#[allow(clippy::too_many_arguments)]
pub(super) fn replay_source_exit_copies(
    function_index: usize,
    source_function: &selected_instructions::SelectedFunction,
    function_boundaries: &[&super::super::evidence::AuthenticatedFixedViewBoundary],
    output_function: &mut selected_instructions::SelectedFunction,
    row: &RegisterInstructionConstraint,
    copy_key: RegisterConstraintKey,
    policy: FixedViewCopyPolicy,
    mut next_instruction: u32,
    mut next_register: u32,
) -> Result<Vec<FixedViewCopy>, FixedViewCopyError> {
    let declared = policy == FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1;
    let mut expected = Vec::new();
    let mut seen = BTreeSet::new();
    for emission in super::super::emission::copy_emissions(function_boundaries) {
        if declared && emission.exit.is_none() {
            return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
                function: function_index,
            });
        }
        let first = emission.members[0];
        let source = source_function
            .virtual_registers
            .iter()
            .find(|register| register.id == first.virtual_register)
            .ok_or(FixedViewCopyError::UnsupportedSourceRegister {
                function: function_index,
                register: first.virtual_register.0,
            })?;
        let source_value = match (declared, source.origin) {
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
                    register: first.virtual_register.0,
                });
            }
        };
        if source.class != first.class
            || (declared && source.entry_fixed_view != Some(first.from_view))
            || !replay_is_u64(source.scalar_type)
            || row.operands[0].class != source.class
            || row.operands[1].class != source.class
        {
            return Err(FixedViewCopyError::UnsupportedSourceRegister {
                function: function_index,
                register: first.virtual_register.0,
            });
        }
        let mut members = Vec::with_capacity(emission.members.len());
        for member in &emission.members {
            let VirtualFixedConstraintSite::Operand {
                instruction,
                operand,
                access,
                ..
            } = member.site
            else {
                return Err(FixedViewCopyError::UnsupportedTransitionSite {
                    function: function_index,
                    register: member.virtual_register.0,
                });
            };
            if access != RegisterOperandAccess::Use || !seen.insert((instruction, operand)) {
                return Err(FixedViewCopyError::NonCanonicalCopies);
            }
            let block = replay_site_block(
                function_index,
                source_function,
                instruction,
                operand,
                source.id,
                member.to_view,
            )?;
            if block != member.block {
                return Err(FixedViewCopyError::SegmentEvidenceMismatch);
            }
            members.push(FixedViewCopyDestination {
                site: member.site,
                block,
                view: member.to_view,
            });
        }
        let (insertion_block, before_instruction) = match emission.exit {
            Some(connector) => (connector.source, connector.terminator),
            None => {
                let VirtualFixedConstraintSite::Operand { instruction, .. } = members[0].site
                else {
                    return Err(FixedViewCopyError::UnsupportedTransitionSite {
                        function: function_index,
                        register: first.virtual_register.0,
                    });
                };
                (first.block, instruction)
            }
        };
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
            from_view: first.from_view,
            to_view: first.to_view,
            insertion_block,
            before_instruction,
            destinations: members,
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
