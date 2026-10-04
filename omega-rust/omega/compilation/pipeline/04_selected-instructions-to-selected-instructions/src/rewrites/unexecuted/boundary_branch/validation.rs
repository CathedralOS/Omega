//! Independent validation of boundary-decided conditional-branch folding.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! fold's legality from the source records — the terminator-carried
//! instruction's strict-ordering branch/kind pairing and emitted
//! zero-operand shape, the flag partition every implicit use must satisfy
//! (flag units resolving through the shared condition-state walk to the
//! one compare whose published definitions cover them, and — for the
//! decided `Jump` outcome — every other observed unit carried on the jump
//! row's own implicit surface), the compare's resolved operand poles, and
//! the pole-outcome table deciding between a `Jump` fold and the
//! `ConditionalBranchNonZero` collapse — then rebuilds the terminator the
//! contract demands and requires the proposal to carry it in the branch's
//! block. Reinserting the source terminator must restore the complete
//! source by content: every other instruction, register, call,
//! settlement, and function included. A producer admission error
//! therefore fails validation even when the proposal is exactly what that
//! producer emitted.
use std::collections::BTreeSet;
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterUnitId};
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedInstructionProvenance, SelectedTerminator,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{BoundaryBranchError, BoundaryBranchReceipt, ValidatedBoundaryBranch};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::terminator_instruction;
use crate::rewrites::condition_state::{
    ConditionStateError, adjacency, backward_cone, boundary_operands, entry_index, instruction_at,
    reaching_event,
};

/// What the resolved operand poles force on the branch's strict ordering
/// over `left - right`, in the validator's own record: the same two
/// outcomes the contract defines, re-decided here so a wrong boundary
/// decision inside the producer's `outcome` cannot pass validation.
#[derive(Clone, Copy)]
enum Outcome {
    /// The predicate is decided for every surviving operand value: the
    /// terminator becomes the target's `Jump` to `when_less` when the
    /// ordering holds, `when_not_less` when it does not.
    Constant(bool),
    /// The predicate holds exactly off the near pole: `0 <u x`,
    /// `x <u u64::MAX`, `i64::MIN <s x`, and `x <s i64::MAX` each coincide
    /// with the nonzero condition (`left != right`) on the same published
    /// flag state, so the terminator becomes `ConditionalBranch` carrying
    /// `ConditionalBranchNonZero` with the arms republished
    /// `when_nonzero`/`when_zero`.
    NonZero,
}

/// The validator's own reconstruction of the fold the contract permits:
/// the admitted branch's block, identity, and emitted provenance, the
/// decided outcome, the jump row a decided fold is built from, the
/// resolved-index successor lists the shared walk ran over, and the
/// surface sizes the measured-step contract charges. It shares no state
/// with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    branch_id: SelectedInstructionId,
    provenance: SelectedInstructionProvenance,
    outcome: Outcome,
    /// The jump row the decided fold rebuilds on — `Outcome::Constant`
    /// only; the collapse retains the branch's own row.
    row: Option<&'source RegisterInstructionConstraint>,
    successors: Vec<Vec<usize>>,
    implicit_uses: usize,
    flag_universe: usize,
    jump_surface: usize,
    compare_defs: usize,
}

/// The branch/kind pairings the validator admits and the carrier domain
/// their predicate orders over: `true` selects the signed poles, `false`
/// the unsigned. `ConditionalBranch` carrying `ConditionalBranchNonZero`
/// reads the equality condition — `left != right` varies with every
/// unknown side, so no single operand pole can fix it — and stays
/// refused. The validator re-decides the pairings itself: a wrong
/// admission inside shared producer code would decide both sides
/// identically.
fn branch_domain(
    terminator: &SelectedTerminator,
    instruction: &SelectedInstruction,
) -> Option<bool> {
    match (terminator, instruction.kind) {
        (
            SelectedTerminator::ConditionalBranchU64LessThan { .. },
            SelectedInstructionKind::ConditionalBranchU64LessThan,
        ) => Some(false),
        (
            SelectedTerminator::ConditionalBranchI64LessThan { .. },
            SelectedInstructionKind::ConditionalBranchI64LessThan,
        ) => Some(true),
        _ => None,
    }
}

/// The boundary decision over the compare's resolved operand poles for a
/// strict less-than in `signed`'s domain. The far poles decide the
/// predicate false without consulting the other side — `x < 0` unsigned
/// and `x < i64::MIN` signed can never hold, and neither can a strict
/// less-than issued from the domain maximum. Two known operands decide
/// the predicate outright, matching the constant family's table. The
/// near poles leave a predicate that coincides with the nonzero
/// condition on the same flag state — `0 <u x` is `x != 0`, `x <u
/// u64::MAX` is `x != u64::MAX`, `i64::MIN <s x` is `x != i64::MIN`,
/// `x <s i64::MAX` is `x != i64::MAX` — so the branch collapses to
/// `ConditionalBranchNonZero` with `when_less` serving `when_nonzero`.
/// Decided outcomes take precedence where two rules could fire:
/// `(0, u64::MAX)` under `U64LessThan` is `true` rather than a collapse
/// that computes the same selection. This table is the fold's legality
/// decision: it lives here, not in `admission`, because a wrong boundary
/// rule in shared producer code would certify the proposal its own bug
/// emitted.
fn pole_outcome(signed: bool, left: Option<u64>, right: Option<u64>) -> Option<Outcome> {
    let (minimum, maximum) = if signed {
        (i64::MIN as u64, i64::MAX as u64)
    } else {
        (0, u64::MAX)
    };
    if right == Some(minimum) || left == Some(maximum) {
        return Some(Outcome::Constant(false));
    }
    if let (Some(left), Some(right)) = (left, right) {
        let holds = if signed {
            (left as i64) < (right as i64)
        } else {
            left < right
        };
        return Some(Outcome::Constant(holds));
    }
    if left == Some(minimum) || right == Some(maximum) {
        return Some(Outcome::NonZero);
    }
    None
}

/// The family's own mapping of a shared condition-state rejection onto
/// its error vocabulary. The producer's `From` implementation lives in
/// `admission`; the validator maps each site itself so a changed producer
/// mapping cannot relabel its refusal.
fn condition_error(error: ConditionStateError) -> BoundaryBranchError {
    match error {
        ConditionStateError::Use => BoundaryBranchError::UnsupportedUse,
        ConditionStateError::Producer | ConditionStateError::Literal => {
            BoundaryBranchError::UnsupportedLiteral
        }
    }
}

/// Reconstruct the legality of folding `branch` from first principles:
/// locate the terminator-carried instruction, verify the emitted
/// branch/kind pairing and zero-operand flag-reader shape, build the flag
/// universe from the target's compare rows and the jump surface from its
/// jump row, run each used flag unit's reaching event on the module's
/// shared condition-state walk, require the one compare site whose
/// published definitions cover them all, and decide the outcome on the
/// validator's own pole table. A decided fold then demands the jump
/// row's exact implicit-surface match and a carrier for every non-flag
/// observation; the collapse instead keeps the branch's own row, so it
/// demands the conditional-branch constraint and resolves every
/// flag-universe unit — not only the ones the source branch declared —
/// to the same published compare. Nothing in this audit reads the
/// producer's admission decision.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    branch: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, BoundaryBranchError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(BoundaryBranchError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(BoundaryBranchError::SourceMismatch)?;
    // The branch names its terminator-carried instruction: the block whose
    // terminator instruction carries the id holds the read position at the
    // end of its body stream.
    let block_index = function
        .blocks
        .iter()
        .position(|block| terminator_instruction(&block.terminator).id == branch)
        .ok_or(BoundaryBranchError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let branch_instruction = terminator_instruction(&block.terminator);
    // The terminator must be the emitted zero-operand strict-ordering flag
    // reader: the variant/kind pairing must match — a
    // `ConditionalBranchU64LessThan` terminator carrying a nonzero kind is
    // not the shape selection emits — explicit operands are never present,
    // and a use roster holding no flag unit has no condition to decide.
    let signed = branch_domain(&block.terminator, branch_instruction)
        .ok_or(BoundaryBranchError::UnsupportedInstruction)?;
    if !branch_instruction.operands.is_empty() || branch_instruction.implicit_uses.is_empty() {
        return Err(BoundaryBranchError::UnsupportedInstruction);
    }
    let keys = environment.selected_keys();
    // The flag universe partitions the branch's implicit uses: the units
    // the target's three compare rows publish are the condition state the
    // fold decides, and every other observed unit must survive on the jump
    // row's own implicit surface when the fold lands on `Jump`.
    let mut flag_universe = BTreeSet::new();
    for key in [
        keys.compare_i64,
        keys.compare_i64_immediate,
        keys.compare_i64_zero,
    ] {
        flag_universe.extend(
            environment
                .constraint(key)
                .ok_or(BoundaryBranchError::ConstraintMismatch)?
                .implicit_defs
                .iter()
                .copied(),
        );
    }
    let jump_row = environment
        .constraint(keys.jump)
        .ok_or(BoundaryBranchError::ConstraintMismatch)?;
    let jump_surface: BTreeSet<RegisterUnitId> = jump_row
        .implicit_uses
        .iter()
        .chain(jump_row.implicit_defs.iter())
        .chain(jump_row.clobbers.iter())
        .copied()
        .collect();
    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition
    // state bounds every path, and mark the cone of blocks that can reach
    // the branch's block — only their entry sets can feed the result.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(BoundaryBranchError::SourceMismatch)?;
    let cone = backward_cone(&predecessors, block_index);
    // Every flag unit the branch reads must reach from the same compare:
    // on every execution path to the terminator, the last event touching
    // each used flag unit is that one instruction, and the unit is among
    // its published definitions — a clobber there would leave the
    // observed value unknown. The validator resolves each unit's reaching
    // event on its own, so a producer that admitted divergent sites or an
    // uncovered unit cannot pass validation.
    let read_position = block.instructions.len();
    let mut flag_uses = 0usize;
    let mut compare_site = None;
    for unit in &branch_instruction.implicit_uses {
        if !flag_universe.contains(unit) {
            continue;
        }
        flag_uses += 1;
        let event = reaching_event(
            function,
            entry,
            &successors,
            &cone,
            block_index,
            read_position,
            *unit,
        )
        .map_err(condition_error)?;
        if compare_site.is_some_and(|site| site != event) {
            return Err(BoundaryBranchError::UnsupportedUse);
        }
        compare_site = Some(event);
    }
    if flag_uses == 0 {
        return Err(BoundaryBranchError::UnsupportedInstruction);
    }
    let compare_site = compare_site.ok_or(BoundaryBranchError::UnsupportedUse)?;
    let compare_instruction = instruction_at(function, compare_site);
    if branch_instruction
        .implicit_uses
        .iter()
        .filter(|unit| flag_universe.contains(unit))
        .any(|unit| !compare_instruction.implicit_defs.contains(unit))
    {
        return Err(BoundaryBranchError::UnsupportedUse);
    }
    let (left, right) =
        boundary_operands(function, compare_instruction).map_err(condition_error)?;
    let outcome =
        pole_outcome(signed, left, right).ok_or(BoundaryBranchError::UnsupportedLiteral)?;
    let row = match outcome {
        Outcome::Constant(_) => {
            // The rebuilt terminator publishes the jump row's surface
            // where the branch's stood, so the two must agree on
            // definitions and clobbers: a definition or clobber the jump
            // lacks would hand downstream readers an older event at this
            // site, and one the branch lacked would publish new state the
            // source never had. The branch's non-flag observations — its
            // program-counter read — must likewise survive inside the
            // jump row's own implicit surface.
            if branch_instruction
                .implicit_uses
                .iter()
                .any(|unit| !flag_universe.contains(unit) && !jump_surface.contains(unit))
            {
                return Err(BoundaryBranchError::UnsupportedUse);
            }
            if !jump_row.operands.is_empty()
                || BTreeSet::from_iter(jump_row.implicit_defs.iter().copied())
                    != BTreeSet::from_iter(branch_instruction.implicit_defs.iter().copied())
                || BTreeSet::from_iter(jump_row.clobbers.iter().copied())
                    != BTreeSet::from_iter(branch_instruction.clobbers.iter().copied())
            {
                return Err(BoundaryBranchError::ConstraintMismatch);
            }
            Some(jump_row)
        }
        Outcome::NonZero => {
            // The collapse keeps the branch's own row — the
            // conditional-branch kinds share it — so the rebuilt
            // instruction retains the complete implicit surface while its
            // kind alone changes. Because the nonzero reader can observe
            // any flag unit its encoding implies, every flag-universe
            // unit must resolve to the same compare at this position —
            // not only the units the source branch happened to declare —
            // and must be among the compare's published definitions.
            if branch_instruction.constraint != keys.conditional_branch {
                return Err(BoundaryBranchError::ConstraintMismatch);
            }
            for unit in &flag_universe {
                let event = reaching_event(
                    function,
                    entry,
                    &successors,
                    &cone,
                    block_index,
                    read_position,
                    *unit,
                )
                .map_err(|_| BoundaryBranchError::UnsupportedUse)?;
                if event != compare_site || !compare_instruction.implicit_defs.contains(unit) {
                    return Err(BoundaryBranchError::UnsupportedUse);
                }
            }
            None
        }
    };
    Ok(Reconstructed {
        function,
        block_index,
        branch_id: branch,
        provenance: branch_instruction.provenance.clone(),
        outcome,
        row,
        successors,
        implicit_uses: branch_instruction.implicit_uses.len(),
        flag_universe: flag_universe.len(),
        jump_surface: jump_surface.len(),
        compare_defs: compare_instruction.implicit_defs.len(),
    })
}

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: one step per block plus one per instruction
/// across the plan, two scans of the reconstructed function's blocks for
/// the producer audit, and the shared walk's setup and fixpoint bound
/// charged per walked unit — the branch's use roster plus the whole flag
/// universe, so the bound stays independent of the outcome split.
fn measured_steps(
    plan: &SelectedInstructionPlan,
    reconstructed: &Reconstructed<'_>,
) -> Result<u64, BoundaryBranchError> {
    let function = reconstructed.function;
    let function_scan = function
        .blocks
        .iter()
        .try_fold(0usize, |total, block| {
            total.checked_add(block.instructions.len())?.checked_add(1)
        })
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let edge_count = reconstructed
        .successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let block_count = function.blocks.len();
    // Producer scans walk the whole function once per compared register —
    // two at most. The shared walk's setup resolves every edge's target
    // index, fills the predecessor lists, and marks the backward cone.
    // Each walked unit pays its partition checks — flag-universe and
    // jump-surface membership and the compare-definitions lookup — plus
    // the block-prefix scan and — on an in-block miss — every block's
    // stream for its last event, after which entry-set propagation
    // requeues a block only while its set grows: a set holds at most one
    // element per event site plus the unknown marker, so pops stay under
    // `blocks × (elements + 1)` and each pop visits its out-edges — no
    // more than the widest terminator's — at a bounded union cost.
    // Charging the collapse's full flag-universe walk on top of the
    // branch use roster keeps the bound independent of the outcome split.
    let elements = function_scan
        .checked_add(1)
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let pops = block_count
        .checked_mul(
            elements
                .checked_add(1)
                .ok_or(BoundaryBranchError::IdentityOverflow)?,
        )
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let widest_out = reconstructed
        .successors
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
                reconstructed
                    .flag_universe
                    .checked_add(reconstructed.jump_surface)?
                    .checked_add(reconstructed.compare_defs)?,
            )
        })
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let walk_setup = edge_count
        .checked_mul(
            block_count
                .checked_add(1)
                .ok_or(BoundaryBranchError::IdentityOverflow)?,
        )
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .and_then(|total| total.checked_add(reconstructed.flag_universe))
        .and_then(|total| total.checked_add(reconstructed.jump_surface))
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    let reach_scan = reconstructed
        .implicit_uses
        .checked_add(reconstructed.flag_universe)
        .and_then(|total| total.checked_mul(per_unit))
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
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
        .ok_or(BoundaryBranchError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| BoundaryBranchError::IdentityOverflow)
}

/// Build the terminator the contract demands from the validator's own
/// record: a decided predicate becomes the target's own jump row — zero
/// operands by the audit — carrying the selected successor record
/// verbatim while the branch's instruction identity and provenance stay
/// with the terminator; a near-pole collapse keeps the source
/// instruction whole — the conditional-branch kinds share one row — and
/// swaps only its kind field for `ConditionalBranchNonZero`,
/// republishing `when_less`/`when_not_less` as `when_nonzero`/`when_zero`.
/// The producer's `rewritten` constructor is not consulted; both sides
/// derive the same terminator from the source alone.
fn expected(reconstructed: &Reconstructed<'_>) -> SelectedTerminator {
    let block = &reconstructed.function.blocks[reconstructed.block_index];
    let (when_less, when_not_less) = match &block.terminator {
        SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } => (when_less, when_not_less),
        _ => unreachable!("reconstruction gated the terminator variant"),
    };
    match reconstructed.outcome {
        Outcome::Constant(holds) => {
            let row = reconstructed.row.expect("constant outcome carries its row");
            SelectedTerminator::Jump {
                instruction: SelectedInstruction {
                    id: reconstructed.branch_id,
                    kind: SelectedInstructionKind::Jump,
                    constraint: row.key,
                    operands: Vec::new(),
                    implicit_uses: row.implicit_uses.clone(),
                    implicit_defs: row.implicit_defs.clone(),
                    clobbers: row.clobbers.clone(),
                    provenance: reconstructed.provenance.clone(),
                },
                successor: if holds {
                    when_less.clone()
                } else {
                    when_not_less.clone()
                },
            }
        }
        Outcome::NonZero => {
            let mut instruction = terminator_instruction(&block.terminator).clone();
            instruction.kind = SelectedInstructionKind::ConditionalBranchNonZero;
            SelectedTerminator::ConditionalBranch {
                instruction,
                when_nonzero: when_less.clone(),
                when_zero: when_not_less.clone(),
            }
        }
    }
}

/// Reinsert the source terminator: undoing the validator's expected edit
/// must restore the complete source by content — every other
/// instruction, register, roster row, call, and settlement included.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
    function_index: usize,
) -> Result<SelectedInstructionPlan, BoundaryBranchError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(function_index)
        .ok_or(BoundaryBranchError::ReplayMismatch)?;
    let block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(BoundaryBranchError::ReplayMismatch)?;
    block.terminator = reconstructed.function.blocks[reconstructed.block_index]
        .terminator
        .clone();
    Ok(restored)
}

/// Independently consume the proposed program: the validator reconstructs
/// the fold's preconditions from the source, requires the proposed
/// terminator in the branch's block to equal the form its own record
/// produces — the `Jump` a decided pole permits or the
/// `ConditionalBranchNonZero` collapse a near pole permits — and restores
/// the complete source by content: every other instruction, register,
/// roster row, call, and settlement included. The producer's
/// `admission::admit` is never consulted, so a wrong legality decision
/// fails here even when the proposal matches the edit the producer
/// emitted.
pub fn validate_boundary_branch_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    branch: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedBoundaryBranch, BoundaryBranchError> {
    let reconstructed = reconstruct(source, function_index, branch, environment)?;
    if measured_steps(source.selected_plan(), &reconstructed)? > budget.validation_steps() {
        return Err(BoundaryBranchError::WorkBudgetExceeded);
    }
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.block_index))
        .map(|block| &block.terminator)
        != Some(&expected(&reconstructed))
    {
        return Err(BoundaryBranchError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed, function_index)? != *source.selected_plan() {
        return Err(BoundaryBranchError::ReplayMismatch);
    }
    Ok(ValidatedBoundaryBranch {
        receipt: BoundaryBranchReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
