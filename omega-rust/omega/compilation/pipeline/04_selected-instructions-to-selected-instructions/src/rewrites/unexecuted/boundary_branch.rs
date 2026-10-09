//! Boundary-decided conditional-branch folding on the selected CFG.
//!
//! `rewrites/constant_branch` jumps a conditional-branch terminator whose
//! flag units resolve to a compile-time-constant compare, and
//! `rewrites/boundary_boolean` is the pole-operand twin on the materialized
//! side: a `MaterializeBoolean*` ordering reader whose compare carries one
//! operand at a carrier-domain pole is decided — or collapsed to equality —
//! by that pole alone. The same pole-decided flag state can feed the other
//! strict-ordering flag reader in the selected catalog — a
//! `ConditionalBranchU64LessThan` or `ConditionalBranchI64LessThan`
//! terminator — where a far pole (`x < 0` unsigned, `x < i64::MIN` signed,
//! `u64::MAX < x`, `i64::MAX < x`) names a branch that never takes, and a
//! near pole (`0 < x` unsigned, `x < u64::MAX`, `i64::MIN < x`,
//! `x < i64::MAX`) is exactly the nonzero condition on the identical
//! published flag state.
//!
//! This rewrite is that compare/test selection at full selected-CFG
//! standing. The branch's implicit uses partition under the flag universe
//! the target's three compare rows publish, exactly as the constant fold
//! partitions them: flag units must resolve to one compare through the
//! shared `condition_state` entry-event walk read at the terminator
//! position and must be among that compare's published definitions, and
//! every other observed unit must lie in the jump row's implicit surface.
//! `condition_state::boundary_operands` then resolves each compare operand
//! to the bit pattern its unique `MaterializeI64` producer guarantees —
//! `CompareI64Immediate` carrying its encoded literal, `CompareI64Zero`
//! reading as `left - 0` — keeping the side nothing pins open. When both
//! sides resolve the predicate is decided outright and the terminator
//! becomes the target's own `Jump` carrying the selected successor record
//! verbatim; when a far pole alone decides the predicate false the same
//! `Jump` lands on `when_not_less`. A near pole instead collapses the
//! ordering to the nonzero condition (`0 < x` unsigned holds exactly when
//! `x != 0`, and `x < u64::MAX` when `x != u64::MAX`; likewise `i64::MIN <
//! x` and `x < i64::MAX` signed), so the terminator rewrites to
//! `ConditionalBranch` carrying `ConditionalBranchNonZero` with
//! `when_less` republished as `when_nonzero` and `when_not_less` as
//! `when_zero` — the conditional-branch kinds share one constraint row, so
//! the rebuilt instruction keeps the branch's constraint, operands,
//! implicit uses, definitions, clobbers, identity, and provenance while
//! reading the equality condition the same compare already publishes.
//! Because the collapsed reader can observe any flag unit its kind's
//! encoding implies, the collapse admits only when every flag-universe unit
//! — not only the ones the source branch declared — resolves to that
//! compare at the terminator position. A `ConditionalBranch` carrying
//! `ConditionalBranchNonZero` reads the equality condition itself, which
//! no single pole decides, and stays refused; so does a compare reading
//! the same register twice, the identity case the constant family already
//! owns.
//!
//! Either way the compare stays published for every other reader — boolean
//! materializations and sibling branches included — and a `Jump` fold's
//! dropped edge never executes, so nothing it carried needs preserving.
//! Validation consumes the proposed program, requires the terminator in
//! the branch's block to equal the independently computed form — the
//! `Jump` a decided pole permits or the `ConditionalBranchNonZero`
//! collapse a near pole permits — and restores the complete source by
//! content: every other instruction, register, roster row, call, and
//! settlement is retained bit-identical. The validator re-derives the
//! fold's preconditions on its own audit — the branch/kind pairing and
//! zero-operand shape, the flag partition every implicit use must
//! satisfy, the pole-outcome table deciding the outcome, and the
//! outcome-scoped surface requirements — never consulting the producer's
//! `admission` routine; only the module's shared condition-state walk and
//! boundary-operand audit are common to both sides.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{
    SelectedInstruction, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, SelectedTerminator,
};
use semantic_vocabulary::FuelScheduleIdentity;

pub(crate) use validation::validate_boundary_branch_fold;

/// An accepted boundary-decided conditional-branch fold with its replay
/// receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedBoundaryBranch {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: BoundaryBranchReceipt,
}

validated_transformed_receipt!(ValidatedBoundaryBranch, BoundaryBranchReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundaryBranchReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

receipt_selected_identities!(BoundaryBranchReceipt);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundaryBranchError {
    SourceMismatch,
    /// Not the emitted zero-operand strict-ordering flag reader: a
    /// non-branch terminator, a `ConditionalBranch`/`ConditionalBranchNonZero`
    /// pair reading the equality condition no pole decides, a variant/kind
    /// pairing that does not match, explicit operands, or a use roster
    /// holding no flag unit.
    UnsupportedInstruction,
    /// A flag observation the boundary fold cannot reproduce: a used flag
    /// unit whose reaching event — in-block or through the predecessor
    /// walk — is a clobber, a non-compare definition, or a different
    /// instruction than its siblings; a used unit whose reaching set holds
    /// several events, the entry unknown, or none at all; a unit the
    /// resolved compare does not publish; or a non-flag use the jump row's
    /// implicit surface does not carry. For the collapse outcome, any
    /// flag-universe unit that fails the same resolution also refuses.
    UnsupportedUse,
    /// The resolved compare carries no operand pole that decides the
    /// predicate: both sides unknown, a non-pole literal, or the identity
    /// `register - register` case the constant family owns.
    UnsupportedLiteral,
    /// The jump row the decided fold rebuilds on — or the
    /// conditional-branch row the collapse rebuilds on — does not match
    /// the surface the source branch carried.
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for BoundaryBranchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid boundary branch fold: {self:?}")
    }
}

impl std::error::Error for BoundaryBranchError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Outcome {
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

/// The branch/kind pairings admission accepts and the carrier domain their
/// predicate orders over: `true` selects the signed poles, `false` the
/// unsigned. `ConditionalBranch` carrying `ConditionalBranchNonZero` reads
/// the equality condition — `left != right` varies with every unknown
/// side, so no single operand pole can fix it — and stays refused.
pub(super) fn branch_domain(
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
/// less-than issued from the domain maximum. Two known operands decide the
/// predicate outright, matching the constant family's table. The near
/// poles leave a predicate that coincides with the nonzero condition on
/// the same flag state — `0 <u x` is `x != 0`, `x <u u64::MAX` is `x !=
/// u64::MAX`, `i64::MIN <s x` is `x != i64::MIN`, `x <s i64::MAX` is `x !=
/// i64::MAX` — so the branch collapses to `ConditionalBranchNonZero` with
/// `when_less` serving `when_nonzero`. Decided outcomes take precedence
/// where two rules could fire: `(0, u64::MAX)` under `U64LessThan` is
/// `true` rather than a collapse that computes the same selection.
pub(super) fn outcome(signed: bool, left: Option<u64>, right: Option<u64>) -> Option<Outcome> {
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

/// The constant collapse's surface agreement: the jump row that replaces the
/// branch's must observe every non-flag unit the branch read and carry the
/// same definition and clobber sets — the audit both the admission and the
/// replay run before keeping the jump row.
pub(super) fn constant_row<'a>(
    jump_row: &'a register_model::RegisterInstructionConstraint,
    branch_instruction: &selected_instructions::SelectedInstruction,
    flag_universe: &std::collections::BTreeSet<register_model::RegisterUnitId>,
    jump_surface: &std::collections::BTreeSet<register_model::RegisterUnitId>,
) -> Result<Option<&'a register_model::RegisterInstructionConstraint>, BoundaryBranchError> {
    use std::collections::BTreeSet;
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
    Ok(Some(jump_row))
}

/// The nonzero collapse's flag resolution: every flag-universe unit must
/// reach from the same compare at this position and be among its published
/// definitions — the audit both the admission and the replay run.
pub(super) fn nonzero_flag_resolution(
    function: &selected_instructions::SelectedFunction,
    entry: usize,
    successors: &[Vec<usize>],
    cone: &[bool],
    block_index: usize,
    read_position: usize,
    flag_universe: &std::collections::BTreeSet<register_model::RegisterUnitId>,
    branch_instruction: &selected_instructions::SelectedInstruction,
    compare_instruction: &selected_instructions::SelectedInstruction,
    compare_site: crate::rewrites::condition_state::EventSite,
    keys: &selected_instructions::SelectedConstraintKeys,
) -> Result<(), BoundaryBranchError> {
    use crate::rewrites::condition_state::reaching_event;
    if branch_instruction.constraint != keys.conditional_branch {
        return Err(BoundaryBranchError::ConstraintMismatch);
    }
    for unit in flag_universe {
        let event = reaching_event(
            function,
            entry,
            successors,
            cone,
            block_index,
            read_position,
            *unit,
        )
        .map_err(|_| BoundaryBranchError::UnsupportedUse)?;
        if event != compare_site || !compare_instruction.implicit_defs.contains(unit) {
            return Err(BoundaryBranchError::UnsupportedUse);
        }
    }
    Ok(())
}

/// The site-and-outcome derive the admission producer and the replay
/// validator compute identically: locate the terminator-carried
/// instruction, check the flag-reader shape, partition the implicit uses
/// under the flag universe, require the one compare site whose reaching
/// events cover every used unit, and decide the outcome on the compare's
/// operand poles. `map` converts the shared walk's own rejection — each
/// caller names its conversion so the other's cannot relabel it.
pub(super) struct BoundaryBranchDerivation<'source> {
    pub(super) block_index: usize,
    pub(super) provenance: selected_instructions::SelectedInstructionProvenance,
    pub(super) implicit_uses: usize,
    pub(super) flag_universe: std::collections::BTreeSet<register_model::RegisterUnitId>,
    pub(super) jump_surface: std::collections::BTreeSet<register_model::RegisterUnitId>,
    pub(super) successors: Vec<Vec<usize>>,
    pub(super) compare_defs: usize,
    pub(super) outcome: Outcome,
    pub(super) row: Option<&'source register_model::RegisterInstructionConstraint>,
}

pub(super) fn derive_branch_fold<'source>(
    function: &'source selected_instructions::SelectedFunction,
    branch: selected_instructions::SelectedInstructionId,
    environment: &'source register_environment::ValidatedTargetRegisterEnvironment,
    map: impl Fn(crate::rewrites::condition_state::ConditionStateError) -> BoundaryBranchError + Copy,
) -> Result<BoundaryBranchDerivation<'source>, BoundaryBranchError> {
    use crate::rewrites::condition_state::{
        adjacency, backward_cone, boundary_operands, entry_index, instruction_at, reaching_event,
    };
    use selected_instructions::terminator_instruction;

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
    // reader: the variant/kind pairing must match, explicit operands are
    // never present, and a use roster holding no flag unit has no
    // condition to decide.
    let signed = branch_domain(&block.terminator, branch_instruction)
        .ok_or(BoundaryBranchError::UnsupportedInstruction)?;
    if !branch_instruction.operands.is_empty() || branch_instruction.implicit_uses.is_empty() {
        return Err(BoundaryBranchError::UnsupportedInstruction);
    }
    let keys = environment.selected_keys();
    let mut flag_universe = std::collections::BTreeSet::new();
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
    let jump_surface: std::collections::BTreeSet<register_model::RegisterUnitId> = jump_row
        .implicit_uses
        .iter()
        .chain(jump_row.implicit_defs.iter())
        .chain(jump_row.clobbers.iter())
        .copied()
        .collect();
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(BoundaryBranchError::SourceMismatch)?;
    let cone = backward_cone(&predecessors, block_index);
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
        .map_err(map)?;
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
    let (left, right) = boundary_operands(function, compare_instruction).map_err(map)?;
    let outcome = outcome(signed, left, right).ok_or(BoundaryBranchError::UnsupportedLiteral)?;
    let row = match outcome {
        Outcome::Constant(_) => {
            constant_row(jump_row, branch_instruction, &flag_universe, &jump_surface)?
        }
        Outcome::NonZero => {
            nonzero_flag_resolution(
                function,
                entry,
                &successors,
                &cone,
                block_index,
                read_position,
                &flag_universe,
                branch_instruction,
                compare_instruction,
                compare_site,
                &keys,
            )?;
            None
        }
    };
    Ok(BoundaryBranchDerivation {
        block_index,
        provenance: branch_instruction.provenance.clone(),
        implicit_uses: branch_instruction.implicit_uses.len(),
        flag_universe,
        jump_surface,
        successors,
        compare_defs: compare_instruction.implicit_defs.len(),
        outcome,
        row,
    })
}
