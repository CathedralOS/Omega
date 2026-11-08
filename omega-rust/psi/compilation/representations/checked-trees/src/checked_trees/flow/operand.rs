use arena::{Handle, HandleSpan};
use typed_trees::expression::ExpressionHandle;

use super::FlowConstraintRef;

/// A payload access at its actual evaluation epoch. Geometry names a case;
/// these live constraints must separately establish that case before access.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowOperandReferent {
    /// Descriptor-formation obligations for this exact namespace, independently
    /// of cases and contents reached below the saved pointer. Zero has no
    /// historical evidence and requires ordinary current case checking.
    pub formation: Handle<FlowReferentFormation>,
    pub root: facts::PlaceRoot,
    pub segments: HandleSpan<facts::PlaceSegment>,
}

/// Provenance of a completed reference descriptor, never a pointee snapshot.
/// Evidence names an earlier actual payload read or completed invocation. A nonzero `owner`
/// names an owned inventory whose conditional leaf has not been loaded yet:
/// its `path` must be selected on that completed value at the actual load.
/// The saved namespace ends at this descriptor; every later suffix remains
/// subject to current case evidence. Inventory presence proves no case.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowReferentFormation {
    /// Actual source coordinates paired with the formation evidence. The owned path is
    /// relative to the value, not a suffix to trim from physical geometry.
    pub source: Handle<FlowOperandReferent>,
    pub evidence: FlowReferentFormationEvidence,
    pub owner: Handle<FlowOperandFact>,
    pub path: HandleSpan<facts::PlaceSegment>,
    pub root: facts::PlaceRoot,
    pub segments: HandleSpan<facts::PlaceSegment>,
}

/// Descriptor formation is established either by a checked read here or by
/// a checked callee returning that descriptor. Neither establishes current
/// case facts about the pointee below the completed descriptor's namespace.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum FlowReferentFormationEvidence {
    #[default]
    Unknown,
    Access(Handle<FlowCaseAccessFact>),
    Return(Handle<super::FlowCallFact>),
}

impl FlowReferentFormation {
    pub fn source_access(&self) -> Handle<FlowCaseAccessFact> {
        match self.evidence {
            FlowReferentFormationEvidence::Access(access) => access,
            FlowReferentFormationEvidence::Unknown | FlowReferentFormationEvidence::Return(_) => {
                Handle::invalid()
            }
        }
    }
}

/// Independent lifetime dependencies and spatial associations of one completed
/// view. Roots form only a dependency frontier; descriptor and enclosure links
/// establish geometry separately. None supplies disposition authority.
/// A valid view row with no roots is known empty. An aggregate has no own
/// view dependency; its declared leaves still require independent coverage.
/// An invalid retention handle means that dependency was unavailable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowOperandRetention {
    /// A checked producer formed a new projected view from these completed
    /// descriptors. Empty withholds formation evidence. Unlike `suppliers`,
    /// these rows do not transport an existing pointer or grant permission;
    /// the resource owner must authenticate every source and borrowing parent.
    pub reborrow_sources: HandleSpan<FlowOperandReborrowSource>,
    /// Complete nonempty possible contents footprint captured by the producing
    /// operation. Unknown element selectors retain structural dimensions only.
    /// This permits interference checks, never address equality, membership,
    /// permission ancestry or value transfer. Empty means unavailable.
    pub possible_storage: HandleSpan<FlowOperandReferent>,
    /// Checked pre-interpretation storage enclosing this pointer. Recasts and
    /// ordinary access views retain the completed source capture, not its later
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
    /// Actual completed inputs selected by the applied declared lifetime
    /// relation. This preserves typed return obligations through opaque owned
    /// carriers. It supplies neither descriptor geometry nor new permission;
    /// those remain with the captured input and its checked loans.
    pub lifetime_sources: Option<HandleSpan<FlowOperandRetainedSupplier>>,
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowOperandRetainedRegion {
    pub operand: Handle<FlowOperandFact>,
    pub projection: HandleSpan<facts::PlaceSegment>,
}

/// One declared input relationship: lending the referent and transferring a
/// stored descriptor have different contracts even when their paths are empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum ViewReturnInputSource {
    Referent,
    Stored,
}

/// The original applied call contract bound on a lifetime supplier. This is
/// not a promise inferred from the older actual capture reached through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowOperandLifetimeContract {
    pub state_symbol: symbols::SymbolHandle,
    pub statement_index: usize,
    pub invocation: ExpressionHandle,
    pub call_ordinal: usize,
    /// None names the returned value; Some names an overwritten parameter
    /// leaf. Both retain the original declaration's output path below.
    pub output_parameter: Option<usize>,
    pub input: Handle<FlowOperandFact>,
    pub parameter_index: usize,
    pub source: ViewReturnInputSource,
    pub source_type: typed_trees::types::TypeReferenceHandle,
    pub source_path: HandleSpan<facts::PlaceSegment>,
    pub owner_path: HandleSpan<facts::PlaceSegment>,
    /// The complete contents selected before this invocation writes. Stored
    /// edges keep produced descriptors and unchanged incoming slots together;
    /// a later read of the output variable cannot reconstruct this list.
    pub stored_sources: HandleSpan<FlowOperandStoredSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum FlowOperandStoredSource {
    Value {
        path: HandleSpan<facts::PlaceSegment>,
        operand: Handle<FlowOperandFact>,
        reference: typed_trees::types::TypeReferenceHandle,
        projection: HandleSpan<facts::PlaceSegment>,
        slot: Option<FlowOperandReferent>,
    },
    Incoming {
        path: HandleSpan<facts::PlaceSegment>,
        slot: FlowOperandReferent,
    },
}

impl Default for FlowOperandStoredSource {
    fn default() -> Self {
        Self::Value {
            path: HandleSpan::empty(),
            operand: Handle::invalid(),
            reference: Handle::invalid(),
            projection: HandleSpan::empty(),
            slot: None,
        }
    }
}

/// Nonspatial input obligations of one admitted parameter-output generation.
/// Descriptor invalidation cannot turn this into a pointer association. The
/// borrowing ledger must independently select this exact generation at release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowOutputLifetimeSources {
    pub output: facts::InvocationOutputIdentity,
    pub owner_path: HandleSpan<facts::PlaceSegment>,
    pub sources: Option<HandleSpan<FlowOperandRetainedSupplier>>,
}

impl Default for FlowOutputLifetimeSources {
    fn default() -> Self {
        Self {
            output: facts::InvocationOutputIdentity {
                state_symbol: Handle::invalid(),
                statement_index: 0,
                call_ordinal: 0,
                parameter_symbol: Handle::invalid(),
            },
            owner_path: HandleSpan::empty(),
            sources: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowOperandRetainedSupplier {
    pub operand: Handle<FlowOperandFact>,
    pub projection: HandleSpan<facts::PlaceSegment>,
    /// Present on a declared lifetime edge, independently of permission and
    /// descriptor suppliers, whose selected field contract stays with its owner.
    pub contract: Option<FlowOperandLifetimeContract>,
}

/// Keep the reference actually read when a helper returns a view of its data.
/// The source path selects a descriptor in that captured value; only the new
/// suffix projects into its referent. A completed invocation authenticates
/// the return relation separately from the source's borrowing permission.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowOperandReborrowSource {
    pub operand: Handle<FlowOperandFact>,
    pub source_path: HandleSpan<facts::PlaceSegment>,
    pub projection: HandleSpan<facts::PlaceSegment>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowOperandRetainedLeaf {
    pub path: HandleSpan<facts::PlaceSegment>,
    pub retention: Handle<FlowOperandRetention>,
}

/// A saved pointer keeps the event that permits its use. Loading an incoming
/// descriptor needs its completed read; a helper's output needs its normal
/// completion. Neither event can stand in for the other.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum FlowDescriptorPermission {
    #[default]
    Unavailable,
    Loaded {
        loan: Handle<super::super::BorrowLoanFact>,
        read: Handle<FlowValueObservationFact>,
    },
    SuppliedOutput {
        loan: Handle<super::super::BorrowLoanFact>,
    },
}

impl FlowDescriptorPermission {
    pub fn loaded_loan(self) -> Handle<super::super::BorrowLoanFact> {
        match self {
            Self::Loaded { loan, .. } => loan,
            Self::Unavailable | Self::SuppliedOutput { .. } => Handle::invalid(),
        }
    }

    pub fn loaded_read(self) -> Handle<FlowValueObservationFact> {
        match self {
            Self::Loaded { read, .. } => read,
            Self::Unavailable | Self::SuppliedOutput { .. } => Handle::invalid(),
        }
    }
}

/// The evaluated receiver path and whether it loads a value or forms a view.
/// A formed view keeps the collection's address; a reference-valued place loads
/// a descriptor whose own permission must be selected separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum FlowOperandAddressProjection {
    Place(HandleSpan<facts::PlaceSegment>),
    View(HandleSpan<facts::PlaceSegment>),
}

impl FlowOperandAddressProjection {
    pub fn segments(self) -> HandleSpan<facts::PlaceSegment> {
        match self {
            Self::Place(segments) | Self::View(segments) => segments,
        }
    }
}

/// Facts at completion of one evaluated operand or stored value. Scalar copies and
/// by-value carriers with stable observable contents keep this value's facts
/// even if a later operand changes its former source storage — the bound
/// operand is a detached copy. View carriers such as references and slices
/// denote storage they read at invocation, so `referents` records the exact
/// referent custody: the clause's evidence is the facts live on that storage
/// at invocation, including ones a later operand established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowOperandFact {
    pub descriptor_permission: FlowDescriptorPermission,
    pub expression: ExpressionHandle,
    /// The state and statement whose evaluation produced this observation.
    /// Retained stores can outlive their statement-local write prefix.
    pub state_symbol: symbols::SymbolHandle,
    pub statement_index: usize,
    /// Guards and the two outgoing targets can evaluate the same source node.
    /// Its captured value must consume custody from this exact execution lane.
    pub evaluation: super::FlowPermissionEvaluation,
    /// Bound destination carrier for this evaluation, including a first
    /// store whose local is not yet in the preceding statement's scope.
    pub type_reference: typed_trees::types::TypeReferenceHandle,
    /// Value formation at this exact evaluation occurrence.
    pub completion: FlowOperandCompletion,
    /// Actual receiver or source descriptor dependency, independently of value identity.
    pub backing: Handle<FlowOperandFact>,
    /// Typed path below the completed address receiver. Present even when its
    /// absolute address is unavailable; None withholds projection evidence,
    /// while Some(empty) denotes the same complete receiver window.
    pub address_projection: Option<FlowOperandAddressProjection>,
    /// Captured computation operands in evaluation order, including a selected
    /// short-circuit route. Presence alone does not guarantee execution on every
    /// completion. These are neither stored children nor extra backing loans.
    pub evaluated_inputs: HandleSpan<FlowOperandFact>,
    /// Start of the inputs whose execution depends on a selected route. None
    /// guarantees the whole roster; Some(prefix) guarantees only that prefix.
    /// This is an evaluation fact, independent of scalar predicates that later
    /// writes may invalidate. A consumer must check the prefix fits the roster.
    pub conditional_inputs_from: Option<usize>,
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

impl Default for FlowOperandFact {
    fn default() -> Self {
        Self {
            descriptor_permission: Default::default(),
            expression: Default::default(),
            state_symbol: Default::default(),
            statement_index: 0,
            // A synthesized descriptor has not executed a statement lane.
            evaluation: super::FlowPermissionEvaluation::Unknown,
            type_reference: Default::default(),
            completion: Default::default(),
            backing: Default::default(),
            address_projection: None,
            evaluated_inputs: HandleSpan::empty(),
            conditional_inputs_from: None,
            value_source: Default::default(),
            entry_origin_writes: HandleSpan::empty(),
            entry_origin_unknown_write: false,
            storage_snapshot: Default::default(),
            entry_binding_writes: HandleSpan::empty(),
            entry_binding_unknown_write: false,
            case_access: Default::default(),
            constraints: HandleSpan::empty(),
            referents: HandleSpan::empty(),
            retention: Default::default(),
            reference_leaves: HandleSpan::empty(),
        }
    }
}

/// Retain the evaluated value graph without enumerating combinations of fields
/// and choices. A selected arm refers to its actual completion, so projections
/// of one saved selection share its identity and observation epoch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum FlowOperandCompletion {
    #[default]
    Observation,
    /// Contents of an admitted state input at the captured storage cut. This
    /// names its incoming slot namespace, not an evaluated child or address.
    /// Descriptor consumers must authenticate the selected incoming loan.
    IncomingContents {
        root: facts::PlaceRoot,
        path: HandleSpan<facts::PlaceSegment>,
    },
    /// A descriptor leaf observed after this actual invocation completed.
    /// This preserves its result coordinate, not a second evaluation, exact
    /// pointer geometry, or permission inferred from its lifetime frontier.
    InvocationResult {
        invocation: Handle<super::FlowCallFact>,
        result_type: typed_trees::types::TypeReferenceHandle,
        path: HandleSpan<facts::PlaceSegment>,
    },
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
            | FlowOperandCompletion::IncomingContents { .. }
            | FlowOperandCompletion::InvocationResult { .. }
            | FlowOperandCompletion::Selected(_)
            | FlowOperandCompletion::Copied { .. } => HandleSpan::empty(),
        }
    }
}

/// One reachable arm of a completed selection, in first-match order. The
/// parent operand supplies the evaluation identity; source syntax alone does
/// not distinguish repeated evaluations of that choice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowOperandAlternative {
    /// The earlier subject/pattern prefix of this first-match selection.
    /// Only its evaluation inputs are prerequisites; its arm value did not run.
    pub prior_evaluation: Handle<FlowOperandAlternative>,
    /// Inputs evaluated since that prefix. The first arm includes the actual
    /// subject; later arms add only their own pattern or condition evaluation.
    pub evaluation_inputs: HandleSpan<FlowOperandFact>,
    pub source_arm: Handle<typed_trees::expression::TableMatchArm>,
    pub value: Handle<FlowOperandFact>,
}

/// One checked case decision on the unchanged owned home of a completed
/// subject. The statement owns these rows in evaluation order. They introduce
/// no transfer or disposal; cleanup still meets the actual branch outcomes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowOwnedCaseSelection {
    pub machine_symbol: symbols::SymbolHandle,
    pub state_symbol: symbols::SymbolHandle,
    pub statement_index: usize,
    pub statement: Handle<super::FlowStatementFact>,
    pub expression: ExpressionHandle,
    pub source_arm: Handle<typed_trees::expression::TableMatchArm>,
    pub evaluation: super::FlowPermissionEvaluation,
    pub subject: Handle<FlowOperandFact>,
    pub root: facts::PlaceRoot,
    pub path: HandleSpan<facts::PlaceSegment>,
    pub variant: symbols::SymbolHandle,
    /// Only a total case pattern's failure excludes that case. A failed guard
    /// or payload equality may still have observed the same active case.
    pub miss_excludes_case: bool,
    pub captured_storage: Handle<super::FlowStorageSnapshot>,
    pub selected_storage: Handle<super::FlowStorageSnapshot>,
}

/// One authored child installed at its resolved destination in a completed
/// record, case, or array. Rows are in evaluation order; paths name declaration
/// identities and are stored in `FlowControlFacts::operand_child_paths`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowDescriptorContextObservation {
    pub context: facts::FactContextHandle,
    pub observation: Handle<FlowValueObservationFact>,
}

/// Actual source operands of one performed atomic expression. The generated
/// update formula and result placeholder are not executed inputs. Each source
/// operand retains its completion before later inputs or the final store.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowAtomicExecutionFact {
    pub state_symbol: symbols::SymbolHandle,
    pub statement_index: usize,
    pub expression: ExpressionHandle,
    pub operands: HandleSpan<FlowOperandFact>,
}

/// One copy completed before selected target operands. `source` is the actual
/// entry-time value observation; the reference inventory retains descriptors,
/// never a snapshot of their referee contents.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowCopiedValue {
    pub source: Handle<FlowOperandFact>,
    pub references: HandleSpan<FlowCapturedReferenceLeaf>,
    pub uses: HandleSpan<FlowCopiedValueUse>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowCapturedReferenceLeaf {
    pub path: HandleSpan<facts::PlaceSegment>,
    pub type_reference: typed_trees::types::TypeReferenceHandle,
    pub referents: HandleSpan<FlowOperandReferent>,
    pub retention: Handle<FlowOperandRetention>,
    pub constraints: HandleSpan<FlowConstraintRef>,
}

/// An actual read tied to its selected copy, retained by the evaluator rather
/// than rediscovered by scanning unrelated operands or matching source paths.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowCopiedValueUse {
    pub expression: ExpressionHandle,
    pub evaluation: super::FlowPermissionEvaluation,
}
