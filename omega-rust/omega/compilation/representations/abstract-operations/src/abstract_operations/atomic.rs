//! Normalized atomic memory events and their proof-static ordering evidence.
//!
//! The concurrency contract makes ordering explicit proof-static operation
//! data rather than a runtime policy choice: a load admits
//! `NoOrdering | Receive | GlobalOrder`, a store admits
//! `NoOrdering | Publish | GlobalOrder`, read-modify-write success admits all
//! five orderings, and a compare-exchange failure ordering is read-only — it
//! cannot publish and cannot exceed the success ordering. `Atomic::fence`
//! accepts a dedicated `Receive | Publish | ReceivePublish` ordering and
//! emits a normalized event even when target refinement permits no machine
//! instruction.
//!
//! Each event retains its admitted ordering so independent verification can
//! reconstruct the legality relation instead of trusting producer assertion;
//! [`AbstractAtomicEvent::ordering_is_legal`] is that recheck. A `Swap` or
//! read-modify-write `prior` binds the resident value the atomic instruction
//! itself observed — a separately loaded prior is not a valid implementation.
//! `CompareExchangeOnce` keeps its three-case structural outcome and the
//! canonical single-attempt custody identity so an uncommitted attempt
//! retains its distinct outcome and custody rather than erasing into the
//! decisive scalar observed carrier.
//!
//! The concurrency contract's `reads_from`/`modification_order` axioms are
//! retained likewise: every observing event carries an [`AtomicReadsFrom`]
//! edge naming the write it claims to have read, every write event carries
//! an [`AtomicModificationAfter`] edge naming the member it immediately
//! follows, and [`happens_before_atomic_coherence_violation`]
//! independently replays the coherence axiom under the activation's
//! bounded `happens_before` derivation — a named write must happen before
//! the event and be the modification-order-latest write to the place on
//! every execution path — instead of trusting the producer's claim.
//! An `AfterEffect` boundary instead records the exact latest opaque access,
//! independently replayed on every path. It expires knowledge of the writer
//! inside that access: the next atomic instruction observes current residency,
//! never an inferred stored value. This is serial sequencing evidence, not a
//! cross-activation reads-from or synchronization claim.
//! `synchronizes_with`, `global_sequential_order`, and fence-pair
//! synchronization constrain only cross-activation observation, and land
//! with the concurrent-execution route.

use std::collections::{BTreeMap, BTreeSet};

use language_core::atomic::{
    AtomicCompareExchangeOnceResultCustody, AtomicExpressionResultCustody, AtomicOrderingPlan,
    MemoryOrdering,
};
use semantic_vocabulary::{BlockId, OperationId, PlaceId, StructuralFieldId, ValueId};
use terminal_psi::{StructuralOperationResult, StructuralPathSegment};

use crate::{AbstractOperation, AbstractResult};

/// The fetch-family arithmetic one normalized atomic read-modify-write
/// performs. `AtomicOrderingPlan::ReadModifyWrite` deliberately erases this
/// distinction into one ordering class; the event retains it because each
/// kind selects a different target instruction under refinement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbstractAtomicReadModifyWrite {
    FetchAdd,
    FetchSub,
    FetchXor,
    FetchOr,
    FetchAnd,
}

impl From<terminal_psi::AtomicReadModifyWrite> for AbstractAtomicReadModifyWrite {
    fn from(operation: terminal_psi::AtomicReadModifyWrite) -> Self {
        match operation {
            terminal_psi::AtomicReadModifyWrite::FetchAdd => Self::FetchAdd,
            terminal_psi::AtomicReadModifyWrite::FetchSub => Self::FetchSub,
            terminal_psi::AtomicReadModifyWrite::FetchAnd => Self::FetchAnd,
            terminal_psi::AtomicReadModifyWrite::FetchOr => Self::FetchOr,
            terminal_psi::AtomicReadModifyWrite::FetchXor => Self::FetchXor,
        }
    }
}

impl From<AbstractAtomicReadModifyWrite> for terminal_psi::AtomicReadModifyWrite {
    fn from(operation: AbstractAtomicReadModifyWrite) -> Self {
        match operation {
            AbstractAtomicReadModifyWrite::FetchAdd => Self::FetchAdd,
            AbstractAtomicReadModifyWrite::FetchSub => Self::FetchSub,
            AbstractAtomicReadModifyWrite::FetchAnd => Self::FetchAnd,
            AbstractAtomicReadModifyWrite::FetchOr => Self::FetchOr,
            AbstractAtomicReadModifyWrite::FetchXor => Self::FetchXor,
        }
    }
}

/// The dedicated `Atomic::fence` ordering: exactly
/// `Receive | Publish | ReceivePublish`. The dedicated type keeps
/// `NoOrdering` and `GlobalOrder` fences unrepresentable rather than
/// rejected by a later check.
pub use language_core::atomic::FenceOrdering as AbstractAtomicFenceOrdering;

/// The retained `reads_from` edge of one observing atomic event: which
/// write in its place's modification order the event claims to have read.
///
/// The edge names the write by the operation identity under which it stands
/// in the checked activation — never by the value it stored. A load's
/// result is a fresh definition, so value equality cannot witness a read
/// edge, and the concurrency contract's `reads_from` relation exists only
/// through this retained claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicReadsFrom {
    /// The event observes the place's residency as it stood before the
    /// activation: no earlier write to the place may precede the
    /// observation on any execution path.
    InitialResidency,
    /// The event observes the atomic event standing under this operation
    /// identity in the checked activation.
    Write { operation: OperationId },
    /// The latest potentially modifying operation is opaque to this function.
    /// Observe current residency with the atomic instruction, without claiming
    /// which write inside that operation supplied it or retaining old facts.
    AfterEffect { operation: OperationId },
}

/// The retained `modification_order` edge of one write event: which
/// modification-order member the event claims to immediately follow in its
/// place.
///
/// Like [`AtomicReadsFrom`], the edge names the preceding write by the
/// operation identity it stands under rather than by position or value.
/// Inside one activation the modification order is exactly the per-place
/// write sequence the bounded `happens_before` derivation reconstructs, so
/// the claim is checkable: the named write must be the
/// modification-order-latest write to the place on every execution path, or
/// the still-initial residency when no write may precede. Where execution
/// paths converge on different latest writes no single predecessor exists
/// and the event can carry no checkable edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicModificationAfter {
    /// The event is its place's first modification-order member: the
    /// residency it writes over is still initial on every execution path.
    InitialResidency,
    /// The event immediately follows the write standing under this
    /// operation identity in the checked activation.
    Write { operation: OperationId },
    /// Append to current residency after this opaque access. This is a
    /// sequencing boundary, not an assertion about its internal atomic events.
    AfterEffect { operation: OperationId },
}

/// Why a retained [`AtomicReadsFrom`] edge fails the coherence axiom
/// replayed by [`happens_before_atomic_coherence_violation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicReadsFromViolation {
    /// A store or fence carries a reads-from witness although it observes
    /// no resident.
    NonObservingWitness,
    /// An observing event carries no witness: under the coherence axiom
    /// every observation must name the write — or the still-initial
    /// residency — it reads.
    MissingWitness,
    /// The event claims the pre-activation residency although a write to
    /// its place may precede the observation on some execution path.
    InitialResidencyAfterWrite,
    /// The claimed identity resolves to no single atomic event in the
    /// activation: it names nothing atomic, or is ambiguous under a shared
    /// operation identity.
    UnresolvedObservedWrite { claimed: OperationId },
    /// The claimed identity resolves to an event outside the observer's
    /// place's modification order — a different place, or a load or fence
    /// that writes nothing.
    WriteOutsideModificationOrder { claimed: OperationId },
    /// The resolved write does not happen before the observation: it stands
    /// later in the same block or in a block that does not dominate the
    /// observer's, so an execution path reaches the observation without it.
    ObservedWriteNotHappensBefore { claimed: OperationId },
    /// The resolved write happens before the observation but is not the
    /// modification-order-latest write to the observer's place on every
    /// execution path: a later write may supersede it, or the residency may
    /// be initial on a path where the write never ran.
    ObservedWriteOverwritten { claimed: OperationId },
}

/// Why a retained [`AtomicModificationAfter`] edge fails the coherence
/// axiom replayed by [`happens_before_atomic_coherence_violation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicModificationAfterViolation {
    /// A load or fence carries a modification-order edge although it joins
    /// no modification order.
    NonMemberEdge,
    /// A write carries no edge: every modification-order member must name
    /// the member — or the still-initial residency — it immediately
    /// follows.
    MissingPredecessor,
    /// The event claims to follow the pre-activation residency although a
    /// write to its place may precede it on some execution path.
    InitialResidencyAfterWrite,
    /// The claimed identity resolves to no single atomic event in the
    /// activation: it names nothing atomic, or is ambiguous under a shared
    /// operation identity.
    UnresolvedPredecessor { claimed: OperationId },
    /// The claimed identity resolves to an event outside the writer's
    /// place's modification order — a different place, or a load or fence
    /// that writes nothing.
    PredecessorOutsideModificationOrder { claimed: OperationId },
    /// The resolved write does not happen before this event: it stands
    /// later in the same block or in a block that does not dominate the
    /// writer's, so an execution path reaches this write without it.
    PredecessorNotHappensBefore { claimed: OperationId },
    /// The resolved write happens before this event but is not the
    /// modification-order-latest write to its place on every execution
    /// path: a closer write may supersede it, or the residency may be
    /// initial on a path where the claimed write never ran.
    PredecessorNotLatest { claimed: OperationId },
}

/// Why a retained atomic-ordering edge fails the coherence axiom replayed
/// by [`happens_before_atomic_coherence_violation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicCoherenceViolation {
    /// The retained `reads_from` edge fails its replay.
    ReadsFrom(AtomicReadsFromViolation),
    /// The retained `modification_after` edge fails its replay.
    ModificationAfter(AtomicModificationAfterViolation),
}

/// The exact atomic location one event accesses: a scalar field of the
/// record a static carrier path selects beneath a root place. Atomic cells
/// are record fields, so this is the same (root, path, field) triple a scalar
/// field store names. Roots are local storage names: control edges can rename
/// the same storage. Coherence follows those exact bindings before comparing
/// locations; distinct fields or elements remain distinct locations.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbstractAtomicLocation {
    pub root: PlaceId,
    pub path: Vec<StructuralPathSegment>,
    pub field: StructuralFieldId,
}

/// Distinct simultaneous exclusive arrivals have disjoint live storage after
/// ownership validation, and quoted bytes are immutable image storage that
/// holds and reaches no atomic cell — lending them, as a console write of a
/// string literal does, writes nothing. Shared roots, unknown roots, and roots
/// from different arrival groups supply no such evidence; the latter can
/// rename the same home. A stored-reference crossing leaves its carrier storage,
/// so exclusive carrier roots do not establish disjoint referents. The producer
/// naming each event's reaching write and its independent replay share this judgment.
pub fn atomic_projections_may_alias<'parameters>(
    places: &[terminal_psi::StructuralPlaceDeclaration],
    arrivals: impl Iterator<Item = &'parameters [terminal_psi::StructuralParameterDeclaration]>,
    left: PlaceId,
    left_path: &[StructuralPathSegment],
    right: PlaceId,
    right_path: &[StructuralPathSegment],
) -> bool {
    let immutable_image = |root| {
        places.iter().any(|place| {
            place.id == root
                && matches!(
                    place.kind,
                    semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral { .. }
                )
        })
    };
    left == right
        || (!immutable_image(left)
            && !immutable_image(right)
            && (left_path.contains(&StructuralPathSegment::Referent)
                || right_path.contains(&StructuralPathSegment::Referent)
                || !arrivals.into_iter().any(|parameters| {
                    let exclusive = |root| {
                        parameters.iter().any(|parameter| {
                            parameter.place == root
                                && matches!(
                                    parameter.access,
                                    terminal_psi::StructuralAccess::Owned
                                        | terminal_psi::StructuralAccess::MutableBorrow
                                )
                        })
                    };
                    exclusive(left) && exclusive(right)
                })))
}

/// One normalized atomic memory event.
///
/// `location` names the exact atomic location whose per-location
/// modification order the event joins; a fence accesses no location because
/// it orders events rather than observing or modifying one. This is a semantic event, not a
/// target instruction: realization may implement an admitted ordering with a
/// stronger instruction, but may not weaken it or substitute a separately
/// loaded prior for the instruction-observed one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractAtomicEvent {
    /// Observe the resident under `ordering`.
    Load {
        location: AbstractAtomicLocation,
        ordering: MemoryOrdering,
        result: AbstractResult,
    },
    /// Replace the resident with `value` under `ordering`; produces no
    /// result value.
    Store {
        location: AbstractAtomicLocation,
        ordering: MemoryOrdering,
        value: ValueId,
    },
    /// One read-modify-write under `ordering`: `prior` binds the resident
    /// the instruction observed and the resident becomes
    /// `operation(prior, operand)`.
    ReadModifyWrite {
        location: AbstractAtomicLocation,
        operation: AbstractAtomicReadModifyWrite,
        ordering: MemoryOrdering,
        operand: ValueId,
        prior: AbstractResult,
    },
    /// Exchange the resident for `value` under `ordering`; `prior` binds the
    /// displaced resident the instruction observed.
    Swap {
        location: AbstractAtomicLocation,
        ordering: MemoryOrdering,
        value: ValueId,
        prior: AbstractResult,
    },
    /// Decisive observing compare-exchange. When the resident equals
    /// `expected`'s encoding the resident becomes `replacement`; either way
    /// `observed` binds the resident the instruction itself read. Success
    /// applies `success`; the read-only failure applies `failure`, which
    /// cannot publish and cannot exceed `success`.
    CompareExchange {
        location: AbstractAtomicLocation,
        success: MemoryOrdering,
        failure: MemoryOrdering,
        expected: ValueId,
        replacement: ValueId,
        observed: AbstractResult,
    },
    /// One observing compare-exchange attempt. `outcome` carries the
    /// three-case result — `Mismatched(observed)`, `Exchanged`, or
    /// `Uncommitted(observed)` — and `custody` retains the canonical
    /// single-attempt identity. The custody fields are deliberately
    /// redundant with the operation so every phase rechecks their agreement
    /// and a decisive operation, two-arm shape, or sibling outcome identity
    /// cannot substitute under an unchanged ordering plan.
    CompareExchangeOnce {
        location: AbstractAtomicLocation,
        success: MemoryOrdering,
        failure: MemoryOrdering,
        expected: ValueId,
        replacement: ValueId,
        outcome: StructuralOperationResult,
        custody: AtomicCompareExchangeOnceResultCustody,
    },
    /// `Atomic::fence`: one normalized ordering event that accesses no
    /// place. A fence alone publishes no memory; synchronization still
    /// requires a qualifying atomic observation.
    Fence {
        ordering: AbstractAtomicFenceOrdering,
    },
}

impl AbstractAtomicEvent {
    /// Apply `visit` to every `ValueId` the event reads: store and swap
    /// values, the read-modify-write operand, and both compare-exchange
    /// operands. Result bindings (`result`, `prior`, `observed`, `outcome`)
    /// are produced by the event rather than read and are skipped.
    pub fn for_each_input_mut(&mut self, mut visit: impl FnMut(&mut ValueId)) {
        match self {
            Self::Store { value, .. } | Self::Swap { value, .. } => visit(value),
            Self::ReadModifyWrite { operand, .. } => visit(operand),
            Self::CompareExchange {
                expected,
                replacement,
                ..
            }
            | Self::CompareExchangeOnce {
                expected,
                replacement,
                ..
            } => {
                visit(expected);
                visit(replacement);
            }
            Self::Load { .. } | Self::Fence { .. } => {}
        }
    }
}

impl AbstractAtomicEvent {
    /// The location whose modification order this event joins. A fence joins
    /// no modification order because it accesses no location.
    pub const fn location(&self) -> Option<&AbstractAtomicLocation> {
        match self {
            Self::Load { location, .. }
            | Self::Store { location, .. }
            | Self::ReadModifyWrite { location, .. }
            | Self::Swap { location, .. }
            | Self::CompareExchange { location, .. }
            | Self::CompareExchangeOnce { location, .. } => Some(location),
            Self::Fence { .. } => None,
        }
    }

    /// The root place the event's location lies beneath: the structural
    /// custody the event keeps live.
    pub fn place(&self) -> Option<PlaceId> {
        self.location().map(|location| location.root)
    }

    /// The retained proof-static ordering plan. A fence has no plan
    /// analogue: its dedicated ordering already spans exactly the admitted
    /// fence space, so no legality recheck remains to reconstruct.
    pub const fn ordering_plan(&self) -> Option<AtomicOrderingPlan> {
        match self {
            Self::Load { ordering, .. } => Some(AtomicOrderingPlan::Load(*ordering)),
            Self::Store { ordering, .. } => Some(AtomicOrderingPlan::Store(*ordering)),
            Self::ReadModifyWrite { ordering, .. } => {
                Some(AtomicOrderingPlan::ReadModifyWrite(*ordering))
            }
            Self::Swap { ordering, .. } => Some(AtomicOrderingPlan::Swap(*ordering)),
            Self::CompareExchange {
                success, failure, ..
            } => Some(AtomicOrderingPlan::CompareExchange {
                success: *success,
                failure: *failure,
            }),
            Self::CompareExchangeOnce {
                success, failure, ..
            } => Some(AtomicOrderingPlan::CompareExchangeOnce {
                success: *success,
                failure: *failure,
            }),
            Self::Fence { .. } => None,
        }
    }

    /// The checked result-custody identity this event carries. Store and
    /// fence events produce no result and therefore carry no custody.
    pub const fn result_custody(&self) -> Option<AtomicExpressionResultCustody> {
        match self {
            Self::Load { .. }
            | Self::ReadModifyWrite { .. }
            | Self::Swap { .. }
            | Self::CompareExchange { .. } => Some(AtomicExpressionResultCustody::Scalar),
            Self::CompareExchangeOnce { custody, .. } => Some(
                AtomicExpressionResultCustody::ObservingCompareExchangeOnce(*custody),
            ),
            Self::Store { .. } | Self::Fence { .. } => None,
        }
    }

    /// Re-check the source-admitted ordering legality rather than trusting
    /// producer assertion: a load cannot publish, a store cannot receive,
    /// and a compare-exchange failure cannot publish or exceed its success
    /// ordering. Read-modify-write and swap admit all five orderings, and a
    /// fence ordering is legal by construction of its dedicated type.
    pub const fn ordering_is_legal(&self) -> bool {
        match self {
            Self::Load { ordering, .. } => ordering.valid_for_load(),
            Self::Store { ordering, .. } => ordering.valid_for_store(),
            Self::ReadModifyWrite { .. } | Self::Swap { .. } => true,
            Self::CompareExchange {
                success, failure, ..
            }
            | Self::CompareExchangeOnce {
                success, failure, ..
            } => failure.valid_compare_exchange_failure(*success),
            Self::Fence { .. } => true,
        }
    }

    /// Re-check that the retained result custody is valid for the retained
    /// ordering plan. A single-attempt event requires the canonical
    /// observing three-case custody; every other result-carrying event
    /// carries the scalar custody, and result-free events are consistent by
    /// construction.
    pub const fn custody_is_consistent(&self) -> bool {
        match (self.ordering_plan(), self.result_custody()) {
            (Some(plan), Some(custody)) => custody.is_valid_for(plan),
            (None, None) | (_, None) => true,
            (None, Some(_)) => false,
        }
    }

    /// Whether this event observes its place's resident and therefore must
    /// carry a [`AtomicReadsFrom`] edge. Store and fence events observe
    /// nothing and cannot carry one.
    pub const fn observes_resident(&self) -> bool {
        match self {
            Self::Load { .. }
            | Self::ReadModifyWrite { .. }
            | Self::Swap { .. }
            | Self::CompareExchange { .. }
            | Self::CompareExchangeOnce { .. } => true,
            Self::Store { .. } | Self::Fence { .. } => false,
        }
    }

    /// Whether this event appends a member to its place's modification
    /// order. A compare-exchange counts unconditionally: on success it
    /// writes the replacement, and on failure the resident it leaves is the
    /// resident it observed, so a serial observer naming it reads the
    /// correct residency either way. Loads and fences join no modification
    /// order.
    pub const fn joins_modification_order(&self) -> bool {
        match self {
            Self::Store { .. }
            | Self::ReadModifyWrite { .. }
            | Self::Swap { .. }
            | Self::CompareExchange { .. }
            | Self::CompareExchangeOnce { .. } => true,
            Self::Load { .. } | Self::Fence { .. } => false,
        }
    }
}

/// Replay the coherence axiom over one activation's block graph: each
/// observing atomic event's retained [`AtomicReadsFrom`] edge must resolve
/// to a write that happens before the observation and is the
/// modification-order-latest write to its place on every execution path —
/// or claim the pre-activation residency only when no write may precede —
/// and each write event's retained [`AtomicModificationAfter`] edge must
/// name the member it immediately follows under the same derivation.
///
/// Inside one activation `happens_before` is execution-path sequencing:
/// every rule that forms `synchronizes_with` either pairs a publication
/// with an observation on distinct activations — which a single function
/// never expresses — or composes sequenced-before edges already inside the
/// closure. A write therefore happens before an event exactly when
/// it precedes it in their shared block or stands in a block dominating
/// the event's; `predecessors` and `dominators` are the function's
/// validated control-flow relations. A finite backward worklist follows every
/// predecessor while translating the queried root through its exact storage
/// binding. Each path stops at its latest write or at the actual invocation
/// entrance. Only the singleton initial fact or one exact latest writer can
/// authorize a claim; revisiting a cycle supplies no initial-residency fact.
/// `projection_before_edge` receives predecessor, successor and successor-local root;
/// it returns the exact source root and projection prefix. `None` means
/// the edge cannot preserve that storage coordinate.
/// `projections_may_alias` reconstructs current root authority independently: distinct
/// local names alone do not establish disjoint storage.
///
/// `global_sequential_order` and fence-pair synchronization likewise
/// constrain only cross-activation observation; they carry no checkable
/// content inside one activation and land with the concurrent-execution
/// route.
///
/// Non-atomic writes and calls lending the backing invalidate any known writer.
/// An `AfterEffect` boundary must name that exact latest operation on every
/// path; it supplies no knowledge of internal writes or their values. Effects
/// with no retained operation identity and ambiguous frontiers still refuse.
/// Edge bindings compose their exact prefix with the queried path; the field
/// remains unchanged. Projection-changing cycles remain unknown. Returns the
/// offending event's block and node position together with the refusal,
/// or `None` when every retained edge is coherent.
pub fn happens_before_atomic_coherence_violation(
    blocks: &BTreeMap<BlockId, Vec<&AbstractOperation>>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    entry: BlockId,
    projection_before_edge: impl Fn(
        BlockId,
        BlockId,
        PlaceId,
    ) -> Option<(PlaceId, Vec<StructuralPathSegment>)>,
    projections_may_alias: impl Fn(
        PlaceId,
        &[StructuralPathSegment],
        PlaceId,
        &[StructuralPathSegment],
    ) -> bool,
) -> Option<(BlockId, usize, AtomicCoherenceViolation)> {
    // Atomic events claimable by a retained edge, keyed by the operation
    // identity they stand under across the whole activation. Duplicate
    // identities stay listed so an edge naming them refuses as ambiguous.
    let mut events: BTreeMap<OperationId, Vec<(BlockId, usize, &AbstractAtomicEvent)>> =
        BTreeMap::new();
    for (block, operations) in blocks {
        for (index, operation) in operations.iter().enumerate() {
            if let AbstractOperation::AtomicEvent {
                psi_operation,
                event,
                ..
            } = *operation
            {
                events
                    .entry(*psi_operation)
                    .or_default()
                    .push((*block, index, event));
            }
        }
    }
    for (block, operations) in blocks {
        for (index, operation) in operations.iter().enumerate() {
            let AbstractOperation::AtomicEvent {
                event,
                reads_from,
                modification_after,
                ..
            } = *operation
            else {
                continue;
            };
            let reached = event
                .location()
                .map(|location| {
                    reaching_writes(
                        blocks,
                        predecessors,
                        entry,
                        *block,
                        index,
                        location,
                        &projection_before_edge,
                        &projections_may_alias,
                    )
                })
                .unwrap_or_default();
            let violation = match (event.observes_resident(), *reads_from) {
                (false, Some(_)) => Some(AtomicReadsFromViolation::NonObservingWitness),
                (true, None) => Some(AtomicReadsFromViolation::MissingWitness),
                (false, None) => None,
                (true, Some(witness)) => reads_from_violation(
                    event, witness, *block, index, &events, &reached, dominators,
                ),
            }
            .map(AtomicCoherenceViolation::ReadsFrom)
            .or_else(|| {
                match (event.joins_modification_order(), *modification_after) {
                    (false, Some(_)) => Some(AtomicModificationAfterViolation::NonMemberEdge),
                    (true, None) => Some(AtomicModificationAfterViolation::MissingPredecessor),
                    (false, None) => None,
                    (true, Some(predecessor)) => modification_after_violation(
                        event,
                        predecessor,
                        *block,
                        index,
                        &events,
                        &reached,
                        dominators,
                    ),
                }
                .map(AtomicCoherenceViolation::ModificationAfter)
            });
            if let Some(violation) = violation {
                return Some((*block, index, violation));
            }
        }
    }
    None
}

/// A path ends at its nearest write, the invocation entrance, or an effect
/// whose write cannot be represented by an atomic-event edge.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ReachingWrite {
    Initial,
    Atomic(BlockId, usize),
    Effect(OperationId),
    Unknown,
}

#[allow(clippy::too_many_arguments)]
fn reaching_writes(
    blocks: &BTreeMap<BlockId, Vec<&AbstractOperation>>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    entry: BlockId,
    block: BlockId,
    index: usize,
    location: &AbstractAtomicLocation,
    projection_before_edge: &impl Fn(
        BlockId,
        BlockId,
        PlaceId,
    ) -> Option<(PlaceId, Vec<StructuralPathSegment>)>,
    projections_may_alias: &impl Fn(
        PlaceId,
        &[StructuralPathSegment],
        PlaceId,
        &[StructuralPathSegment],
    ) -> bool,
) -> BTreeSet<ReachingWrite> {
    let mut reached = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut pending = vec![(block, index, location.clone(), Vec::new())];
    while let Some((block, end, location, mut ancestry)) = pending.pop() {
        if !visited.insert((block, end, location.clone())) {
            continue;
        }
        let coordinate = (block, end, location.root);
        if ancestry.contains(&coordinate) {
            reached.insert(ReachingWrite::Unknown);
            continue;
        }
        ancestry.push(coordinate);
        let Some(operations) = blocks.get(&block) else {
            reached.insert(ReachingWrite::Unknown);
            continue;
        };
        let mut stopped = false;
        for (index, operation) in operations[..end].iter().enumerate().rev() {
            if let AbstractOperation::AtomicEvent { event, .. } = operation
                && event.joins_modification_order()
                && event.location() == Some(&location)
            {
                reached.insert(ReachingWrite::Atomic(block, index));
                stopped = true;
                break;
            }
            if invalidates_location(operation, &location, projections_may_alias) {
                reached.insert(
                    operation
                        .psi_operation()
                        .map_or(ReachingWrite::Unknown, ReachingWrite::Effect),
                );
                stopped = true;
                break;
            }
        }
        if stopped {
            continue;
        }
        if block == entry {
            reached.insert(ReachingWrite::Initial);
        }
        let incoming = predecessors.get(&block);
        if block != entry && incoming.is_none_or(BTreeSet::is_empty) {
            reached.insert(ReachingWrite::Unknown);
        }
        for predecessor in incoming.into_iter().flatten() {
            let Some((root, mut path)) = projection_before_edge(*predecessor, block, location.root)
            else {
                reached.insert(ReachingWrite::Unknown);
                continue;
            };
            let Some(operations) = blocks.get(predecessor) else {
                reached.insert(ReachingWrite::Unknown);
                continue;
            };
            path.extend_from_slice(&location.path);
            pending.push((
                *predecessor,
                operations.len(),
                AbstractAtomicLocation {
                    root,
                    path,
                    field: location.field,
                },
                ancestry.clone(),
            ));
        }
    }
    // A closed, unreachable cycle has no invocation residency to claim.
    if reached.is_empty() {
        reached.insert(ReachingWrite::Unknown);
    }
    reached
}

fn invalidates_location(
    operation: &AbstractOperation,
    location: &AbstractAtomicLocation,
    projections_may_alias: &impl Fn(
        PlaceId,
        &[StructuralPathSegment],
        PlaceId,
        &[StructuralPathSegment],
    ) -> bool,
) -> bool {
    // Establishing a local home begins its residency. A descriptor may capture
    // existing backing, so retain the exact establishment without inventing an
    // initial value or an internal atomic writer for the referenced storage.
    if operation
        .structural_result()
        .is_some_and(|(_, result)| result.place == location.root)
    {
        return true;
    }
    let called_arguments = match operation {
        crate::boundary_or_structural_call_pair_pattern!(structural_arguments, ..)
        | AbstractOperation::ExternalCompletion {
            structural_arguments,
            ..
        } => structural_arguments.as_slice(),
        _ => &[],
    };
    if called_arguments.iter().any(|argument| {
        argument.place == location.root
            || projections_may_alias(
                argument.place,
                &argument.path,
                location.root,
                &location.path,
            )
    }) {
        return true;
    }
    operation.storage_mutations().into_iter().any(|mutation| {
        if mutation.place != location.root {
            // A different root can select this very cell through another loan.
            // Its path and field namespace cannot prove cross-root separation.
            return projections_may_alias(
                mutation.place,
                mutation.path,
                location.root,
                &location.path,
            );
        }
        // These writes select a scalar leaf exactly. Other mutation families
        // may carry aliases or replace a parent, so their root remains a fence.
        match operation {
            AbstractOperation::StructuralScalarFieldStore { .. }
            | AbstractOperation::AtomicEvent { .. } => {
                mutation.path.contains(&StructuralPathSegment::Referent)
                    || location.path.contains(&StructuralPathSegment::Referent)
                    || mutation.field == Some(location.field)
                        && carrier_paths_may_overlap(mutation.path, &location.path)
            }
            _ => true,
        }
    })
}

/// Atomic carriers are static record/array projections. Only unequal declared
/// fields or unequal fixed elements establish disjointness; a runtime selector
/// can hit the atomic element, and unsupported vocabulary grants no exclusion.
fn carrier_paths_may_overlap(
    left: &[StructuralPathSegment],
    right: &[StructuralPathSegment],
) -> bool {
    for (left, right) in left.iter().zip(right) {
        match (left, right) {
            (StructuralPathSegment::Field(left), StructuralPathSegment::Field(right))
                if left != right =>
            {
                return false;
            }
            (StructuralPathSegment::FixedIndex(left), StructuralPathSegment::FixedIndex(right))
                if left != right =>
            {
                return false;
            }
            (StructuralPathSegment::Field(_), StructuralPathSegment::Field(_))
            | (StructuralPathSegment::FixedIndex(_), StructuralPathSegment::FixedIndex(_))
            | (StructuralPathSegment::FixedIndex(_), StructuralPathSegment::RuntimeIndex { .. })
            | (StructuralPathSegment::RuntimeIndex { .. }, StructuralPathSegment::FixedIndex(_)) => {
            }
            _ => return true,
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn reads_from_violation(
    event: &AbstractAtomicEvent,
    witness: AtomicReadsFrom,
    block: BlockId,
    index: usize,
    events: &BTreeMap<OperationId, Vec<(BlockId, usize, &AbstractAtomicEvent)>>,
    reached: &BTreeSet<ReachingWrite>,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Option<AtomicReadsFromViolation> {
    let Some(location) = event.location() else {
        return Some(AtomicReadsFromViolation::MissingWitness);
    };
    match witness {
        AtomicReadsFrom::AfterEffect { operation } => (reached
            != &BTreeSet::from([ReachingWrite::Effect(operation)]))
            .then_some(AtomicReadsFromViolation::UnresolvedObservedWrite { claimed: operation }),
        AtomicReadsFrom::InitialResidency => (reached != &BTreeSet::from([ReachingWrite::Initial]))
            .then_some(AtomicReadsFromViolation::InitialResidencyAfterWrite),
        AtomicReadsFrom::Write { operation: claimed } => {
            let Some([(writer, position, resolved)]) = events.get(&claimed).map(Vec::as_slice)
            else {
                return Some(AtomicReadsFromViolation::UnresolvedObservedWrite { claimed });
            };
            if !resolved.joins_modification_order()
                || (resolved.location() != Some(location)
                    && !reached.contains(&ReachingWrite::Atomic(*writer, *position)))
            {
                return Some(AtomicReadsFromViolation::WriteOutsideModificationOrder { claimed });
            }
            // The claimed write must happen before this observation: a
            // predecessor position in the same block, or a block that
            // dominates the observer's. A sibling-branch or successor
            // write reaches the observation on no `happens_before` edge.
            let happens_before = if *writer == block {
                *position < index
            } else {
                dominators
                    .get(&block)
                    .is_some_and(|dominating| dominating.contains(writer))
            };
            if !happens_before {
                return Some(AtomicReadsFromViolation::ObservedWriteNotHappensBefore { claimed });
            }
            (reached != &BTreeSet::from([ReachingWrite::Atomic(*writer, *position)]))
                .then_some(AtomicReadsFromViolation::ObservedWriteOverwritten { claimed })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn modification_after_violation(
    event: &AbstractAtomicEvent,
    predecessor: AtomicModificationAfter,
    block: BlockId,
    index: usize,
    events: &BTreeMap<OperationId, Vec<(BlockId, usize, &AbstractAtomicEvent)>>,
    reached: &BTreeSet<ReachingWrite>,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Option<AtomicModificationAfterViolation> {
    let Some(location) = event.location() else {
        return Some(AtomicModificationAfterViolation::MissingPredecessor);
    };
    match predecessor {
        AtomicModificationAfter::AfterEffect { operation } => {
            (reached != &BTreeSet::from([ReachingWrite::Effect(operation)])).then_some(
                AtomicModificationAfterViolation::UnresolvedPredecessor { claimed: operation },
            )
        }
        AtomicModificationAfter::InitialResidency => (reached
            != &BTreeSet::from([ReachingWrite::Initial]))
            .then_some(AtomicModificationAfterViolation::InitialResidencyAfterWrite),
        AtomicModificationAfter::Write { operation: claimed } => {
            let Some([(writer, position, resolved)]) = events.get(&claimed).map(Vec::as_slice)
            else {
                return Some(AtomicModificationAfterViolation::UnresolvedPredecessor { claimed });
            };
            if !resolved.joins_modification_order()
                || (resolved.location() != Some(location)
                    && !reached.contains(&ReachingWrite::Atomic(*writer, *position)))
            {
                return Some(
                    AtomicModificationAfterViolation::PredecessorOutsideModificationOrder {
                        claimed,
                    },
                );
            }
            // The claimed predecessor must happen before this write: a
            // predecessor position in the same block, or a block that
            // dominates the writer's. A sibling-branch or successor write
            // precedes it on no execution path.
            let happens_before = if *writer == block {
                *position < index
            } else {
                dominators
                    .get(&block)
                    .is_some_and(|dominating| dominating.contains(writer))
            };
            if !happens_before {
                return Some(
                    AtomicModificationAfterViolation::PredecessorNotHappensBefore { claimed },
                );
            }
            (reached != &BTreeSet::from([ReachingWrite::Atomic(*writer, *position)]))
                .then_some(AtomicModificationAfterViolation::PredecessorNotLatest { claimed })
        }
    }
}
