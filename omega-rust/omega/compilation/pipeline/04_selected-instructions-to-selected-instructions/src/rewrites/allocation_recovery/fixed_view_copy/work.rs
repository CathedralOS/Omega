//! Checked work-domain composition shared by production and independent replay.

use optimization_core::OptimizationWorkUsage;

use crate::FixedViewCopyError;

use selected_instructions::VirtualFixedConstraintSite;

use super::FixedViewCopy;

pub(super) fn combined_usage(
    evidence: OptimizationWorkUsage,
    transformation: OptimizationWorkUsage,
) -> Result<OptimizationWorkUsage, FixedViewCopyError> {
    crate::work_accounting::combine(evidence, transformation)
        .map_err(|_| FixedViewCopyError::WorkOverflow)
}

pub(super) fn add(left: u64, right: u64) -> Result<u64, FixedViewCopyError> {
    left.checked_add(right)
        .ok_or(FixedViewCopyError::WorkOverflow)
}

/// Apply one planned fixed-view copy: rewrite each destination site's use
/// operand to the copy's result register, then insert the copy immediately
/// before the fixed-use instruction and shift the boundary settlements the
/// insertion crosses. `require_use_access` is the validator's stricter site
/// contract — production admits any `Operand` site because the plan only
/// emits use sites, while replay re-proves it.
pub(super) fn apply_fixed_view_copy(
    function_index: usize,
    function: &mut selected_instructions::SelectedFunction,
    copy: &FixedViewCopy,
    row: &register_model::RegisterInstructionConstraint,
    require_use_access: bool,
) -> Result<(), FixedViewCopyError> {
    let source = function
        .virtual_register(copy.source_virtual_register)
        .cloned()
        .ok_or(FixedViewCopyError::UnsupportedSourceRegister {
            function: function_index,
            register: copy.source_virtual_register.0,
        })?;
    for destination in &copy.destinations {
        let VirtualFixedConstraintSite::Operand {
            instruction,
            operand,
            access,
            ..
        } = destination.site
        else {
            return Err(FixedViewCopyError::UnsupportedTransitionSite {
                function: function_index,
                register: copy.source_virtual_register.0,
            });
        };
        if require_use_access && access != register_model::RegisterOperandAccess::Use {
            return Err(FixedViewCopyError::UnsupportedTransitionSite {
                function: function_index,
                register: copy.source_virtual_register.0,
            });
        }
        let destination_block = function
            .blocks
            .iter_mut()
            .find(|block| block.id == destination.block)
            .ok_or(FixedViewCopyError::MissingDestination {
                function: function_index,
                instruction: instruction.0,
            })?;
        let site_instruction =
            if selected_instructions::terminator_instruction(&destination_block.terminator).id
                == instruction
            {
                selected_instructions::terminator_instruction_mut(&mut destination_block.terminator)
            } else {
                destination_block
                    .instructions
                    .iter_mut()
                    .find(|candidate| candidate.id == instruction)
                    .ok_or(FixedViewCopyError::MissingDestination {
                        function: function_index,
                        instruction: instruction.0,
                    })?
            };
        site_instruction
            .operands
            .iter_mut()
            .find(|candidate| candidate.operand == operand)
            .ok_or(FixedViewCopyError::MissingDestination {
                function: function_index,
                instruction: instruction.0,
            })?
            .virtual_register = copy.result_virtual_register;
    }
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == copy.insertion_block)
        .ok_or(FixedViewCopyError::MissingDestination {
            function: function_index,
            instruction: copy.before_instruction.0,
        })?;
    // The copy sits immediately before the fixed-use instruction: at the end
    // of the block's instruction list for a terminator site, or at the site's
    // own position for an ordinary instruction.
    let insertion_index = if selected_instructions::terminator_instruction(&block.terminator).id
        == copy.before_instruction
    {
        block.instructions.len()
    } else {
        block.instruction_index(copy.before_instruction).ok_or(
            FixedViewCopyError::InvalidInsertionSite {
                function: function_index,
                instruction: copy.before_instruction.0,
            },
        )?
    };
    function
        .virtual_registers
        .push(selected_instructions::VirtualRegister {
            id: copy.result_virtual_register,
            scalar_type: source.scalar_type,
            class: source.class,
            origin: selected_instructions::VirtualRegisterOrigin::InstructionResult {
                instruction: copy.copy_instruction,
                source_value: copy.source_value,
            },
            definition_site: Some(copy.source_definition_site),
            entry_fixed_view: None,
        });
    let insertion_index =
        u32::try_from(insertion_index).map_err(|_| FixedViewCopyError::WorkOverflow)?;
    for settlement in &mut function.boundary_settlements {
        if settlement.block == block.id && settlement.instruction_index >= insertion_index {
            settlement.instruction_index = settlement
                .instruction_index
                .checked_add(1)
                .ok_or(FixedViewCopyError::WorkOverflow)?;
        }
    }
    block.instructions.insert(
        usize::try_from(insertion_index).map_err(|_| FixedViewCopyError::WorkOverflow)?,
        selected_instructions::SelectedInstruction {
            id: copy.copy_instruction,
            kind: selected_instructions::SelectedInstructionKind::CopyI64,
            constraint: copy.copy_constraint,
            operands: vec![
                crate::rewrites::selected_operands::selected_operand(
                    &row.operands[0],
                    copy.source_virtual_register,
                ),
                crate::rewrites::selected_operands::selected_operand(
                    &row.operands[1],
                    copy.result_virtual_register,
                ),
            ],
            implicit_uses: row.implicit_uses.clone(),
            implicit_defs: row.implicit_defs.clone(),
            clobbers: row.clobbers.clone(),
            provenance: selected_instructions::SelectedInstructionProvenance {
                operations: Vec::new(),
                values: vec![copy.source_value],
                edges: Vec::new(),
                obligations: Vec::new(),
                fuel: Vec::new(),
            },
        },
    );
    Ok(())
}
