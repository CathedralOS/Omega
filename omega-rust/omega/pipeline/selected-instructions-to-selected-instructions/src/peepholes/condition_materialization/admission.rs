//! Producer admission of a decided condition materialization pair.
//!
//! The descriptor table drives the admission: the consumer instruction's
//! semantic kind selects the declared consumers, the flag-reaching
//! producer's kind completes the pair, and the rule's axes — operand
//! resolution, unit flow, effect surface — state every gate the concrete
//! records must satisfy. What no catalog declaration can attest is checked
//! on the function itself: the flag units' reaching events, the operand
//! registers' unique materialization producers, and the whole-function work
//! all charge into the caller's budget. The replay in `replay` re-derives
//! all of it without the table.

use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    MachineEffectDeclaration, MachineSemanticKind, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedOperand,
    ValidatedMachineEffectCatalog,
};
use semantic_vocabulary::IntegerValue;

use super::ConditionMaterializationError;
use super::pair::{
    CONDITION_MATERIALIZATION_RULES, condition_materialization_for, declared_consumer,
    declared_producers,
};
use crate::ValidatedSelectedAnalysis;
use crate::machine_semantic_kind;
use crate::peepholes::condition_flow;

/// One admitted condition-materialization pair: everything `rewrite` needs
/// to rebuild the consumer instruction and everything `replay` needs to
/// re-check it.
pub(super) struct AdmittedPair<'environment> {
    pub(super) block_index: usize,
    /// The consumer's position in its block's instruction stream.
    pub(super) position: usize,
    pub(super) consumer_id: SelectedInstructionId,
    pub(super) provenance: SelectedInstructionProvenance,
    /// The consumer's `Def` operand record, carried verbatim onto the
    /// rewritten materialization: the decided value lands in the same
    /// register home.
    pub(super) operand: SelectedOperand,
    /// The decided zero or one the rewritten `MaterializeI64` publishes.
    pub(super) value: IntegerValue,
    pub(super) row: &'environment RegisterInstructionConstraint,
}

/// Admit `consumer` — a body instruction in `function_index`'s function —
/// as a decided-materialization pair under the descriptor table.
///
/// The order of gates matters for the error vocabulary: the record shape
/// (kind, operands, implicit uses) refuses first, then the unit flow —
/// flag-universe partition, reaching resolution, and the republish
/// contract — then the producer kind and its operand resolution, then the
/// catalog declarations' encoded surface, and last the bounded-work charge.
pub(super) fn admit<'environment>(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    consumer_id: SelectedInstructionId,
    environment: &'environment ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
    budget: OptimizationWorkBudget,
) -> Result<AdmittedPair<'environment>, ConditionMaterializationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ConditionMaterializationError::SourceMismatch);
    }
    // The bound catalog must describe this plan's target and this
    // environment's constraint catalog and selected-key inventory: a
    // foreign catalog's declarations cannot attest this program's encoded
    // surfaces even where constraint keys coincide numerically.
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
    // The consumer is a body instruction: a flag reader's observation is a
    // mid-block position. A terminator's carried instruction is not a
    // materialization form the family declares.
    let (block_index, position, consumer) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .enumerate()
                .find(|(_, instruction)| instruction.id == consumer_id)
                .map(|(position, instruction)| (block_index, position, instruction))
        })
        .ok_or(ConditionMaterializationError::UnsupportedConsumer)?;
    if !declared_consumer(consumer.kind) {
        return Err(ConditionMaterializationError::UnsupportedConsumer);
    }
    // The emitted materialization form is a single `Def` operand at
    // position 0 carrying no unit binding — the result channel the
    // rewritten record inherits.
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
    // Every declared rule rewrites to the same `MaterializeI64` row: the
    // row the rewrite binds, and the row the unit-surface contract holds
    // against.
    let rewritten_key = keys
        .for_semantic(MachineSemanticKind::MaterializeI64)
        .ok_or(ConditionMaterializationError::ConstraintMismatch)?;
    let rewritten_row = environment
        .constraint(rewritten_key)
        .ok_or(ConditionMaterializationError::ConstraintMismatch)?;
    if !(rewritten_row.operands.len() == 1
        && rewritten_row.operands[0].operand == 0
        && rewritten_row.operands[0].access == RegisterOperandAccess::Def)
    {
        return Err(ConditionMaterializationError::ConstraintMismatch);
    }
    // The flag universe — the implicit units the environment's
    // condition-state producer rows may define — partitions the consumer's
    // uses into the flag channel the rewrite retires and the uses the
    // rewritten row would have to preserve; `MaterializeI64` reads none.
    let mut flag_universe = BTreeSet::new();
    for producer_kind in declared_producers() {
        let key = keys
            .for_semantic(producer_kind)
            .ok_or(ConditionMaterializationError::ConstraintMismatch)?;
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
        // A materialization observing no condition state has no predicate
        // to decide.
        return Err(ConditionMaterializationError::UnsupportedUse);
    }
    // Resolve every flag use's reaching event at the consumer's own body
    // position; all must agree on the one producer instruction, and that
    // producer must publish every used unit.
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
    let producer_kind = machine_semantic_kind(producer.kind);
    let consumer_kind = machine_semantic_kind(consumer.kind);
    let rule = condition_materialization_for(producer_kind, consumer_kind)
        .ok_or(ConditionMaterializationError::UnsupportedProducer)?;
    if !rule.matches_producer(producer.kind) {
        return Err(ConditionMaterializationError::UnsupportedProducer);
    }
    debug_assert!(
        CONDITION_MATERIALIZATION_RULES
            .iter()
            .all(|declared| declared.rewritten() == rule.rewritten()),
        "every declared materialization pair rewrites to MaterializeI64"
    );
    // The declared unit-flow relationship, checked against the concrete
    // records: the flag uses retire, every non-flag use must stay read on
    // the rewritten row, and the rewritten row republishes the consumer's
    // implicit definitions and clobbers verbatim.
    if !rule.admits_unit_flow(&flag_uses, &plain_uses, consumer, rewritten_row) {
        return Err(ConditionMaterializationError::UnsupportedUse);
    }
    // The declared operand grammar decides the predicate on the producer's
    // constant operands; the consumer kind names which predicate the
    // materialization publishes.
    let (left, right) =
        condition_flow::resolved_operands(function, producer, rule.operand_resolution())
            .map_err(|_| ConditionMaterializationError::UndecidedOperands)?;
    let value = rule.decided_value(left, right);
    // The declared effect surface, checked against the bound effect
    // catalog: the consumer's condition read and the rewritten
    // materialization are both effect-isolated fall-through rows, the
    // consumer's encoded implicit uses stay inside the flag universe, and
    // the rewritten alternatives carry no implicit traffic at all.
    let consumer_declaration =
        effect_declaration(effect_catalog, consumer_kind, consumer.constraint)
            .ok_or(ConditionMaterializationError::EffectSurfaceMismatch)?;
    let rewritten_declaration =
        effect_declaration(effect_catalog, rule.rewritten(), rewritten_row.key)
            .ok_or(ConditionMaterializationError::EffectSurfaceMismatch)?;
    if !rule.admits_declarations(&flag_universe, consumer_declaration, rewritten_declaration) {
        return Err(ConditionMaterializationError::EffectSurfaceMismatch);
    }
    // Charge the walk: each flag use pays the block-prefix scan and — on an
    // in-block miss — the per-block last-event scan plus fixpoint pops, each
    // bounded by the widest terminator's out-edges; the operand resolution
    // walks the function once per compared register — two at most.
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
    Ok(AdmittedPair {
        block_index,
        position,
        consumer_id,
        provenance: consumer.provenance.clone(),
        operand: *operand,
        value,
        row: rewritten_row,
    })
}

/// The one-instruction proposal shape shared with replay: the target's own
/// materialize row — single-`Def` by admission — supplies the implicit
/// surface while the consumer's instruction identity, provenance, and `Def`
/// operand record stay: the decided value lands in the same register home.
pub(super) fn rewritten(admitted: &AdmittedPair<'_>) -> SelectedInstruction {
    SelectedInstruction {
        id: admitted.consumer_id,
        kind: SelectedInstructionKind::MaterializeI64 {
            value: admitted.value,
        },
        constraint: admitted.row.key,
        operands: vec![admitted.operand],
        implicit_uses: admitted.row.implicit_uses.clone(),
        implicit_defs: admitted.row.implicit_defs.clone(),
        clobbers: admitted.row.clobbers.clone(),
        provenance: admitted.provenance.clone(),
    }
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
