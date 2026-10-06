use arena::{Handle, HandleSpan};
use typed_trees::expression::ExpressionHandle;

use super::FlowConstraintRef;

/// One storage place an evaluated operand exactly denotes: a canonical root
/// plus its field/case/index suffix, in the semantic fact plan's `Place`
/// vocabulary. Produced only when the reference resolver can name the
/// referent precisely; an empty span keeps the operand on context-identity
/// transport.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowOperandReferent {
    pub root: facts::PlaceRoot,
    pub segments: HandleSpan<facts::PlaceSegment>,
}

/// Facts at completion of one authored invocation operand. Scalar copies and
/// by-value carriers with stable observable contents keep this value's facts
/// even if a later operand changes its former source storage — the bound
/// operand is a detached copy. View carriers such as references and slices
/// denote storage they read at invocation, so `referents` records the exact
/// referent custody: the clause's evidence is the facts live on that storage
/// at invocation, including ones a later operand established.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowOperandFact {
    pub expression: ExpressionHandle,
    /// Constructed children, attached to this exact evaluation occurrence.
    /// Each child retains its own completion facts before later child effects.
    pub children: HandleSpan<FlowOperandChild>,
    /// Writes already evaluated in this statement when the operand completed.
    /// Later writes cannot change this prefix or its invocation-entry origin.
    pub entry_origin_writes: HandleSpan<FlowOperandReferent>,
    pub entry_origin_unknown_write: bool,
    pub constraints: HandleSpan<FlowConstraintRef>,
    /// Exact referent storage when the operand is a view carrier. Empty for
    /// detached carriers and for referents the resolver cannot place exactly.
    pub referents: HandleSpan<FlowOperandReferent>,
}

/// One authored child installed at its resolved destination in a completed
/// record, case, or array. Rows are in evaluation order; paths name declaration
/// identities and are stored in `FlowControlFacts::operand_child_paths`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowOperandChild {
    pub path: HandleSpan<facts::PlaceSegment>,
    pub value: Handle<FlowOperandFact>,
    /// Declared destination type, including reference access/qualification.
    /// An unresolved type remains the invalid handle, never the source type.
    pub type_reference: typed_trees::types::TypeReferenceHandle,
}
