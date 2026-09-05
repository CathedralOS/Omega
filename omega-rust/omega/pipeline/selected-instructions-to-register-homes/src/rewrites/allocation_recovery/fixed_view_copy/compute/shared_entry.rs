//! Shared-entry-after-compare-before-branch policy mechanics.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn build_shared_entry_copy(
    function_index: usize,
    function: &selected_instructions::SelectedFunction,
    boundaries: &[&super::super::evidence::AuthenticatedFixedViewBoundary],
    row: &RegisterInstructionConstraint,
    copy_constraint: register_model::RegisterConstraintKey,
    next_instruction: u32,
    next_register: u32,
) -> Result<Option<FixedViewCopy>, FixedViewCopyError> {
    if boundaries.is_empty() {
        return Ok(None);
    }
    if boundaries.len() != 2
        || boundaries[0].virtual_register != boundaries[1].virtual_register
        || boundaries[0].source_segment != boundaries[1].source_segment
        || boundaries[0].source_domain != boundaries[1].source_domain
        || boundaries[0].from_view != boundaries[1].from_view
        || boundaries[0].to_view != boundaries[1].to_view
        || boundaries.iter().any(|boundary| {
            boundary.function != function_index
                || boundary.machine != function.machine
                || boundary.incoming.is_none()
        })
    {
        return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    }
    let first = boundaries[0];
    let source = function
        .virtual_registers
        .get(usize::try_from(first.virtual_register.0).map_err(|_| {
            FixedViewCopyError::UnsupportedSourceRegister {
                function: function_index,
                register: first.virtual_register.0,
            }
        })?)
        .filter(|source| {
            source.id == first.virtual_register
                && source.class == first.class
                && source.entry_fixed_view == Some(first.from_view)
        })
        .ok_or(FixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: first.virtual_register.0,
        })?;
    let VirtualRegisterOrigin::EntryParameter { source_value, .. } = source.origin else {
        return Err(FixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: source.id.0,
        });
    };
    if !is_u64(source.scalar_type)
        || row.operands[0].class != source.class
        || row.operands[1].class != source.class
    {
        return Err(FixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: source.id.0,
        });
    }
    let entry = function
        .blocks
        .iter()
        .find(|block| block.id == function.entry_block)
        .ok_or(FixedViewCopyError::FunctionMismatch {
            function: function_index,
        })?;
    if entry.instructions.len() != 1
        || entry.instructions[0].kind != SelectedInstructionKind::CompareI64Zero
    {
        return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    }
    let SelectedTerminator::ConditionalBranch {
        instruction: branch,
        when_nonzero,
        when_zero,
    } = &entry.terminator
    else {
        return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    };
    if when_nonzero.block == when_zero.block {
        return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    }
    let successor_blocks = BTreeSet::from([when_nonzero.block, when_zero.block]);
    let from_view = first.from_view;
    let mut destinations = Vec::with_capacity(2);
    let mut destination_blocks = BTreeSet::new();
    for boundary in boundaries {
        let VirtualFixedConstraintSite::Operand {
            instruction,
            operand,
            access: RegisterOperandAccess::Use,
            ..
        } = boundary.site
        else {
            return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
                function: function_index,
            });
        };
        let block = find_leaf_block(
            function_index,
            function,
            instruction,
            operand,
            source.id,
            boundary.to_view,
        )?;
        if block != boundary.block {
            return Err(FixedViewCopyError::SegmentEvidenceMismatch);
        }
        let leaf = function
            .blocks
            .iter()
            .find(|candidate| candidate.id == block)
            .ok_or(FixedViewCopyError::MissingDestination {
                function: function_index,
                instruction: instruction.0,
            })?;
        if !leaf.instructions.is_empty() || !destination_blocks.insert(block) {
            return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
                function: function_index,
            });
        }
        destinations.push(FixedViewCopyDestination {
            site: boundary.site,
            block,
            view: boundary.to_view,
        });
    }
    destinations.sort_by_key(|destination| match destination.site {
        VirtualFixedConstraintSite::Operand {
            instruction,
            operand,
            ..
        } => (instruction.0, operand),
        VirtualFixedConstraintSite::Entry => (u32::MAX, u16::MAX),
    });
    if destination_blocks != successor_blocks {
        return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    }
    Ok(Some(FixedViewCopy {
        function: u32::try_from(function_index).map_err(|_| {
            FixedViewCopyError::IdentifierOverflow {
                function: function_index,
            }
        })?,
        machine: function.machine,
        source_virtual_register: source.id,
        source_value,
        source_definition_site: source.definition_site,
        from_view,
        to_view: destinations[0].view,
        insertion_block: entry.id,
        before_instruction: branch.id,
        destinations,
        copy_instruction: SelectedInstructionId(next_instruction),
        result_virtual_register: VirtualRegisterId(next_register),
        copy_constraint,
    }))
}
