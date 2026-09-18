//! Independent replay of a condition-materialization fold.
//!
//! The validator never consults
//! [`ConditionMaterializationRule`](super::pair::ConditionMaterializationRule)
//! or the declared table: it re-derives the whole grammar from the
//! instruction records — the consumer kind matched against the emitted
//! `MaterializeBoolean*` forms, the flag-universe partition computed from
//! the environment's own compare rows, the reaching walk over the backward
//! cone with the read position at the consumer's own body index, the
//! producer's operand grammar read off its kind, and the decided predicate
//! the consumer kind names — then rebuilds the expected `MaterializeI64`
//! instruction, requires the proposal to equal it, and restores the
//! complete source by content.

use std::collections::BTreeSet;
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterOperandAccess, RegisterUnitId};
use selected_instructions::{
    MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSemanticKind, MachineTrapBehavior, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, ValidatedMachineEffectCatalog,
};
use semantic_vocabulary::IntegerValue;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    ConditionMaterializationError, ConditionMaterializationReceipt,
    ValidatedConditionMaterialization,
};
use crate::ValidatedSelectedAnalysis;
use crate::peepholes::condition_flow;

/// Independently consume the proposed program: the replay re-derives the
/// decided materialization and its reconstructed `MaterializeI64` from the
/// source records — re-walking the flag units' reaching events at the
/// consumer's body position, re-resolving the producer's constant operands,
/// and re-deciding the predicate the consumer kind names — requires the
/// proposed instruction to equal that reconstruction, and reinserts the
/// source instruction to restore the complete source by content: every
/// other instruction, terminator, register, roster row, call, settlement,
/// and function included. The pair descriptor table is never read.
pub fn validate_condition_materialization_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    consumer: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedConditionMaterialization, ConditionMaterializationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ConditionMaterializationError::SourceMismatch);
    }
    // The replay re-derives the catalog binding too: the bound catalog must
    // describe this plan's target and this environment's constraint catalog
    // and selected-key inventory.
    let catalog = effect_catalog.catalog();
    if catalog.target != plan.target
        || catalog.register_constraints != environment.constraints().identity()
        || catalog.selected_keys != environment.selected_keys()
    {
        return Err(ConditionMaterializationError::EffectSurfaceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(ConditionMaterializationError::SourceMismatch)?;
    let (block_index, position, consumer) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .enumerate()
                .find(|(_, instruction)| instruction.id == consumer)
                .map(|(position, instruction)| (block_index, position, instruction))
        })
        .ok_or(ConditionMaterializationError::UnsupportedConsumer)?;
    // The replay's own grammar: the consumer kind must be one of the
    // emitted condition-state materialization forms — a body instruction
    // carrying a single `Def` operand and reading at least one unit.
    let consumer_kind = match consumer.kind {
        SelectedInstructionKind::MaterializeBooleanEqual => {
            MachineSemanticKind::MaterializeBooleanEqual
        }
        SelectedInstructionKind::MaterializeBooleanU64LessThan => {
            MachineSemanticKind::MaterializeBooleanU64LessThan
        }
        SelectedInstructionKind::MaterializeBooleanI64LessThan => {
            MachineSemanticKind::MaterializeBooleanI64LessThan
        }
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => {
            MachineSemanticKind::MaterializeBooleanU64LessOrEqual
        }
        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            MachineSemanticKind::MaterializeBooleanI64LessOrEqual
        }
        _ => return Err(ConditionMaterializationError::UnsupportedConsumer),
    };
    let [operand] = consumer.operands.as_slice() else {
        return Err(ConditionMaterializationError::UnsupportedConsumer);
    };
    if operand.operand != 0
        || operand.access != RegisterOperandAccess::Def
        || operand.fixed_view.is_some()
        || operand.tied_to.is_some()
        || operand.early_clobber
    {
        return Err(ConditionMaterializationError::UnsupportedConsumer);
    }
    if consumer.implicit_uses.is_empty() {
        return Err(ConditionMaterializationError::UnsupportedUse);
    }
    let keys = environment.selected_keys();
    let consumer_row = environment
        .constraint(consumer.constraint)
        .ok_or(ConditionMaterializationError::ConstraintMismatch)?;
    if !(consumer_row.operands.len() == 1
        && consumer_row.operands[0].operand == 0
        && consumer_row.operands[0].access == RegisterOperandAccess::Def)
    {
        return Err(ConditionMaterializationError::ConstraintMismatch);
    }
    let rewritten_row = environment
        .constraint(
            keys.for_semantic(MachineSemanticKind::MaterializeI64)
                .ok_or(ConditionMaterializationError::ConstraintMismatch)?,
        )
        .ok_or(ConditionMaterializationError::ConstraintMismatch)?;
    if !(rewritten_row.operands.len() == 1
        && rewritten_row.operands[0].operand == 0
        && rewritten_row.operands[0].access == RegisterOperandAccess::Def)
    {
        return Err(ConditionMaterializationError::ConstraintMismatch);
    }
    // The flag universe is the environment's own vocabulary: the implicit
    // units the three condition-state producer rows define. The replay
    // computes it from the selected keys directly, not from the declared
    // table.
    let mut flag_universe = BTreeSet::new();
    for key in [
        keys.compare_i64,
        keys.compare_i64_immediate,
        keys.compare_i64_zero,
    ] {
        let row = environment
            .constraint(key)
            .ok_or(ConditionMaterializationError::ConstraintMismatch)?;
        flag_universe.extend(row.implicit_defs.iter().copied());
    }
    let mut flag_uses = Vec::new();
    let mut plain_uses = Vec::new();
    for unit in &consumer.implicit_uses {
        if flag_universe.contains(unit) {
            flag_uses.push(*unit);
        } else {
            plain_uses.push(*unit);
        }
    }
    if flag_uses.is_empty() {
        return Err(ConditionMaterializationError::UnsupportedUse);
    }
    let entry = condition_flow::entry_index(function)
        .ok_or(ConditionMaterializationError::UnsupportedUse)?;
    let (successors, predecessors) = condition_flow::adjacency(function);
    let cone = condition_flow::backward_cone(&predecessors, block_index);
    let mut producer_site = None;
    for &unit in &flag_uses {
        let event = condition_flow::reaching_event(
            function,
            entry,
            &successors,
            &cone,
            block_index,
            position,
            unit,
        )
        .map_err(|_| ConditionMaterializationError::UnsupportedUse)?;
        if producer_site.is_some_and(|site| site != event) {
            return Err(ConditionMaterializationError::UnsupportedUse);
        }
        producer_site = Some(event);
    }
    let producer_site = producer_site.ok_or(ConditionMaterializationError::UnsupportedUse)?;
    let producer = condition_flow::instruction_at(function, producer_site);
    if flag_uses
        .iter()
        .any(|unit| !producer.implicit_defs.contains(unit))
    {
        return Err(ConditionMaterializationError::UnsupportedUse);
    }
    // The replay restates the unit-flow relationship from the records: the
    // flag uses retire, every non-flag use must stay read on the rewritten
    // row, and the consumer's implicit definitions and clobbers republish
    // verbatim.
    if plain_uses
        .iter()
        .any(|unit| !rewritten_row.implicit_uses.contains(unit))
        || rewritten_row.implicit_defs != consumer.implicit_defs
        || rewritten_row.clobbers != consumer.clobbers
        || rewritten_row
            .implicit_uses
            .iter()
            .any(|unit| !consumer.implicit_uses.contains(unit))
    {
        return Err(ConditionMaterializationError::UnsupportedUse);
    }
    // The operand grammar is re-derived from the producer's kind alone —
    // two register operands, a register and the kind's immediate, or a
    // register and the zero bound — and the predicate from the consumer's
    // kind alone.
    let (left, right) = replayed_operands(function, producer)?;
    let value = decided_value(consumer_kind, left, right);
    // The effect surface is restated on the bound catalog: the consumer's
    // condition read and the rewritten materialization are both
    // effect-isolated fall-through rows, the consumer's encoded implicit
    // uses stay inside the flag universe, and the rewritten alternatives
    // carry no implicit unit traffic at all.
    let consumer_declaration =
        effect_declaration(effect_catalog, consumer_kind, consumer.constraint)
            .ok_or(ConditionMaterializationError::EffectSurfaceMismatch)?;
    let rewritten_declaration = effect_declaration(
        effect_catalog,
        MachineSemanticKind::MaterializeI64,
        rewritten_row.key,
    )
    .ok_or(ConditionMaterializationError::EffectSurfaceMismatch)?;
    if !decided_materialization_declarations(
        &flag_universe,
        consumer_declaration,
        rewritten_declaration,
    ) {
        return Err(ConditionMaterializationError::EffectSurfaceMismatch);
    }
    let expected = SelectedInstruction {
        id: consumer.id,
        kind: SelectedInstructionKind::MaterializeI64 { value },
        constraint: rewritten_row.key,
        operands: vec![*operand],
        implicit_uses: rewritten_row.implicit_uses.clone(),
        implicit_defs: rewritten_row.implicit_defs.clone(),
        clobbers: rewritten_row.clobbers.clone(),
        provenance: consumer.provenance.clone(),
    };
    let proposed_function = proposed
        .functions
        .get(function_index)
        .ok_or(ConditionMaterializationError::ReplayMismatch)?;
    if proposed_function
        .blocks
        .get(block_index)
        .and_then(|block| block.instructions.get(position))
        != Some(&expected)
    {
        return Err(ConditionMaterializationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[block_index].instructions[position] =
        consumer.clone();
    if restored != *source.selected_plan() {
        return Err(ConditionMaterializationError::ReplayMismatch);
    }
    // The replay charges the same bounded walk into the budget even though
    // admission already paid it: a proposal validated on a starved budget
    // must not pass where admission would refuse.
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(ConditionMaterializationError::IdentityOverflow)?;
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(ConditionMaterializationError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    let elements = function_scan
        .checked_add(1)
        .ok_or(ConditionMaterializationError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(ConditionMaterializationError::IdentityOverflow)?,
        )
        .ok_or(ConditionMaterializationError::IdentityOverflow)?;
    let widest_out = successors
        .iter()
        .map(|targets| targets.len())
        .max()
        .unwrap_or(0);
    let per_unit = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .and_then(|total| {
            total.checked_add(
                flag_universe
                    .len()
                    .checked_add(rewritten_row.implicit_uses.len())?
                    .checked_add(producer.implicit_defs.len())?,
            )
        })
        .ok_or(ConditionMaterializationError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(ConditionMaterializationError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .and_then(|total| total.checked_add(flag_universe.len()))
        .and_then(|total| total.checked_add(rewritten_row.implicit_uses.len()))
        .ok_or(ConditionMaterializationError::IdentityOverflow)?;
    let reach_scan = consumer
        .implicit_uses
        .len()
        .checked_mul(per_unit)
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or(ConditionMaterializationError::IdentityOverflow)?;
    let steps = source
        .selected_plan()
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            total
                .checked_add(function_scan.checked_mul(2)?)?
                .checked_add(reach_scan)
        })
        .ok_or(ConditionMaterializationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| ConditionMaterializationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(ConditionMaterializationError::WorkBudgetExceeded);
    }
    Ok(ValidatedConditionMaterialization {
        receipt: ConditionMaterializationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}

/// The `(left, right)` state the replay resolves from the producer's own
/// kind — the descriptor's operand-resolution axis restated as a direct
/// grammar match.
fn replayed_operands(
    function: &selected_instructions::SelectedFunction,
    producer: &SelectedInstruction,
) -> Result<(u64, u64), ConditionMaterializationError> {
    match producer.kind {
        SelectedInstructionKind::CompareI64 => {
            let [left, right] = producer.operands.as_slice() else {
                return Err(ConditionMaterializationError::UndecidedOperands);
            };
            let left = replay_use(left, 0)?;
            let right = replay_use(right, 1)?;
            if left == right {
                return Ok((0, 0));
            }
            Ok((
                condition_flow::materialized_bits(function, left)
                    .map_err(|_| ConditionMaterializationError::UndecidedOperands)?,
                condition_flow::materialized_bits(function, right)
                    .map_err(|_| ConditionMaterializationError::UndecidedOperands)?,
            ))
        }
        SelectedInstructionKind::CompareI64Immediate { immediate } => {
            let [operand] = producer.operands.as_slice() else {
                return Err(ConditionMaterializationError::UndecidedOperands);
            };
            let left = replay_use(operand, 0)?;
            let immediate = match immediate {
                IntegerValue::Signed(value) => u64::try_from(value)
                    .map_err(|_| ConditionMaterializationError::UndecidedOperands)?,
                IntegerValue::Unsigned(value) => u64::try_from(value)
                    .map_err(|_| ConditionMaterializationError::UndecidedOperands)?,
            };
            Ok((
                condition_flow::materialized_bits(function, left)
                    .map_err(|_| ConditionMaterializationError::UndecidedOperands)?,
                immediate,
            ))
        }
        SelectedInstructionKind::CompareI64Zero => {
            let [operand] = producer.operands.as_slice() else {
                return Err(ConditionMaterializationError::UndecidedOperands);
            };
            let left = replay_use(operand, 0)?;
            Ok((
                condition_flow::materialized_bits(function, left)
                    .map_err(|_| ConditionMaterializationError::UndecidedOperands)?,
                0,
            ))
        }
        _ => Err(ConditionMaterializationError::UnsupportedProducer),
    }
}

/// A plain `Use` operand at `position` under the replay's own operand
/// contract: the register the flag computation reads is the operand's own,
/// undecorated.
fn replay_use(
    operand: &selected_instructions::SelectedOperand,
    position: usize,
) -> Result<selected_instructions::VirtualRegisterId, ConditionMaterializationError> {
    if operand.operand != position as u16
        || operand.access != RegisterOperandAccess::Use
        || operand.fixed_view.is_some()
        || operand.tied_to.is_some()
        || operand.early_clobber
    {
        return Err(ConditionMaterializationError::UndecidedOperands);
    }
    Ok(operand.virtual_register)
}

/// The predicate the consumer kind names, decided on the resolved state —
/// the descriptor's decided-value axis restated as a direct match on the
/// emitted materialization kinds.
fn decided_value(consumer: MachineSemanticKind, left: u64, right: u64) -> IntegerValue {
    let holds = match consumer {
        MachineSemanticKind::MaterializeBooleanEqual => left == right,
        MachineSemanticKind::MaterializeBooleanU64LessThan => left < right,
        MachineSemanticKind::MaterializeBooleanI64LessThan => (left as i64) < (right as i64),
        MachineSemanticKind::MaterializeBooleanU64LessOrEqual => left <= right,
        MachineSemanticKind::MaterializeBooleanI64LessOrEqual => (left as i64) <= (right as i64),
        _ => unreachable!("the consumer grammar admits only the materialization kinds"),
    };
    IntegerValue::Unsigned(u128::from(holds))
}

/// The effect surface the replay restates on the bound catalog: both
/// declarations are isolated fall-through rows, the consumer's encoded
/// alternatives carry implicit uses only inside `flag_universe` — the
/// condition-state read is the whole input — and the rewritten form's
/// alternatives carry no implicit unit traffic at all.
fn decided_materialization_declarations(
    flag_universe: &BTreeSet<RegisterUnitId>,
    consumer: &MachineEffectDeclaration,
    rewritten: &MachineEffectDeclaration,
) -> bool {
    isolated_declaration(consumer)
        && isolated_declaration(rewritten)
        && consumer
            .alternatives
            .iter()
            .all(|alternative| materialization_alternative(alternative, flag_universe))
        && rewritten.alternatives.iter().all(materialize_alternative)
}

/// The declared surface a decided-materialization form must carry: no
/// memory access, the operation itself never traps, no control-flow
/// barrier, and no call or cleanup — the plain fall-through isolation both
/// the flag read and the literal materialization publish.
fn isolated_declaration(declaration: &MachineEffectDeclaration) -> bool {
    declaration.memory == MachineMemoryEffect::NoneV1
        && declaration.trap == MachineTrapBehavior::NeverV1
        && declaration.barrier == MachineBarrier::None
        && declaration.call == MachineCallEffect::NoneV1
        && declaration.cleanup == MachineCleanupEffect::NoneV1
}

/// The encoded surface a condition-reading alternative must carry: no
/// memory access, unchanged stack, no trap surface, a plain fall-through —
/// and implicit uses only inside the flag universe, with no implicit
/// definitions or clobbers: the condition-state read is the whole input.
fn materialization_alternative(
    alternative: &MachineAlternative,
    flag_universe: &BTreeSet<RegisterUnitId>,
) -> bool {
    let encoded = &alternative.encoded;
    encoded.memory == MachineEncodedMemoryEffect::NoneV1
        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
        && encoded.trap == MachineEncodedTrapBehavior::NeverV1
        && encoded.control == MachineEncodedControlEffect::FallThroughV1
        && !encoded.implicit_unit_uses.is_empty()
        && encoded
            .implicit_unit_uses
            .iter()
            .all(|unit| flag_universe.contains(unit))
        && encoded.implicit_unit_defs.is_empty()
        && encoded.implicit_unit_clobbers.is_empty()
}

/// The encoded surface a materialization alternative must carry: no memory
/// access, unchanged stack, no trap surface, a plain fall-through — and no
/// implicit unit traffic at all.
fn materialize_alternative(alternative: &MachineAlternative) -> bool {
    let encoded = &alternative.encoded;
    encoded.memory == MachineEncodedMemoryEffect::NoneV1
        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
        && encoded.trap == MachineEncodedTrapBehavior::NeverV1
        && encoded.control == MachineEncodedControlEffect::FallThroughV1
        && encoded.implicit_unit_uses.is_empty()
        && encoded.implicit_unit_defs.is_empty()
        && encoded.implicit_unit_clobbers.is_empty()
}

/// The single catalog declaration for `semantic` bound to `constraint`, or
/// none when the catalog does not declare exactly one such form.
fn effect_declaration(
    catalog: &ValidatedMachineEffectCatalog,
    semantic: MachineSemanticKind,
    constraint: register_model::RegisterConstraintKey,
) -> Option<&MachineEffectDeclaration> {
    let mut matches = catalog.catalog().declarations.iter().filter(|declaration| {
        declaration.semantic == semantic && declaration.constraint == constraint
    });
    let declaration = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(declaration)
}
