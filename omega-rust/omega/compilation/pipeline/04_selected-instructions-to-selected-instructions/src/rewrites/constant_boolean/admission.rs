//! Descriptor-driven admission for constant condition materialization:
//! locate the named `MaterializeBoolean*`, confirm the declared consumer's
//! emitted `[def result]` shape, partition its implicit uses by the flag
//! universe — flag units resolve to the one producer through the shared
//! reaching walk, non-flag units must lie in the materialize row's empty
//! implicit surface — then complete the pair from the
//! [`ConditionMaterializationRule`](super::pair::ConditionMaterializationRule)
//! table: the reached producer's kind selects the rule, whose operand
//! grammar decides the predicate, whose unit-flow axis states the retired
//! and preserved unit roles, and whose effect axis is checked against the
//! bound machine-effect catalog.
use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    MachineEffectDeclaration, MachineSemanticKind, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedOperand,
    ValidatedMachineEffectCatalog, VirtualRegisterId,
};
use semantic_vocabulary::IntegerValue;

use super::ConstantBooleanError;
use super::pair::{
    CONDITION_MATERIALIZATION_RULES, condition_materialization_for, declared_consumer,
    declared_producers,
};
use crate::ValidatedSelectedAnalysis;
use crate::analyses::machine_effects::machine_semantic_kind;
use crate::rewrites::condition_state::{
    ConditionStateError, adjacency, backward_cone, entry_index, instruction_at, reaching_event,
    resolved_operands,
};

impl From<ConditionStateError> for ConstantBooleanError {
    fn from(error: ConditionStateError) -> Self {
        match error {
            ConditionStateError::Use => ConstantBooleanError::UnsupportedUse,
            ConditionStateError::Producer => ConstantBooleanError::UnsupportedProducer,
            ConditionStateError::Literal => ConstantBooleanError::UnsupportedLiteral,
        }
    }
}

pub(super) struct Admission<'source> {
    pub block_index: usize,
    pub materialization_index: usize,
    pub materialization_id: SelectedInstructionId,
    pub provenance: SelectedInstructionProvenance,
    /// The consumer's `Def` operand record, carried verbatim onto the
    /// rewritten materialization: the decided value lands in the same
    /// register home.
    pub operand: SelectedOperand,
    /// The folded predicate outcome the rewritten `MaterializeI64` carries.
    pub value: IntegerValue,
    /// The materialize row the rewritten instruction is built from.
    pub row: &'source RegisterInstructionConstraint,
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

/// Admit `materialization` — a body instruction in `function_index`'s
/// function — as a declared decided-materialization pair.
///
/// The order of gates fixes the error vocabulary: the catalog binding and
/// the record shape (declared consumer kind, operands, implicit uses)
/// refuse first, then the constraint rows, then the unit flow —
/// flag-universe partition, reaching resolution, and the producer's
/// published definitions — then the producer kind the table must pair with
/// this consumer and the rule's unit-flow axis, then the rule's operand
/// grammar, then the catalog declarations' encoded effect surface, and
/// last the bounded-work charge.
pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, ConstantBooleanError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ConstantBooleanError::SourceMismatch);
    }
    // The bound catalog must describe this plan's target and this
    // environment's constraint catalog and selected-key inventory: a
    // foreign catalog's declarations cannot attest this program's encoded
    // effect surfaces even where constraint keys coincide numerically.
    let catalog = effect_catalog.catalog();
    if catalog.target != plan.target
        || catalog.register_constraints != environment.constraints().identity()
        || catalog.selected_keys != environment.selected_keys()
    {
        return Err(ConstantBooleanError::EffectSurfaceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(ConstantBooleanError::SourceMismatch)?;
    let (block_index, materialization_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == materialization)
                .map(|materialization_index| (block_index, materialization_index))
        })
        .ok_or(ConstantBooleanError::SourceMismatch)?;
    let materialization_instruction =
        &function.blocks[block_index].instructions[materialization_index];
    // The materialization must be a declared consumer in its emitted
    // `[def result]` flag-reader shape: the table names the kind, the kind
    // must arrive as the emitted single `Def` operand at position 0 with
    // no unit binding, and a use roster holding no unit has no condition
    // to decide.
    let consumer_kind = machine_semantic_kind(materialization_instruction.kind);
    if !declared_consumer(materialization_instruction.kind)
        || materialization_instruction.operands.len() != 1
        || materialization_instruction.implicit_uses.is_empty()
        || !materialization_instruction.implicit_defs.is_empty()
        || !materialization_instruction.clobbers.is_empty()
    {
        return Err(ConstantBooleanError::UnsupportedInstruction);
    }
    let operand = &materialization_instruction.operands[0];
    if operand.operand != 0
        || operand.access != RegisterOperandAccess::Def
        || operand.fixed_view.is_some()
        || operand.tied_to.is_some()
        || operand.early_clobber
    {
        return Err(ConstantBooleanError::UnsupportedInstruction);
    }
    let result: VirtualRegisterId = operand.virtual_register;
    let result_register = function
        .virtual_registers
        .iter()
        .find(|entry| entry.id == result)
        .ok_or(ConstantBooleanError::UnsupportedUse)?;
    let keys = environment.selected_keys();
    // The declared consumer's bound row must carry the same single-`Def`
    // shape the record does.
    let consumer_row = environment
        .constraint(materialization_instruction.constraint)
        .ok_or(ConstantBooleanError::ConstraintMismatch)?;
    if !(consumer_row.operands.len() == 1
        && consumer_row.operands[0].operand == 0
        && consumer_row.operands[0].access == RegisterOperandAccess::Def)
    {
        return Err(ConstantBooleanError::ConstraintMismatch);
    }
    // Every declared pair rewrites to the same `MaterializeI64` form, so
    // its semantic names the row the rewrite binds: a single `[def]` at
    // the boolean result's class carrying no unit traffic — the fold
    // drops the flag uses, and any implicit definition or clobber the row
    // added would publish unit state the source instruction never had.
    let rewritten_key = keys
        .for_semantic(MachineSemanticKind::MaterializeI64)
        .ok_or(ConstantBooleanError::ConstraintMismatch)?;
    let rewritten_row = environment
        .constraint(rewritten_key)
        .ok_or(ConstantBooleanError::ConstraintMismatch)?;
    if rewritten_row.operands.len() != 1
        || rewritten_row.operands[0].operand != 0
        || rewritten_row.operands[0].access != RegisterOperandAccess::Def
        || rewritten_row.operands[0].class != result_register.class
        || rewritten_row.operands[0].fixed_view.is_some()
        || rewritten_row.operands[0].tied_to.is_some()
        || rewritten_row.operands[0].early_clobber
        || !rewritten_row.implicit_uses.is_empty()
        || !rewritten_row.implicit_defs.is_empty()
        || !rewritten_row.clobbers.is_empty()
    {
        return Err(ConstantBooleanError::ConstraintMismatch);
    }
    // The flag universe partitions the materialization's implicit uses:
    // the units the declared producers' rows publish are the condition
    // state the fold decides, and every other observed unit must lie in
    // the materialize row's own implicit surface — which reads nothing.
    let mut flag_universe = BTreeSet::new();
    for producer_kind in declared_producers() {
        let key = keys
            .for_semantic(producer_kind)
            .ok_or(ConstantBooleanError::ConstraintMismatch)?;
        flag_universe.extend(
            environment
                .constraint(key)
                .ok_or(ConstantBooleanError::ConstraintMismatch)?
                .implicit_defs
                .iter()
                .copied(),
        );
    }
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition state
    // bounds every path, and mark the cone of blocks that can reach the
    // materialization's block — only their entry sets can feed the result.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(ConstantBooleanError::SourceMismatch)?;
    let cone = backward_cone(&predecessors, block_index);
    // Every flag unit the materialization reads must reach from the same
    // producer: on every execution path to the materialization's own body
    // position, the last event touching each used flag unit is that one
    // instruction, and the unit is among its published definitions — a
    // clobber there would leave the observed value unknown. The producer
    // stays published for readers the rewrite leaves behind.
    let mut flag_uses = Vec::new();
    let mut plain_uses = Vec::new();
    let mut compare_site = None;
    for unit in &materialization_instruction.implicit_uses {
        if !flag_universe.contains(unit) {
            plain_uses.push(*unit);
            continue;
        }
        flag_uses.push(*unit);
        let event = reaching_event(
            function,
            entry,
            &successors,
            &cone,
            block_index,
            materialization_index,
            *unit,
        )?;
        if compare_site.is_some_and(|site| site != event) {
            return Err(ConstantBooleanError::UnsupportedUse);
        }
        compare_site = Some(event);
    }
    if flag_uses.is_empty() {
        return Err(ConstantBooleanError::UnsupportedUse);
    }
    let compare_site = compare_site.ok_or(ConstantBooleanError::UnsupportedUse)?;
    let compare_instruction = instruction_at(function, compare_site);
    if flag_uses
        .iter()
        .any(|unit| !compare_instruction.implicit_defs.contains(unit))
    {
        return Err(ConstantBooleanError::UnsupportedUse);
    }
    // The reached producer completes the pair: the table must declare a
    // rule for this (producer, consumer) combination. Its unit-flow axis
    // then states the concrete roles — the flag uses retire, every
    // non-flag use must stay read on the rewritten row, and the rewritten
    // row republishes the consumer's implicit definitions and clobbers —
    // and its operand grammar decides the predicate.
    let rule = condition_materialization_for(
        machine_semantic_kind(compare_instruction.kind),
        consumer_kind,
    )
    .ok_or(ConstantBooleanError::UnsupportedProducer)?;
    if !rule.matches_producer(compare_instruction.kind) {
        return Err(ConstantBooleanError::UnsupportedProducer);
    }
    debug_assert!(
        CONDITION_MATERIALIZATION_RULES
            .iter()
            .all(|declared| declared.rewritten() == rule.rewritten()),
        "every declared materialization pair rewrites to MaterializeI64"
    );
    if !rule.admits_unit_flow(
        &flag_uses,
        &plain_uses,
        materialization_instruction,
        rewritten_row,
    ) {
        return Err(ConstantBooleanError::UnsupportedUse);
    }
    let (left, right) =
        resolved_operands(function, compare_instruction, rule.operand_resolution())?;
    let value = rule.decided_value(left, right);
    // The declared effect relationship, checked against the bound catalog:
    // the consumer's condition read and the rewritten materialization are
    // both effect-isolated fall-through rows, the consumer's encoded
    // implicit uses stay inside the flag universe, and the rewritten
    // alternatives carry no implicit unit traffic at all.
    let consumer_declaration = effect_declaration(
        effect_catalog,
        consumer_kind,
        materialization_instruction.constraint,
    )
    .ok_or(ConstantBooleanError::EffectSurfaceMismatch)?;
    let rewritten_declaration =
        effect_declaration(effect_catalog, rule.rewritten(), rewritten_row.key)
            .ok_or(ConstantBooleanError::EffectSurfaceMismatch)?;
    if !rule.admits_declarations(&flag_universe, consumer_declaration, rewritten_declaration) {
        return Err(ConstantBooleanError::EffectSurfaceMismatch);
    }
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    // Producer scans walk the whole function once per compared register —
    // two at most. The flag walk's shared setup resolves every edge's
    // target index, fills the predecessor lists, and marks the backward
    // cone. Each used unit then pays its partition checks — flag-universe
    // and materialize-surface membership and the producer-definitions
    // lookup — plus, for a flag unit, the block-prefix scan and — on an
    // in-block miss — every block's stream for its last event, after
    // which entry-set propagation requeues a block only while its set
    // grows: a set holds at most one element per event site plus the
    // unknown marker, so pops stay under `blocks × (elements + 1)` and
    // each pop visits its out-edges — no more than the widest
    // terminator's — at a bounded union cost. Charging every use for the
    // full flag walk keeps the bound independent of the partition split.
    let elements = function_scan
        .checked_add(1)
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(ConstantBooleanError::IdentityOverflow)?,
        )
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let widest_out = successors
        .iter()
        .map(|targets| targets.len())
        .max()
        .unwrap_or(0);
    let materialize_surface = rewritten_row
        .implicit_uses
        .iter()
        .chain(rewritten_row.implicit_defs.iter())
        .chain(rewritten_row.clobbers.iter())
        .collect::<BTreeSet<_>>()
        .len();
    let per_unit = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .and_then(|total| {
            total.checked_add(
                flag_universe
                    .len()
                    .checked_add(materialize_surface)?
                    .checked_add(compare_instruction.implicit_defs.len())?,
            )
        })
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(ConstantBooleanError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .and_then(|total| total.checked_add(flag_universe.len()))
        .and_then(|total| total.checked_add(materialize_surface))
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let reach_scan = materialization_instruction
        .implicit_uses
        .len()
        .checked_mul(per_unit)
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let steps = plan
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
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| ConstantBooleanError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(ConstantBooleanError::WorkBudgetExceeded);
    }
    Ok(Admission {
        block_index,
        materialization_index,
        materialization_id: materialization,
        provenance: materialization_instruction.provenance.clone(),
        operand: *operand,
        value,
        row: rewritten_row,
    })
}

/// The one-instruction proposal shape shared with replay: the target's own
/// materialize row — single-`Def` by admission — supplies the implicit
/// surface while the consumer's instruction identity, provenance, and `Def`
/// operand record stay: the decided value lands in the same register home.
pub(super) fn rewritten(admitted: &Admission<'_>) -> SelectedInstruction {
    SelectedInstruction {
        id: admitted.materialization_id,
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
