//! Independent replay of a terminator-pair fold.
//!
//! The validator never consults [`TerminatorPairRule`](super::pair::TerminatorPairRule)
//! or the declared table: it re-derives the whole grammar from the
//! instruction records — the terminator variant and its carried kind, the
//! flag-universe partition computed from the environment's own compare
//! rows, the reaching walk over the backward cone, the producer's operand
//! grammar read off its kind, and the arm the decided state selects — then
//! rebuilds the expected `Jump` terminator, requires the proposal to equal
//! it, and restores the complete source by content.

use std::collections::BTreeSet;
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSemanticKind, MachineTrapBehavior, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedTerminator,
    ValidatedMachineEffectCatalog,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::condition_flow;
use super::{TerminatorPairError, TerminatorPairReceipt, ValidatedTerminatorPair};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: the replay re-derives the
/// decided branch and its reconstructed `Jump` from the source records —
/// re-walking the flag units' reaching events, re-resolving the producer's
/// constant operands, and re-selecting the successor arm — requires the
/// proposed terminator to equal that reconstruction, and reinserts the
/// source terminator to restore the complete source by content: every other
/// instruction, register, roster row, call, settlement, and function
/// included. The pair descriptor table is never read.
pub fn validate_terminator_pair_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    terminator: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedTerminatorPair, TerminatorPairError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(TerminatorPairError::SourceMismatch);
    }
    // The replay re-derives the catalog binding too: the bound catalog must
    // describe this plan's target and this environment's constraint catalog
    // and selected-key inventory.
    let catalog = effect_catalog.catalog();
    if catalog.target != plan.target
        || catalog.register_constraints != environment.constraints().identity()
        || catalog.selected_keys != environment.selected_keys()
    {
        return Err(TerminatorPairError::EffectSurfaceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(TerminatorPairError::SourceMismatch)?;
    let (block_index, block) = function
        .blocks
        .iter()
        .enumerate()
        .find(|(_, block)| {
            condition_flow::terminator_instruction(&block.terminator).id == terminator
        })
        .ok_or(TerminatorPairError::UnsupportedTerminator)?;
    let consumer = condition_flow::terminator_instruction(&block.terminator);
    // The replay's own grammar: the terminator variant and its carried kind
    // must pair as one of the emitted conditional-branch flag readers —
    // decided branches carry no explicit operands and read at least one
    // unit.
    let consumer_kind = match (&block.terminator, consumer.kind) {
        (
            SelectedTerminator::ConditionalBranch { .. },
            SelectedInstructionKind::ConditionalBranchNonZero,
        ) => MachineSemanticKind::ConditionalBranchNonZero,
        (
            SelectedTerminator::ConditionalBranchU64LessThan { .. },
            SelectedInstructionKind::ConditionalBranchU64LessThan,
        ) => MachineSemanticKind::ConditionalBranchU64LessThan,
        (
            SelectedTerminator::ConditionalBranchI64LessThan { .. },
            SelectedInstructionKind::ConditionalBranchI64LessThan,
        ) => MachineSemanticKind::ConditionalBranchI64LessThan,
        _ => return Err(TerminatorPairError::UnsupportedTerminator),
    };
    if !consumer.operands.is_empty() {
        return Err(TerminatorPairError::UnsupportedTerminator);
    }
    if consumer.implicit_uses.is_empty() {
        return Err(TerminatorPairError::UnsupportedUse);
    }
    let keys = environment.selected_keys();
    let Some(consumer_row) = environment.constraint(consumer.constraint) else {
        return Err(TerminatorPairError::ConstraintMismatch);
    };
    if !consumer_row.operands.is_empty() {
        return Err(TerminatorPairError::ConstraintMismatch);
    }
    let rewritten_row = environment
        .constraint(keys.jump)
        .ok_or(TerminatorPairError::ConstraintMismatch)?;
    if !rewritten_row.operands.is_empty() {
        return Err(TerminatorPairError::ConstraintMismatch);
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
            .ok_or(TerminatorPairError::ConstraintMismatch)?;
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
        return Err(TerminatorPairError::UnsupportedUse);
    }
    let entry = condition_flow::entry_index(function).ok_or(TerminatorPairError::UnsupportedUse)?;
    let (successors, predecessors) = condition_flow::adjacency(function);
    let cone = condition_flow::backward_cone(&predecessors, block_index);
    let read_position = block.instructions.len();
    let mut producer_site = None;
    for &unit in &flag_uses {
        let event = condition_flow::reaching_event(
            function,
            entry,
            &successors,
            &cone,
            block_index,
            read_position,
            unit,
        )
        .map_err(|_| TerminatorPairError::UnsupportedUse)?;
        if producer_site.is_some_and(|site| site != event) {
            return Err(TerminatorPairError::UnsupportedUse);
        }
        producer_site = Some(event);
    }
    let producer_site = producer_site.ok_or(TerminatorPairError::UnsupportedUse)?;
    let producer = condition_flow::instruction_at(function, producer_site);
    if flag_uses
        .iter()
        .any(|unit| !producer.implicit_defs.contains(unit))
    {
        return Err(TerminatorPairError::UnsupportedUse);
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
        return Err(TerminatorPairError::UnsupportedUse);
    }
    // The operand grammar is re-derived from the producer's kind alone —
    // two register operands, a register and the kind's immediate, or a
    // register and the zero bound.
    let (left, right) = replayed_operands(function, producer)?;
    let successor = decided_successor(&block.terminator, consumer_kind, left, right)
        .ok_or(TerminatorPairError::UnsupportedTerminator)?
        .clone();
    // The control-flow relationship is restated on the bound catalog: the
    // consumer encodes a conditional relative branch and the rewritten row
    // an unconditional one, both otherwise isolated.
    let consumer_declaration =
        effect_declaration(effect_catalog, consumer_kind, consumer.constraint)
            .ok_or(TerminatorPairError::EffectSurfaceMismatch)?;
    let rewritten_declaration =
        effect_declaration(effect_catalog, MachineSemanticKind::Jump, rewritten_row.key)
            .ok_or(TerminatorPairError::EffectSurfaceMismatch)?;
    if !decided_branch_declarations(consumer_declaration, rewritten_declaration) {
        return Err(TerminatorPairError::EffectSurfaceMismatch);
    }
    let expected = SelectedTerminator::Jump {
        instruction: SelectedInstruction {
            id: terminator,
            kind: SelectedInstructionKind::Jump,
            constraint: rewritten_row.key,
            operands: Vec::new(),
            implicit_uses: rewritten_row.implicit_uses.clone(),
            implicit_defs: rewritten_row.implicit_defs.clone(),
            clobbers: rewritten_row.clobbers.clone(),
            provenance: consumer.provenance.clone(),
        },
        successor,
    };
    let proposed_function = proposed
        .functions
        .get(function_index)
        .ok_or(TerminatorPairError::ReplayMismatch)?;
    if proposed_function
        .blocks
        .get(block_index)
        .map(|block| &block.terminator)
        != Some(&expected)
    {
        return Err(TerminatorPairError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[block_index].terminator = block.terminator.clone();
    if restored != *source.selected_plan() {
        return Err(TerminatorPairError::ReplayMismatch);
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
        .ok_or(TerminatorPairError::IdentityOverflow)?;
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(TerminatorPairError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    let elements = function_scan
        .checked_add(1)
        .ok_or(TerminatorPairError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(TerminatorPairError::IdentityOverflow)?,
        )
        .ok_or(TerminatorPairError::IdentityOverflow)?;
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
        .ok_or(TerminatorPairError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(TerminatorPairError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .and_then(|total| total.checked_add(flag_universe.len()))
        .and_then(|total| total.checked_add(rewritten_row.implicit_uses.len()))
        .ok_or(TerminatorPairError::IdentityOverflow)?;
    let reach_scan = consumer
        .implicit_uses
        .len()
        .checked_mul(per_unit)
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or(TerminatorPairError::IdentityOverflow)?;
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
        .ok_or(TerminatorPairError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| TerminatorPairError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(TerminatorPairError::WorkBudgetExceeded);
    }
    Ok(ValidatedTerminatorPair {
        receipt: TerminatorPairReceipt {
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
) -> Result<(u64, u64), TerminatorPairError> {
    match producer.kind {
        SelectedInstructionKind::CompareI64 => {
            let [left, right] = producer.operands.as_slice() else {
                return Err(TerminatorPairError::UndecidedOperands);
            };
            let left = replay_use(left, 0)?;
            let right = replay_use(right, 1)?;
            if left == right {
                return Ok((0, 0));
            }
            Ok((
                condition_flow::materialized_bits(function, left)
                    .map_err(|_| TerminatorPairError::UndecidedOperands)?,
                condition_flow::materialized_bits(function, right)
                    .map_err(|_| TerminatorPairError::UndecidedOperands)?,
            ))
        }
        SelectedInstructionKind::CompareI64Immediate { immediate } => {
            let [operand] = producer.operands.as_slice() else {
                return Err(TerminatorPairError::UndecidedOperands);
            };
            let left = replay_use(operand, 0)?;
            let immediate = match immediate {
                semantic_vocabulary::IntegerValue::Signed(value) => {
                    u64::try_from(value).map_err(|_| TerminatorPairError::UndecidedOperands)?
                }
                semantic_vocabulary::IntegerValue::Unsigned(value) => {
                    u64::try_from(value).map_err(|_| TerminatorPairError::UndecidedOperands)?
                }
            };
            Ok((
                condition_flow::materialized_bits(function, left)
                    .map_err(|_| TerminatorPairError::UndecidedOperands)?,
                immediate,
            ))
        }
        SelectedInstructionKind::CompareI64Zero => {
            let [operand] = producer.operands.as_slice() else {
                return Err(TerminatorPairError::UndecidedOperands);
            };
            let left = replay_use(operand, 0)?;
            Ok((
                condition_flow::materialized_bits(function, left)
                    .map_err(|_| TerminatorPairError::UndecidedOperands)?,
                0,
            ))
        }
        _ => Err(TerminatorPairError::UnsupportedProducer),
    }
}

/// A plain `Use` operand at `position` under the replay's own operand
/// contract: the register the flag computation reads is the operand's own,
/// undecorated.
fn replay_use(
    operand: &selected_instructions::SelectedOperand,
    position: usize,
) -> Result<selected_instructions::VirtualRegisterId, TerminatorPairError> {
    if operand.operand != position as u16
        || operand.access != register_model::RegisterOperandAccess::Use
        || operand.fixed_view.is_some()
        || operand.tied_to.is_some()
        || operand.early_clobber
    {
        return Err(TerminatorPairError::UndecidedOperands);
    }
    Ok(operand.virtual_register)
}

/// The arm the replay selects for the decided state, matched on the
/// terminator variant and its carried kind together — never the declared
/// table.
fn decided_successor(
    terminator: &SelectedTerminator,
    consumer_kind: MachineSemanticKind,
    left: u64,
    right: u64,
) -> Option<&selected_instructions::SelectedSuccessor> {
    match (terminator, consumer_kind) {
        (
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            },
            MachineSemanticKind::ConditionalBranchNonZero,
        ) => Some(if left != right {
            when_nonzero
        } else {
            when_zero
        }),
        (
            SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            },
            MachineSemanticKind::ConditionalBranchU64LessThan,
        ) => Some(if left < right {
            when_less
        } else {
            when_not_less
        }),
        (
            SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            },
            MachineSemanticKind::ConditionalBranchI64LessThan,
        ) => Some(if (left as i64) < (right as i64) {
            when_less
        } else {
            when_not_less
        }),
        _ => None,
    }
}

/// The control-flow surface the replay restates on the bound catalog: the
/// consumer encodes a conditional relative branch on every alternative and
/// the rewritten form an unconditional one, with both declarations carrying
/// the control-flow barrier and otherwise effect-isolated — no declared or
/// encoded memory, stack, or trap traffic beyond the architectural fault a
/// relative-branch encoding can raise, and no call or cleanup surface.
fn decided_branch_declarations(
    consumer: &MachineEffectDeclaration,
    rewritten: &MachineEffectDeclaration,
) -> bool {
    isolated_declaration(consumer)
        && isolated_declaration(rewritten)
        && consumer.alternatives.iter().all(|alternative| {
            branch_alternative(
                alternative,
                MachineEncodedControlEffect::ConditionalRelativeBranchV1,
            )
        })
        && rewritten.alternatives.iter().all(|alternative| {
            branch_alternative(
                alternative,
                MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
            )
        })
}

/// The declared surface a decided-branch form must carry: no memory access,
/// the operation itself never traps, no call or cleanup — and the
/// control-flow barrier both branch semantics publish.
fn isolated_declaration(declaration: &MachineEffectDeclaration) -> bool {
    declaration.memory == MachineMemoryEffect::NoneV1
        && declaration.trap == MachineTrapBehavior::NeverV1
        && declaration.barrier == MachineBarrier::ControlFlow
        && declaration.call == MachineCallEffect::NoneV1
        && declaration.cleanup == MachineCleanupEffect::NoneV1
}

/// The encoded surface a decided-branch alternative must carry: no memory
/// access, unchanged stack, the architectural-fault surface a
/// relative-branch encoding can raise, and the expected control effect.
fn branch_alternative(
    alternative: &MachineAlternative,
    control: MachineEncodedControlEffect,
) -> bool {
    let encoded = &alternative.encoded;
    encoded.memory == MachineEncodedMemoryEffect::NoneV1
        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
        && encoded.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        && encoded.control == control
}

/// The single catalog declaration for `semantic` bound to `constraint`, or
/// none when the catalog does not declare exactly one such form.
fn effect_declaration<'catalog>(
    catalog: &'catalog ValidatedMachineEffectCatalog,
    semantic: selected_instructions::MachineSemanticKind,
    constraint: register_model::RegisterConstraintKey,
) -> Option<&'catalog selected_instructions::MachineEffectDeclaration> {
    let mut matches = catalog.catalog().declarations.iter().filter(|declaration| {
        declaration.semantic == semantic && declaration.constraint == constraint
    });
    let declaration = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(declaration)
}
