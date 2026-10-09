//! Boundary-decided predicate materialization on the selected CFG.
//!
//! The constant-flag cluster decides a flag reader when the compare it
//! observes is compile-time constant on both operands
//! (`rewrites/constant_boolean` materializes the decided boolean,
//! `rewrites/constant_branch` jumps the decided edge), and the literal
//! selection folds (the selected-lowering compare pair rules) only change
//! the compare's form. Between those cases a compare whose
//! single known operand sits at a carrier-domain pole still reaches a
//! reader whose predicate the pole decides alone: `x < 0` unsigned and
//! `x < i64::MIN` signed can never hold, `0 <= x` unsigned and
//! `x <= i64::MAX` signed always hold, and `x <= 0` unsigned — like
//! `u64::MAX <= x`, `x <= i64::MIN`, or `i64::MAX <= x` — collapses to
//! equality at the pole. The surviving operand needs no producer at all.
//!
//! This rewrite is that compare/test selection at full selected-CFG
//! standing. A `MaterializeBooleanU64LessThan`,
//! `MaterializeBooleanI64LessThan`, `MaterializeBooleanU64LessOrEqual`, or
//! `MaterializeBooleanI64LessOrEqual` whose every implicit condition-state
//! use resolves to one compare — the last flag event on every path reaching
//! the materialization, through the shared least-fixpoint entry-event walk
//! — is admitted when the compare's operand audit yields a pole that fixes
//! the predicate. The compare is any of the three forms: `CompareI64`
//! resolves each side through its unique `MaterializeI64` producer where
//! one exists, `CompareI64Immediate` carries its encoded literal, and
//! `CompareI64Zero` reads as `left - 0`. A side nothing pins is simply
//! unknown; the pole must decide without it. The strict less-than forms
//! fold at the far pole only — `0 < x` unsigned is `x != 0` and `x < MAX`
//! is `x != MAX`, negations no boolean kind materializes — while the
//! less-or-equal forms at the near pole are exactly the equality condition
//! (`x <= 0` unsigned holds precisely when `x == 0`), so the reader's kind
//! rewrites to `MaterializeBooleanEqual` over the identical published
//! state: the flag-reading kinds share one constraint row, so operands,
//! implicit uses, and clobbers carry over verbatim. A compare reading the
//! same register twice is the identity case the constant family already
//! decides and stays refused here.
//!
//! A decided predicate rewrites the reader to the target's own
//! `MaterializeI64` of the outcome — the same emission the constant fold
//! uses — dropping the flag uses while the compare keeps publishing flag
//! state for every other reader. An equality collapse keeps the reader a
//! flag consumer: only its kind field changes. Either way the instruction
//! keeps its identity, position, result register, and provenance.
//!
//! Validation consumes the proposed program, requires the instruction at
//! the materialization's position to equal the independently computed
//! fold, and restores the complete source by content: every other
//! instruction, register, roster row, call, and settlement is retained
//! bit-identical. The validator re-derives the fold's preconditions on
//! its own audit — the flag-reader shape, the one compare every used
//! unit's reaching event must resolve to, the boundary operand poles the
//! predicate outcome is decided from, and the materialize row a constant
//! fold is built from — never consulting the producer's `admission`
//! routine; only the module's shared condition-state walk and operand
//! audit are common to both sides.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{
    SelectedInstructionKind, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
};
use semantic_vocabulary::FuelScheduleIdentity;

pub(crate) use validation::validate_boundary_boolean_fold;

/// An accepted boundary-decided predicate materialization with its replay
/// receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedBoundaryBoolean {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: BoundaryBooleanReceipt,
}

validated_transformed_receipt!(ValidatedBoundaryBoolean, BoundaryBooleanReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundaryBooleanReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

receipt_selected_identities!(BoundaryBooleanReceipt);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundaryBooleanError {
    SourceMismatch,
    /// The named instruction is not one of the four ordering
    /// `MaterializeBoolean*` kinds in the emitted `[def]` flag-reader
    /// shape — `MaterializeBooleanEqual` reads a condition no pole can
    /// decide.
    UnsupportedInstruction,
    /// The resolved compare carries no operand pole that decides the
    /// reader's predicate: both sides unknown, a non-pole literal, the
    /// identity `register - register` case the constant family owns, or a
    /// strict less-than collapsed to a negation no boolean kind
    /// materializes.
    UnsupportedLiteral,
    /// A flag observation the resolution cannot pin to the compare: a
    /// clobber, a different instruction's event, unknown entry state, an
    /// eventless path, paths that disagree, a unit the compare does not
    /// publish, or a compare operand position that is not a plain use.
    UnsupportedUse,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for BoundaryBooleanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid boundary boolean fold: {self:?}")
    }
}

impl std::error::Error for BoundaryBooleanError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Outcome {
    /// The predicate is decided for every surviving operand value: the
    /// reader becomes the target's own `MaterializeI64` of the constant.
    Constant(bool),
    /// The predicate holds exactly at the pole: `x <= 0` unsigned,
    /// `u64::MAX <= x` unsigned, `x <= i64::MIN` signed, and
    /// `i64::MAX <= x` signed each coincide with the equality condition on
    /// the same published state, so the reader keeps its shape and only
    /// its kind becomes `MaterializeBooleanEqual`.
    Equal,
}

pub(super) fn decidable_kind(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::MaterializeBooleanU64LessThan
            | SelectedInstructionKind::MaterializeBooleanI64LessThan
            | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
            | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
    )
}

/// The boundary decision over the compare's resolved operand poles. The
/// strict orderings fold only at the far pole — a strict less-than against
/// the domain minimum, or from the domain maximum, can never hold — and the
/// remaining pole cases collapse to a negation (`x != bound`) that no
/// boolean materialization kind expresses. The inclusive orderings are
/// tautologies at the near pole and equalities at the far one. Constant
/// outcomes take precedence where two rules could fire: `(0, 0)` under
/// `U64LessOrEqual` is decided `true` rather than rewritten to the equality
/// reader that would compute the same value.
pub(super) fn outcome(
    kind: SelectedInstructionKind,
    left: Option<u64>,
    right: Option<u64>,
) -> Option<Outcome> {
    match kind {
        SelectedInstructionKind::MaterializeBooleanU64LessThan => {
            // `x <u 0` never holds; `u64::MAX <u x` never holds. The
            // remaining poles are `0 <u x` ≡ `x != 0` and `x <u MAX` ≡
            // `x != MAX`, negations no materialization kind selects.
            if right == Some(0) || left == Some(u64::MAX) {
                Some(Outcome::Constant(false))
            } else {
                None
            }
        }
        SelectedInstructionKind::MaterializeBooleanI64LessThan => {
            // `x <s i64::MIN` never holds; `i64::MAX <s x` never holds.
            if right == Some(i64::MIN as u64) || left == Some(i64::MAX as u64) {
                Some(Outcome::Constant(false))
            } else {
                None
            }
        }
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => {
            // `0 <=u x` and `x <=u u64::MAX` always hold; `x <=u 0` and
            // `u64::MAX <=u x` each hold exactly at equality.
            if left == Some(0) || right == Some(u64::MAX) {
                Some(Outcome::Constant(true))
            } else if right == Some(0) || left == Some(u64::MAX) {
                Some(Outcome::Equal)
            } else {
                None
            }
        }
        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            // `i64::MIN <=s x` and `x <=s i64::MAX` always hold;
            // `x <=s i64::MIN` and `i64::MAX <=s x` hold exactly at
            // equality.
            if left == Some(i64::MIN as u64) || right == Some(i64::MAX as u64) {
                Some(Outcome::Constant(true))
            } else if right == Some(i64::MIN as u64) || left == Some(i64::MAX as u64) {
                Some(Outcome::Equal)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// The validation work this audit performs, in the measured-step contract
/// the family publishes: the shared rewrite scan (one step per block
/// and instruction of the function, one per plan function), two scans of the reconstructed function's blocks for
/// the producer audit, and the shared walk's setup and fixpoint bound at
/// one per used flag unit.
pub(super) fn measured_steps(
    plan: &selected_instructions::SelectedInstructionPlan,
    function: &selected_instructions::SelectedFunction,
    successors: &[Vec<usize>],
    implicit_uses: usize,
) -> Result<usize, BoundaryBooleanError> {
    let cone = crate::rewrites::cone_propagation(function, successors, || {
        BoundaryBooleanError::IdentityOverflow
    })?;
    let function_scan = cone.function_scan;
    let per_unit = cone.per_unit_base;
    let walk_setup = cone.walk_setup_base;
    let reach_scan = implicit_uses
        .checked_mul(per_unit)
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or(BoundaryBooleanError::IdentityOverflow)?;
    crate::rewrites::rewrite_scan(plan, function)
        .and_then(|total| {
            total
                .checked_add(function_scan.checked_mul(2)?)?
                .checked_add(reach_scan)
        })
        .ok_or(BoundaryBooleanError::IdentityOverflow)
}

/// The flag-reach and outcome derive the admission producer and the replay
/// validator compute identically: walk each used unit's reaching event on
/// the shared condition-state walk, require the one compare site whose
/// published definitions cover them all, resolve the compare's operand
/// poles, and carry the row contract the outcome demands. `map` converts
/// the shared walk's own rejection — each caller names its conversion so
/// the other's cannot relabel it.
pub(super) struct BoundaryBooleanDerivation<'source> {
    pub(super) successors: Vec<Vec<usize>>,
    pub(super) outcome: Outcome,
    pub(super) row: Option<&'source register_model::RegisterInstructionConstraint>,
}

pub(super) fn derive_boolean_fold<'source>(
    function: &'source selected_instructions::SelectedFunction,
    block_index: usize,
    materialization_index: usize,
    materialization_instruction: &'source selected_instructions::SelectedInstruction,
    result_class: register_model::RegisterClassId,
    environment: &'source register_environment::ValidatedTargetRegisterEnvironment,
    map: impl Fn(crate::rewrites::condition_state::ConditionStateError) -> BoundaryBooleanError + Copy,
) -> Result<BoundaryBooleanDerivation<'source>, BoundaryBooleanError> {
    use crate::rewrites::condition_state::{
        adjacency, backward_cone, boundary_operands, entry_index, instruction_at, reaching_event,
    };

    // The flag walk crosses block boundaries: build the block-indexed
    // adjacency once, locate the entry block whose unseeded condition
    // state bounds every path, and mark the cone of blocks that can reach
    // the materialization's block — only their entry sets can feed the
    // result.
    let (successors, predecessors) = adjacency(function);
    let entry = entry_index(function).ok_or(BoundaryBooleanError::SourceMismatch)?;
    let cone = backward_cone(&predecessors, block_index);
    // Every flag unit the materialization reads must reach from the same
    // compare: on every execution path to the materialization, the last
    // event touching each used unit is that one instruction, and the unit
    // is among its published definitions — a clobber there would leave
    // the observed value unknown.
    let mut compare_site = None;
    for unit in &materialization_instruction.implicit_uses {
        let event = reaching_event(
            function,
            entry,
            &successors,
            &cone,
            block_index,
            materialization_index,
            *unit,
        )
        .map_err(map)?;
        if compare_site.is_some_and(|site| site != event) {
            return Err(BoundaryBooleanError::UnsupportedUse);
        }
        compare_site = Some(event);
    }
    let compare_site = compare_site.ok_or(BoundaryBooleanError::UnsupportedUse)?;
    let compare_instruction = instruction_at(function, compare_site);
    if materialization_instruction
        .implicit_uses
        .iter()
        .any(|unit| !compare_instruction.implicit_defs.contains(unit))
    {
        return Err(BoundaryBooleanError::UnsupportedUse);
    }
    let (left, right) = boundary_operands(function, compare_instruction).map_err(map)?;
    let outcome = outcome(materialization_instruction.kind, left, right)
        .ok_or(BoundaryBooleanError::UnsupportedLiteral)?;
    // A decided predicate rewrites to the target's own materialize row: a
    // single `[def]` at the boolean result's class carrying no unit
    // traffic — the fold drops the flag uses, and any implicit definition
    // or clobber the row added would publish unit state the source
    // instruction never had. The equality collapse needs no row at all:
    // the reader keeps its own — every flag-reading boolean kind shares
    // it.
    let row = match outcome {
        Outcome::Constant(_) => {
            let row = environment
                .constraint(environment.selected_keys().materialize_i64)
                .ok_or(BoundaryBooleanError::ConstraintMismatch)?;
            if row.operands.len() != 1
                || row.operands[0].operand != 0
                || row.operands[0].access != register_model::RegisterOperandAccess::Def
                || row.operands[0].class != result_class
                || row.operands[0].fixed_view.is_some()
                || row.operands[0].tied_to.is_some()
                || row.operands[0].early_clobber
                || !row.implicit_uses.is_empty()
                || !row.implicit_defs.is_empty()
                || !row.clobbers.is_empty()
            {
                return Err(BoundaryBooleanError::ConstraintMismatch);
            }
            Some(row)
        }
        Outcome::Equal => None,
    };
    Ok(BoundaryBooleanDerivation {
        successors,
        outcome,
        row,
    })
}
