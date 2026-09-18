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
//! Proposal and independent replay share only the admission predicates and
//! the rebuilt-terminator constructor. Validation re-derives the admitted
//! branch from the source, requires the proposed terminator to equal the
//! reconstructed form, and restores the complete source by content: every
//! other instruction, register, roster row, call, and settlement is
//! retained bit-identical.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::fold_selected_boundary_branch;
pub use validation::validate_boundary_branch_fold;

#[cfg(test)]
mod tests;

/// An accepted boundary-decided conditional-branch fold with its replay
/// receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedBoundaryBranch {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: BoundaryBranchReceipt,
}

impl ValidatedBoundaryBranch {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &BoundaryBranchReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryBranchReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl BoundaryBranchReceipt {
    pub const fn source_selected(&self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn transformed_selected(&self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn optimization_unit(&self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(&self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryBranchError {
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
