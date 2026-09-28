//! Copied-call-operand rerouting through declarative pairs.
//!
//! The instruction-pair descriptors under `rewrites/selected_lowering`
//! declare a materialized literal folded into a consumer operand — a
//! `Use`-position constant substitution — and the flag-reading families
//! under `peepholes` declare consumers whose whole input arrives through
//! implicit physical units. Between those vocabularies sits the pair a
//! transparent register copy forms with the direct internal call that
//! reads its result: the `CopyI64` producer publishes an ordinary `Def`
//! register — no literal to substitute, no displacement to absorb — and
//! the consumer's `Use` argument operands name that register because the
//! copy forwarded the value the call must observe. The rewritten form is
//! the consumer's *same* constraint row reading the copy's source register
//! at every operand that named the copy's destination. The
//! [`CopiedCallOperandRule`](pair::CopiedCallOperandRule) table under
//! `copied_call_operand/pair` declares that relationship: each rule pairs
//! the `CopyI64` producer with one direct-call consumer kind — `CallUnit`,
//! `CallScalar`, or `CallAggregate` — and the axes the relationship must
//! satisfy: `CopiedCallOperandShape` for the leading-`Use`, trailing-`Def`
//! operand roster, `CopiedCallOperandUnitFlow` for the call's retained
//! implicit-unit and ABI-view surface, and `CopiedCallOperandEffects` for
//! the catalog-declared machine-effect relationship.
//!
//! The declared family is the copied-call-operand reroute — the
//! substitution `rewrites::copy_removal` performs everywhere *except* call
//! contracts, restated as a declared pair that keeps the copy published:
//! the call's operand contract is a fixed-ABI-view roster the bespoke
//! copy-removal rewrite refuses to rebind, while this family leaves the
//! producer's register published for every other reader and rebinds only
//! the operand registers that named its destination. A call whose `Use`
//! operand is the block's last definition of a `CopyI64` result is
//! admitted when the producer is a clean `[use source, def destination]`
//! copy, no instruction in the open interval redefines the source, the
//! register classes agree through the bound constraint rows, and the
//! consumer's record restates its constraint row verbatim — dense operand
//! numbers, ABI `fixed_view` pins, the complete implicit-unit and
//! caller-saved clobber roster. The copy already published `source`'s
//! value into `destination`, so a rebound operand observes the identical
//! value at the call.
//!
//! Under `CopiedCallOperandEffects::CallLifecycleRetained` the rewrite
//! carries the *stack-carrying* relationship no earlier descriptor could
//! name: the folded form performs the consumer's call unchanged, so the
//! bound catalog must declare the call row — the consumer's and the
//! rewritten form's alike — with its complete activation surface:
//! `DirectRelativeCallV1` control, the `Call` barrier, the
//! `DirectInternalNormalReturnV1` call effect, the retained
//! `MayArchitecturalFaultV1` trap, and the target's stack lifecycle —
//! `WriteReturnAddressBelowStackPointerV1` memory paired with
//! `CallReturnAddressLifecycleV1` stack on x86-64, `NoneV1` paired with
//! `UnchangedV1` where the link register holds the return address on
//! AArch64 — while the retained `CopyI64` producer is fully
//! effect-isolated. This is the first declared pair whose consumer pushes
//! and restores stack state; a target whose copy or call rows encode
//! anything else cannot admit the pair.
//!
//! The rewritten record keeps the consumer's identity, provenance, callee,
//! constraint row, operand order and ABI views, implicit uses, implicit
//! definitions, clobber roster, and result registers verbatim; only the
//! `Use` operands that named the copy's destination rebind to the source
//! register. The producer stays: its defined register remains published
//! for every other reader the function still holds, and producer
//! elimination stays with the producer-elimination rules.
//!
//! The descriptor declares admissibility; it never certifies it. The
//! producer consults its own rule and axes in `admission`; the independent
//! replay in `replay` re-derives the call's operand roster, the
//! last-definition and interval scans, the class agreement, and the
//! catalog surfaces from the instruction records alone — never consulting
//! the pair table — and restores the complete source by content.

mod admission;
mod pair;
mod replay;
mod rewrite;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use replay::validate_copied_call_operand_fold;
pub use rewrite::fold_selected_copied_call_operand;

#[cfg(test)]
mod tests;

/// An accepted copied-call-operand reroute with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCopiedCallOperand {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: CopiedCallOperandReceipt,
}

impl ValidatedCopiedCallOperand {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &CopiedCallOperandReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiedCallOperandReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl CopiedCallOperandReceipt {
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
pub enum CopiedCallOperandError {
    SourceMismatch,
    /// Not the emitted direct-call roster the family declares: a body
    /// instruction of a kind no declared consumer names — the normalized
    /// foreign call's boundary custody places it outside the direct
    /// internal call contract, and every other kind carries no argument
    /// roster at all — or a declared kind carrying a malformed operand
    /// shape: operand numbers that do not equal their positions, a `Use`
    /// trailing a `Def`, or a result roster outside the consumer
    /// semantic's declared count.
    UnsupportedConsumer,
    /// The named producer is not the copy the call's operands read: not a
    /// body instruction in the consumer's block ahead of it, not a clean
    /// `CopyI64` — another kind, a decorated or unit-carrying record, or a
    /// `[use, def]` shape mismatch — not the destination register's last
    /// definition before the call, or a copy whose source and destination
    /// name one register.
    UnsupportedProducer,
    /// A dataflow fact the fold cannot reproduce: no `Use` operand of the
    /// call reads the copy's destination register, a missing register
    /// roster entry, a source register whose class the rebound operand
    /// cannot carry, or the source register redefined inside the
    /// producer–consumer interval.
    UnsupportedUse,
    /// The consumer's or the producer's constraint row is missing from the
    /// bound environment or does not restate the record verbatim — operand
    /// shape, classes, ABI `fixed_view` pins, and the implicit-unit and
    /// clobber surfaces the rewritten record republishes.
    ConstraintMismatch,
    /// The bound effect catalog does not describe this plan's target,
    /// constraints, and selected keys, does not declare the consumer or
    /// producer form under the record's constraint key, or the
    /// declarations do not satisfy the rule's retained-call axis — the
    /// call's `Call` barrier, `DirectInternalNormalReturnV1` effect,
    /// `DirectRelativeCallV1` control, retained fault surface, and the
    /// target's stack lifecycle carried verbatim, the retained copy fully
    /// effect-isolated.
    EffectSurfaceMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for CopiedCallOperandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid copied call operand fold: {self:?}")
    }
}

impl std::error::Error for CopiedCallOperandError {}
