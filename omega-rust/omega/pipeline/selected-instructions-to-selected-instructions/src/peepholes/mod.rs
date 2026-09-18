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
//!
//! `condition_flow` owns the machinery both families share: the
//! least-fixpoint flag-reaching walk parameterized on the read position,
//! the producer operand-resolution grammars, and the unique-materialization
//! lookup the decisions rest on.

mod condition_flow;
mod condition_materialization;
mod terminator_pair;

pub use condition_materialization::{
    ConditionMaterializationError, ConditionMaterializationReceipt,
    ValidatedConditionMaterialization, fold_selected_condition_materialization,
    validate_condition_materialization_fold,
};
pub use terminator_pair::{
    TerminatorPairError, TerminatorPairReceipt, ValidatedTerminatorPair,
    fold_selected_terminator_pair, validate_terminator_pair_fold,
};
