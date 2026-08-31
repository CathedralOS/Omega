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
    FixedViewCopy, FixedViewCopyDestination, FixedViewCopyError, FunctionAllocationLegality,
    VirtualFixedConstraintSite,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_shared_entry_copy(
    function_index: usize,
    function: &omega_selected_instructions::SelectedFunction,
    legality: &FunctionAllocationLegality,
    row: &RegisterInstructionConstraint,
    constraint: RegisterConstraintKey,
    instruction_id: u32,
    register_id: u32,
) -> Result<Option<FixedViewCopy>, FixedViewCopyError> {
    let mut requirements = Vec::new();
    for register in &legality.virtual_registers {
        for transition in &register.entry_transitions {
            requirements.push((register, transition));
        }
    }
    if requirements.is_empty() {
        return Ok(None);
    }
    if requirements.len() != 2
        || requirements[0].0.virtual_register != requirements[1].0.virtual_register
        || requirements[0].1.from_view != requirements[1].1.from_view
        || requirements[0].1.to_view != requirements[1].1.to_view
    {
        return Err(FixedViewCopyError::UnsupportedSharedTransitionSet {
            function: function_index,
        });
    }
    let requirement = requirements[0].0;
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
        || source.entry_fixed_view != Some(requirements[0].1.from_view)
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
    let from_view = requirements[0].1.from_view;
    for (_, transition) in requirements {
        let VirtualFixedConstraintSite::Operand {
            instruction,
            operand,
            access,
            ..
        } = transition.to_site
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
            transition.to_view,
        )?;
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
            site: transition.to_site,
            block,
            view: transition.to_view,
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
