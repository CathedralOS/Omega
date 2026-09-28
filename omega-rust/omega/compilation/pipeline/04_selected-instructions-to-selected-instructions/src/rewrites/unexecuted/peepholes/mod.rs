//! Optimizer module role: stage group. Declarative selected-instruction pair
//! descriptors for peephole families beyond the literal-fold grammar.
//!
//! The pair descriptors under `rewrites/selected_lowering` declare
//! (producer, consumer, rewritten) triples for consumers that carry the
//! folded register in an explicit operand — an ordinary instruction in the
//! block's body. The families here generalize that declaration discipline to
//! consumers the instruction-pair grammar cannot name: the terminator-carried
//! instruction every block ends on, whose operand list is empty and whose
//! traffic — condition-state observations, the control unit, the encoded
//! control effect — flows through implicit physical units and the bound
//! machine-effect catalog rather than through operands.
//!
//! Each family declares its pairs as data — the producer kind, consumer kind,
//! rewritten kind, and the admissibility axes the relationship must satisfy —
//! and keeps the same producer/replay split the landed descriptors hold:
//! admission consults its own descriptor, while the independent validator
//! re-derives the grammar, the flow facts, and the rewrite from the
//! instruction records alone and never reads the table.
//!
//! - `terminator_pair` declares the decided condition-state branch family:
//!   a conditional-branch terminator whose flag uses resolve to one
//!   constant-operand condition-state producer becomes the `Jump` carrying
//!   the decided successor — the first descriptor to carry a control-flow
//!   relationship (`ConditionalRelativeBranchV1` to
//!   `UnconditionalRelativeBranchV1`) and unit roles beyond retired implicit
//!   definitions: retired condition-state *uses*, the preserved control-unit
//!   use, and republished implicit definitions.
//! - `condition_materialization` declares the decided condition-state
//!   materialization family: a `MaterializeBoolean*` instruction — the
//!   flag-consuming form no operand-carrying descriptor could name, whose
//!   whole input arrives through implicit physical units — whose flag uses
//!   resolve to one constant-operand producer becomes the `MaterializeI64`
//!   publishing the decided zero or one. Its consumer is the first
//!   instruction-position flag consumer: the uses retire exactly as the
//!   terminator family's do, at a body read position rather than a
//!   terminator one.
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
//! - `copied_call_operand` declares the copied-call-operand reroute
//!   family: a `CopyI64` producer whose defined register feeds `Use`
//!   argument operands of a `CallUnit`, `CallScalar`, or `CallAggregate`
//!   becomes the same call reading the copy's source register — the
//!   substitution `copy_removal` performs everywhere except call
//!   contracts, restated as a declared pair that keeps the producer
//!   published. It is the first descriptor whose consumer carries a stack
//!   effect: the rewritten call retains its complete activation surface —
//!   `Call` barrier, `DirectInternalNormalReturnV1` call effect,
//!   `DirectRelativeCallV1` control, retained `MayArchitecturalFaultV1`
//!   trap, the row's implicit-unit and caller-saved clobber roster, the
//!   ABI `fixed_view` pins, and the target's stack lifecycle
//!   (`CallReturnAddressLifecycleV1` on stack-pushing targets,
//!   `UnchangedV1` under a link register) — verbatim.
//!
//! `condition_flow` owns the machinery both flag-reading families share:
//! the least-fixpoint flag-reaching walk parameterized on the read
//! position, the producer operand-resolution grammars, and the
//! unique-materialization lookup the decisions rest on.

mod condition_flow;
mod condition_materialization;
mod copied_call_operand;
mod projected_access;
mod terminator_pair;

pub use condition_materialization::{
    ConditionMaterializationError, ConditionMaterializationReceipt,
    ValidatedConditionMaterialization, fold_selected_condition_materialization,
    validate_condition_materialization_fold,
};
pub use copied_call_operand::{
    CopiedCallOperandError, CopiedCallOperandReceipt, ValidatedCopiedCallOperand,
    fold_selected_copied_call_operand, validate_copied_call_operand_fold,
};
pub use projected_access::{
    ProjectedAccessError, ProjectedAccessReceipt, ValidatedProjectedAccess,
    fold_selected_projected_access, validate_projected_access_fold,
};
pub use terminator_pair::{
    TerminatorPairError, TerminatorPairReceipt, ValidatedTerminatorPair,
    fold_selected_terminator_pair, validate_terminator_pair_fold,
};
