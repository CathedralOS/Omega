//! Shared-source-exit copy construction: the placement decision both
//! shared-exit policies apply over the boundary partition. An emission whose
//! members leave one source segment end places a single copy at the end of
//! the shared exit block — immediately before its terminator — dominating
//! every member site; a lone boundary keeps a copy in its site's own block
//! under the default leg and refuses under the declared selection, whose
//! admission is the shared leg only.
use super::apply::apply_copy;
use super::site::{admitted_source, destination_block};
use super::{
    BTreeSet, FixedViewCopy, FixedViewCopyDestination, FixedViewCopyError, FixedViewCopyPolicy,
    RegisterInstructionConstraint, RegisterOperandAccess, SelectedInstructionId,
    VirtualFixedConstraintSite, VirtualRegisterId,
};
use register_model::RegisterConstraintKey;

/// Build and apply the function's emitted copies in partition order. The
/// declared shared-entry selection keeps its entry-parameter admission gate
/// and refuses any boundary the partition could not share; the default-path
/// leg admits the immediate form's origin set and falls back to a site copy.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_source_exit_copies(
    function_index: usize,
    source_function: &selected_instructions::SelectedFunction,
    boundaries: &[&super::super::evidence::AuthenticatedFixedViewBoundary],
    transformed: &mut selected_instructions::SelectedFunction,
    copy_row: &RegisterInstructionConstraint,
    copy_key: RegisterConstraintKey,
    policy: FixedViewCopyPolicy,
    mut next_instruction: u32,
    mut next_register: u32,
) -> Result<Vec<FixedViewCopy>, FixedViewCopyError> {
    let declared = policy == FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1;
    let mut copies = Vec::new();
    let mut destinations = BTreeSet::new();
    for emission in super::super::emission::copy_emissions(boundaries) {
        if declared && emission.exit.is_none() {
            return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
                function: function_index,
            });
        }
        let first = emission.members[0];
        let (source_register, source_value) =
            admitted_source(function_index, source_function, first, declared)?;
        if copy_row.operands[0].class != source_register.class
            || copy_row.operands[1].class != source_register.class
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
                access: RegisterOperandAccess::Use,
                ..
            } = member.site
            else {
                return Err(FixedViewCopyError::UnsupportedTransitionSite {
                    function: function_index,
                    register: member.virtual_register.0,
                });
            };
            if !destinations.insert((instruction, operand)) {
                return Err(FixedViewCopyError::NonCanonicalCopies);
            }
            let block = destination_block(
                function_index,
                source_function,
                member,
                instruction,
                operand,
                source_register.id,
                false,
            )?;
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
            source_virtual_register: source_register.id,
            source_value,
            source_definition_site: source_register.definition_site.ok_or(
                FixedViewCopyError::UnsupportedSourceRegister {
                    function: function_index,
                    register: source_register.id.0,
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
        apply_copy(function_index, transformed, &copy, copy_row)?;
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
    Ok(copies)
}
