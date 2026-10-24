//! Descriptor-driven admission for constant condition branch folding:
//! locate the named conditional-branch terminator, confirm the declared
//! consumer's emitted zero-operand shape, partition its implicit uses by
//! the flag universe — flag units resolve to the one producer through the
//! shared reaching walk, non-flag units must lie in the jump row's implicit
//! surface — then complete the pair from the [`TerminatorPairRule`] table:
//! the reached producer's kind selects the rule, whose operand grammar
//! decides the successor, whose unit-flow axis states the retired and
//! preserved unit roles, and whose control-flow axis is checked against the
//! bound machine-effect catalog.
use crate::rewrites::operand_probes::effect_declaration;
use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterUnitId};
use selected_instructions::{
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedSuccessor, SelectedTerminator,
    ValidatedMachineEffectCatalog, terminator_instruction,
};

use super::ConstantBranchError;
use super::pair::{
    TERMINATOR_PAIR_RULES, TerminatorPairRule, declared_consumer, declared_producers,
    terminator_pair_for,
};
use crate::ValidatedSelectedAnalysis;
use crate::analyses::machine_effects::machine_semantic_kind;
use crate::rewrites::block_edges::function_at;
use crate::rewrites::condition_state::{
    ConditionStateError, adjacency, backward_cone, entry_index, instruction_at, reaching_event,
    resolved_operands,
};

impl From<ConditionStateError> for ConstantBranchError {
    fn from(error: ConditionStateError) -> Self {
        match error {
            ConditionStateError::Use => ConstantBranchError::UnsupportedUse,
            ConditionStateError::Producer => ConstantBranchError::UnsupportedProducer,
            ConditionStateError::Literal => ConstantBranchError::UnsupportedLiteral,
        }
    }
}

pub(super) struct Admission<'source> {
    pub block_index: usize,
    pub branch_id: SelectedInstructionId,
    pub provenance: SelectedInstructionProvenance,
    /// The decided successor the rebuilt `Jump` terminator carries
    /// verbatim — bindings, structural payloads, case custody, and fuel
    /// included.
    pub successor: SelectedSuccessor,
    /// The jump row the rebuilt terminator instruction is built from.
    pub row: &'source RegisterInstructionConstraint,
}

/// The single catalog declaration for `semantic` bound to `constraint`, or
/// none when the catalog does not declare exactly one such form.
/// Admit `branch` — the instruction a block's terminator carries — as a
/// declared decided-branch pair.
///
/// The order of gates fixes the error vocabulary: the catalog binding and
/// the record shape (declared consumer kind, terminator variant, operands,
/// implicit uses) refuse first, then the constraint rows, then the unit
/// flow — flag-universe partition, reaching resolution, and the producer's
/// published definitions — then the producer kind the table must pair with
/// this consumer and the rule's unit-flow axis, then the rule's operand
/// grammar, then the catalog declarations' encoded control relationship,
/// and last the bounded-work charge.
pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    branch: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, ConstantBranchError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ConstantBranchError::SourceMismatch);
    }
    // The bound catalog must describe this plan's target and this
    // environment's constraint catalog and selected-key inventory: a
    // foreign catalog's declarations cannot attest this program's encoded
    // control surfaces even where constraint keys coincide numerically.
    let catalog = effect_catalog.catalog();
    if catalog.target != plan.target
        || catalog.register_constraints != environment.constraints().identity()
        || catalog.selected_keys != environment.selected_keys()
    {
        return Err(ConstantBranchError::EffectSurfaceMismatch);
    }
    let function = function_at(plan, function_index, ConstantBranchError::SourceMismatch)?;
    // The branch names its terminator-carried instruction: the block whose
    // terminator instruction carries the id holds the read position at the
    // end of its body stream.
    let block_index = function
        .blocks
        .iter()
        .position(|block| terminator_instruction(&block.terminator).id == branch)
        .ok_or(ConstantBranchError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let branch_instruction = terminator_instruction(&block.terminator);
    // The terminator must be a declared consumer in its emitted
    // zero-operand flag-reader shape: the table names the kind, the kind
    // must pair with the terminator variant it rides — a
    // `ConditionalBranch` terminator carrying a less-than kind is not the
    // shape selection emits — explicit operands are never present, and a
    // use roster holding no unit has no condition to decide.
    let consumer_kind = machine_semantic_kind(branch_instruction.kind);
    let candidate_rules: Vec<&TerminatorPairRule> = TERMINATOR_PAIR_RULES
        .iter()
        .filter(|rule| rule.consumer() == consumer_kind)
        .collect();
    if !declared_consumer(branch_instruction.kind)
        || !candidate_rules
            .iter()
            .any(|rule| rule.matches_terminator(&block.terminator))
        || !branch_instruction.operands.is_empty()
        || branch_instruction.implicit_uses.is_empty()
    {
        return Err(ConstantBranchError::UnsupportedInstruction);
    }
    let keys = environment.selected_keys();
    // The flag universe partitions the branch's implicit uses: the units
    // the declared producers' rows publish are the condition state the
    // fold decides, and every other observed unit must survive on the jump
    // row's own implicit surface.
    let mut flag_universe = BTreeSet::new();
    for producer_kind in declared_producers() {
        let key = keys
            .for_semantic(producer_kind)
            .ok_or(ConstantBranchError::ConstraintMismatch)?;
        flag_universe.extend(
            environment
                .constraint(key)
                .ok_or(ConstantBranchError::ConstraintMismatch)?
                .implicit_defs
                .iter()
                .copied(),
        );
    }
    // Every declared pair rewrites to the same `Jump` form, so the first
    // candidate's declaration names the row the rewrite binds.
    let jump_key = keys
        .for_semantic(candidate_rules[0].rewritten())
        .ok_or(ConstantBranchError::ConstraintMismatch)?;
    let jump_row = environment
        .constraint(jump_key)
        .ok_or(ConstantBranchError::ConstraintMismatch)?;
    let jump_surface: BTreeSet<RegisterUnitId> = jump_row
        .implicit_uses
        .iter()
        .chain(jump_row.implicit_defs.iter())
        .chain(jump_row.clobbers.iter())
        .copied()
        .collect();
    // The rebuilt terminator publishes the jump row's surface where the
    // branch's stood, so the two must agree on definitions and clobbers:
    // a definition or clobber the jump lacks would hand downstream readers
    // an older event at this site, and one the branch lacked would publish
    // new state the source never had.
    if !jump_row.operands.is_empty()
        || BTreeSet::from_iter(jump_row.implicit_defs.iter().copied())
            != BTreeSet::from_iter(branch_instruction.implicit_defs.iter().copied())
        || BTreeSet::from_iter(jump_row.clobbers.iter().copied())
            != BTreeSet::from_iter(branch_instruction.clobbers.iter().copied())
    {
        return Err(ConstantBranchError::ConstraintMismatch);
    }
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition state
    // bounds every path, and mark the cone of blocks that can reach the
    // branch's block — only their entry sets can feed the result.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(ConstantBranchError::SourceMismatch)?;
    let cone = backward_cone(&predecessors, block_index);
    // Every flag unit the branch reads must reach from the same producer:
    // on every execution path to the terminator, the last event touching
    // each used flag unit is that one instruction, and the unit is among
    // its published definitions — a clobber there would leave the observed
    // value unknown. Every other used unit must keep its observation on
    // the jump row's surface. The producer stays published for readers the
    // rewrite leaves behind.
    let read_position = block.instructions.len();
    let mut flag_uses = Vec::new();
    let mut plain_uses = Vec::new();
    let mut compare_site = None;
    for unit in &branch_instruction.implicit_uses {
        if !flag_universe.contains(unit) {
            if !jump_surface.contains(unit) {
                return Err(ConstantBranchError::UnsupportedUse);
            }
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
            read_position,
            *unit,
        )?;
        if compare_site.is_some_and(|site| site != event) {
            return Err(ConstantBranchError::UnsupportedUse);
        }
        compare_site = Some(event);
    }
    if flag_uses.is_empty() {
        return Err(ConstantBranchError::UnsupportedInstruction);
    }
    let compare_site = compare_site.ok_or(ConstantBranchError::UnsupportedUse)?;
    let compare_instruction = instruction_at(function, compare_site);
    if flag_uses
        .iter()
        .any(|unit| !compare_instruction.implicit_defs.contains(unit))
    {
        return Err(ConstantBranchError::UnsupportedUse);
    }
    // The reached producer completes the pair: the table must declare a
    // rule for this (producer, consumer) combination. Its unit-flow axis
    // then states the concrete roles — flag uses retire, every non-flag
    // use stays read on the rewritten row, and the rewritten row
    // republishes the branch's implicit definitions and clobbers — and its
    // operand grammar decides the predicate.
    let rule = terminator_pair_for(
        machine_semantic_kind(compare_instruction.kind),
        consumer_kind,
    )
    .ok_or(ConstantBranchError::UnsupportedProducer)?;
    if !rule.matches_producer(compare_instruction.kind) {
        return Err(ConstantBranchError::UnsupportedProducer);
    }
    if !rule.admits_unit_flow(&flag_uses, &plain_uses, branch_instruction, jump_row) {
        return Err(ConstantBranchError::UnsupportedUse);
    }
    let (left, right) =
        resolved_operands(function, compare_instruction, rule.operand_resolution())?;
    let successor = rule
        .decided_successor(&block.terminator, left, right)
        .ok_or(ConstantBranchError::UnsupportedInstruction)?
        .clone();
    // The declared control-flow relationship, checked against the bound
    // catalog: the branch encodes a conditional relative branch, the jump
    // row an unconditional one, and both are otherwise effect-isolated.
    let consumer_declaration =
        effect_declaration(effect_catalog, consumer_kind, branch_instruction.constraint)
            .ok_or(ConstantBranchError::EffectSurfaceMismatch)?;
    let rewritten_declaration = effect_declaration(effect_catalog, rule.rewritten(), jump_row.key)
        .ok_or(ConstantBranchError::EffectSurfaceMismatch)?;
    if !rule.admits_declarations(consumer_declaration, rewritten_declaration) {
        return Err(ConstantBranchError::EffectSurfaceMismatch);
    }
    let steps = crate::rewrites::constant_fold_steps(
        plan,
        function,
        &successors,
        branch_instruction.implicit_uses.len(),
        flag_universe.len(),
        jump_surface.len(),
        compare_instruction.implicit_defs.len(),
        || ConstantBranchError::IdentityOverflow,
    )?;
    crate::rewrites::charge_steps(
        steps,
        budget,
        || ConstantBranchError::IdentityOverflow,
        || ConstantBranchError::WorkBudgetExceeded,
    )?;
    Ok(Admission {
        block_index,
        branch_id: branch,
        provenance: branch_instruction.provenance.clone(),
        successor,
        row: jump_row,
    })
}

/// The one-terminator proposal shape shared with replay: the target's own
/// jump row — zero-operand by admission — supplies the implicit surface
/// while the branch's instruction identity and provenance stay with the
/// terminator, and the decided successor carries its record verbatim.
pub(super) fn rewritten(admitted: &Admission<'_>) -> SelectedTerminator {
    SelectedTerminator::Jump {
        instruction: SelectedInstruction {
            id: admitted.branch_id,
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
