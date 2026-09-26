//! Decided condition-state branch folding through declarative terminator
//! pairs.
//!
//! The instruction-pair descriptors under `rewrites/selected_lowering`
//! declare (producer, consumer, rewritten) triples whose consumer carries
//! the folded register in an explicit operand — so they can never name the
//! consumer a block actually ends on: the terminator-carried instruction,
//! whose flag traffic flows through implicit physical units. The
//! [`TerminatorPairRule`](pair::TerminatorPairRule) table under
//! `terminator_pair/pair` extends the same declaration discipline to that
//! position: each rule declares one condition-state producer kind, one
//! conditional-branch consumer kind, the `Jump` rewrite, and the axes the
//! relationship must satisfy — `ConditionOperandResolution` for the
//! producer's operand grammar, `TerminatorPairUnitFlow` for the implicit
//! unit relationship, and `TerminatorPairControlFlow` for the encoded
//! control effect the rewrite carries.
//!
//! The declared family is the decided conditional branch. A
//! `ConditionalBranch`, `ConditionalBranchU64LessThan`, or
//! `ConditionalBranchI64LessThan` terminator is admitted when its carried
//! instruction is the emitted zero-operand flag reader, every flag-universe
//! unit it implicitly uses resolves to the same producer as that unit's
//! last event on every path — the `condition_flow` walk with the read
//! position at the terminator — and the producer's record publishes every
//! used unit. The producer's operands then decide the predicate under the
//! rule's declared operand grammar: a `CompareI64` resolves both register
//! operands to the bits their unique in-function `MaterializeI64` producers
//! publish (or `(0, 0)` outright when both name the same register), a
//! `CompareI64Immediate` resolves operand 0 against the kind's carried
//! literal, and a `CompareI64Zero` against the zero bound. The consumer
//! kind selects the edge: `left != right` takes `when_nonzero`, the
//! unsigned and signed less-than kinds take `when_less` when their ordering
//! holds.
//!
//! Under `TerminatorPairUnitFlow::ConditionStateResolved` the flag uses
//! retire — the compile-time decision replaces the observation — while
//! every non-flag unit the branch uses must stay read on the rewritten row
//! and the branch's implicit definitions and clobbers republish verbatim:
//! the control unit carrying the relative displacement is preserved, and
//! the program counter the branch defined is still defined by the jump.
//! Under `TerminatorPairControlFlow::ConditionalResolvedToUnconditional`
//! the bound effect catalog must declare the consumer's alternatives as
//! `ConditionalRelativeBranchV1` and the jump row's as
//! `UnconditionalRelativeBranchV1`, with both declarations carrying the
//! control-flow barrier and otherwise effect-isolated — no memory, stack,
//! call, or cleanup traffic, and no trap surface beyond the architectural
//! fault a relative-branch encoding can raise — a target whose rows encode
//! anything else cannot admit the pair.
//!
//! The `Jump` terminator's carried instruction keeps the branch's identity
//! and provenance under the target's own `jump` row, and the decided
//! `SelectedSuccessor` moves across unchanged — bindings, structural
//! payloads, case custody, and fuel included. The producer stays: its flag
//! definitions remain published for every other reader the function still
//! holds, and producer elimination stays with the producer-elimination
//! rules.
//!
//! The descriptor declares admissibility; it never certifies it. The
//! producer consults its own rule and axes in `admission`; the independent
//! replay in `replay` re-derives the consumer grammar, the flag-unit
//! resolution, the operand constants, and the decided edge from the
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

pub use replay::validate_terminator_pair_fold;
pub use rewrite::fold_selected_terminator_pair;

#[cfg(test)]
mod tests;

/// An accepted terminator-pair fold with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminatorPair {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: TerminatorPairReceipt,
}

impl ValidatedTerminatorPair {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &TerminatorPairReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminatorPairReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl TerminatorPairReceipt {
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
pub enum TerminatorPairError {
    SourceMismatch,
    /// Not the emitted zero-operand conditional-branch flag reader: a
    /// non-branch terminator, a branch/kind pairing that does not match, an
    /// instruction kind no declared consumer names, or explicit operands —
    /// a branch carrying operands is not the form the family declares.
    UnsupportedTerminator,
    /// A flag observation the fold cannot reproduce: a used flag unit whose
    /// reaching event — in-block or through the predecessor walk — is a
    /// clobber, a non-producer definition, or a different instruction than
    /// its siblings; a used unit whose reaching set holds several events,
    /// the entry unknown, or none at all; a used unit outside the flag
    /// universe that the rewritten row's implicit surface does not carry;
    /// a producer that does not publish every used flag unit; or a
    /// rewritten row that does not republish the consumer's implicit
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
    /// from the bound environment or does not declare the zero-operand
    /// shape the pair requires.
    ConstraintMismatch,
    /// The bound effect catalog does not declare the consumer or rewritten
    /// form under the record's constraint key, or the declarations do not
    /// satisfy the rule's control-flow axis — a conditional relative branch
    /// resolving to an unconditional one with both sides otherwise
    /// effect-isolated.
    EffectSurfaceMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for TerminatorPairError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid terminator pair fold: {self:?}")
    }
}

impl std::error::Error for TerminatorPairError {}
