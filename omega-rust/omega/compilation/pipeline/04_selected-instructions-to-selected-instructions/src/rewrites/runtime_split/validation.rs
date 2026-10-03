//! Independent validation of one runtime live-range split.
//!
//! The validator never calls [`super::admission`]: it re-derives the split's
//! legality from the source records — the named register's single pinned
//! definition, its scalar and definition-site custody, the target's copy
//! row, the reference scan over instruction operands, terminator operands
//! and outgoing transports, and the dominance the definition block must hold
//! over every use block — then builds the one transformed plan that audit
//! permits and requires the proposal to equal it by content. Equality covers
//! stripping: removing the copy and the fresh register and rebinding the
//! references back restores the complete source, because every other record
//! of the expected plan is the source verbatim. A producer admission error
//! therefore fails here even when the proposal is exactly what that producer
//! emitted.

use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use optimization_unit::ValueDefinitionSite;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedCaseDispatchSource, SelectedCasePayloadTransport, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedInstructionProvenance, SelectedOperand, SelectedStructuralTransport,
    SelectedValueTransport, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::ScalarType;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{RuntimeSplitError, RuntimeSplitReceipt, ValidatedRuntimeSplit, shifted_position};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::runtime_spill::{
    control, control_mut, control_successors_mut, require_dominated_uses,
};

/// Rebuild the complete transformed plan the split contract permits for
/// `register`, from the source records alone.
fn expected_plan(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<SelectedInstructionPlan, RuntimeSplitError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(RuntimeSplitError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(RuntimeSplitError::SourceMismatch)?;
    let victim = function
        .virtual_registers
        .iter()
        .find(|value| value.id == register)
        .ok_or(RuntimeSplitError::UnsupportedValue)?;
    let VirtualRegisterOrigin::InstructionResult { source_value, .. } = victim.origin else {
        return Err(RuntimeSplitError::UnsupportedValue);
    };
    let payload = match victim.scalar_type {
        ScalarType::Boolean => true,
        ScalarType::IeeeFloat(_) => false,
        ScalarType::Integer(integer) => {
            !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64)
        }
    };
    let custody = matches!(
        victim.definition_site,
        Some(ValueDefinitionSite::FunctionParameter(_))
            | Some(ValueDefinitionSite::BlockParameter { .. })
            | Some(ValueDefinitionSite::Node { .. })
    );
    if !payload || !custody || victim.entry_fixed_view.is_some() {
        return Err(RuntimeSplitError::UnsupportedValue);
    }
    let row = environment
        .constraint(environment.selected_keys().copy_i64)
        .ok_or(RuntimeSplitError::ConstraintMismatch)?;
    let [read, write] = row.operands.as_slice() else {
        return Err(RuntimeSplitError::ConstraintMismatch);
    };
    for (operand, access) in [
        (read, RegisterOperandAccess::Use),
        (write, RegisterOperandAccess::Def),
    ] {
        if operand.access != access
            || operand.class != victim.class
            || operand.fixed_view.is_some()
            || operand.tied_to.is_some()
            || operand.early_clobber
        {
            return Err(RuntimeSplitError::ConstraintMismatch);
        }
    }
    if !row.implicit_uses.is_empty() || !row.implicit_defs.is_empty() || !row.clobbers.is_empty() {
        return Err(RuntimeSplitError::ConstraintMismatch);
    }

    // Locate the one instruction writing the victim — by its write, not the
    // origin's selection-time ordinal — and require that write to be pinned.
    // A second writing instruction is a redefinition and rejects.
    let mut located = None;
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (instruction_index, instruction) in block.instructions.iter().enumerate() {
            let writes = instruction.operands.iter().any(|operand| {
                operand.virtual_register == register
                    && matches!(
                        operand.access,
                        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                    )
            });
            if writes {
                if located.is_some() {
                    return Err(RuntimeSplitError::UnsupportedUse);
                }
                located = Some((block_index, instruction_index));
            }
        }
    }
    let (definition_block, definition_index) =
        located.ok_or(RuntimeSplitError::UnsupportedValue)?;
    let defining = &function.blocks[definition_block].instructions[definition_index];
    let written: Vec<&SelectedOperand> = defining
        .operands
        .iter()
        .filter(|operand| operand.virtual_register == register)
        .collect();
    let [written] = written.as_slice() else {
        return Err(RuntimeSplitError::UnsupportedValue);
    };
    if written.access != RegisterOperandAccess::Def
        || written.fixed_view.is_none()
        || written.tied_to.is_some()
        || written.early_clobber
        || written.class != victim.class
    {
        return Err(RuntimeSplitError::UnsupportedValue);
    }

    // Audit every other reference and collect the blocks that read it.
    let plain_read = |operand: &SelectedOperand| {
        operand.access == RegisterOperandAccess::Use
            && operand.tied_to.is_none()
            && !operand.early_clobber
            && operand.class == victim.class
    };
    let mut reads = 0usize;
    let mut read_blocks = Vec::new();
    for (block_index, block) in function.blocks.iter().enumerate() {
        let before = reads;
        for (instruction_index, instruction) in block.instructions.iter().enumerate() {
            let position = (block_index, instruction_index);
            if position == (definition_block, definition_index) {
                continue;
            }
            let later = block_index != definition_block || instruction_index > definition_index;
            for operand in instruction
                .operands
                .iter()
                .filter(|operand| operand.virtual_register == register)
            {
                if !later || !plain_read(operand) {
                    return Err(RuntimeSplitError::UnsupportedUse);
                }
                reads = reads
                    .checked_add(1)
                    .ok_or(RuntimeSplitError::IdentityOverflow)?;
            }
        }
        let (terminal, successors) = control(&block.terminator);
        for operand in terminal
            .operands
            .iter()
            .filter(|operand| operand.virtual_register == register)
        {
            if !plain_read(operand) {
                return Err(RuntimeSplitError::UnsupportedUse);
            }
            reads = reads
                .checked_add(1)
                .ok_or(RuntimeSplitError::IdentityOverflow)?;
        }
        for successor in successors.into_iter().flatten() {
            for binding in &successor.bindings {
                match binding.transport {
                    SelectedValueTransport::Registers { parameter, .. }
                        if parameter == register =>
                    {
                        return Err(RuntimeSplitError::UnsupportedUse);
                    }
                    SelectedValueTransport::Registers { argument, .. } if argument == register => {
                        if binding.semantic.argument != source_value
                            || binding.semantic.scalar_type != victim.scalar_type
                        {
                            return Err(RuntimeSplitError::UnsupportedUse);
                        }
                        reads = reads
                            .checked_add(1)
                            .ok_or(RuntimeSplitError::IdentityOverflow)?;
                    }
                    _ => {}
                }
            }
            for binding in &successor.structural_bindings {
                let named = match binding.transport {
                    SelectedStructuralTransport::WholeValue { argument, .. } => {
                        argument == register
                    }
                    SelectedStructuralTransport::Address {
                        base: selected_instructions::SelectedAddressBase::Register(base),
                        ..
                    } => base == register,
                    _ => false,
                };
                if named {
                    return Err(RuntimeSplitError::UnsupportedUse);
                }
            }
            if let Some(case) = &successor.structural_case {
                if let SelectedCaseDispatchSource::Borrowed { pointer, .. } = case.source
                    && pointer == register
                {
                    return Err(RuntimeSplitError::UnsupportedUse);
                }
                for payload in &case.payloads {
                    let named = match payload.transport {
                        SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter,
                        } => argument == register || parameter == register,
                        SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                            parameter == register
                        }
                        SelectedCasePayloadTransport::Unused => false,
                    };
                    if named {
                        return Err(RuntimeSplitError::UnsupportedUse);
                    }
                }
            }
        }
        if reads != before {
            read_blocks.push(block_index);
        }
    }
    if reads == 0 {
        return Err(RuntimeSplitError::UnsupportedValue);
    }
    let mut steps = 0usize;
    for measured in &plan.functions {
        for block in &measured.blocks {
            steps = steps
                .checked_add(block.instructions.len())
                .and_then(|total| total.checked_add(1))
                .ok_or(RuntimeSplitError::IdentityOverflow)?;
        }
    }
    let steps = reads
        .checked_mul(2)
        .and_then(|charged| steps.checked_add(charged))
        .and_then(|total| total.checked_add(function.blocks.len().checked_mul(2)?))
        .ok_or(RuntimeSplitError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| RuntimeSplitError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(RuntimeSplitError::WorkBudgetExceeded);
    }
    require_dominated_uses(function, definition_block, &read_blocks).map_err(
        |error| match error {
            crate::RuntimeSpillError::SourceMismatch => RuntimeSplitError::SourceMismatch,
            _ => RuntimeSplitError::UnsupportedUse,
        },
    )?;

    // Fresh identities follow every existing instruction and register.
    let mut last_instruction = 0u32;
    for block in &function.blocks {
        for instruction in &block.instructions {
            last_instruction = last_instruction.max(instruction.id.0);
        }
        last_instruction = last_instruction.max(control(&block.terminator).0.id.0);
    }
    let copy_id = SelectedInstructionId(
        last_instruction
            .checked_add(1)
            .ok_or(RuntimeSplitError::IdentityOverflow)?,
    );
    let last_register = function
        .virtual_registers
        .iter()
        .fold(0u32, |last, value| last.max(value.id.0));
    let fresh = VirtualRegisterId(
        last_register
            .checked_add(1)
            .ok_or(RuntimeSplitError::IdentityOverflow)?,
    );
    let row_operand = |constraint: &register_model::RegisterOperandConstraint,
                       register: VirtualRegisterId| SelectedOperand {
        operand: constraint.operand,
        virtual_register: register,
        access: constraint.access,
        class: constraint.class,
        fixed_view: constraint.fixed_view,
        tied_to: constraint.tied_to,
        early_clobber: constraint.early_clobber,
    };
    let copy = SelectedInstruction {
        id: copy_id,
        kind: SelectedInstructionKind::CopyI64,
        constraint: row.key,
        operands: vec![row_operand(read, register), row_operand(write, fresh)],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance {
            values: vec![source_value],
            ..Default::default()
        },
    };
    let fresh_register = VirtualRegister {
        id: fresh,
        scalar_type: victim.scalar_type,
        class: victim.class,
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction: copy_id,
            source_value,
        },
        definition_site: victim.definition_site,
        entry_fixed_view: None,
    };

    // Build the expected plan: the source with every audited reference
    // rebound, the copy placed right after the definition, the fresh register
    // appended, and the definition block's later settlements shifted.
    let mut expected = plan.clone();
    let target = &mut expected.functions[function_index];
    let rebind = |value: &mut VirtualRegisterId| {
        if *value == register {
            *value = fresh;
        }
    };
    for (block_index, block) in target.blocks.iter_mut().enumerate() {
        let first_rebound = if block_index == definition_block {
            definition_index + 1
        } else {
            0
        };
        for instruction in block.instructions.iter_mut().skip(first_rebound) {
            for operand in &mut instruction.operands {
                rebind(&mut operand.virtual_register);
            }
        }
        for operand in &mut control_mut(&mut block.terminator).operands {
            rebind(&mut operand.virtual_register);
        }
        for successor in control_successors_mut(&mut block.terminator)
            .into_iter()
            .flatten()
        {
            for binding in &mut successor.bindings {
                if let SelectedValueTransport::Registers { argument, .. } = &mut binding.transport {
                    rebind(argument);
                }
            }
        }
    }
    let definition_block_id = target.blocks[definition_block].id;
    target.blocks[definition_block]
        .instructions
        .insert(definition_index + 1, copy);
    target.virtual_registers.push(fresh_register);
    let definition_position =
        u32::try_from(definition_index).map_err(|_| RuntimeSplitError::IdentityOverflow)?;
    for settlement in &mut target.boundary_settlements {
        if settlement.block == definition_block_id {
            settlement.instruction_index =
                shifted_position(settlement.instruction_index, definition_position)
                    .ok_or(RuntimeSplitError::IdentityOverflow)?;
        }
    }
    Ok(expected)
}

/// Accept `proposed` only when it equals, by content, the one plan the
/// independently audited split contract permits over `source`.
pub fn validate_runtime_split(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRuntimeSplit, RuntimeSplitError> {
    let expected = expected_plan(source, function_index, register, environment, budget)?;
    if proposed != expected {
        return Err(RuntimeSplitError::ReplayMismatch);
    }
    Ok(ValidatedRuntimeSplit {
        receipt: RuntimeSplitReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
