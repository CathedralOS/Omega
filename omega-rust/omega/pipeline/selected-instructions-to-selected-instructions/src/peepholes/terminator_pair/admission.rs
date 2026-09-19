//! Producer admission of a decided-branch terminator pair.
//!
//! The descriptor table drives the admission: the terminator's semantic
//! kind selects the declared consumers, the flag-reaching producer's kind
//! completes the pair, and the rule's axes — operand resolution, unit flow,
//! control flow — state every gate the concrete records must satisfy. What
//! no catalog declaration can attest is checked on the function itself:
//! the flag units' reaching events, the operand registers' unique
//! materialization producers, and the whole-function work all charge into
//! the caller's budget. The replay in `validate` re-derives all of it
//! without the table.

use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    MachineEffectDeclaration, MachineSemanticKind, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedSuccessor, SelectedTerminator,
    ValidatedMachineEffectCatalog,
};

use super::TerminatorPairError;
use super::pair::{
    TERMINATOR_PAIR_RULES, TerminatorPairRule, declared_consumer, declared_producers,
    terminator_pair_for,
};
use crate::ValidatedSelectedAnalysis;
use crate::machine_semantic_kind;
use crate::peepholes::condition_flow;

/// One admitted terminator pair: everything `rewrite` needs to rebuild the
/// terminator and everything `validate` needs to re-check it.
pub(super) struct AdmittedPair<'environment> {
    pub(super) block_index: usize,
    pub(super) terminator_id: SelectedInstructionId,
    pub(super) provenance: SelectedInstructionProvenance,
    pub(super) successor: SelectedSuccessor,
    pub(super) row: &'environment RegisterInstructionConstraint,
}

/// Admit `terminator` — the instruction the block's terminator carries — as
/// a decided-branch pair under the descriptor table in `function_index`'s
/// function.
///
/// The order of gates matters for the error vocabulary: the record shape
/// (kind, variant, operands, implicit uses) refuses first, then the unit
/// flow — flag-universe partition, reaching resolution, and preserved
/// surface — then the producer kind and its operand resolution, then the
/// catalog declarations' encoded control relationship, and last the
/// bounded-work charge.
pub(super) fn admit<'environment>(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    terminator: SelectedInstructionId,
    environment: &'environment ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
    budget: OptimizationWorkBudget,
) -> Result<AdmittedPair<'environment>, TerminatorPairError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(TerminatorPairError::SourceMismatch);
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
    if !declared_consumer(consumer.kind) || !consumer.operands.is_empty() {
        return Err(TerminatorPairError::UnsupportedTerminator);
    }
    let consumer_kind = machine_semantic_kind(consumer.kind);
    let candidate_rules: Vec<&TerminatorPairRule> = TERMINATOR_PAIR_RULES
        .iter()
        .filter(|rule| rule.consumer() == consumer_kind)
        .collect();
    if candidate_rules
        .iter()
        .all(|rule| !rule.matches_terminator(&block.terminator))
    {
        return Err(TerminatorPairError::UnsupportedTerminator);
    }
    if consumer.implicit_uses.is_empty() {
        return Err(TerminatorPairError::UnsupportedUse);
    }
    let keys = environment.selected_keys();
    let consumer_row = environment
        .constraint(consumer.constraint)
        .ok_or(TerminatorPairError::ConstraintMismatch)?;
    if !consumer_row.operands.is_empty() {
        return Err(TerminatorPairError::ConstraintMismatch);
    }
    // Every candidate rule declares the same rewritten form, so the first
    // candidate's declaration names the row the rewrite binds.
    let rewritten_key = keys
        .for_semantic(candidate_rules[0].rewritten())
        .ok_or(TerminatorPairError::ConstraintMismatch)?;
    let rewritten_row = environment
        .constraint(rewritten_key)
        .ok_or(TerminatorPairError::ConstraintMismatch)?;
    if !rewritten_row.operands.is_empty() {
        return Err(TerminatorPairError::ConstraintMismatch);
    }
    // The flag universe — the implicit units the environment's
    // condition-state producer rows may define — partitions the consumer's
    // uses into the flag channel the rewrite retires and the control-unit
    // and other uses the rewritten row must preserve.
    let mut flag_universe = BTreeSet::new();
    for producer_kind in declared_producers() {
        let key = keys
            .for_semantic(producer_kind)
            .ok_or(TerminatorPairError::ConstraintMismatch)?;
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
        // A branch observing no condition state has no predicate to decide.
        return Err(TerminatorPairError::UnsupportedUse);
    }
    // Resolve every flag use's reaching event; all must agree on the one
    // producer instruction, and that producer must publish every used unit.
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
    let producer_kind = machine_semantic_kind(producer.kind);
    let rule = terminator_pair_for(producer_kind, consumer_kind)
        .ok_or(TerminatorPairError::UnsupportedProducer)?;
    if !rule.matches_producer(producer.kind) {
        return Err(TerminatorPairError::UnsupportedProducer);
    }
    // The declared unit-flow relationship, checked against the concrete
    // records: the flag uses retire, every non-flag use must stay read on
    // the rewritten row, and the rewritten row republishes the consumer's
    // implicit definitions and clobbers verbatim.
    if !rule.admits_unit_flow(&flag_uses, &plain_uses, consumer, rewritten_row) {
        return Err(TerminatorPairError::UnsupportedUse);
    }
    // The declared operand grammar decides the predicate on the producer's
    // constant operands.
    let (left, right) =
        condition_flow::resolved_operands(function, producer, rule.operand_resolution())
            .map_err(|_| TerminatorPairError::UndecidedOperands)?;
    let successor = rule
        .decided_successor(&block.terminator, left, right)
        .ok_or(TerminatorPairError::UnsupportedTerminator)?
        .clone();
    // The declared control-flow relationship, checked against the bound
    // effect catalog: the consumer encodes a conditional relative branch,
    // the rewritten row an unconditional one, and both are otherwise
    // effect-isolated.
    let consumer_declaration =
        effect_declaration(effect_catalog, consumer_kind, consumer.constraint)
            .ok_or(TerminatorPairError::EffectSurfaceMismatch)?;
    let rewritten_declaration =
        effect_declaration(effect_catalog, rule.rewritten(), rewritten_row.key)
            .ok_or(TerminatorPairError::EffectSurfaceMismatch)?;
    if !rule.admits_declarations(consumer_declaration, rewritten_declaration) {
        return Err(TerminatorPairError::EffectSurfaceMismatch);
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
    Ok(AdmittedPair {
        block_index,
        terminator_id: terminator,
        provenance: consumer.provenance.clone(),
        successor,
        row: rewritten_row,
    })
}

/// The one-terminator proposal shape shared with replay: the target's own
/// jump row — zero-operand by admission — supplies the implicit surface
/// while the terminator's instruction identity and provenance stay, and the
/// decided successor carries its record verbatim.
pub(super) fn rewritten(admitted: &AdmittedPair<'_>) -> SelectedTerminator {
    SelectedTerminator::Jump {
        instruction: SelectedInstruction {
            id: admitted.terminator_id,
            kind: SelectedInstructionKind::Jump,
            constraint: admitted.row.key,
            operands: Vec::new(),
            implicit_uses: admitted.row.implicit_uses.clone(),
            implicit_defs: admitted.row.implicit_defs.clone(),
            clobbers: admitted.row.clobbers.clone(),
            provenance: admitted.provenance.clone(),
        },
        successor: admitted.successor.clone(),
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
