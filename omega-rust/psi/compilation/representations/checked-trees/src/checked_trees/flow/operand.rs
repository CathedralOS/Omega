use arena::{Handle, HandleSpan};
use typed_trees::expression::ExpressionHandle;

use super::FlowConstraintRef;

/// A payload access at its actual evaluation epoch. Geometry names a case;
/// these live constraints must separately establish that case before access.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowCaseAccessFact {
    pub expression: ExpressionHandle,
    pub statement_index: usize,
    pub constraints: HandleSpan<FlowConstraintRef>,
}

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

/// Independent lifetime dependencies and spatial associations of one completed
/// view. Roots form only a dependency frontier; descriptor and enclosure links
/// establish geometry separately. None supplies disposition authority.
/// A valid view row with no roots is known empty. An aggregate has no own
/// view dependency; its declared leaves still require independent coverage.
/// An invalid retention handle means that dependency was unavailable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowOperandRetention {
    /// Checked pre-interpretation storage enclosing this pointer. Recasts and
    /// ordinary placed views retain the completed source capture, not its later
    /// descriptor home. This is a conservative compatibility footprint only:
    /// no scalar identity, pointee contents, access or ancestry follows from it.
    /// Zero withholds enclosure evidence. Reference-valued children need their
    /// own pointer relation and cannot inherit their containing storage's scope.
    pub enclosing_storage: Handle<FlowOperandFact>,
    /// Complete conservative input regions promised by an admitted local
    /// dispatch slot. These name possible backing, never exact descriptor
    /// identity, pointee contents or permission suppliers.
    pub enclosing_regions: HandleSpan<FlowOperandRetainedRegion>,
    /// Body-derived association to a completed returned descriptor. This is
    /// a post-call observation, not an evaluated child, loan authority, scalar
    /// equality or a snapshot of the referent. Zero withholds exact geometry.
    pub descriptor: Handle<FlowOperandFact>,
    /// Pointer scope authenticated by an immutable literal producer. Empty
    /// lifetime roots alone never establish image backing. Copies retain this
    /// scope; borrowing a holder does not inherit its pointee's scope.
    pub immutable_image: bool,
    pub roots: HandleSpan<facts::PlaceRoot>,
    /// Exact completed inputs retaining the permission carried by a call store.
    /// These are supplier associations, not value equality or pointee snapshots.
    pub suppliers: HandleSpan<FlowOperandRetainedSupplier>,
    /// Mutually exclusive completed profiles retained by a value join.
    /// Their stored-value links preserve nested descriptor observations;
    /// unioning backing roots alone would lose those completed contents.
    pub alternatives: HandleSpan<FlowOperandRetention>,
    /// Lifetime relations at declared output paths. These are not evaluated
    /// child values and carry no expression, type or predicate observation.
    pub leaves: HandleSpan<FlowOperandRetainedLeaf>,
    /// Completed carrier lent through this view, when its stored reference
    /// leaves were available at address formation. Later operands cannot
    /// replace this observation with newer descriptor contents.
    pub stored_value: Handle<FlowOperandFact>,
    /// A selected subtree of that captured value. Selection retains lifetime
    /// evidence without inventing an evaluated child or address observation.
    pub stored_projection: HandleSpan<facts::PlaceSegment>,
}

/// One completed input region enclosing a returned footprint. Projection
/// selects its captured descriptor, not that descriptor's former storage home.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowOperandRetainedRegion {
    pub operand: Handle<FlowOperandFact>,
    pub projection: HandleSpan<facts::PlaceSegment>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowOperandRetainedSupplier {
    pub operand: Handle<FlowOperandFact>,
    pub projection: HandleSpan<facts::PlaceSegment>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowOperandRetainedLeaf {
    pub path: HandleSpan<facts::PlaceSegment>,
    pub retention: Handle<FlowOperandRetention>,
}

/// Facts at completion of one evaluated operand or stored value. Scalar copies and
/// by-value carriers with stable observable contents keep this value's facts
/// even if a later operand changes its former source storage — the bound
/// operand is a detached copy. View carriers such as references and slices
/// denote storage they read at invocation, so `referents` records the exact
/// referent custody: the clause's evidence is the facts live on that storage
/// at invocation, including ones a later operand established.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowOperandFact {
    pub expression: ExpressionHandle,
    /// The state and statement whose evaluation produced this observation.
    /// Retained stores can outlive their statement-local write prefix.
    pub state_symbol: symbols::SymbolHandle,
    pub statement_index: usize,
    /// Bound destination carrier for this evaluation, including a first
    /// store whose local is not yet in the preceding statement's scope.
    pub type_reference: typed_trees::types::TypeReferenceHandle,
    /// Constructed children, attached to this exact evaluation occurrence.
    /// Each child retains its own completion facts before later child effects.
    pub children: HandleSpan<FlowOperandChild>,
    /// Complete mutually exclusive result captures at a value join. Each
    /// alternative retains its own completion epoch and must establish every
    /// requested reference leaf; one resolved arm cannot cover another arm.
    /// These are alternatives, never simultaneously installed product children.
    pub alternatives: HandleSpan<FlowOperandFact>,
    /// Earlier completed whole value supplying an identity-preserving read,
    /// independently of physical backing and pointee contents. This is not
    /// an owned completion or a new custody/cleanup alternative.
    pub value_source: arena::Handle<FlowOperandFact>,
    /// Writes already evaluated in this statement when the operand completed.
    /// Later writes cannot change this prefix or its invocation-entry origin.
    pub entry_origin_writes: HandleSpan<FlowOperandReferent>,
    pub entry_origin_unknown_write: bool,
    /// Exact storage history at this completed observation. Later writes or
    /// sibling branches cannot change the predecessor this handle names.
    pub storage_snapshot: arena::Handle<super::FlowStorageSnapshot>,
    pub constraints: HandleSpan<FlowConstraintRef>,
    /// Exact referent storage when the operand is a view carrier. Empty for
    /// detached carriers and for referents the resolver cannot place exactly.
    pub referents: HandleSpan<FlowOperandReferent>,
    /// Complete backing lifetime frontier, independently of exact geometry.
    /// Aggregate reference leaves retain this relation in their child captures.
    pub retention: Handle<FlowOperandRetention>,
}

/// One authored child installed at its resolved destination in a completed
/// record, case, or array. Rows are in evaluation order; paths name declaration
/// identities and are stored in `FlowControlFacts::operand_child_paths`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowOperandChild {
    /// The actual constructing occurrence, retained even when a later place
    /// projection or reference keeps only its selected storage. Together with
    /// the child's state/statement and path this names one completed capture.
    pub parent: ExpressionHandle,
    pub path: HandleSpan<facts::PlaceSegment>,
    pub value: Handle<FlowOperandFact>,
    /// Declared destination type, including reference access/qualification.
    /// An unresolved type remains the invalid handle, never the source type.
    pub type_reference: typed_trees::types::TypeReferenceHandle,
}

/// One completed storage observation and the receiver actually evaluated for
/// it. A scalar result retains no loan, but its read still needs the receiver's
/// backing until completion. These contexts exclude later statement effects.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowValueObservationFact {
    pub expression: ExpressionHandle,
    pub evaluation: super::FlowPermissionEvaluation,
    pub receiver: Handle<FlowOperandFact>,
    /// The completed value, when captured by its consumer. Its exact retained
    /// lifetime frontier determines whether the observed home may die here.
    /// Zero is unavailable evidence, not a reference-free result.
    pub result: Handle<FlowOperandFact>,
    pub semantic_contexts: HandleSpan<super::FlowSemanticContextRef>,
    pub constraints: HandleSpan<FlowConstraintRef>,
}

/// Actual source operands of one performed atomic expression. The generated
/// update formula and result placeholder are not executed inputs. Each source
/// operand retains its completion before later inputs or the final store.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowAtomicExecutionFact {
    pub state_symbol: symbols::SymbolHandle,
    pub statement_index: usize,
    pub expression: ExpressionHandle,
    pub operands: HandleSpan<FlowOperandFact>,
}
