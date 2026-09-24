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
use crate::rewrites::unexecuted::condition_state::{
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

#[cfg(test)]
mod independence_tests {
    use std::sync::Arc;

    use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
    use optimization_unit::ValueDefinitionSite;
    use register_environment::{
        ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
    };
    use register_model::{RegisterInstructionConstraint, RegisterUnitId};
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
        SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator, VirtualRegister,
        VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
        ScalarType, ValueId,
    };
    use target::NativeTarget;
    use target_operations_to_selected_instructions::selected_instruction_plan_identity;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::{
        BoundaryBranchError, BoundaryBranchReceipt, ValidatedBoundaryBranch,
        validate_boundary_branch_fold,
    };

    const MATERIALIZE_A: SelectedInstructionId = SelectedInstructionId(2);
    const MATERIALIZE_B: SelectedInstructionId = SelectedInstructionId(3);
    const COMPARE: SelectedInstructionId = SelectedInstructionId(4);
    const BRANCH: SelectedInstructionId = SelectedInstructionId(5);
    const TERMINAL: SelectedInstructionId = SelectedInstructionId(6);

    const LITA: VirtualRegisterId = VirtualRegisterId(0);
    const LITB: VirtualRegisterId = VirtualRegisterId(1);
    const PARAM: VirtualRegisterId = VirtualRegisterId(2);

    fn budget() -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(100, 100, 100_000, 100, 100).unwrap()
    }

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

    fn arm(block: SelectedBlockId, target: u64, edge: u64) -> SelectedSuccessor {
        SelectedSuccessor {
            role: SelectedSuccessorRole::Semantic,
            psi_edge: EdgeId::new(edge).unwrap(),
            block,
            source_target: BlockId::new(target).unwrap(),
            bindings: Vec::new(),
            structural_bindings: Vec::new(),
            structural_case: None,
            fuel: Vec::new(),
        }
    }

    /// The proposal a defective producer emits for the named branch:
    /// either the `Jump` a decided pole permits, or the
    /// `ConditionalBranchNonZero` collapse a near pole permits — emitted
    /// whether or not the fold's legality actually holds.
    enum Forged {
        /// `Jump` on the target's own jump row carrying the `when_less`
        /// arm when `holds`, `when_not_less` otherwise — the decided
        /// fold's edit.
        Jump(bool),
        /// The branch kept whole with only its kind rewritten to
        /// `ConditionalBranchNonZero` and the arms republished — the
        /// near-pole collapse's edit.
        Collapse,
    }

    /// `ra = materialize left; rb = materialize right; compare lo, ro;`
    /// then a `ConditionalBranchU64LessThan` terminator on the published
    /// flags to two returning blocks. `None` for either compare side reads
    /// the entry parameter `PARAM`, so the pole audit leaves that side
    /// open: `x <u 0` is the legal decided fold and `0 <u x` the legal
    /// collapse. `edit` then breaks one legality premise, and the returned
    /// proposal is what a defective producer would emit anyway — the
    /// branch rewritten though the poles no longer decide it.
    fn forged(
        target: NativeTarget,
        left: Option<u64>,
        right: Option<u64>,
        emitted: Forged,
        edit: impl FnOnce(&mut SelectedFunction, &ValidatedTargetRegisterEnvironment),
    ) -> (
        ValidatedBoundaryBranch,
        ValidatedTargetRegisterEnvironment,
        SelectedInstructionPlan,
    ) {
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.selected_keys();
        let materialize_row = environment.constraint(keys.materialize_i64).unwrap();
        let compare_row = environment.constraint(keys.compare_i64).unwrap();
        let branch_row = environment.constraint(keys.conditional_branch).unwrap();
        let return_row = environment.constraint(keys.return_unit).unwrap();
        let class = compare_row.operands[0].class;
        let machine = MachineId::new(1).unwrap();
        let register = |id: VirtualRegisterId, origin: VirtualRegisterOrigin| VirtualRegister {
            id,
            scalar_type: u64_scalar(),
            class,
            origin,
            definition_site: None,
            entry_fixed_view: None,
        };
        let function = SelectedFunction {
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
                register(
                    LITA,
                    VirtualRegisterOrigin::InstructionResult {
                        instruction: MATERIALIZE_A,
                        source_value: ValueId::new(2).unwrap(),
                    },
                ),
                register(
                    LITB,
                    VirtualRegisterOrigin::InstructionResult {
                        instruction: MATERIALIZE_B,
                        source_value: ValueId::new(3).unwrap(),
                    },
                ),
                VirtualRegister {
                    id: PARAM,
                    scalar_type: u64_scalar(),
                    class,
                    origin: VirtualRegisterOrigin::EntryParameter {
                        source_value: ValueId::new(4).unwrap(),
                        parameter_index: 0,
                    },
                    definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
                    entry_fixed_view: None,
                },
            ],
            blocks: vec![
                SelectedBlock {
                    id: SelectedBlockId(0),
                    origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                    instructions: vec![
                        instruction(
                            MATERIALIZE_A,
                            SelectedInstructionKind::MaterializeI64 {
                                value: IntegerValue::Unsigned(u128::from(left.unwrap_or(9))),
                            },
                            materialize_row,
                            &[LITA],
                        ),
                        instruction(
                            MATERIALIZE_B,
                            SelectedInstructionKind::MaterializeI64 {
                                value: IntegerValue::Unsigned(u128::from(right.unwrap_or(9))),
                            },
                            materialize_row,
                            &[LITB],
                        ),
                        instruction(
                            COMPARE,
                            SelectedInstructionKind::CompareI64,
                            compare_row,
                            &[
                                left.map(|_| LITA).unwrap_or(PARAM),
                                right.map(|_| LITB).unwrap_or(PARAM),
                            ],
                        ),
                    ],
                    terminator: SelectedTerminator::ConditionalBranchU64LessThan {
                        instruction: instruction(
                            BRANCH,
                            SelectedInstructionKind::ConditionalBranchU64LessThan,
                            branch_row,
                            &[],
                        ),
                        when_less: arm(SelectedBlockId(1), 2, 2),
                        when_not_less: arm(SelectedBlockId(2), 3, 3),
                    },
                },
                SelectedBlock {
                    id: SelectedBlockId(1),
                    origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
                    instructions: Vec::new(),
                    terminator: SelectedTerminator::Return {
                        instruction: instruction(
                            TERMINAL,
                            SelectedInstructionKind::ReturnUnit,
                            return_row,
                            &[],
                        ),
                        psi_return_edge: EdgeId::new(4).unwrap(),
                    },
                },
                SelectedBlock {
                    id: SelectedBlockId(2),
                    origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
                    instructions: Vec::new(),
                    terminator: SelectedTerminator::Return {
                        instruction: instruction(
                            SelectedInstructionId(7),
                            SelectedInstructionKind::ReturnUnit,
                            return_row,
                            &[],
                        ),
                        psi_return_edge: EdgeId::new(5).unwrap(),
                    },
                },
            ],
        };
        let plan = SelectedInstructionPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target,
            entry: machine,
            functions: vec![function].into(),
        };
        let identity = selected_instruction_plan_identity(&plan);
        let mut source = ValidatedBoundaryBranch {
            receipt: BoundaryBranchReceipt {
                source_selected: identity,
                transformed_selected: identity,
                optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
                fuel_schedule: plan.fuel_schedule,
            },
            transformed: Arc::new(plan),
        };
        edit(
            &mut Arc::make_mut(&mut source.transformed).functions[0],
            &environment,
        );
        let identity = selected_instruction_plan_identity(&source.transformed);
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        // The proposal a defective producer would emit: the branch
        // rewritten though its legality preconditions no longer hold —
        // the `Jump` carrying the selected arm on the jump row, or the
        // collapse republishing the arms under `ConditionalBranchNonZero`,
        // exactly as the producer's `rewritten` builds them.
        let jump_row = environment.constraint(keys.jump).unwrap();
        let source_terminator = &source.transformed().functions[0].blocks[0].terminator;
        let SelectedTerminator::ConditionalBranchU64LessThan {
            instruction: source_instruction,
            when_less,
            when_not_less,
        } = source_terminator
        else {
            unreachable!("the fixture terminator is a ConditionalBranchU64LessThan")
        };
        let terminator = match emitted {
            Forged::Jump(holds) => SelectedTerminator::Jump {
                instruction: SelectedInstruction {
                    id: source_instruction.id,
                    kind: SelectedInstructionKind::Jump,
                    constraint: jump_row.key,
                    operands: Vec::new(),
                    implicit_uses: jump_row.implicit_uses.clone(),
                    implicit_defs: jump_row.implicit_defs.clone(),
                    clobbers: jump_row.clobbers.clone(),
                    provenance: source_instruction.provenance.clone(),
                },
                successor: if holds {
                    when_less.clone()
                } else {
                    when_not_less.clone()
                },
            },
            Forged::Collapse => {
                let mut rewritten = source_instruction.clone();
                rewritten.kind = SelectedInstructionKind::ConditionalBranchNonZero;
                SelectedTerminator::ConditionalBranch {
                    instruction: rewritten,
                    when_nonzero: when_less.clone(),
                    when_zero: when_not_less.clone(),
                }
            }
        };
        let mut proposed = source.transformed().clone();
        proposed.functions[0].blocks[0].terminator = terminator;
        (source, environment, proposed)
    }

    /// The compare's right operand materializes 5, not a pole: `x <u 5`
    /// neither always holds nor never holds, so the predicate stays
    /// runtime-decided. A producer that folded anyway would publish a
    /// plan whose branch is a `Jump` the source never established; the
    /// validator's own pole audit must refuse with `UnsupportedLiteral`,
    /// not merely diff the proposal. Feeding that forged proposal is the
    /// observable proof that validation no longer relies on the
    /// producer's admission routine.
    #[test]
    fn validator_refuses_a_decided_jump_the_pole_does_not_decide() {
        let target = NativeTarget::linux_x64();
        let (source, environment, proposed) =
            forged(target, None, Some(0), Forged::Jump(false), |function, _| {
                function.blocks[0].instructions[1].kind = SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(5),
                };
            });
        assert_eq!(
            validate_boundary_branch_fold(&source, 0, BRANCH, &environment, budget(), proposed)
                .unwrap_err(),
            BoundaryBranchError::UnsupportedLiteral
        );
    }

    /// The same non-pole literal under the collapse: `5 <u x` is neither
    /// a far pole's decided false nor the near pole's nonzero
    /// coincidence, so the rewrite to `ConditionalBranchNonZero` is a
    /// legality error a defective producer could still emit.
    #[test]
    fn validator_refuses_a_collapse_the_pole_does_not_decide() {
        let target = NativeTarget::linux_x64();
        let (source, environment, proposed) =
            forged(target, Some(0), None, Forged::Collapse, |function, _| {
                function.blocks[0].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(5),
                };
            });
        assert_eq!(
            validate_boundary_branch_fold(&source, 0, BRANCH, &environment, budget(), proposed)
                .unwrap_err(),
            BoundaryBranchError::UnsupportedLiteral
        );
    }

    /// A compare reading the same register twice publishes the identity
    /// `ra - ra` state — the constant family's case, carrying no operand
    /// pole at all. A producer that folded it anyway emits a `Jump` the
    /// boundary audit never decided.
    #[test]
    fn validator_refuses_a_fold_over_the_identity_compare() {
        let target = NativeTarget::linux_x64();
        let (source, environment, proposed) =
            forged(target, None, Some(0), Forged::Jump(false), |function, _| {
                let compare = &mut function.blocks[0].instructions[2];
                compare.operands[0].virtual_register = LITA;
                compare.operands[1].virtual_register = LITA;
            });
        assert_eq!(
            validate_boundary_branch_fold(&source, 0, BRANCH, &environment, budget(), proposed)
                .unwrap_err(),
            BoundaryBranchError::UnsupportedLiteral
        );
    }

    /// A clobbered flag unit: the resolved event is the compare site but
    /// the unit is not among its published definitions, so the observed
    /// value is unknown and no pole can decide the branch. The
    /// validator's own publication audit must refuse the forged `Jump`.
    #[test]
    fn validator_refuses_a_fold_over_a_unit_the_compare_does_not_publish() {
        let target = NativeTarget::linux_x64();
        let (source, environment, proposed) =
            forged(target, None, Some(0), Forged::Jump(false), |function, _| {
                let compare = &mut function.blocks[0].instructions[2];
                compare.clobbers = compare.implicit_defs.clone();
                compare.implicit_defs = Vec::new();
            });
        assert_eq!(
            validate_boundary_branch_fold(&source, 0, BRANCH, &environment, budget(), proposed)
                .unwrap_err(),
            BoundaryBranchError::UnsupportedUse
        );
    }

    /// The branch's use roster gains a unit that is neither a flag the
    /// fold decides nor a unit on the jump row's implicit surface — the
    /// rebuilt `Jump` would silently drop the observation. A producer
    /// that skipped the decided fold's surface partition still emits the
    /// `Jump`; the validator's own partition refuses with
    /// `UnsupportedUse`.
    #[test]
    fn validator_refuses_an_observation_the_jump_cannot_carry() {
        let target = NativeTarget::linux_x64();
        let (source, environment, proposed) =
            forged(target, None, Some(0), Forged::Jump(false), |function, _| {
                let instruction = match &mut function.blocks[0].terminator {
                    SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. } => {
                        instruction
                    }
                    _ => unreachable!(),
                };
                instruction.implicit_uses.push(RegisterUnitId(u16::MAX));
            });
        assert_eq!(
            validate_boundary_branch_fold(&source, 0, BRANCH, &environment, budget(), proposed)
                .unwrap_err(),
            BoundaryBranchError::UnsupportedUse
        );
    }

    /// The collapse rebuilds on the shared conditional-branch row: a
    /// branch instruction re-pinned to the jump row's constraint cannot
    /// keep its implicit surface verbatim, so a producer that emitted
    /// the collapse anyway publishes a terminator the contract never
    /// permitted. The validator's own row audit refuses with
    /// `ConstraintMismatch`.
    #[test]
    fn validator_refuses_a_collapse_off_the_branch_row() {
        let target = NativeTarget::linux_x64();
        let (source, environment, proposed) = forged(
            target,
            Some(0),
            None,
            Forged::Collapse,
            |function, environment| {
                let instruction = match &mut function.blocks[0].terminator {
                    SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. } => {
                        instruction
                    }
                    _ => unreachable!(),
                };
                instruction.constraint = environment.selected_keys().jump;
            },
        );
        assert_eq!(
            validate_boundary_branch_fold(&source, 0, BRANCH, &environment, budget(), proposed)
                .unwrap_err(),
            BoundaryBranchError::ConstraintMismatch
        );
    }

    /// The compare is gone: every flag unit the branch reads reaches only
    /// the entry's unknown state. A producer that mistook the read for a
    /// resolved one still emits the `Jump`; the validator's own reaching
    /// walk refuses with `UnsupportedUse`.
    #[test]
    fn validator_refuses_flags_that_never_reached_a_compare() {
        let target = NativeTarget::linux_x64();
        let (source, environment, proposed) =
            forged(target, None, Some(0), Forged::Jump(false), |function, _| {
                function.blocks[0]
                    .instructions
                    .retain(|instruction| instruction.id != COMPARE);
            });
        assert_eq!(
            validate_boundary_branch_fold(&source, 0, BRANCH, &environment, budget(), proposed)
                .unwrap_err(),
            BoundaryBranchError::UnsupportedUse
        );
    }
}
