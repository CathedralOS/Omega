//! Constant condition branch folding on the selected CFG.
//!
//! `rewrites/constant_boolean` is the consumer-side twin: a
//! `MaterializeBoolean*` whose flag units all reach one constant-operand
//! compare becomes a `MaterializeI64` of the decided predicate. The same
//! constant flag state can feed the other flag reader in the selected
//! catalog — a `ConditionalBranch` terminator — where the decided
//! predicate does not name a value but an edge: the branch always takes
//! the same successor, so the terminator is a `Jump` carrying that
//! successor record verbatim and the untaken edge leaves the plan.
//!
//! This rewrite is that fold. A `ConditionalBranch`,
//! `ConditionalBranchU64LessThan`, or `ConditionalBranchI64LessThan`
//! terminator is admitted when its carried instruction is the emitted
//! zero-operand flag reader — no explicit operands — and every implicit
//! use it declares resolves under the flag partition: a use inside the
//! flag universe — the union of the target's `CompareI64`,
//! `CompareI64Immediate`, and `CompareI64Zero` rows' implicit
//! definitions — must reach the one compare on every path, through the
//! shared `condition_state` entry-event walk with the read position at the
//! terminator (`block.instructions.len()`), and must be among that
//! compare's published definitions; a use outside the flag universe — the
//! branch's program-counter read — must lie inside the jump row's own
//! implicit surface, so the rebuilt terminator still observes every unit
//! the branch did apart from the decided flags. The jump row must
//! republish exactly the branch's implicit definitions and clobbers: a
//! unit definition that disappeared would hand downstream readers an
//! older event, and a new one would publish state the source never had.
//! With the partition satisfied, `condition_state::constant_operands`
//! decides `left - right` at compile time and the branch kind selects the
//! edge: `ConditionalBranchNonZero` takes `when_nonzero` exactly when
//! `left != right`, `ConditionalBranchU64LessThan` takes `when_less` when
//! `left < right` unsigned, and `ConditionalBranchI64LessThan` when the
//! signed comparison holds — matching the successor order selection
//! established.
//!
//! The `Jump` terminator's carried instruction keeps the branch's
//! identity and provenance under the target's own `jump` row, and the
//! decided `SelectedSuccessor` moves across unchanged — bindings,
//! structural payloads, case custody, and fuel included. The dropped edge
//! never executes, so nothing it carried needs preserving, and every
//! surviving path executes the same instruction and edge prefix it did
//! before, so no boundary settlement observes a changed prefix and no
//! settlement audit is needed. The compare itself is retained: boolean
//! materializations and other branches can still observe the same flag
//! definitions, and producer elimination stays with the
//! producer-elimination rules.
//!
//! Validation consumes the proposed program, requires the terminator in
//! the branch's block to equal the independently computed `Jump`, and
//! restores the complete source by content: every other instruction,
//! register, roster row, call, and settlement is retained bit-identical.
//! The validator re-derives the fold's preconditions on its own audit —
//! the branch/kind pairing and zero-operand shape, the flag partition
//! every implicit use must satisfy, the jump row's exact implicit-surface
//! match, and the constant operands the successor is decided from —
//! never consulting the producer's `admission` routine; only the
//! module's shared condition-state walk and operand audit are common to
//! both sides.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::fold_selected_constant_branch;
pub use validation::validate_constant_branch_fold;

#[cfg(test)]
mod tests;

/// An accepted constant condition branch fold with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedConstantBranch {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: ConstantBranchReceipt,
}

impl ValidatedConstantBranch {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &ConstantBranchReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstantBranchReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl ConstantBranchReceipt {
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
pub enum ConstantBranchError {
    SourceMismatch,
    /// Not the emitted zero-operand conditional-branch flag reader: a
    /// non-branch terminator, a branch/kind pairing that does not match,
    /// explicit operands, or a use roster holding no flag unit — a branch
    /// observing no condition state has no predicate to decide.
    UnsupportedInstruction,
    /// A flag observation the constant fold cannot reproduce: a used flag
    /// unit whose reaching event — in-block or through the predecessor
    /// walk — is a clobber, a non-compare definition, or a different
    /// instruction than its siblings; a used unit whose reaching set holds
    /// several events, the entry unknown, or none at all; or a used unit
    /// outside the flag universe that the jump row's implicit surface does
    /// not carry.
    UnsupportedUse,
    UnsupportedProducer,
    UnsupportedLiteral,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for ConstantBranchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid constant branch fold: {self:?}")
    }
}

impl std::error::Error for ConstantBranchError {}
