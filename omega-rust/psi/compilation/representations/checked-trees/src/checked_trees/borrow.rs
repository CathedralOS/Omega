use arena::{Arena, Handle, HandleSpan, OwnedIndex};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

/// Handle-keyed maps take the integer hasher; none is iterated for output order.
type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;

/// One exact structural place captured for compatibility checking.
///
/// Identity is the storage namespace plus resolved root and ordered semantic
/// path. An incoming descriptor has an opaque referent namespace, distinct
/// from the parameter storage that holds it.
/// Named storage and computed backing retain the same canonical root vocabulary
/// as flow places. Dropping an expression root would erase a temporary's loan
/// before the ordinary transfer and escape judgments can inspect it.
/// Source labels are deliberately absent. Runtime or otherwise unresolved
/// selectors may remain as expression handles, but cannot establish a positive
/// spatial result until a checked tactic understands them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapturedPlace {
    /// A parameter supplies this opaque referent occurrence. Zero denotes
    /// ordinary storage; an incoming descriptor's referent is not its slot.
    pub incoming_reference: Handle<IncomingReferenceIdentity>,
    pub root: facts::PlaceRoot,
    pub segments: Vec<facts::PlaceSegment>,
}

/// One reference permission supplied by a state invocation's declared carrier.
/// The caller must supply its actual backing; this identity distinguishes the
/// callee-relative referent from the storage holding its descriptor. Moving a
/// descriptor retains this identity, rather than creating a borrow of its slot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IncomingReferenceIdentity {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub origin: IncomingReferenceOrigin,
    pub carrier_type: typed_trees::types::TypeReferenceHandle,
    pub owner_path: HandleSpan<BorrowLoanOwnerSegment>,
}

/// The checked boundary that supplies a reference permission. An invocation
/// result keeps its exact performed call; it is never its temporary descriptor
/// slot and never proves spatial separation from another incoming permission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncomingReferenceOrigin {
    Parameter {
        symbol: SymbolHandle,
    },
    Invocation {
        expression: ExpressionHandle,
        target: SymbolHandle,
    },
}

impl Default for IncomingReferenceOrigin {
    fn default() -> Self {
        Self::Parameter {
            symbol: SymbolHandle::invalid(),
        }
    }
}

impl IncomingReferenceIdentity {
    pub fn parameter_symbol(&self) -> SymbolHandle {
        match self.origin {
            IncomingReferenceOrigin::Parameter { symbol } => symbol,
            IncomingReferenceOrigin::Invocation { .. } => SymbolHandle::invalid(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CapturedPlaceContainment {
    #[default]
    None,
    Same,
    LeftContainsRight,
    RightContainsLeft,
}

/// Transient result of the one checked captured-place compatibility judgment.
///
/// These conclusions are deliberately independent: two places can be both
/// disjoint and non-interfering, while two shared reads can be non-interfering
/// even when one place contains the other. Access polarity is an input to the
/// judgment and is not retained here as proof authority.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapturedPlaceCompatibility {
    pub left: CapturedPlace,
    pub right: CapturedPlace,
    pub disjoint: bool,
    pub containment: CapturedPlaceContainment,
    pub non_interfering: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BorrowRootKind {
    #[default]
    OwnedData,
    LocalData,
    MutableParameter,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowWritableRootFact {
    pub symbol: SymbolHandle,
    pub kind: BorrowRootKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub writable_roots: HandleSpan<BorrowWritableRootFact>,
    pub mutable_parameter_count: usize,
    pub calls: HandleSpan<BorrowCallFact>,
    pub loans: HandleSpan<BorrowLoanFact>,
    pub returned_loans: HandleSpan<BorrowReturnedLoanFact>,
}

/// One reference leaf formed by a normal return. Its origin is captured while
/// the statement's current carrier contents and incoming descriptors are known.
/// These are result leaves, not local activations or lifecycle resource events.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowReturnedLoanFact {
    pub lineage: BorrowLoanLineage,
    pub statement_index: usize,
    /// The selected normal-return arm. Zero denotes a final expression.
    /// Mutually exclusive arms never authenticate each other's formations.
    pub transition_target: typed_trees::statement::TransitionTargetHandle,
    pub formation_expression: ExpressionHandle,
    pub incoming_reference: Handle<IncomingReferenceIdentity>,
    pub root: facts::PlaceRoot,
    pub segments: HandleSpan<facts::PlaceSegment>,
    pub owner_path: HandleSpan<BorrowLoanOwnerSegment>,
    pub kind: BorrowAccessKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BorrowAccessKind {
    #[default]
    Read,
    Mutable,
    WriteOnly,
}

impl BorrowAccessKind {
    pub fn is_exclusive(&self) -> bool {
        matches!(self, Self::Mutable | Self::WriteOnly)
    }

    /// Classify the exact checked-only lifecycle effect of a direct reborrow.
    ///
    /// This is deliberately distinct from [`Self::is_exclusive`]: mutable
    /// authority may attenuate to write-only authority, while write-only
    /// authority may never acquire observation. The result records resource
    /// bookkeeping only and grants no restored-use or Terminal authority.
    pub fn direct_reborrow_effect(&self, child: &Self) -> Option<CheckedReborrowAccessEffect> {
        match (self, child) {
            (Self::Read, Self::Read) => Some(CheckedReborrowAccessEffect::SharedRelease),
            (Self::Mutable, Self::Read) => Some(CheckedReborrowAccessEffect::SharedFreeze),
            (Self::Mutable, Self::Mutable)
            | (Self::Mutable, Self::WriteOnly)
            | (Self::WriteOnly, Self::WriteOnly) => {
                Some(CheckedReborrowAccessEffect::ExclusiveSuspension)
            }
            (Self::Read, Self::Mutable)
            | (Self::Read, Self::WriteOnly)
            | (Self::WriteOnly, Self::Read)
            | (Self::WriteOnly, Self::Mutable) => None,
        }
    }
}

/// Exact non-authorizing lifecycle class for one retained direct reborrow.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CheckedReborrowAccessEffect {
    /// A shared child of shared authority releases independently; the parent
    /// was never suspended and requires no restoration event.
    #[default]
    SharedRelease,
    /// Shared descendants freeze a mutable parent as one finite cohort. The
    /// original mutable access is classified for restoration only after the
    /// final member ends.
    SharedFreeze,
    /// One exclusive descendant suspends its exact parent occurrence.
    ExclusiveSuspension,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowArgumentAccessFact {
    /// The evaluated operand that produced this access in its owning call's
    /// ordered roster. Equal places do not identify equal evaluations.
    pub expression: ExpressionHandle,
    pub root: facts::PlaceRoot,
    pub segments: HandleSpan<facts::PlaceSegment>,
    pub kind: BorrowAccessKind,
    /// First subsequent access in this call's roster after this observation
    /// ends. None retains the access through invocation. Copied reads end
    /// immediately; temporary nested-call loans end when that call returns.
    /// A payload a transition arm's dispatch copied out of its saved subject
    /// ended before the target's first access: 0.
    pub end_before_access: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowCallFact {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub receiver_symbol: SymbolHandle,
    /// Exact executable callee state, or the resolved signature declaration
    /// when no body is available. The source occurrence separately retains
    /// whether this is an ordinary invocation or an internal state transfer.
    pub target_symbol: SymbolHandle,
    pub has_receiver: bool,
    pub accesses: HandleSpan<BorrowArgumentAccessFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowLoanFact {
    pub incoming_reference: Handle<IncomingReferenceIdentity>,
    pub statement_index: usize,
    /// The evaluated source occurrence that makes this loan available within
    /// its statement. Zero denotes a statement-call completion. Source replay
    /// reconstructs this identity; equal storage does not imply equal timing.
    pub formation_expression: ExpressionHandle,
    /// Input storage held by the originating call, distinct from this leaf's
    /// exact referent. Transport keeps the occurrence and captured prefix.
    pub retained_operand: Option<BorrowRetainedOperand>,
    pub last_use_statement_index: usize,
    pub owner_symbol: SymbolHandle,
    /// Storage holding the descriptor, distinct from the lexical binding that
    /// keeps it live. A reference to computed aggregate backing can keep its
    /// nested descriptors live without storing them in the reference variable.
    pub storage_owner: facts::PlaceRoot,
    /// Projection within the owner that carries this loan. An empty path means
    /// the whole owner; dynamic indexes conservatively overlap every element.
    pub owner_path: HandleSpan<BorrowLoanOwnerSegment>,
    pub source_owner_symbol: SymbolHandle,
    /// Immediate historical descriptor alternatives transported into this
    /// leaf, in `BorrowFacts::loan_suppliers`. These are not reborrow parents.
    /// A shared dependency graph retains every supplier without enumerating
    /// complete histories when conditional replacements repeatedly converge.
    pub supplying_loans: HandleSpan<Handle<BorrowLoanFact>>,
    /// Checked formation lineage for this loan occurrence.
    ///
    /// An explicit reborrow with one exact prior source occurrence names a
    /// parent regardless of the descriptor's storage shape. Transports and
    /// ambiguous source aliases do not manufacture a reborrow relationship.
    pub lineage: BorrowLoanLineage,
    pub root: facts::PlaceRoot,
    pub segments: HandleSpan<facts::PlaceSegment>,
    pub kind: BorrowAccessKind,
}

/// A returned view retains its originating call's supplied input region.
/// The depth is a prefix of the loan's captured segments in the same root and
/// incoming-reference namespace. Some(depth == segments.len()) still records
/// an identity call's obligation; ordinary reborrows have no such obligation.
/// The occurrence identifies sibling results of one invocation, which may
/// use their distinct referents while external access keeps the whole input
/// suspended. It never exempts overlapping referents from compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorrowRetainedOperand {
    pub formation_expression: ExpressionHandle,
    pub depth: usize,
}

/// Checked-only formation lineage for one loan occurrence.
///
/// This classification does not grant authority or change borrow admission.
/// In particular, `UnretainedDerived` is a fence rather than an inferred
/// parent relation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BorrowLoanLineage {
    #[default]
    DirectRoot,
    Reborrow {
        parent_loan: Handle<BorrowLoanFact>,
    },
    UnretainedDerived,
    /// An opaque output retains this input's lifetime, but does not establish
    /// that its pointer addresses any part of the input's storage.
    LifetimeDependency,
}

impl BorrowLoanLineage {
    /// Transport custody without inventing a spatial reborrow predecessor.
    pub fn derived(&self) -> Self {
        match self {
            Self::LifetimeDependency => Self::LifetimeDependency,
            _ => Self::UnretainedDerived,
        }
    }
}

/// Exact state-invocation parent lifetime for one direct-root loan.
///
/// This checked-only identity does not create authority. It names the
/// state-owned root whose already-validated authority is temporarily
/// constrained by the loan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedDirectBorrowParentLifetime {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub root: facts::PlaceRoot,
}

/// A borrowed leaf of one completed call operand. Its owning evaluation and
/// checked access stay distinct from another operand at equal storage.
/// Construction records existing authority, not permission to park.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedCallBorrowLoanResource {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub call: Handle<crate::FlowCallFact>,
    pub owner_operand: Handle<crate::FlowOperandFact>,
    pub operand: Handle<crate::FlowOperandFact>,
    pub owner_path: HandleSpan<BorrowLoanOwnerSegment>,
    /// The same leaf in the closed actual carrier's structural namespace.
    pub capture_path: HandleSpan<crate::CheckedUnitStructuralPathSegment>,
    /// Exact stored occurrence or invocation access supplying this permission.
    pub source: CheckedCallBorrowLoanSource,
    pub access: BorrowAccessKind,
    pub parent_lifetime: CheckedDirectBorrowParentLifetime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedCallBorrowLoanSource {
    /// Transport preserves the exact stored occurrence observed at completion.
    StoredLoan { loan: Handle<BorrowLoanFact> },
    /// The call holds this access until completion. The referent and ancestors
    /// were captured at this operand's evaluation, not at invocation entry.
    CallAccess {
        access: Handle<BorrowArgumentAccessFact>,
        parents: HandleSpan<Handle<BorrowLoanFact>>,
    },
}

impl Default for CheckedCallBorrowLoanSource {
    fn default() -> Self {
        Self::StoredLoan {
            loan: Handle::invalid(),
        }
    }
}

/// Restoration obligation retained for one direct-root loan.
///
/// The row records where checked flow ends the loan and must restore the
/// parent root's availability. It is not evidence that restoration occurred
/// and grants no compatibility authority.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedDirectBorrowRestorationObligation {
    pub parent: CheckedDirectBorrowParentLifetime,
    pub weakening_source: crate::FlowInvalidationSource,
    pub weakening_reason: crate::FlowBorrowWeakeningReason,
}

/// Checked-only resource closure for one state-local loan captured directly
/// from a root authority occurrence.
///
/// `lineage` must be `DirectRoot`. Compatibility certificates remain a
/// separate proof ledger and cannot manufacture one of these rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedDirectBorrowLoanResource {
    pub loan: Handle<BorrowLoanFact>,
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub owner_symbol: SymbolHandle,
    pub owner_path: Vec<BorrowLoanOwnerSegment>,
    pub captured_place: CapturedPlace,
    pub access: BorrowAccessKind,
    pub activation_source: crate::FlowInvalidationSource,
    pub weakening_source: crate::FlowInvalidationSource,
    pub weakening_reason: crate::FlowBorrowWeakeningReason,
    pub parent_lifetime: CheckedDirectBorrowParentLifetime,
    pub restoration: CheckedDirectBorrowRestorationObligation,
}

/// Typed parent-resource identity for one retained direct reborrow.
///
/// The handle points into one of the two checked-only resource arenas. It
/// records the immediate resource occurrence and is not authority to use or
/// reactivate that resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedParentBorrowResource {
    DirectRoot {
        resource: Handle<CheckedDirectBorrowLoanResource>,
    },
    Reborrow {
        resource: Handle<CheckedReborrowLoanResource>,
    },
}

impl Default for CheckedParentBorrowResource {
    fn default() -> Self {
        Self::DirectRoot {
            resource: Handle::invalid(),
        }
    }
}

/// Pending restoration obligation for one retained direct reborrow.
///
/// This row binds the child's weakening to its immediate parent occurrence.
/// It does not prove that the parent remained active, that the two lifetimes
/// are temporally contained, or that the parent was reactivated.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedReborrowRestorationObligation {
    pub child_loan: Handle<BorrowLoanFact>,
    pub parent_loan: Handle<BorrowLoanFact>,
    pub parent_resource: CheckedParentBorrowResource,
    pub child_weakening_source: crate::FlowInvalidationSource,
    pub child_weakening_reason: crate::FlowBorrowWeakeningReason,
}

/// Exact checked-flow boundary at which an explicit direct reborrow suspends
/// use through its immediate parent occurrence.
///
/// The parent constraint is the unique occurrence present immediately before
/// the child activation. This checked-only join says nothing about the parent
/// after formation: it is not interval containment, reactivation, completed
/// restoration, or Terminal authority.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedReborrowParentSuspensionBoundary {
    pub child_loan: Handle<BorrowLoanFact>,
    pub parent_loan: Handle<BorrowLoanFact>,
    pub parent_resource: CheckedParentBorrowResource,
    pub child_activation: Handle<crate::FlowBorrowActivationFact>,
    pub parent_entry_constraint: Handle<crate::FlowConstraintRef>,
    pub source: crate::FlowInvalidationSource,
}

/// Lexical disposition of the immediate parent carrier at the child's end.
///
/// This classification describes only source/flow liveness. In particular,
/// `LivePastChild` is not proof of authority reactivation, and a retired parent
/// does not imply that authority was returned, cascaded, or discarded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ParentLexicalStatusAtChildEnd {
    RetiredBeforeChild,
    #[default]
    RetiredWithChild,
    LivePastChild,
}

/// Exact checked weakening-order join for one retained direct reborrow.
///
/// The two handles identify the authoritative flow events; `status` is derived
/// from their semantic statement phases rather than arena insertion order.
/// This checked-only row grants no suspension-containment, restoration,
/// reactivation, cascade, or Terminal authority.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedReborrowParentEndStatus {
    pub child_loan: Handle<BorrowLoanFact>,
    pub parent_loan: Handle<BorrowLoanFact>,
    pub parent_resource: CheckedParentBorrowResource,
    pub child_weakening: Handle<crate::FlowBorrowWeakeningFact>,
    pub parent_weakening: Handle<crate::FlowBorrowWeakeningFact>,
    pub status: ParentLexicalStatusAtChildEnd,
}

/// Semantic phase used by checked-only borrow-resource lifecycle replay.
///
/// The ordering is source-defined rather than arena-defined: last-use expiry
/// precedes statement entry. A replacement's right-hand side forms its loans
/// while the displaced carrier is still live; only then does reassignment
/// retire that carrier. In particular, a stored view can be rebound to its own
/// child without forming a loan from an already retired parent. State exit is
/// the final boundary. These phases do not grant authority by themselves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckedBorrowResourceLifecyclePhase {
    #[default]
    LastUseExpired,
    Activation,
    LocalReassigned,
    StateExit,
}

/// Final checked-only target of one reborrow disposition event.
///
/// A retained resource target names an exact loan occurrence. A direct-root
/// lifetime target names the state-local root beyond the last retired loan
/// carrier. Neither variant proves that authority was returned to the target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedBorrowResourceDispositionTarget {
    ParentResource(CheckedParentBorrowResource),
    DirectRootLifetime(CheckedDirectBorrowParentLifetime),
}

impl Default for CheckedBorrowResourceDispositionTarget {
    fn default() -> Self {
        Self::ParentResource(CheckedParentBorrowResource::default())
    }
}

/// Checked-only disposition selected when an available reborrow carrier ends.
///
/// These names classify the resource-ledger route required at the boundary;
/// they are not evidence of completed return, post-return use legality,
/// cleanup, or Terminal authority.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CheckedReborrowResourceDisposition {
    /// One exclusive branch ended while a sibling still retained its own
    /// projection. This does not restore whole-parent authority.
    BranchRelease,
    /// One shared descendant ended without restoring parent authority. For a
    /// `Read` parent no restoration is required; for a mutable parent this is
    /// a non-final shared-cohort release.
    SharedRelease,
    /// The final member of one exact shared cohort ended and the checked
    /// classifier restored its still-live mutable parent in replay state only.
    RestoreSharedCohort,
    #[default]
    Reactivate,
    CascadeThroughRetiredParent,
    /// The child and its final retained parent carrier close at the same
    /// non-authorizing semantic boundary. This closes the checked lineage; it
    /// does not return root custody or authorize disposal.
    SameBoundaryLineageClosure,
    /// A state-exit boundary reaches the exact direct-root lifetime after its
    /// final retained descendant closes. This is checked-only root handoff and
    /// grants no cleanup, transfer, or linear-discharge authority.
    StateExitDirectRootHandoff,
}

/// One retired carrier traversed by a cascading disposition.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedRetiredParentResourceDispositionStep {
    pub resource: CheckedParentBorrowResource,
    pub weakening: Handle<crate::FlowBorrowWeakeningFact>,
}

/// One independently replayable checked-only reborrow disposition event.
///
/// `retired_parent_path` is ordered from the immediate retired parent toward
/// the final target. An empty path is required for direct reactivation. The
/// row preserves exact resource and flow handles but authorizes no use.
///
/// When the event is a checked post-restoration call use — the child's
/// weakening boundary is the exact receiver-free, single-argument mutating
/// call that consumes the directly restored mutable parent — `restored_call`,
/// `restored_borrow_call` and `restored_call_access` name that call and its
/// whole-root argument access. All three stay absent handles otherwise.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedReborrowResourceDispositionEvent {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub child_loan: Handle<BorrowLoanFact>,
    pub child_resource: Handle<CheckedReborrowLoanResource>,
    pub child_activation: Handle<crate::FlowBorrowActivationFact>,
    pub child_weakening: Handle<crate::FlowBorrowWeakeningFact>,
    pub parent_loan: Handle<BorrowLoanFact>,
    pub parent_resource: CheckedParentBorrowResource,
    pub boundary_source: crate::FlowInvalidationSource,
    pub boundary_phase: CheckedBorrowResourceLifecyclePhase,
    /// Exact shared cohort observed at this boundary. Exclusive events retain
    /// an empty roster; shared-release events keep at least their child.
    pub shared_cohort: Vec<Handle<CheckedReborrowLoanResource>>,
    pub retired_parent_path: Vec<CheckedRetiredParentResourceDispositionStep>,
    pub final_target: CheckedBorrowResourceDispositionTarget,
    pub disposition: CheckedReborrowResourceDisposition,
    pub restored_call: Handle<crate::FlowCallFact>,
    pub restored_borrow_call: Handle<BorrowCallFact>,
    pub restored_call_access: Handle<BorrowArgumentAccessFact>,
}

/// Checked-only resource closure for one explicit direct reborrow.
///
/// The row retains the child's exact activation/weakening lifecycle and a
/// typed link to the immediate checked parent resource. It is a pending
/// restoration obligation only: aggregate transfers, temporal containment,
/// parent activity/reactivation, and Terminal authority remain outside this
/// representation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedReborrowLoanResource {
    pub loan: Handle<BorrowLoanFact>,
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub owner_symbol: SymbolHandle,
    pub owner_path: Vec<BorrowLoanOwnerSegment>,
    pub captured_place: CapturedPlace,
    pub access: BorrowAccessKind,
    pub parent_access: BorrowAccessKind,
    pub access_effect: CheckedReborrowAccessEffect,
    pub activation_source: crate::FlowInvalidationSource,
    pub weakening_source: crate::FlowInvalidationSource,
    pub weakening_reason: crate::FlowBorrowWeakeningReason,
    pub parent_loan: Handle<BorrowLoanFact>,
    pub parent_resource: CheckedParentBorrowResource,
    pub parent_suspension: CheckedReborrowParentSuspensionBoundary,
    pub parent_end_status: CheckedReborrowParentEndStatus,
    pub restoration: CheckedReborrowRestorationObligation,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BorrowLoanOwnerSegment {
    Field(SymbolHandle),
    Case(SymbolHandle),
    FixedIndex(usize),
    #[default]
    DynamicIndex,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowFacts {
    pub incoming_references: Arena<IncomingReferenceIdentity>,
    pub writable_roots: Arena<BorrowWritableRootFact>,
    pub access_segments: Arena<facts::PlaceSegment>,
    pub owner_segments: Arena<BorrowLoanOwnerSegment>,
    pub argument_accesses: Arena<BorrowArgumentAccessFact>,
    pub calls: Arena<BorrowCallFact>,
    pub loans: Arena<BorrowLoanFact>,
    pub loan_suppliers: Arena<Handle<BorrowLoanFact>>,
    pub returned_loans: Arena<BorrowReturnedLoanFact>,
    pub states: Arena<StateBorrowFact>,
    /// `(machine, state) -> state row` lookup owned by this ledger. The
    /// index carries the `states` arena's revision, so a mutation rebuilds
    /// it on the next query; the ledger's lifetime bounds it.
    pub state_index: OwnedIndex<BorrowStateIndex>,
    /// Non-authorizing direct-root resource closures reconstructed from the
    /// exact loan activation/weakening ledger. Reborrow parent identity is
    /// retained separately.
    pub direct_loan_resources: Arena<CheckedDirectBorrowLoanResource>,
    /// Non-authorizing resource closures for explicit direct-reborrow lineage.
    /// Rows form a topological arena through typed parent
    /// resource handles. Aggregate and otherwise unretained transfers have no
    /// row.
    pub reborrow_loan_resources: Arena<CheckedReborrowLoanResource>,
    pub call_loan_resources: Arena<CheckedCallBorrowLoanResource>,
    pub call_loan_parents: Arena<Handle<BorrowLoanFact>>,
    pub call_loan_paths: Arena<crate::CheckedUnitStructuralPathSegment>,
    /// Checked-only resource-lifecycle dispositions for available direct
    /// reborrows at their weakening boundary. Suspended carriers remain
    /// pending and therefore have no premature disposition row. An event whose
    /// boundary is a checked post-restoration mutating call also names that
    /// call through its `restored_*` handles.
    pub reborrow_disposition_events: Arena<CheckedReborrowResourceDispositionEvent>,
}

impl BorrowFacts {
    pub fn with_roots(
        incoming_references: Arena<IncomingReferenceIdentity>,
        writable_roots: Arena<BorrowWritableRootFact>,
        access_segments: Arena<facts::PlaceSegment>,
        owner_segments: Arena<BorrowLoanOwnerSegment>,
        argument_accesses: Arena<BorrowArgumentAccessFact>,
        calls: Arena<BorrowCallFact>,
        loans: Arena<BorrowLoanFact>,
        loan_suppliers: Arena<Handle<BorrowLoanFact>>,
        returned_loans: Arena<BorrowReturnedLoanFact>,
        states: Arena<StateBorrowFact>,
    ) -> Self {
        Self {
            incoming_references,
            writable_roots,
            access_segments,
            owner_segments,
            argument_accesses,
            calls,
            loans,
            loan_suppliers,
            returned_loans,
            states,
            state_index: OwnedIndex::default(),
            direct_loan_resources: Arena::new(),
            reborrow_loan_resources: Arena::new(),
            call_loan_resources: Arena::new(),
            call_loan_parents: Arena::new(),
            call_loan_paths: Arena::new(),
            reborrow_disposition_events: Arena::new(),
        }
    }

    pub fn access_segments(&self, access: &BorrowArgumentAccessFact) -> &[facts::PlaceSegment] {
        self.access_segments.span_or_empty(access.segments)
    }

    pub fn loan_segments(&self, loan: &BorrowLoanFact) -> &[facts::PlaceSegment] {
        self.access_segments.span_or_empty(loan.segments)
    }

    pub fn loan_owner_path(&self, loan: &BorrowLoanFact) -> &[BorrowLoanOwnerSegment] {
        self.owner_segments.span_or_empty(loan.owner_path)
    }

    pub fn state_owns_loan(&self, state: &StateBorrowFact, loan: Handle<BorrowLoanFact>) -> bool {
        self.loans_contain(state.loans, loan)
    }

    /// Membership retains the loan arena's generation, not just its index.
    pub fn loans_contain(
        &self,
        loans: HandleSpan<BorrowLoanFact>,
        loan: Handle<BorrowLoanFact>,
    ) -> bool {
        self.loans.is_valid(loan) && handle_span_contains(loans, loan)
    }

    /// Borrow the recorded group with each source's durable identity. One
    /// descriptor can retain several alternative sources without minting a new
    /// loan or allocating a second roster.
    pub fn loan_group(
        &self,
        loans: HandleSpan<BorrowLoanFact>,
    ) -> Option<impl Iterator<Item = (Handle<BorrowLoanFact>, &BorrowLoanFact)>> {
        let sources = self.loans.span(loans)?;
        Some(
            sources
                .iter()
                .enumerate()
                .map(move |(loan_ordinal, source)| (loans.handle_at(loan_ordinal as u32), source)),
        )
    }

    /// Borrow production appends each statement's local loans together. Retain
    /// that complete group without a second roster. An interleaved group is a
    /// malformed producer relationship, not permission to select a subset.
    pub fn local_loans(
        &self,
        state: &StateBorrowFact,
        owner: SymbolHandle,
        statement_index: usize,
    ) -> Option<HandleSpan<BorrowLoanFact>> {
        self.storage_loans(state, facts::PlaceRoot::Symbol(owner), statement_index)
    }

    /// One storage occurrence owns its descriptor roster independently of the
    /// lexical binding that keeps it live. Computed backing uses the same join.
    pub fn storage_loans(
        &self,
        state: &StateBorrowFact,
        owner: facts::PlaceRoot,
        statement_index: usize,
    ) -> Option<HandleSpan<BorrowLoanFact>> {
        let mut group = HandleSpan::empty();
        for (handle, loan) in self.loan_group(state.loans)? {
            if loan.storage_owner != owner || loan.statement_index != statement_index {
                continue;
            }
            if !group.is_empty() && !group.is_followed_by(handle) {
                return None;
            }
            let start = if group.is_empty() {
                handle
            } else {
                group.start()
            };
            group = HandleSpan::from_parts(start, group.count().checked_add(1)?);
        }
        Some(group)
    }
}

fn handle_span_contains<T>(span: HandleSpan<T>, handle: Handle<T>) -> bool {
    handle.is_valid()
        && !span.is_empty()
        && handle.generation() == span.start().generation()
        && handle.arena_index() >= span.start().arena_index()
        && handle.arena_index() < span.start().arena_index().saturating_add(span.count())
}

/// `states` rows bucketed by `(machine symbol, state symbol)`. A state's
/// borrow row is addressed by its recorded identity rather than position,
/// so consumers look rows up through this index instead of rescanning the
/// arena — one build per ledger revision replaces O(states × queries).
#[derive(Clone, Default)]
pub struct BorrowStateIndex {
    by_key: HashMap<(SymbolHandle, SymbolHandle), Vec<Handle<StateBorrowFact>>>,
}

impl BorrowStateIndex {
    /// The rows recorded under `(machine, state)`, in arena order.
    pub fn bucket(
        &self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    ) -> &[Handle<StateBorrowFact>] {
        self.by_key
            .get(&(machine_symbol, state_symbol))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

impl BorrowFacts {
    /// Run `run` against this ledger's owned `(machine, state)` row index,
    /// rebuilding when the `states` arena's revision moved.
    pub fn with_state_index<R>(&self, run: impl FnOnce(&BorrowStateIndex) -> R) -> R {
        self.state_index.with(
            self.states.revision(),
            || {
                let mut index = BorrowStateIndex::default();
                for (handle, state) in self.states.iter() {
                    index
                        .by_key
                        .entry((state.machine_symbol, state.state_symbol))
                        .or_default()
                        .push(handle);
                }
                index
            },
            run,
        )
    }
}

#[cfg(test)]
mod index_tests {
    use super::{BorrowFacts, StateBorrowFact, SymbolHandle};

    #[test]
    fn state_index_preserves_duplicate_order_and_mutation_visibility() {
        let machine = SymbolHandle::from_parts(1, 1);
        let state = SymbolHandle::from_parts(2, 1);
        let replacement = SymbolHandle::from_parts(2, 2);
        let mut facts = BorrowFacts::default();
        let queries = [
            (machine, state),
            (machine, replacement),
            (SymbolHandle::invalid(), state),
        ];
        let assert_matches_roster = |facts: &BorrowFacts| {
            for (machine, state) in queries {
                let expected = facts
                    .states
                    .iter()
                    .filter(|(_, row)| row.machine_symbol == machine && row.state_symbol == state)
                    .map(|(handle, _)| handle)
                    .collect::<Vec<_>>();
                facts.with_state_index(|index| assert_eq!(index.bucket(machine, state), expected));
            }
        };
        assert_matches_roster(&facts);
        let first = facts.states.insert(StateBorrowFact {
            machine_symbol: machine,
            state_symbol: state,
            ..StateBorrowFact::default()
        });
        assert_matches_roster(&facts);
        facts.states.insert(StateBorrowFact {
            machine_symbol: machine,
            state_symbol: state,
            ..StateBorrowFact::default()
        });
        assert_matches_roster(&facts);
        let snapshot = facts.clone();
        facts.states.get_mut(first).state_symbol = replacement;
        assert_matches_roster(&facts);
        assert_matches_roster(&snapshot);
    }
}
