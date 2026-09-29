//! Independent validation of constant condition materialization.
//!
//! The validator never calls [`super::admission`] and never reads the
//! `pair` descriptor table the producer consults: it re-derives the
//! fold's legality from the source records — the named instruction's
//! `MaterializeBoolean*` kind and emitted `[def result]` shape, the flag
//! units it reads partitioned against the environment's own compare rows
//! and resolving through the shared condition-state walk to one compare's
//! published definitions, that compare's compile-time constant operands,
//! and the target's own materialize row the rewritten instruction is
//! built from — then rebuilds the instruction the contract demands and
//! requires the proposal to carry it at the materialization's position.
//! The catalog effect surface is restated here too: the consumer's row
//! must encode an effect-isolated fall-through whose implicit uses stay
//! inside the flag universe, and the materialize row an isolated
//! fall-through with no implicit unit traffic at all — read off the bound
//! catalog directly. Reinserting the source boolean must restore the
//! complete source by content: every other instruction, register, call,
//! and function included. A producer admission error therefore fails
//! validation even when the proposal is exactly what that producer
//! emitted.
use std::collections::BTreeSet;
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess, RegisterUnitId};
use selected_instructions::{
    MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSemanticKind, MachineTrapBehavior, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedInstructionProvenance, SelectedOperand, ValidatedMachineEffectCatalog,
};
use semantic_vocabulary::IntegerValue;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{ConstantBooleanError, ConstantBooleanReceipt, ValidatedConstantBoolean};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::condition_state::{
    ConditionStateError, adjacency, backward_cone, constant_operands, entry_index, instruction_at,
    reaching_event,
};

/// The validator's own reconstruction of the fold the contract permits:
/// the admitted materialization's coordinates and emitted provenance, the
/// `Def` operand record carrying verbatim onto the rewritten
/// materialization, the folded predicate outcome, the target's
/// materialize row, the resolved-index successor lists the shared walk
/// ran over, and the surface sizes for the measured-step contract. It
/// shares no state with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    materialization_index: usize,
    materialization_id: SelectedInstructionId,
    provenance: SelectedInstructionProvenance,
    operand: SelectedOperand,
    value: IntegerValue,
    row: &'source RegisterInstructionConstraint,
    successors: Vec<Vec<usize>>,
    implicit_uses: usize,
    flag_universe: usize,
    materialize_surface: usize,
    producer_defs: usize,
}

/// The constant a flag-reading `MaterializeBoolean*` materializes when
/// the compare observed `(left, right)` — the predicates the selected
/// catalog defines over `left - right`, evaluated at compile time. The
/// validator re-decides the table from the instruction kind alone: a
/// wrong predicate inside shared producer code would decide both sides
/// identically, so the fold's decision lives here, not in `admission`.
fn predicate_outcome(kind: SelectedInstructionKind, left: u64, right: u64) -> Option<bool> {
    match kind {
        SelectedInstructionKind::MaterializeBooleanEqual => Some(left == right),
        SelectedInstructionKind::MaterializeBooleanU64LessThan => Some(left < right),
        SelectedInstructionKind::MaterializeBooleanI64LessThan => {
            Some((left as i64) < (right as i64))
        }
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => Some(left <= right),
        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            Some((left as i64) <= (right as i64))
        }
        _ => None,
    }
}

/// The materialization semantic the validator reads off the instruction
/// kind alone — never the producer's descriptor table.
fn materialization_semantic(kind: SelectedInstructionKind) -> Option<MachineSemanticKind> {
    match kind {
        SelectedInstructionKind::MaterializeBooleanEqual => {
            Some(MachineSemanticKind::MaterializeBooleanEqual)
        }
        SelectedInstructionKind::MaterializeBooleanU64LessThan => {
            Some(MachineSemanticKind::MaterializeBooleanU64LessThan)
        }
        SelectedInstructionKind::MaterializeBooleanI64LessThan => {
            Some(MachineSemanticKind::MaterializeBooleanI64LessThan)
        }
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => {
            Some(MachineSemanticKind::MaterializeBooleanU64LessOrEqual)
        }
        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            Some(MachineSemanticKind::MaterializeBooleanI64LessOrEqual)
        }
        _ => None,
    }
}

/// The `Def` operand record of an emitted `[def result]` flag reader, or
/// none: the validator re-decides the emitted shape — a consumer kind the
/// materialization grammar names, one plain definition, no implicit
/// definitions or clobbers, and a nonempty flag-use roster, since a
/// boolean observing no condition state has no constant to evaluate.
fn flag_reader(instruction: &SelectedInstruction) -> Option<SelectedOperand> {
    if materialization_semantic(instruction.kind).is_none()
        || instruction.operands.len() != 1
        || instruction.implicit_uses.is_empty()
        || !instruction.implicit_defs.is_empty()
        || !instruction.clobbers.is_empty()
    {
        return None;
    }
    let result = &instruction.operands[0];
    if result.operand != 0
        || result.access != RegisterOperandAccess::Def
        || result.fixed_view.is_some()
        || result.tied_to.is_some()
        || result.early_clobber
    {
        return None;
    }
    Some(*result)
}

/// The single catalog declaration for `semantic` bound to `constraint`, or
/// none when the catalog does not declare exactly one such form — the
/// validator's own lookup.
fn declaration_for(
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

/// The effect surface a decided-materialization declaration must carry,
/// restated from the catalog row alone: no declared memory, trap,
/// barrier, call, or cleanup effect — the plain fall-through isolation
/// both the flag read and the literal materialization publish.
fn isolated_declaration(declaration: &MachineEffectDeclaration) -> bool {
    declaration.memory == MachineMemoryEffect::NoneV1
        && declaration.trap == MachineTrapBehavior::NeverV1
        && declaration.barrier == MachineBarrier::None
        && declaration.call == MachineCallEffect::NoneV1
        && declaration.cleanup == MachineCleanupEffect::NoneV1
}

/// The encoded surface a condition-reading alternative must carry,
/// restated from the catalog row alone: no memory access, unchanged
/// stack, no trap surface, a plain fall-through — and implicit uses only
/// inside `flag_universe`, with no implicit definitions or clobbers: the
/// condition-state read is the whole input.
fn condition_reading_alternative(
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

/// The encoded surface a literal-materialization alternative must carry,
/// restated from the catalog row alone: no memory access, unchanged
/// stack, no trap surface, a plain fall-through — and no implicit unit
/// traffic at all.
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

/// The family's own mapping of a shared condition-state rejection onto
/// its error vocabulary. The producer's `From` implementation lives in
/// `admission`; the validator maps each site itself so a changed producer
/// mapping cannot relabel its refusal.
fn condition_error(error: ConditionStateError) -> ConstantBooleanError {
    match error {
        ConditionStateError::Use => ConstantBooleanError::UnsupportedUse,
        ConditionStateError::Producer => ConstantBooleanError::UnsupportedProducer,
        ConditionStateError::Literal => ConstantBooleanError::UnsupportedLiteral,
    }
}

/// Reconstruct the legality of folding `materialization` from first
/// principles: locate the instruction, verify the emitted flag-reader
/// shape and the result register, partition the used units against the
/// environment's own compare rows, resolve each flag unit's reaching
/// event on the module's shared condition-state walk and require the one
/// compare site whose published definitions cover them all, restate the
/// unit-flow and effect-surface relationships the rewrite asserts, prove
/// the compare's operands compile-time constant, and check the target's
/// materialize row against the contract the rewrite assumes. Nothing in
/// this audit reads the producer's admission decision or its descriptor
/// table.
#[allow(clippy::too_many_lines)]
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
) -> Result<Reconstructed<'source>, ConstantBooleanError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ConstantBooleanError::SourceMismatch);
    }
    // The validator re-derives the catalog binding: the bound catalog must
    // describe this plan's target, constraint catalog, and selected keys.
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
    let operand = flag_reader(materialization_instruction)
        .ok_or(ConstantBooleanError::UnsupportedInstruction)?;
    let result = operand.virtual_register;
    let result_register = function
        .virtual_registers
        .iter()
        .find(|register| register.id == result)
        .ok_or(ConstantBooleanError::UnsupportedUse)?;
    let keys = environment.selected_keys();
    // The consumer's bound row must carry the same single-`Def` shape the
    // record does.
    let consumer_row = environment
        .constraint(materialization_instruction.constraint)
        .ok_or(ConstantBooleanError::ConstraintMismatch)?;
    if !(consumer_row.operands.len() == 1
        && consumer_row.operands[0].operand == 0
        && consumer_row.operands[0].access == RegisterOperandAccess::Def)
    {
        return Err(ConstantBooleanError::ConstraintMismatch);
    }
    // The rewritten instruction is the target's own materialize row,
    // named by its `MaterializeI64` semantic: a single `[def]` at the
    // boolean result's class carrying no unit traffic — the fold drops
    // the flag uses, and any implicit definition or clobber the row added
    // would publish unit state the source instruction never had.
    let row = environment
        .constraint(
            keys.for_semantic(MachineSemanticKind::MaterializeI64)
                .ok_or(ConstantBooleanError::ConstraintMismatch)?,
        )
        .ok_or(ConstantBooleanError::ConstraintMismatch)?;
    if row.operands.len() != 1
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Def
        || row.operands[0].class != result_register.class
        || row.operands[0].fixed_view.is_some()
        || row.operands[0].tied_to.is_some()
        || row.operands[0].early_clobber
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
        || !row.clobbers.is_empty()
    {
        return Err(ConstantBooleanError::ConstraintMismatch);
    }
    // The flag universe is the environment's own vocabulary: the implicit
    // units the three condition-state producer rows define. The validator
    // computes it from the selected keys directly, not from the declared
    // table.
    let mut flag_universe = BTreeSet::new();
    for key in [
        keys.compare_i64,
        keys.compare_i64_immediate,
        keys.compare_i64_zero,
    ] {
        flag_universe.extend(
            environment
                .constraint(key)
                .ok_or(ConstantBooleanError::ConstraintMismatch)?
                .implicit_defs
                .iter()
                .copied(),
        );
    }
    let mut flag_uses = Vec::new();
    let mut plain_uses = Vec::new();
    for unit in &materialization_instruction.implicit_uses {
        if flag_universe.contains(unit) {
            flag_uses.push(*unit);
        } else {
            plain_uses.push(*unit);
        }
    }
    if flag_uses.is_empty() {
        // A materialization observing no condition state has no predicate
        // to decide.
        return Err(ConstantBooleanError::UnsupportedUse);
    }
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition
    // state bounds every path, and mark the cone of blocks that can reach
    // the materialization's block — only their entry sets can feed the
    // result.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(ConstantBooleanError::SourceMismatch)?;
    let cone = backward_cone(&predecessors, block_index);
    // Every flag unit the materialization reads must reach from the same
    // compare: on every execution path to the materialization, the last
    // event touching each used unit is that one instruction, and the unit
    // is among its published definitions — a clobber there would leave
    // the observed value unknown. The validator resolves each unit's
    // reaching event on its own, so a producer that admitted divergent
    // sites or an uncovered unit cannot pass validation.
    let mut compare_site = None;
    for unit in &flag_uses {
        let event = reaching_event(
            function,
            entry,
            &successors,
            &cone,
            block_index,
            materialization_index,
            *unit,
        )
        .map_err(condition_error)?;
        if compare_site.is_some_and(|site| site != event) {
            return Err(ConstantBooleanError::UnsupportedUse);
        }
        compare_site = Some(event);
    }
    let compare_site = compare_site.ok_or(ConstantBooleanError::UnsupportedUse)?;
    let compare_instruction = instruction_at(function, compare_site);
    if flag_uses
        .iter()
        .any(|unit| !compare_instruction.implicit_defs.contains(unit))
    {
        return Err(ConstantBooleanError::UnsupportedUse);
    }
    // The validator restates the unit-flow relationship from the records:
    // the flag uses retire, every non-flag use must stay read on the
    // rewritten row, and the consumer's implicit definitions and clobbers
    // republish verbatim — the materialize row reads nothing, so a
    // consumer using anything outside the flag universe cannot fold.
    if plain_uses
        .iter()
        .any(|unit| !row.implicit_uses.contains(unit))
        || row.implicit_defs != materialization_instruction.implicit_defs
        || row.clobbers != materialization_instruction.clobbers
        || row
            .implicit_uses
            .iter()
            .any(|unit| !materialization_instruction.implicit_uses.contains(unit))
    {
        return Err(ConstantBooleanError::UnsupportedUse);
    }
    let (left, right) =
        constant_operands(function, compare_instruction).map_err(condition_error)?;
    let value = predicate_outcome(materialization_instruction.kind, left, right)
        .ok_or(ConstantBooleanError::UnsupportedInstruction)?;
    // The effect relationship, restated on the bound catalog: the
    // consumer's condition read and the rewritten materialization are
    // both effect-isolated fall-through rows, the consumer's encoded
    // implicit uses stay inside the flag universe, and the rewritten
    // alternatives carry no implicit unit traffic at all. A target whose
    // rows encode anything else cannot carry the fold.
    let semantic = materialization_semantic(materialization_instruction.kind)
        .ok_or(ConstantBooleanError::UnsupportedInstruction)?;
    let consumer_declaration = declaration_for(
        effect_catalog,
        semantic,
        materialization_instruction.constraint,
    )
    .ok_or(ConstantBooleanError::EffectSurfaceMismatch)?;
    let rewritten_declaration =
        declaration_for(effect_catalog, MachineSemanticKind::MaterializeI64, row.key)
            .ok_or(ConstantBooleanError::EffectSurfaceMismatch)?;
    if !(isolated_declaration(consumer_declaration)
        && isolated_declaration(rewritten_declaration)
        && consumer_declaration
            .alternatives
            .iter()
            .all(|alternative| condition_reading_alternative(alternative, &flag_universe))
        && rewritten_declaration
            .alternatives
            .iter()
            .all(materialize_alternative))
    {
        return Err(ConstantBooleanError::EffectSurfaceMismatch);
    }
    let materialize_surface = row
        .implicit_uses
        .iter()
        .chain(row.implicit_defs.iter())
        .chain(row.clobbers.iter())
        .collect::<BTreeSet<_>>()
        .len();
    Ok(Reconstructed {
        function,
        block_index,
        materialization_index,
        materialization_id: materialization,
        provenance: materialization_instruction.provenance.clone(),
        operand,
        value: IntegerValue::Unsigned(u128::from(value)),
        row,
        successors,
        implicit_uses: materialization_instruction.implicit_uses.len(),
        flag_universe: flag_universe.len(),
        materialize_surface,
        producer_defs: compare_instruction.implicit_defs.len(),
    })
}

/// The flag-universe and materialize-surface sizes the measured-step
/// contract charges — the union of the target's three compare rows'
/// implicit definitions and the materialize row's implicit surface, under
/// the same deduplication the audit's own sets apply. Discovery charges
/// these once per pass, and the reached compare's published definitions
/// stay bounded by the flag universe they are drawn from.
pub(crate) fn surface_sizes(
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(usize, usize), ConstantBooleanError> {
    let keys = environment.selected_keys();
    let mut flag_universe = BTreeSet::new();
    for key in [
        keys.compare_i64,
        keys.compare_i64_immediate,
        keys.compare_i64_zero,
    ] {
        flag_universe.extend(
            environment
                .constraint(key)
                .ok_or(ConstantBooleanError::ConstraintMismatch)?
                .implicit_defs
                .iter()
                .copied(),
        );
    }
    let materialize_row = environment
        .constraint(
            keys.for_semantic(MachineSemanticKind::MaterializeI64)
                .ok_or(ConstantBooleanError::ConstraintMismatch)?,
        )
        .ok_or(ConstantBooleanError::ConstraintMismatch)?;
    let materialize_surface: BTreeSet<RegisterUnitId> = materialize_row
        .implicit_uses
        .iter()
        .chain(materialize_row.implicit_defs.iter())
        .chain(materialize_row.clobbers.iter())
        .copied()
        .collect();
    Ok((flag_universe.len(), materialize_surface.len()))
}

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: one step per block plus one per instruction
/// across the plan, two scans of the reconstructed function's blocks for
/// the producer audit, and the shared walk's setup and fixpoint bound at
/// one per used unit — flag or not — so the bound stays independent of
/// the partition split. The `flag_universe`, `materialize_surface`, and
/// `producer_defs` surface sizes are caller-supplied so discovery can
/// charge the contract before admission runs; a pre-admission caller
/// bounds `producer_defs` by `flag_universe`, which covers the published
/// definitions of every compare row the union was built from.
pub(crate) fn measured_steps(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    successors: &[Vec<usize>],
    implicit_uses: usize,
    flag_universe: usize,
    materialize_surface: usize,
    producer_defs: usize,
) -> Result<u64, ConstantBooleanError> {
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
    // The shared walk's setup resolves every edge's target index, fills
    // the predecessor lists, and marks the backward cone. Each used unit
    // then pays its partition checks — flag-universe and
    // materialize-surface membership and the producer-definitions lookup —
    // plus, for a flag unit, the block-prefix scan and — on an in-block
    // miss — every block's stream for its last event, after which
    // entry-set propagation requeues a block only while its set grows:
    // a set holds at most one element per event site plus the unknown
    // marker, so pops stay under `blocks × (elements + 1)` and each pop
    // visits its out-edges at a bounded union cost.
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
    let per_unit = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .and_then(|total| {
            total.checked_add(
                flag_universe
                    .checked_add(materialize_surface)?
                    .checked_add(producer_defs)?,
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
        .and_then(|total| total.checked_add(flag_universe))
        .and_then(|total| total.checked_add(materialize_surface))
        .ok_or(ConstantBooleanError::IdentityOverflow)?;
    let reach_scan = implicit_uses
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
    u64::try_from(steps).map_err(|_| ConstantBooleanError::IdentityOverflow)
}

/// Build the instruction the contract demands from the validator's own
/// record: the target's materialize row supplies the implicit surface
/// while the consumer's instruction identity, provenance, and `Def`
/// operand record stay — the decided value lands in the same register
/// home. The producer's `rewritten` constructor is not consulted; both
/// sides derive the same instruction from the source alone.
fn expected(reconstructed: &Reconstructed<'_>) -> SelectedInstruction {
    SelectedInstruction {
        id: reconstructed.materialization_id,
        kind: SelectedInstructionKind::MaterializeI64 {
            value: reconstructed.value,
        },
        constraint: reconstructed.row.key,
        operands: vec![reconstructed.operand],
        implicit_uses: reconstructed.row.implicit_uses.clone(),
        implicit_defs: reconstructed.row.implicit_defs.clone(),
        clobbers: reconstructed.row.clobbers.clone(),
        provenance: reconstructed.provenance.clone(),
    }
}

/// Reinsert the source boolean: undoing the validator's expected edit
/// must restore the complete source by content — every other
/// instruction, register, call, and settlement included.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
    function_index: usize,
) -> Result<SelectedInstructionPlan, ConstantBooleanError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(function_index)
        .ok_or(ConstantBooleanError::ReplayMismatch)?;
    let block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(ConstantBooleanError::ReplayMismatch)?;
    if block.instructions.len() <= reconstructed.materialization_index {
        return Err(ConstantBooleanError::ReplayMismatch);
    }
    block.instructions[reconstructed.materialization_index] = reconstructed.function.blocks
        [reconstructed.block_index]
        .instructions[reconstructed.materialization_index]
        .clone();
    Ok(restored)
}

/// Independently consume the proposed program: the validator reconstructs
/// the fold's preconditions from the source, requires the proposed
/// instruction at the materialization's position to equal the instruction
/// its own record produces, and restores the complete source by content —
/// every other instruction, register, call, and settlement included. The
/// producer's `admission::admit` is never consulted, so a wrong legality
/// decision fails here even when the proposal matches the edit the
/// producer emitted.
pub fn validate_constant_boolean_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedConstantBoolean, ConstantBooleanError> {
    let reconstructed = reconstruct(
        source,
        function_index,
        materialization,
        environment,
        effect_catalog,
    )?;
    if measured_steps(
        source.selected_plan(),
        reconstructed.function,
        &reconstructed.successors,
        reconstructed.implicit_uses,
        reconstructed.flag_universe,
        reconstructed.materialize_surface,
        reconstructed.producer_defs,
    )? > budget.validation_steps()
    {
        return Err(ConstantBooleanError::WorkBudgetExceeded);
    }
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.block_index))
        .and_then(|block| block.instructions.get(reconstructed.materialization_index))
        != Some(&expected(&reconstructed))
    {
        return Err(ConstantBooleanError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed, function_index)? != *source.selected_plan() {
        return Err(ConstantBooleanError::ReplayMismatch);
    }
    Ok(ValidatedConstantBoolean {
        receipt: ConstantBooleanReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
            function_index,
            materialization,
        },
        transformed: Arc::new(proposed),
    })
}

#[cfg(test)]
mod independence_tests {
    use std::sync::Arc;

    use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
    use register_environment::baseline_target_register_environment;
    use register_model::RegisterInstructionConstraint;
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
        SelectedTerminator, ValidatedMachineEffectCatalog, VirtualRegister, VirtualRegisterId,
        VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
        ScalarType, ValueId,
    };
    use target::NativeTarget;
    use target_operations_to_selected_instructions::selected_instruction_plan_identity;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::{
        ConstantBooleanError, ConstantBooleanReceipt, ValidatedConstantBoolean,
        validate_constant_boolean_fold,
    };
    use crate::analyses::validated_machine_effect_catalog;
    use register_environment::ValidatedTargetRegisterEnvironment;
    use register_model::RegisterUnitId;

    const MATERIALIZE_A: SelectedInstructionId = SelectedInstructionId(2);
    const MATERIALIZE_AGAIN: SelectedInstructionId = SelectedInstructionId(3);
    const MATERIALIZE_B: SelectedInstructionId = SelectedInstructionId(4);
    const COMPARE: SelectedInstructionId = SelectedInstructionId(5);
    const BOOLEAN: SelectedInstructionId = SelectedInstructionId(6);
    const TERMINAL: SelectedInstructionId = SelectedInstructionId(7);

    const LITA: VirtualRegisterId = VirtualRegisterId(0);
    const LITB: VirtualRegisterId = VirtualRegisterId(1);
    const OUTPUT: VirtualRegisterId = VirtualRegisterId(2);

    fn instruction(
        id: SelectedInstructionId,
        kind: SelectedInstructionKind,
        row: &RegisterInstructionConstraint,
        registers: &[VirtualRegisterId],
    ) -> SelectedInstruction {
        SelectedInstruction {
            id,
            kind,
            constraint: row.key,
            operands: row
                .operands
                .iter()
                .zip(registers)
                .map(|(operand, register)| SelectedOperand {
                    operand: operand.operand,
                    virtual_register: *register,
                    access: operand.access,
                    class: operand.class,
                    fixed_view: operand.fixed_view,
                    tied_to: operand.tied_to,
                    early_clobber: operand.early_clobber,
                })
                .collect(),
            implicit_uses: row.implicit_uses.clone(),
            implicit_defs: row.implicit_defs.clone(),
            clobbers: row.clobbers.clone(),
            provenance: Default::default(),
        }
    }

    fn u64_scalar() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
    }

    /// `ra = materialize 3; rb = materialize 5; compare ra, rb;
    /// rout = boolean; return` — a legal fold where the equality decides
    /// zero. Each test mutates one legality premise, so the source names a
    /// fold the contract must refuse while a defective producer would
    /// still emit it.
    fn plan(
        environment: &ValidatedTargetRegisterEnvironment,
        target: NativeTarget,
    ) -> SelectedInstructionPlan {
        let keys = environment.selected_keys();
        let materialize = environment.constraint(keys.materialize_i64).unwrap();
        let compare = environment.constraint(keys.compare_i64).unwrap();
        let boolean = environment.constraint(keys.materialize_boolean).unwrap();
        let terminal_row = environment.constraint(keys.return_unit).unwrap();
        let class = compare.operands[0].class;
        let machine = MachineId::new(1).unwrap();
        SelectedInstructionPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target,
            entry: machine,
            functions: vec![SelectedFunction {
                machine,
                attachment: None,
                provenance: Default::default(),
                structural: None,
                local_storage_slots: Vec::new(),
                outgoing_arguments: Vec::new(),
                calls: Vec::new(),
                normalized_foreign_calls: Vec::new(),
                memory_accesses: Vec::new(),
                boundary_settlements: Vec::new(),
                entry_block: SelectedBlockId(0),
                virtual_registers: vec![
                    VirtualRegister {
                        id: LITA,
                        scalar_type: u64_scalar(),
                        class,
                        origin: VirtualRegisterOrigin::InstructionResult {
                            instruction: MATERIALIZE_A,
                            source_value: ValueId::new(2).unwrap(),
                        },
                        definition_site: None,
                        entry_fixed_view: None,
                    },
                    VirtualRegister {
                        id: LITB,
                        scalar_type: u64_scalar(),
                        class,
                        origin: VirtualRegisterOrigin::InstructionResult {
                            instruction: MATERIALIZE_B,
                            source_value: ValueId::new(3).unwrap(),
                        },
                        definition_site: None,
                        entry_fixed_view: None,
                    },
                    VirtualRegister {
                        id: OUTPUT,
                        scalar_type: u64_scalar(),
                        class,
                        origin: VirtualRegisterOrigin::InstructionResult {
                            instruction: BOOLEAN,
                            source_value: ValueId::new(4).unwrap(),
                        },
                        definition_site: None,
                        entry_fixed_view: None,
                    },
                ],
                blocks: vec![SelectedBlock {
                    id: SelectedBlockId(0),
                    origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                    instructions: vec![
                        instruction(
                            MATERIALIZE_A,
                            SelectedInstructionKind::MaterializeI64 {
                                value: IntegerValue::Unsigned(3),
                            },
                            materialize,
                            &[LITA],
                        ),
                        instruction(
                            MATERIALIZE_B,
                            SelectedInstructionKind::MaterializeI64 {
                                value: IntegerValue::Unsigned(5),
                            },
                            materialize,
                            &[LITB],
                        ),
                        instruction(
                            COMPARE,
                            SelectedInstructionKind::CompareI64,
                            compare,
                            &[LITA, LITB],
                        ),
                        instruction(
                            BOOLEAN,
                            SelectedInstructionKind::MaterializeBooleanEqual,
                            boolean,
                            &[OUTPUT],
                        ),
                    ],
                    terminator: SelectedTerminator::Return {
                        instruction: instruction(
                            TERMINAL,
                            SelectedInstructionKind::ReturnUnit,
                            terminal_row,
                            &[],
                        ),
                        psi_return_edge: EdgeId::new(2).unwrap(),
                    },
                }],
            }]
            .into(),
        }
    }

    fn source(plan: SelectedInstructionPlan) -> ValidatedConstantBoolean {
        let identity = selected_instruction_plan_identity(&plan);
        ValidatedConstantBoolean {
            receipt: ConstantBooleanReceipt {
                source_selected: identity,
                transformed_selected: identity,
                optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
                fuel_schedule: plan.fuel_schedule,
                function_index: 0,
                materialization: BOOLEAN,
            },
            transformed: Arc::new(plan),
        }
    }

    /// The proposal a defective producer emits for this source: the boolean
    /// folded to the `MaterializeI64` of zero under the target's own
    /// materialize row — the edit the contract demands only when the
    /// fold's legality actually holds.
    fn forged_materialize(
        source: &ValidatedConstantBoolean,
        environment: &ValidatedTargetRegisterEnvironment,
    ) -> SelectedInstructionPlan {
        let materialize_row = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap();
        let mut proposed = source.transformed().clone();
        let consumer_index = proposed.functions[0].blocks[0]
            .instructions
            .iter()
            .position(|instruction| instruction.id == BOOLEAN)
            .unwrap();
        let consumer = &proposed.functions[0].blocks[0].instructions[consumer_index];
        proposed.functions[0].blocks[0].instructions[consumer_index] = SelectedInstruction {
            id: consumer.id,
            kind: SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(0),
            },
            constraint: materialize_row.key,
            operands: consumer.operands.clone(),
            implicit_uses: materialize_row.implicit_uses.clone(),
            implicit_defs: materialize_row.implicit_defs.clone(),
            clobbers: materialize_row.clobbers.clone(),
            provenance: consumer.provenance.clone(),
        };
        proposed
    }

    fn budget() -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(100, 100, 100_000, 100, 100).unwrap()
    }

    /// The machine-effect catalog the stage binds for `environment`.
    fn catalog(environment: &ValidatedTargetRegisterEnvironment) -> ValidatedMachineEffectCatalog {
        validated_machine_effect_catalog(environment.target(), environment.constraints())
            .unwrap()
            .as_ref()
            .clone()
    }

    /// The boolean record is rebound to the materialize row's constraint
    /// key: the bound catalog declares no `MaterializeBoolean*` semantic
    /// under that key, so nothing attests the decided-materialization
    /// relationship. A producer that skipped the catalog binding still
    /// emits the `MaterializeI64`; the validator's own catalog restatement
    /// refuses with `EffectSurfaceMismatch`. A catalog bound to another
    /// target refuses the untouched legal source the same way.
    #[test]
    fn validator_refuses_a_materialize_surface_the_catalog_never_declared() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let mut plan = plan(&environment, target);
        let materialize_key = environment.selected_keys().materialize_i64;
        plan.functions[0].blocks[0].instructions[3].constraint = materialize_key;
        let rebound = source(plan);
        let proposed = forged_materialize(&rebound, &environment);
        assert_eq!(
            validate_constant_boolean_fold(
                &rebound,
                0,
                BOOLEAN,
                &environment,
                &catalog(&environment),
                budget(),
                proposed
            )
            .unwrap_err(),
            ConstantBooleanError::EffectSurfaceMismatch
        );
        let legal = source(self::plan(&environment, target));
        let foreign = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
        let proposed = forged_materialize(&legal, &environment);
        assert_eq!(
            validate_constant_boolean_fold(
                &legal,
                0,
                BOOLEAN,
                &environment,
                &catalog(&foreign),
                budget(),
                proposed
            )
            .unwrap_err(),
            ConstantBooleanError::EffectSurfaceMismatch
        );
    }

    /// `ra = materialize 3; ra = materialize 9; rb = materialize 5;
    /// compare ra, rb; rout = boolean; return` — the compare's left
    /// operand has two producers, so its value is not pinned
    /// function-wide and the fold's constant was never proven. A
    /// producer that admitted it anyway would publish a plan whose
    /// boolean is rewritten to a materialize the source never
    /// established; the validator's own operand audit must refuse with
    /// `UnsupportedProducer`, not merely diff the proposal. Feeding that
    /// forged proposal is the observable proof that validation no longer
    /// relies on the producer's admission routine.
    #[test]
    fn validator_refuses_a_proposal_its_own_audit_rejects() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap();
        let mut plan = plan(&environment, target);
        plan.functions[0].blocks[0].instructions.insert(
            1,
            instruction(
                MATERIALIZE_AGAIN,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(9),
                },
                materialize,
                &[LITA],
            ),
        );
        let source = source(plan);
        let proposed = forged_materialize(&source, &environment);
        assert_eq!(
            validate_constant_boolean_fold(
                &source,
                0,
                BOOLEAN,
                &environment,
                &catalog(&environment),
                budget(),
                proposed
            )
            .unwrap_err(),
            ConstantBooleanError::UnsupportedProducer
        );
    }

    /// The boolean's use roster gains a unit that is neither a flag the
    /// fold decides nor a unit on the materialize row's implicit surface —
    /// the rebuilt instruction would silently drop the observation. A
    /// producer that skipped the use partition still emits the
    /// `MaterializeI64`; the validator's own partition refuses with
    /// `UnsupportedUse`.
    #[test]
    fn validator_refuses_an_observation_the_materialize_cannot_carry() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let mut plan = plan(&environment, target);
        plan.functions[0].blocks[0].instructions[3]
            .implicit_uses
            .push(RegisterUnitId(u16::MAX));
        let source = source(plan);
        let proposed = forged_materialize(&source, &environment);
        assert_eq!(
            validate_constant_boolean_fold(
                &source,
                0,
                BOOLEAN,
                &environment,
                &catalog(&environment),
                budget(),
                proposed
            )
            .unwrap_err(),
            ConstantBooleanError::UnsupportedUse
        );
    }

    /// The compare is gone: every flag unit the boolean reads reaches only
    /// the entry's unknown state. A producer that mistook the read for a
    /// resolved one still emits the `MaterializeI64`; the validator's own
    /// reaching walk refuses with `UnsupportedUse`.
    #[test]
    fn validator_refuses_flags_that_never_reached_a_compare() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let mut plan = plan(&environment, target);
        plan.functions[0].blocks[0]
            .instructions
            .retain(|instruction| instruction.id != COMPARE);
        let source = source(plan);
        let proposed = forged_materialize(&source, &environment);
        assert_eq!(
            validate_constant_boolean_fold(
                &source,
                0,
                BOOLEAN,
                &environment,
                &catalog(&environment),
                budget(),
                proposed
            )
            .unwrap_err(),
            ConstantBooleanError::UnsupportedUse
        );
    }
}
