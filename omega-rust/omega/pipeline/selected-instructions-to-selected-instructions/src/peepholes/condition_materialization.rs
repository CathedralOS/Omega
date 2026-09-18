//! Decided condition-state materialization folding through declarative
//! pairs.
//!
//! The instruction-pair descriptors under `rewrites/selected_lowering`
//! declare consumers that carry the folded register in an explicit operand,
//! and the terminator-pair descriptors under `peepholes` declare the
//! flag-reading consumer a block ends on. Between them sits the remaining
//! flag-consuming form: the `MaterializeBoolean*` instruction, a mid-block
//! consumer whose whole input arrives through implicit physical units and
//! whose only operand is the `Def` result — no `Use` operand exists for a
//! literal to occupy, so the operand-carrying grammar cannot name it and
//! the terminator grammar cannot reach it.
//!
//! The [`ConditionMaterializationRule`](pair::ConditionMaterializationRule)
//! table under `condition_materialization/pair` declares that consumer
//! position: each rule pairs one condition-state producer kind —
//! `CompareI64`, `CompareI64Immediate`, or `CompareI64Zero` — with one
//! `MaterializeBoolean*` consumer kind and the `MaterializeI64` rewrite
//! publishing the decided zero or one, under the shared
//! `ConditionOperandResolution` producer grammars, the family's
//! `ConditionMaterializationUnitFlow`, and its
//! `ConditionMaterializationEffects` surface.
//!
//! The declared family is the decided condition materialization. A
//! `MaterializeBooleanEqual`, `MaterializeBooleanU64LessThan`,
//! `MaterializeBooleanI64LessThan`, `MaterializeBooleanU64LessOrEqual`, or
//! `MaterializeBooleanI64LessOrEqual` instruction is admitted when every
//! flag-universe unit it implicitly uses resolves to the same producer as
//! that unit's last event on every path — the shared `condition_flow`
//! least-fixpoint walk, with the read position at the consumer's own body
//! index rather than the terminator's — and the producer's record defines
//! every used unit. The producer's operands then decide the predicate under
//! the shared operand grammars, exactly as the terminator family's branches
//! decide their edge: `left == right` materializes one for the equality
//! reader, the unsigned and signed orderings materialize one when theirs
//! holds.
//!
//! Under `ConditionMaterializationUnitFlow::ConditionStateResolved` the
//! flag uses retire — the compile-time decision replaces the observation —
//! under the same republish contract the terminator family declares: every
//! non-flag unit the consumer used must stay read on the rewritten row, and
//! the rewritten row republishes the consumer's implicit definitions and
//! clobbers verbatim. The `MaterializeI64` row carries no implicit traffic
//! at all, so a consumer using anything outside the flag universe cannot
//! fold. Under `ConditionMaterializationEffects::Isolated` both catalog
//! declarations are effect-isolated fall-through rows — the encoded flag
//! read is the only unit traffic either side carries — so the rewrite
//! touches no memory, stack, trap, or control surface.
//!
//! The rewritten `MaterializeI64` keeps the consumer's identity,
//! provenance, and `Def` operand record — the result register is the same
//! home — under the target's own materialize row. The producer stays: its
//! flag definitions remain published for every other reader the function
//! still holds — a branch terminator can observe the same compare while the
//! materialization folds — and producer elimination stays with the
//! producer-elimination rules.
//!
//! The descriptor declares admissibility; it never certifies it. The
//! producer consults its own rule and axes in `admission`; the independent
//! replay in `replay` re-derives the consumer grammar, the flag-unit
//! resolution, the operand constants, and the decided value from the
//! instruction records alone — never consulting the pair table — and
//! restores the complete source by content.

mod admission;
mod pair;
mod replay;
mod rewrite;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use replay::validate_condition_materialization_fold;
pub use rewrite::fold_selected_condition_materialization;

#[cfg(test)]
mod tests;

/// An accepted condition-materialization fold with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedConditionMaterialization {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: ConditionMaterializationReceipt,
}

impl ValidatedConditionMaterialization {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &ConditionMaterializationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionMaterializationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl ConditionMaterializationReceipt {
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
pub enum ConditionMaterializationError {
    SourceMismatch,
    /// Not an emitted single-`Def` condition-state reader: an instruction
    /// kind no declared consumer names, a `Use` operand — a materialization
    /// carrying one is not the form the family declares — a `Def` operand
    /// at another position or carrying a unit binding, or a record reading
    /// no unit at all — nothing observed means no predicate to decide.
    UnsupportedConsumer,
    /// A flag observation the fold cannot reproduce: a used flag unit whose
    /// reaching event — in-block or through the predecessor walk — is a
    /// clobber, a non-producer definition, or a different instruction than
    /// its siblings; a used unit whose reaching set holds several events,
    /// the entry unknown, or none at all; a used unit outside the flag
    /// universe — the `MaterializeI64` row reads nothing, so any non-flag
    /// use refuses; a producer that does not publish every used flag unit;
    /// or a rewritten row that does not republish the consumer's implicit
    /// definitions and clobbers verbatim.
    UnsupportedUse,
    /// The instruction the flag uses resolve to is not a producer kind any
    /// declared rule pairs with this consumer.
    UnsupportedProducer,
    /// A producer operand the declared resolution grammar cannot make
    /// constant: a register with no producer or several, a producer that is
    /// not a clean `MaterializeI64`, a decorated operand, or a literal that
    /// does not fit the published width.
    UndecidedOperands,
    /// The consumer's or the rewritten form's constraint row is missing
    /// from the bound environment or does not declare the single-`Def`
    /// shape the pair requires.
    ConstraintMismatch,
    /// The bound effect catalog does not declare the consumer or rewritten
    /// form under the record's constraint key, or the declarations do not
    /// satisfy the family's isolated effect surface — an effect-isolated
    /// fall-through materialization replacing an effect-isolated
    /// fall-through condition read.
    EffectSurfaceMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for ConditionMaterializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid condition materialization fold: {self:?}"
        )
    }
}

impl std::error::Error for ConditionMaterializationError {}
