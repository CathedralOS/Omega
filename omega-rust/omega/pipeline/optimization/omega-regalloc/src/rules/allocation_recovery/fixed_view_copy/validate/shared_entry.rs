use super::leaf_destination::{replay_is_u64, replay_leaf_block};
use std::collections::BTreeSet;

use omega_register_model::{
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
};
use omega_selected_instructions::{
    SelectedInstructionId, SelectedInstructionKind, SelectedTerminator, VirtualRegisterId,
    VirtualRegisterOrigin,
};

use crate::{
    FixedViewCopy, FixedViewCopyDestination, FixedViewCopyError, VirtualFixedConstraintSite,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_shared_entry_copy(
    function_index: usize,
    function: &omega_selected_instructions::SelectedFunction,
    boundaries: &[&super::super::evidence::AuthenticatedFixedViewBoundary],
    row: &RegisterInstructionConstraint,
    constraint: RegisterConstraintKey,
    instruction_id: u32,
    register_id: u32,
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
    let requirement = boundaries[0];
    let source = function
        .virtual_registers
        .iter()
        .find(|candidate| candidate.id == requirement.virtual_register)
        .ok_or(FixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: requirement.virtual_register.0,
        })?;
    let VirtualRegisterOrigin::EntryParameter { source_value, .. } = source.origin else {
        return Err(FixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: source.id.0,
        });
    };
    if source.class != requirement.class
        || source.entry_fixed_view != Some(requirement.from_view)
        || !replay_is_u64(source.scalar_type)
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
        .find(|candidate| candidate.id == function.entry_block)
        .ok_or(FixedViewCopyError::FunctionMismatch {
            function: function_index,
        })?;
    let [compare] = entry.instructions.as_slice() else {
        return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    };
    if compare.kind != SelectedInstructionKind::CompareI64Zero {
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
    let expected_leaves = BTreeSet::from([when_nonzero.block, when_zero.block]);
    if expected_leaves.len() != 2 {
        return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    }
    let mut actual_leaves = BTreeSet::new();
    let mut destinations = Vec::new();
    let from_view = requirement.from_view;
    for boundary in boundaries {
        let VirtualFixedConstraintSite::Operand {
            instruction,
            operand,
            access,
            ..
        } = boundary.site
        else {
            return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
                function: function_index,
            });
        };
        if access != RegisterOperandAccess::Use {
            return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
                function: function_index,
            });
        }
        let block = replay_leaf_block(
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
            .unwrap();
        if !leaf.instructions.is_empty() || !actual_leaves.insert(block) {
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
    if actual_leaves != expected_leaves {
        return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
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
        copy_instruction: SelectedInstructionId(instruction_id),
        result_virtual_register: VirtualRegisterId(register_id),
        copy_constraint: constraint,
    }))
}
