//! Independent validation of runtime-value rematerialization.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! regeneration's legality from the source records — the named register's
//! roster row and single `MaterializeI64` definition against the target's
//! materialize constraint, the scalar payload and definition-site custody
//! the regenerable family admits, the use scan over instruction operands,
//! terminator operands and outgoing edge transports, and the dominance the
//! definition block must hold over every use block — then requires the
//! proposal to carry exactly the fresh registers, repeated
//! materializations and rebound operands the audit derives, with the
//! boundary settlements remapped over the insertions. Stripping those
//! additions must restore the complete source by content. A producer
//! admission error therefore fails validation even when the proposal is
//! exactly what that producer emitted.

use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use optimization_unit::ValueDefinitionSite;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedBoundarySettlementPayload, SelectedCasePayloadTransport, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedInstructionProvenance, SelectedOperand, SelectedStructuralTransport,
    SelectedValueTransport, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerValue, ScalarType, ValueId};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    RuntimeRematerializationError, RuntimeRematerializationReceipt,
    ValidatedRuntimeRematerialization,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::runtime_spill::{control, control_mut, require_dominated_uses};

/// The validator's own reconstruction of the regeneration the contract
/// permits: the admitted victim, its proven immediate and materialize row,
/// the use blocks the regeneration serves, and the first fresh instruction
/// and register identities the insertions consume. It shares no state with
/// the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    use_blocks: Vec<usize>,
    victim: &'source VirtualRegister,
    source_value: ValueId,
    value: IntegerValue,
    first_instruction: u32,
    first_register: u32,
    materialize: &'source RegisterInstructionConstraint,
}

/// The fresh register and materialization one admitted use demands, at an
/// instruction operand or at a terminator operand. The validator builds
/// this pair from its own reconstruction — the victim's exact type, class
/// and definition site, the proven immediate, the target row's operand
/// interface, and the source value's provenance — so a wrong emission
/// inside the producer's constructor cannot launder through both sides.
struct ExpectedMaterialization {
    register: VirtualRegister,
    instruction: SelectedInstruction,
}

fn fresh(next: &mut u32) -> Result<u32, RuntimeRematerializationError> {
    let result = *next;
    *next = next
        .checked_add(1)
        .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
    Ok(result)
}

fn expected_materialization(
    reconstructed: &Reconstructed<'_>,
    next_instruction: &mut u32,
    next_register: &mut u32,
) -> Result<ExpectedMaterialization, RuntimeRematerializationError> {
    let instruction = SelectedInstructionId(fresh(next_instruction)?);
    let register = VirtualRegisterId(fresh(next_register)?);
    let operand = &reconstructed.materialize.operands[0];
    Ok(ExpectedMaterialization {
        register: VirtualRegister {
            id: register,
            scalar_type: reconstructed.victim.scalar_type,
            class: reconstructed.victim.class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value: reconstructed.source_value,
            },
            definition_site: reconstructed.victim.definition_site,
            entry_fixed_view: None,
        },
        instruction: SelectedInstruction {
            id: instruction,
            kind: SelectedInstructionKind::MaterializeI64 {
                value: reconstructed.value,
            },
            constraint: reconstructed.materialize.key,
            operands: vec![SelectedOperand {
                operand: operand.operand,
                virtual_register: register,
                access: operand.access,
                class: operand.class,
                fixed_view: operand.fixed_view,
                tied_to: operand.tied_to,
                early_clobber: operand.early_clobber,
            }],
            implicit_uses: reconstructed.materialize.implicit_uses.clone(),
            implicit_defs: reconstructed.materialize.implicit_defs.clone(),
            clobbers: reconstructed.materialize.clobbers.clone(),
            provenance: SelectedInstructionProvenance {
                values: vec![reconstructed.source_value],
                ..Default::default()
            },
        },
    })
}

/// Reconstruct the legality of regenerating `register` before each of its
/// uses from the source records: the victim must be a non-address runtime
/// scalar defined by exactly one `MaterializeI64` — the target row's one
/// flexible def with no implicit traffic, the recorded instruction equal
/// to that row — every operand use must be flexible, untied and
/// non-early-clobber in the victim's own class, no outgoing edge transport
/// may carry it, and the definition block must dominate every use block.
/// The audit charges the family's measured steps against the budget.
/// Nothing in it reads the producer's admission decision, so a
/// producer-side legality error fails here even when the proposal matches
/// the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, RuntimeRematerializationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(RuntimeRematerializationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(RuntimeRematerializationError::SourceMismatch)?;
    let victim = function
        .virtual_registers
        .iter()
        .find(|value| value.id == register)
        .ok_or(RuntimeRematerializationError::UnsupportedValue)?;
    let VirtualRegisterOrigin::InstructionResult {
        instruction: definition,
        source_value,
    } = victim.origin
    else {
        return Err(RuntimeRematerializationError::UnsupportedValue);
    };
    let block_index = function
        .blocks
        .iter()
        .position(|block| {
            block
                .instructions
                .iter()
                .any(|candidate| candidate.id == definition)
        })
        .ok_or(RuntimeRematerializationError::UnsupportedValue)?;
    // The regenerable family is one nonaddress runtime value produced by a
    // target-materialized immediate: Boolean, IEEE scalar, or an 8-to-64-bit
    // integer that is not an address. The victim must carry ordinary
    // definition-site custody and no entry-pinned view.
    let scalar_payload = match victim.scalar_type {
        ScalarType::Boolean | ScalarType::IeeeFloat(_) => true,
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
        return Err(RuntimeRematerializationError::UnsupportedValue);
    }
    let materialize = environment
        .constraint(environment.selected_keys().materialize_i64)
        .ok_or(RuntimeRematerializationError::ConstraintMismatch)?;
    // The target row must be a single flexible def with no hidden effects:
    // implicit uses would make repetition consume unproven state, implicit
    // defs or clobbers would destroy values the replayed stream does not
    // name.
    if materialize.operands.len() != 1
        || !materialize.implicit_uses.is_empty()
        || !materialize.implicit_defs.is_empty()
        || !materialize.clobbers.is_empty()
    {
        return Err(RuntimeRematerializationError::ConstraintMismatch);
    }
    let row_operand = &materialize.operands[0];
    if row_operand.access != RegisterOperandAccess::Def
        || row_operand.class != victim.class
        || row_operand.fixed_view.is_some()
        || row_operand.tied_to.is_some()
        || row_operand.early_clobber
    {
        return Err(RuntimeRematerializationError::ConstraintMismatch);
    }
    let defined_instruction = function.blocks[block_index]
        .instructions
        .iter()
        .find(|candidate| candidate.id == definition)
        .ok_or(RuntimeRematerializationError::UnsupportedValue)?;
    let SelectedInstructionKind::MaterializeI64 { value } = defined_instruction.kind else {
        return Err(RuntimeRematerializationError::UnsupportedValue);
    };
    // The recorded definition must be the validated target row itself with
    // exactly its one def operand on the victim and no other effects.
    if defined_instruction.constraint != materialize.key
        || defined_instruction.operands.len() != 1
        || defined_instruction.implicit_uses != materialize.implicit_uses
        || defined_instruction.implicit_defs != materialize.implicit_defs
        || defined_instruction.clobbers != materialize.clobbers
    {
        return Err(RuntimeRematerializationError::UnsupportedValue);
    }
    let def_operand = &defined_instruction.operands[0];
    if def_operand.virtual_register != register
        || def_operand.operand != row_operand.operand
        || def_operand.access != RegisterOperandAccess::Def
        || def_operand.class != victim.class
        || def_operand.fixed_view.is_some()
        || def_operand.tied_to.is_some()
        || def_operand.early_clobber
    {
        return Err(RuntimeRematerializationError::UnsupportedValue);
    }
    let mut defined = false;
    let mut uses = 0usize;
    let mut use_blocks = Vec::new();
    for (current_block_index, block) in function.blocks.iter().enumerate() {
        let previous_uses = uses;
        let (terminal, successors) = control(&block.terminator);
        // A terminator operand use executes after every block instruction,
        // so the fresh materialize serves it from the end of the block; the
        // dominance check below already covers that position. A fixed view
        // on the operand stays attached and pins the fresh register to the
        // same physical unit, so ABI-pinned returns and hosted exits stay
        // exact. Tied, early-clobber, and defining references stay rejected.
        for operand in &terminal.operands {
            if operand.virtual_register != register {
                continue;
            }
            match operand.access {
                RegisterOperandAccess::Use
                    if operand.tied_to.is_none()
                        && !operand.early_clobber
                        && operand.class == victim.class =>
                {
                    // An output tied to this use would extend the fresh
                    // register's value identity.
                    if terminal
                        .operands
                        .iter()
                        .any(|other| other.tied_to == Some(operand.operand))
                    {
                        return Err(RuntimeRematerializationError::UnsupportedUse);
                    }
                    uses = uses
                        .checked_add(1)
                        .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
                }
                _ => return Err(RuntimeRematerializationError::UnsupportedUse),
            }
        }
        if successors.into_iter().flatten().any(|successor| {
            successor.bindings.iter().any(|binding| {
                matches!(binding.transport,
                SelectedValueTransport::Registers { argument, parameter }
                    if argument == register || parameter == register)
            }) || successor.structural_bindings.iter().any(|binding| {
                matches!(binding.transport,
                SelectedStructuralTransport::Descriptor { argument, .. }
                | SelectedStructuralTransport::Address {
                    base: selected_instructions::SelectedAddressBase::Register(argument),
                    ..
                } if argument == register)
            }) || successor.structural_case.as_ref().is_some_and(|case| {
                case.payloads.iter().any(|payload| match payload.transport {
                    SelectedCasePayloadTransport::Registers {
                        argument,
                        parameter,
                    } => argument == register || parameter == register,
                    SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                        parameter == register
                    }
                    SelectedCasePayloadTransport::Unused => false,
                })
            })
        }) {
            return Err(RuntimeRematerializationError::UnsupportedUse);
        }
        for instruction in &block.instructions {
            for operand in &instruction.operands {
                if operand.virtual_register != register {
                    continue;
                }
                match operand.access {
                    RegisterOperandAccess::Def
                        if !defined
                            && current_block_index == block_index
                            && instruction.id == definition =>
                    {
                        defined = true;
                    }
                    RegisterOperandAccess::Use
                        if (defined || current_block_index != block_index)
                            && instruction.id != definition
                            && operand.fixed_view.is_none()
                            && operand.tied_to.is_none()
                            && !operand.early_clobber
                            && operand.class == victim.class =>
                    {
                        // An output tied to this use would extend the fresh
                        // register's value identity.
                        if instruction
                            .operands
                            .iter()
                            .any(|other| other.tied_to == Some(operand.operand))
                        {
                            return Err(RuntimeRematerializationError::UnsupportedUse);
                        }
                        uses = uses
                            .checked_add(1)
                            .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
                    }
                    _ => return Err(RuntimeRematerializationError::UnsupportedUse),
                }
            }
        }
        if uses != previous_uses {
            use_blocks.push(current_block_index);
        }
    }
    if !defined || uses == 0 {
        return Err(RuntimeRematerializationError::UnsupportedValue);
    }
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| total.checked_add(uses.checked_mul(2)?))
        .and_then(|total| total.checked_add(function.blocks.len().checked_mul(2)?))
        .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| RuntimeRematerializationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(RuntimeRematerializationError::WorkBudgetExceeded);
    }
    require_dominated_uses(function, block_index, &use_blocks).map_err(|error| match error {
        crate::RuntimeSpillError::SourceMismatch => RuntimeRematerializationError::SourceMismatch,
        _ => RuntimeRematerializationError::UnsupportedUse,
    })?;
    let first_instruction = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id.0)
                .chain([control(&block.terminator).0.id.0])
        })
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
    let first_register = function
        .virtual_registers
        .iter()
        .map(|value| value.id.0)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
    Ok(Reconstructed {
        function,
        use_blocks,
        victim,
        source_value,
        value,
        first_instruction,
        first_register,
        materialize,
    })
}

/// Independently consume the proposed instruction stream, checking every fresh
/// materialization and rewritten operand against its original use. Stripping
/// these exact additions must restore the entire admitted source, including
/// calls and fuel.
pub fn validate_runtime_rematerialization(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRuntimeRematerialization, RuntimeRematerializationError> {
    let reconstructed = reconstruct(source, function_index, register, environment, budget)?;
    let function = proposed
        .functions
        .get(function_index)
        .ok_or(RuntimeRematerializationError::ReplayMismatch)?;
    let mut restored = proposed.clone();
    let restored_function = &mut restored.functions[function_index];
    if restored_function.boundary_settlements.len()
        != reconstructed.function.boundary_settlements.len()
    {
        return Err(RuntimeRematerializationError::ReplayMismatch);
    }
    let mut values = function
        .virtual_registers
        .iter()
        .skip(reconstructed.function.virtual_registers.len());
    let mut next_instruction = reconstructed.first_instruction;
    let mut next_register = reconstructed.first_register;
    for (block_index, source_block) in reconstructed.function.blocks.iter().enumerate() {
        if !reconstructed.use_blocks.contains(&block_index) {
            continue;
        }
        let block = function
            .blocks
            .get(block_index)
            .ok_or(RuntimeRematerializationError::ReplayMismatch)?;
        let mut stream = block.instructions.iter();
        let mut boundaries = Vec::new();
        let mut instruction_positions = Vec::new();
        let mut consumed = 0u32;
        for original in &source_block.instructions {
            boundaries.push(consumed);
            let mut restored = original.clone();
            for operand in &mut restored.operands {
                if operand.virtual_register != register
                    || operand.access != RegisterOperandAccess::Use
                {
                    continue;
                }
                let materialization = expected_materialization(
                    &reconstructed,
                    &mut next_instruction,
                    &mut next_register,
                )?;
                if values.next() != Some(&materialization.register)
                    || stream.next() != Some(&materialization.instruction)
                {
                    return Err(RuntimeRematerializationError::ReplayMismatch);
                }
                consumed = consumed
                    .checked_add(1)
                    .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
                operand.virtual_register = materialization.register.id;
            }
            instruction_positions.push(consumed);
            if stream.next() != Some(&restored) {
                return Err(RuntimeRematerializationError::ReplayMismatch);
            }
            consumed = consumed
                .checked_add(1)
                .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
        }
        // The rewrite appends terminator-operand materializations after the
        // last body instruction; replay consumes them in operand order, then
        // requires the proposed terminator to equal the source with exactly
        // those operands redirected to their fresh registers.
        let mut expected_terminator = source_block.terminator.clone();
        for operand in &mut control_mut(&mut expected_terminator).operands {
            if operand.virtual_register != register || operand.access != RegisterOperandAccess::Use
            {
                continue;
            }
            let materialization = expected_materialization(
                &reconstructed,
                &mut next_instruction,
                &mut next_register,
            )?;
            if values.next() != Some(&materialization.register)
                || stream.next() != Some(&materialization.instruction)
            {
                return Err(RuntimeRematerializationError::ReplayMismatch);
            }
            consumed = consumed
                .checked_add(1)
                .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
            operand.virtual_register = materialization.register.id;
        }
        if block.terminator != expected_terminator {
            return Err(RuntimeRematerializationError::ReplayMismatch);
        }
        restored_function.blocks[block_index]
            .terminator
            .clone_from(&source_block.terminator);
        boundaries.push(consumed);
        instruction_positions.push(consumed);
        if stream.next().is_some() {
            return Err(RuntimeRematerializationError::ReplayMismatch);
        }
        for (actual, original) in restored_function
            .boundary_settlements
            .iter_mut()
            .zip(&reconstructed.function.boundary_settlements)
        {
            if original.block != source_block.id {
                continue;
            }
            let positions = if matches!(
                original.settlement,
                SelectedBoundarySettlementPayload::ClaimCompletion(_)
            ) {
                &boundaries
            } else {
                &instruction_positions
            };
            let expected = positions
                .get(original.instruction_index as usize)
                .ok_or(RuntimeRematerializationError::SourceMismatch)?;
            if actual.block != source_block.id || actual.instruction_index != *expected {
                return Err(RuntimeRematerializationError::ReplayMismatch);
            }
            actual.instruction_index = original.instruction_index;
        }
        restored_function.blocks[block_index]
            .instructions
            .clone_from(&source_block.instructions);
    }
    if values.next().is_some() {
        return Err(RuntimeRematerializationError::ReplayMismatch);
    }
    restored_function
        .virtual_registers
        .truncate(reconstructed.function.virtual_registers.len());
    if restored != *source.selected_plan() {
        return Err(RuntimeRematerializationError::ReplayMismatch);
    }
    Ok(ValidatedRuntimeRematerialization {
        receipt: RuntimeRematerializationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
