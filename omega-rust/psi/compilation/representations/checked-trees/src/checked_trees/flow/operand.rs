use arena::{Handle, HandleSpan};
use typed_trees::expression::ExpressionHandle;

use super::FlowConstraintRef;

/// A payload access at its actual evaluation epoch. Geometry names a case;
/// these live constraints must separately establish that case before access.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowCaseAccessFact {
    pub machine_symbol: symbols::SymbolHandle,
    pub state_symbol: symbols::SymbolHandle,
    pub expression: ExpressionHandle,
    pub evaluation: super::FlowPermissionEvaluation,
    pub statement_index: usize,
    pub constraints: HandleSpan<FlowConstraintRef>,
    /// Exact subjects at this access. An empty produced roster is unresolved,
    /// never permission to recover a newer address from source syntax.
    pub places: HandleSpan<FlowOperandReferent>,
}

/// One storage place an evaluated operand exactly denotes: a canonical root
/// plus its field/case/index suffix, in the semantic fact plan's `Place`
/// vocabulary. Produced only when the reference resolver can name the
/// referent precisely; an empty span keeps the operand on context-identity
/// transport.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowOperandReferent {
    /// Descriptor-formation obligations for this exact namespace, independently
    /// of cases and contents reached below the saved pointer. Zero has no
    /// historical evidence and requires ordinary current case checking.
    pub formation: Handle<FlowReferentFormation>,
    pub root: facts::PlaceRoot,
    pub segments: HandleSpan<facts::PlaceSegment>,
}

/// Provenance of a completed reference descriptor, never a pointee snapshot.
/// `source_access` names an earlier actual payload read. A nonzero `owner`
/// names an owned inventory whose conditional leaf has not been loaded yet:
/// its `path` must be selected on that completed value at the actual load.
/// The saved namespace ends at this descriptor; every later suffix remains
/// subject to current case evidence. Inventory presence proves no case.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowReferentFormation {
    /// Actual source coordinates paired with source_access. The owned path is
    /// relative to the value, not a suffix to trim from physical geometry.
    pub source: Handle<FlowOperandReferent>,
    pub source_access: Handle<FlowCaseAccessFact>,
    pub owner: Handle<FlowOperandFact>,
    pub path: HandleSpan<facts::PlaceSegment>,
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
    /// Complete body-proven input descriptor candidates. Projection selects
    /// the captured invocation-entry pointer. Unlike permission suppliers,
    /// this relation establishes descriptor forwarding, never pointee contents.
    /// Only the returned-descriptor producer may install this roster.
    pub descriptor_sources: HandleSpan<FlowOperandRetainedSupplier>,
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
    /// Value formation at this exact evaluation occurrence.
    pub completion: FlowOperandCompletion,
    /// Actual receiver or source descriptor dependency, independently of value identity.
    pub backing: Handle<FlowOperandFact>,
    /// Earlier whole value supplying an identity-preserving read; not custody.
    pub value_source: Handle<FlowOperandFact>,
    /// Writes already evaluated in this statement when the operand completed.
    /// Later writes cannot change this prefix or its invocation-entry origin.
    pub entry_origin_writes: HandleSpan<FlowOperandReferent>,
    pub entry_origin_unknown_write: bool,
    /// Exact immutable storage history at this completed observation.
    pub storage_snapshot: Handle<super::FlowStorageSnapshot>,
    /// Descriptor binding writes are separate from pointee content writes.
    pub entry_binding_writes: HandleSpan<FlowOperandReferent>,
    pub entry_binding_unknown_write: bool,
    pub case_access: Handle<FlowCaseAccessFact>,
    pub constraints: HandleSpan<FlowConstraintRef>,
    /// Exact referent storage when the operand is a view carrier. Empty for
    /// detached carriers and for referents the resolver cannot place exactly.
    pub referents: HandleSpan<FlowOperandReferent>,
    /// Complete backing lifetime frontier, independently of exact geometry.
    /// Aggregate reference leaves retain this relation in their child captures.
    pub retention: Handle<FlowOperandRetention>,
    /// Descriptor values copied or moved with this owned carrier at completion.
    /// Each declared leaf retains exact referents independently of lifetime roots;
    /// later writes to the carrier cannot retarget this completed operand.
    pub reference_leaves: HandleSpan<FlowCapturedReferenceLeaf>,
}

/// Retain the evaluated value graph without enumerating combinations of fields
/// and choices. A selected arm refers to its actual completion, so projections
/// of one saved selection share its identity and observation epoch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlowOperandCompletion {
    #[default]
    Observation,
    Constructed(HandleSpan<FlowOperandChild>),
    Selected(HandleSpan<FlowOperandAlternative>),
    /// A projection of one already completed owned copy. The value's contents
    /// belong to the copy epoch; reference leaves keep their captured descriptors
    /// and separately read live referent contents.
    Copied {
        copy: Handle<FlowCopiedValue>,
        path: HandleSpan<facts::PlaceSegment>,
    },
}

impl FlowOperandFact {
    /// Only construction supplies simultaneous destination children. Selection
    /// consumers must use the alternative join, never treat its arms as fields.
    pub fn constructed_children(&self) -> HandleSpan<FlowOperandChild> {
        match self.completion {
            FlowOperandCompletion::Constructed(children) => children,
            FlowOperandCompletion::Observation
            | FlowOperandCompletion::Selected(_)
            | FlowOperandCompletion::Copied { .. } => HandleSpan::empty(),
        }
    }
}

/// One reachable arm of a completed selection, in first-match order. The
/// parent operand supplies the evaluation identity; source syntax alone does
/// not distinguish repeated evaluations of that choice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowOperandAlternative {
    pub source_arm: Handle<typed_trees::expression::TableMatchArm>,
    pub value: Handle<FlowOperandFact>,
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
    /// Selected storage at the actual read, after its receiver and selectors
    /// completed. Later effects cannot replace these coordinates.
    pub places: HandleSpan<FlowOperandReferent>,
    /// Content-write prefix at completion of this read, independent of the
    /// earlier receiver's epoch. Address equality alone is not value equality.
    pub origin_writes: HandleSpan<FlowOperandReferent>,
    pub origin_unknown_write: bool,
    pub semantic_contexts: HandleSpan<super::FlowSemanticContextRef>,
    pub constraints: HandleSpan<FlowConstraintRef>,
}

/// A current guard context about one completed descriptor read, rather than
/// every possible pointee. Consumers must rejoin its actual receiver, binding epoch,
/// backing lifetime and content-write frontier before using that namespace.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowDescriptorContextObservation {
    pub context: facts::FactContextHandle,
    pub observation: Handle<FlowValueObservationFact>,
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

/// One copy completed before selected target operands. `source` is the actual
/// entry-time value observation; the reference inventory retains descriptors,
/// never a snapshot of their referee contents.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowCopiedValue {
    pub source: Handle<FlowOperandFact>,
    pub references: HandleSpan<FlowCapturedReferenceLeaf>,
    pub uses: HandleSpan<FlowCopiedValueUse>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowCapturedReferenceLeaf {
    pub path: HandleSpan<facts::PlaceSegment>,
    pub type_reference: typed_trees::types::TypeReferenceHandle,
    pub referents: HandleSpan<FlowOperandReferent>,
    pub retention: Handle<FlowOperandRetention>,
    pub constraints: HandleSpan<FlowConstraintRef>,
}

/// An actual read tied to its selected copy, retained by the evaluator rather
/// than rediscovered by scanning unrelated operands or matching source paths.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowCopiedValueUse {
    pub expression: ExpressionHandle,
    pub evaluation: super::FlowPermissionEvaluation,
}
