//! Producer-side admission of one runtime live-range split.

use optimization_core::OptimizationWorkBudget;
use optimization_unit::ValueDefinitionSite;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedCaseDispatchSource, SelectedCasePayloadTransport, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedOperand, SelectedStructuralTransport,
    SelectedValueTransport, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{ScalarType, ValueId};

use super::RuntimeSplitError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::runtime_spill::{
    control, next_instruction_id, next_register_id, require_dominated_uses,
};

pub(super) struct Admission {
    pub block_index: usize,
    pub definition_index: usize,
    pub use_blocks: Vec<usize>,
    pub register: VirtualRegister,
    pub copy: SelectedInstruction,
}

pub(super) fn admit(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, RuntimeSplitError> {
    let (plan, function) =
        crate::rewrites::plan_function(source, function_index, environment, || {
            RuntimeSplitError::SourceMismatch
        })?;
    let victim = function
        .virtual_register(register)
        .ok_or(RuntimeSplitError::UnsupportedValue)?;
    let VirtualRegisterOrigin::InstructionResult { source_value, .. } = victim.origin else {
        return Err(RuntimeSplitError::UnsupportedValue);
    };
    // Address results stay out: the primitive-local establishment replay
    // binds some address operands to their defining register verbatim.
    let scalar_payload = match victim.scalar_type {
        ScalarType::Boolean => true,
        ScalarType::IeeeFloat(_) => false,
        ScalarType::Integer(integer) => {
            !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64)
        }
    };
    if !scalar_payload
        || victim.entry_fixed_view.is_some()
        || !matches!(
            victim.definition_site,
            Some(ValueDefinitionSite::FunctionParameter(_))
                | Some(ValueDefinitionSite::BlockParameter { .. })
                | Some(ValueDefinitionSite::Node { .. })
        )
    {
        return Err(RuntimeSplitError::UnsupportedValue);
    }
    let copy_row = copy_row(environment, victim)?;
    if !function.blocks.has_fixed_definition(register) {
        return Err(RuntimeSplitError::UnsupportedValue);
    }
    // The definition is the one instruction writing the victim. The origin's
    // instruction ordinal is a selection-time position and can name a helper
    // selection emitted between registering the result and emitting its
    // definition (x86 division's zero high half, a divisor copy), so it does
    // not locate the write. Single definition is required below regardless.
    let (block_index, definition_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            let (start_idx, end_idx) = function
                .blocks
                .block_register_instruction_span(block_index, register)?;
            block.instructions[start_idx..=end_idx]
                .iter()
                .position(|candidate| {
                    candidate.operands.iter().any(|operand| {
                        operand.virtual_register == register
                            && operand.access != RegisterOperandAccess::Use
                    })
                })
                .map(|offset| (block_index, start_idx + offset))
        })
        .ok_or(RuntimeSplitError::UnsupportedValue)?;
    let defining = &function.blocks[block_index].instructions[definition_index];
    // Exactly one operand of the defining instruction names the victim: a
    // pinned, untied, non-early-clobber write in the victim's class.
    let mut naming = defining
        .operands
        .iter()
        .filter(|operand| operand.virtual_register == register);
    let Some(written) = naming.next() else {
        return Err(RuntimeSplitError::UnsupportedValue);
    };
    if naming.next().is_some()
        || written.access != RegisterOperandAccess::Def
        || written.fixed_view.is_none()
        || written.tied_to.is_some()
        || written.early_clobber
        || written.class != victim.class
    {
        return Err(RuntimeSplitError::UnsupportedValue);
    }
    let mut uses = 0usize;
    let mut use_blocks = Vec::new();
    for (current_block_index, block) in function.blocks.iter().enumerate() {
        if !function
            .blocks
            .block_references_register(current_block_index, register)
        {
            continue;
        }
        let previous_uses = uses;
        if let Some((start_idx, end_idx)) = function
            .blocks
            .block_register_instruction_span(current_block_index, register)
        {
            for (offset, instruction) in block.instructions[start_idx..=end_idx].iter().enumerate()
            {
                let instruction_index = start_idx + offset;
                if current_block_index == block_index && instruction_index == definition_index {
                    continue;
                }
                let after_definition =
                    current_block_index != block_index || instruction_index > definition_index;
                for operand in &instruction.operands {
                    if operand.virtual_register != register {
                        continue;
                    }
                    if !after_definition || !rebindable_use(operand, victim) {
                        return Err(RuntimeSplitError::UnsupportedUse);
                    }
                    uses = uses
                        .checked_add(1)
                        .ok_or(RuntimeSplitError::IdentityOverflow)?;
                }
            }
        }
        let (terminal, successors) = control(&block.terminator);
        for operand in &terminal.operands {
            if operand.virtual_register != register {
                continue;
            }
            if !rebindable_use(operand, victim) {
                return Err(RuntimeSplitError::UnsupportedUse);
            }
            uses = uses
                .checked_add(1)
                .ok_or(RuntimeSplitError::IdentityOverflow)?;
        }
        for successor in successors.into_iter().flatten() {
            for binding in &successor.bindings {
                let SelectedValueTransport::Registers {
                    argument,
                    parameter,
                } = binding.transport
                else {
                    continue;
                };
                if parameter == register {
                    return Err(RuntimeSplitError::UnsupportedUse);
                }
                if argument != register {
                    continue;
                }
                if binding.semantic.argument != source_value
                    || binding.semantic.scalar_type != victim.scalar_type
                {
                    return Err(RuntimeSplitError::UnsupportedUse);
                }
                uses = uses
                    .checked_add(1)
                    .ok_or(RuntimeSplitError::IdentityOverflow)?;
            }
            if names_structurally(successor, register) {
                return Err(RuntimeSplitError::UnsupportedUse);
            }
        }
        if uses != previous_uses {
            use_blocks.push(current_block_index);
        }
    }
    if uses == 0 {
        return Err(RuntimeSplitError::UnsupportedValue);
    }
    charge(plan, function, uses, budget)?;
    require_dominated_uses(function, block_index, &use_blocks).map_err(|error| match error {
        crate::RuntimeSpillError::SourceMismatch => RuntimeSplitError::SourceMismatch,
        _ => RuntimeSplitError::UnsupportedUse,
    })?;
    let instruction = SelectedInstructionId(next_instruction(function)?);
    let fresh = VirtualRegisterId(next_register(function)?);
    Ok(Admission {
        block_index,
        definition_index,
        use_blocks,
        register: VirtualRegister {
            id: fresh,
            scalar_type: victim.scalar_type,
            class: victim.class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value,
            },
            definition_site: victim.definition_site,
            entry_fixed_view: None,
        },
        copy: copy_instruction(copy_row, instruction, register, fresh, source_value),
    })
}

/// The target's copy row: one flexible use then one flexible def in the
/// victim's class with no implicit traffic, so the move reads only the
/// pinned victim and writes only the fresh register.
pub(super) fn copy_row<'environment>(
    environment: &'environment ValidatedTargetRegisterEnvironment,
    victim: &VirtualRegister,
) -> Result<&'environment RegisterInstructionConstraint, RuntimeSplitError> {
    crate::rewrites::integer_copy_row(environment, victim.class, victim.class, || {
        RuntimeSplitError::ConstraintMismatch
    })
}

pub(super) fn copy_instruction(
    row: &RegisterInstructionConstraint,
    id: SelectedInstructionId,
    victim: VirtualRegisterId,
    fresh: VirtualRegisterId,
    source_value: ValueId,
) -> SelectedInstruction {
    let operand = |row: &register_model::RegisterOperandConstraint, register| SelectedOperand {
        operand: row.operand,
        virtual_register: register,
        access: row.access,
        class: row.class,
        fixed_view: row.fixed_view,
        tied_to: row.tied_to,
        early_clobber: row.early_clobber,
    };
    SelectedInstruction {
        id,
        kind: SelectedInstructionKind::CopyI64,
        constraint: row.key,
        operands: vec![
            operand(&row.operands[0], victim),
            operand(&row.operands[1], fresh),
        ],
        implicit_uses: row.implicit_uses.clone(),
        implicit_defs: row.implicit_defs.clone(),
        clobbers: row.clobbers.clone(),
        provenance: SelectedInstructionProvenance {
            values: vec![source_value],
            ..Default::default()
        },
    }
}

/// A reference the fresh register can serve unchanged: a plain read in the
/// victim's class. A tie onto the read may stay — the tied write then shares
/// the fresh register's home — but a tied or early-clobber read is not a
/// shape this rewrite relocates.
pub(super) fn rebindable_use(operand: &SelectedOperand, victim: &VirtualRegister) -> bool {
    operand.access == RegisterOperandAccess::Use
        && operand.tied_to.is_none()
        && !operand.early_clobber
        && operand.class == victim.class
}

pub(crate) fn names_structurally(
    successor: &selected_instructions::SelectedSuccessor,
    register: VirtualRegisterId,
) -> bool {
    let mut named = false;
    visit_structural_registers(successor, |candidate| named |= candidate == register);
    named
}

pub(crate) fn visit_structural_registers(
    successor: &selected_instructions::SelectedSuccessor,
    mut visit: impl FnMut(VirtualRegisterId),
) {
    for binding in &successor.structural_bindings {
        match binding.transport {
            SelectedStructuralTransport::WholeValue { argument, .. }
            | SelectedStructuralTransport::ScalarReference {
                base: selected_instructions::SelectedAddressBase::Register(argument),
                ..
            }
            | SelectedStructuralTransport::Address {
                base: selected_instructions::SelectedAddressBase::Register(argument),
                ..
            } => visit(argument),
            _ => {}
        }
    }
    if let Some(case) = &successor.structural_case {
        if let SelectedCaseDispatchSource::Borrowed { pointer, .. } = case.source {
            visit(pointer);
        }
        for payload in &case.payloads {
            match payload.transport {
                SelectedCasePayloadTransport::Registers {
                    argument,
                    parameter,
                } => {
                    visit(argument);
                    visit(parameter);
                }
                SelectedCasePayloadTransport::Unmaterialized { parameter } => visit(parameter),
                SelectedCasePayloadTransport::Unused => {}
            }
        }
    }
}

pub(super) fn charge(
    plan: &selected_instructions::SelectedInstructionPlan,
    function: &SelectedFunction,
    uses: usize,
    budget: OptimizationWorkBudget,
) -> Result<(), RuntimeSplitError> {
    let steps = crate::rewrites::rewrite_scan(plan, function)
        .and_then(|total| total.checked_add(uses.checked_mul(2)?))
        .and_then(|total| total.checked_add(function.blocks.len().checked_mul(2)?))
        .ok_or(RuntimeSplitError::IdentityOverflow)?;
    crate::rewrites::charge_steps(
        steps,
        crate::rewrites::required_rewrite_budget(plan, function, budget)
            .ok_or(RuntimeSplitError::IdentityOverflow)?,
        || RuntimeSplitError::IdentityOverflow,
        || RuntimeSplitError::WorkBudgetExceeded,
    )?;
    Ok(())
}

pub(super) fn next_instruction(function: &SelectedFunction) -> Result<u32, RuntimeSplitError> {
    next_instruction_id(function).ok_or(RuntimeSplitError::IdentityOverflow)
}

pub(super) fn next_register(function: &SelectedFunction) -> Result<u32, RuntimeSplitError> {
    next_register_id(function).ok_or(RuntimeSplitError::IdentityOverflow)
}
