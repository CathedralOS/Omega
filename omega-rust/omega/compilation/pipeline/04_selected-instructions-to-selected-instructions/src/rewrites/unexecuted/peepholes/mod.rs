//! Optimizer module role: stage group. Declarative selected-instruction pair
//! descriptors for peephole families beyond the literal-fold grammar.
//!
//! The pair descriptors under `rewrites/selected_lowering` declare
//! (producer, consumer, rewritten) triples for consumers that carry the
//! folded register in an explicit operand — an ordinary instruction in the
//! block's body. The families here generalize that declaration discipline to
//! consumers the instruction-pair grammar cannot name: flag readers whose
//! whole input — condition-state observations and the encoded effect —
//! flows through implicit physical units and the bound machine-effect
//! catalog rather than through operands, and dereferences that absorb an
//! address projection.
//!
//! Each family declares its pairs as data — the producer kind, consumer kind,
//! rewritten kind, and the admissibility axes the relationship must satisfy —
//! and keeps the same producer/replay split the landed descriptors hold:
//! admission consults its own descriptor, while the independent validator
//! re-derives the grammar, the flow facts, and the rewrite from the
//! instruction records alone and never reads the table.
//!
//! - The decided condition-state branch family formerly staged here as
//!   `terminator_pair` — a conditional-branch terminator whose flag uses
//!   resolve to one constant-operand producer becomes the `Jump` carrying
//!   the decided successor — now drives the executed
//!   `rewrites::constant_branch` rule (`SelectedConstantBranchFoldV1`)
//!   from `constant_branch/pair`, with the control-flow relationship
//!   (`ConditionalRelativeBranchV1` to `UnconditionalRelativeBranchV1`)
//!   bound to the stage's machine-effect catalog on both sides.
//! - The decided condition-state materialization family formerly staged
//!   here as `condition_materialization` — a `MaterializeBoolean*`
//!   instruction, the flag-consuming form no operand-carrying descriptor
//!   could name, whose flag uses resolve to one constant-operand producer
//!   becomes the `MaterializeI64` publishing the decided zero or one — now
//!   drives the executed `rewrites::constant_boolean` rule
//!   (`SelectedConstantBooleanFoldV1`) from `constant_boolean/pair`, the
//!   same integration the branch family completed. Its flag-reaching walk
//!   and operand grammars live in the shared `rewrites::condition_state`
//!   vocabulary; the staged `condition_flow` copy left with it.
//! - The copied-call-operand family formerly staged here as
//!   `copied_call_operand` — a `CopyI64` producer whose defined register
//!   feeds `Use` argument operands of a `CallUnit`, `CallScalar`, or
//!   `CallAggregate` becomes the same call reading the copy's source
//!   register — now drives the executed `rewrites::copied_call_operand`
//!   rule (`SelectedCopiedCallOperandFoldV1`) from
//!   `copied_call_operand/pair`, the first descriptor whose consumer
//!   carries a stack effect: the rewritten call retains its complete
//!   activation surface — `Call` barrier,
//!   `DirectInternalNormalReturnV1` call effect, `DirectRelativeCallV1`
//!   control, retained `MayArchitecturalFaultV1` trap, the row's
//!   implicit-unit and caller-saved clobber roster, the ABI `fixed_view`
//!   pins, and the target's stack lifecycle
//!   (`CallReturnAddressLifecycleV1` on stack-pushing targets,
//!   `UnchangedV1` under a link register) — verbatim, while the producer
//!   stays published for every other reader.
//! - `projected_access` declares the address-projection access family: an
//!   `AddressOffset` producer whose projected register feeds operand 0 of a
//!   `Load8`, `Load16`, `Load32`, `Load64`, or `Store` becomes the same
//!   access reading the projection's base at the combined displacement —
//!   the first descriptor whose consumer and rewritten kind are identical,
//!   and the first to declare a trap-*preserving* relationship: the
//!   dereference's `MayArchitecturalFaultV1` surface rides the fold
//!   verbatim because the rewritten form performs the identical access,
//!   where `FaultDischargedBy*` declares faults the literal or obligation
//!   retires. Its `Store` consumer is also the first declared pair whose
//!   consumer writes memory (`WritePointerV1`).
//!
mod projected_access;

pub use projected_access::{
    ProjectedAccessError, ProjectedAccessReceipt, ValidatedProjectedAccess,
    fold_selected_projected_access, validate_projected_access_fold,
};
