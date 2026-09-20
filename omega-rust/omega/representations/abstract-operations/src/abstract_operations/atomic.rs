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
//! edge naming the write it claims to have read, and
//! [`happens_before_atomic_coherence_violation`] independently replays the
//! coherence axiom under the activation's bounded `happens_before`
//! derivation — the observed write must happen before the observation and
//! be the modification-order-latest write to the place on every execution
//! path — instead of trusting the producer's claim. `synchronizes_with`,
//! `global_sequential_order`, and fence-pair synchronization constrain only
//! cross-activation observation, and land with the concurrent-execution
//! route.

use std::collections::{BTreeMap, BTreeSet};

use language_core::atomic::{
    AtomicCompareExchangeOnceResultCustody, AtomicExpressionResultCustody, AtomicOrderingPlan,
    MemoryOrdering,
};
use semantic_vocabulary::{BlockId, OperationId, PlaceId, ValueId};
use terminal_psi::StructuralOperationResult;

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

/// The dedicated `Atomic::fence` ordering: exactly
/// `Receive | Publish | ReceivePublish`. The dedicated type keeps
/// `NoOrdering` and `GlobalOrder` fences unrepresentable rather than
/// rejected by a later check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbstractAtomicFenceOrdering {
    Receive,
    Publish,
    ReceivePublish,
}

/// The retained `reads_from` edge of one observing atomic event: which
/// write in its place's modification order the event claims to have read.
///
/// The edge names the write by the operation identity under which it stands
/// in the checked activation — never by the value it stored. A load's
/// result is a fresh definition, so value equality cannot witness a read
/// edge, and the concurrency contract's `reads_from`/`modification_order`
/// relations exist only through this retained claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicReadsFrom {
    /// The event observes the place's residency as it stood before the
    /// activation: no earlier write to the place may precede the
    /// observation on any execution path.
    InitialResidency,
    /// The event observes the atomic event standing under this operation
    /// identity in the checked activation.
    Write { operation: OperationId },
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

/// One normalized atomic memory event.
///
/// `place` names the exact atomic location whose per-location modification
/// order the event joins; a fence accesses no place because it orders events
/// rather than observing or modifying one. This is a semantic event, not a
/// target instruction: realization may implement an admitted ordering with a
/// stronger instruction, but may not weaken it or substitute a separately
/// loaded prior for the instruction-observed one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractAtomicEvent {
    /// Observe the resident under `ordering`.
    Load {
        place: PlaceId,
        ordering: MemoryOrdering,
        result: AbstractResult,
    },
    /// Replace the resident with `value` under `ordering`; produces no
    /// result value.
    Store {
        place: PlaceId,
        ordering: MemoryOrdering,
        value: ValueId,
    },
    /// One read-modify-write under `ordering`: `prior` binds the resident
    /// the instruction observed and the resident becomes
    /// `operation(prior, operand)`.
    ReadModifyWrite {
        place: PlaceId,
        operation: AbstractAtomicReadModifyWrite,
        ordering: MemoryOrdering,
        operand: ValueId,
        prior: AbstractResult,
    },
    /// Exchange the resident for `value` under `ordering`; `prior` binds the
    /// displaced resident the instruction observed.
    Swap {
        place: PlaceId,
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
        place: PlaceId,
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
        place: PlaceId,
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
    /// The place whose modification order this event joins. A fence joins no
    /// modification order because it accesses no place.
    pub const fn place(&self) -> Option<PlaceId> {
        match self {
            Self::Load { place, .. }
            | Self::Store { place, .. }
            | Self::ReadModifyWrite { place, .. }
            | Self::Swap { place, .. }
            | Self::CompareExchange { place, .. }
            | Self::CompareExchangeOnce { place, .. } => Some(*place),
            Self::Fence { .. } => None,
        }
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
/// or claim the pre-activation residency only when no write may precede.
///
/// Inside one activation `happens_before` is execution-path sequencing:
/// every rule that forms `synchronizes_with` either pairs a publication
/// with an observation on distinct activations — which a single function
/// never expresses — or composes sequenced-before edges already inside the
/// closure. A write therefore happens before an observation exactly when
/// it precedes it in their shared block or stands in a block dominating
/// the observer's; `predecessors` and `dominators` are the function's
/// validated control-flow relations. The reaching-writes fixpoint unions
/// each predecessor's modification-order tail, so a claim must hold under
/// every path rather than one replay: an empty reaching set means the
/// place's residency is still initial on all of them, and a claim naming
/// a write that reaches the observer on only some paths has no
/// `happens_before` edge to stand on.
///
/// `global_sequential_order` and fence-pair synchronization likewise
/// constrain only cross-activation observation; they carry no checkable
/// content inside one activation and land with the concurrent-execution
/// route.
///
/// Non-atomic operations do not participate: they join no atomic
/// modification order and cannot be named by a witness. Returns the
/// offending event's block and node position together with the refusal,
/// or `None` when every retained edge is coherent.
pub fn happens_before_atomic_coherence_violation(
    blocks: &BTreeMap<BlockId, Vec<&AbstractOperation>>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Option<(BlockId, usize, AtomicReadsFromViolation)> {
    // Atomic events claimable by a witness, keyed by the operation identity
    // they stand under across the whole activation. Duplicate identities
    // stay listed so a witness naming them refuses as ambiguous.
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
    // Reaching-writes fixpoint: on entering a block each place's
    // modification-order tail is the union of its predecessors' exits; a
    // write inside the block replaces the tail with itself, since every
    // path through the block runs the whole sequence.
    let mut entry_latest: BTreeMap<BlockId, BTreeMap<PlaceId, BTreeSet<(BlockId, usize)>>> =
        BTreeMap::new();
    let mut exit_latest: BTreeMap<BlockId, BTreeMap<PlaceId, BTreeSet<(BlockId, usize)>>> =
        BTreeMap::new();
    loop {
        let mut changed = false;
        for (block, operations) in blocks {
            let mut latest: BTreeMap<PlaceId, BTreeSet<(BlockId, usize)>> = BTreeMap::new();
            for predecessor in predecessors.get(block).into_iter().flatten() {
                for (place, writes) in exit_latest.get(predecessor).into_iter().flatten() {
                    latest
                        .entry(*place)
                        .or_default()
                        .extend(writes.iter().copied());
                }
            }
            if entry_latest.get(block) != Some(&latest) {
                entry_latest.insert(*block, latest.clone());
                changed = true;
            }
            let mut exit = latest;
            for (index, operation) in operations.iter().enumerate() {
                let AbstractOperation::AtomicEvent { event, .. } = *operation else {
                    continue;
                };
                if event.joins_modification_order()
                    && let Some(place) = event.place()
                {
                    exit.insert(place, BTreeSet::from([(*block, index)]));
                }
            }
            if exit_latest.get(block) != Some(&exit) {
                exit_latest.insert(*block, exit);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for (block, operations) in blocks {
        let mut latest = entry_latest.get(block).cloned().unwrap_or_default();
        for (index, operation) in operations.iter().enumerate() {
            let AbstractOperation::AtomicEvent {
                event, reads_from, ..
            } = *operation
            else {
                continue;
            };
            let violation = match (event.observes_resident(), *reads_from) {
                (false, Some(_)) => Some(AtomicReadsFromViolation::NonObservingWitness),
                (true, None) => Some(AtomicReadsFromViolation::MissingWitness),
                (false, None) => None,
                (true, Some(witness)) => reads_from_violation(
                    event, witness, *block, index, &events, &latest, dominators,
                ),
            };
            if let Some(violation) = violation {
                return Some((*block, index, violation));
            }
            if event.joins_modification_order()
                && let Some(place) = event.place()
            {
                latest.insert(place, BTreeSet::from([(*block, index)]));
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn reads_from_violation(
    event: &AbstractAtomicEvent,
    witness: AtomicReadsFrom,
    block: BlockId,
    index: usize,
    events: &BTreeMap<OperationId, Vec<(BlockId, usize, &AbstractAtomicEvent)>>,
    latest: &BTreeMap<PlaceId, BTreeSet<(BlockId, usize)>>,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Option<AtomicReadsFromViolation> {
    let Some(place) = event.place() else {
        return Some(AtomicReadsFromViolation::MissingWitness);
    };
    match witness {
        AtomicReadsFrom::InitialResidency => latest
            .get(&place)
            .is_some_and(|writes| !writes.is_empty())
            .then_some(AtomicReadsFromViolation::InitialResidencyAfterWrite),
        AtomicReadsFrom::Write { operation: claimed } => {
            let Some([(writer, position, resolved)]) = events.get(&claimed).map(Vec::as_slice)
            else {
                return Some(AtomicReadsFromViolation::UnresolvedObservedWrite { claimed });
            };
            if resolved.place() != Some(place) || !resolved.joins_modification_order() {
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
            (latest.get(&place) != Some(&BTreeSet::from([(*writer, *position)])))
                .then_some(AtomicReadsFromViolation::ObservedWriteOverwritten { claimed })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{
        AbstractAtomicEvent, AbstractAtomicFenceOrdering, AbstractAtomicReadModifyWrite,
        AtomicReadsFrom, AtomicReadsFromViolation, happens_before_atomic_coherence_violation,
    };
    use crate::{AbstractOperation, AbstractResult};
    use language_core::atomic::{
        AtomicCompareExchangeOnceResultCustody, AtomicOrderingPlan, MemoryOrdering as O,
    };
    use semantic_vocabulary::{
        BlockId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId, ScalarType,
        StructuralTypeId, ValueId,
    };
    use terminal_psi::{StructuralMultiplicity, StructuralOperationResult};

    type E = AbstractAtomicEvent;
    type W = AtomicReadsFrom;
    type V = AtomicReadsFromViolation;

    fn scalar_result(raw: u64) -> AbstractResult {
        AbstractResult {
            value: ValueId::new(raw).expect("test value identities are nonzero"),
            scalar_type: ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 32).expect("32 is a valid integer width"),
            ),
        }
    }

    fn structural_outcome(raw: u64) -> StructuralOperationResult {
        StructuralOperationResult {
            place: place(raw),
            structural_type: StructuralTypeId::new(raw)
                .expect("test structural type identities are nonzero"),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }
    }

    fn place(raw: u64) -> PlaceId {
        PlaceId::new(raw).expect("test place identities are nonzero")
    }

    fn value(raw: u64) -> ValueId {
        ValueId::new(raw).expect("test value identities are nonzero")
    }

    fn operation(raw: u64) -> OperationId {
        OperationId::new(raw).expect("test operation identities are nonzero")
    }

    fn atomic(raw: u64, event: E, reads_from: Option<W>) -> AbstractOperation {
        AbstractOperation::AtomicEvent {
            psi_operation: operation(raw),
            event,
            reads_from,
        }
    }

    fn load(place: PlaceId, result: u64) -> E {
        E::Load {
            place,
            ordering: O::NoOrdering,
            result: scalar_result(result),
        }
    }

    fn store(place: PlaceId, stored: u64) -> E {
        E::Store {
            place,
            ordering: O::NoOrdering,
            value: value(stored),
        }
    }

    fn return_unit(edge: u64) -> AbstractOperation {
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(edge).expect("test edge identities are nonzero"),
            cleanup_actions: Vec::new(),
        }
    }

    fn block(raw: u64) -> BlockId {
        BlockId::new(raw).expect("test block identities are nonzero")
    }

    /// A single-sequence adapter: the sequence stands alone in one block,
    /// so the bounded derivation reduces to serial sequencing.
    fn coherence(operations: &[AbstractOperation]) -> Option<(usize, V)> {
        let blocks = BTreeMap::from([(block(1), operations.iter().collect())]);
        let predecessors = BTreeMap::from([(block(1), BTreeSet::new())]);
        let dominators = BTreeMap::from([(block(1), BTreeSet::from([block(1)]))]);
        happens_before_atomic_coherence_violation(&blocks, &predecessors, &dominators)
            .map(|(_, index, violation)| (index, violation))
    }

    /// An explicit block-graph adapter: each entry names a block's
    /// operation sequence, inbound control-flow edges, and dominator set
    /// directly, so a test can pin the derivation without the unit's
    /// validated-CFG reconstruction.
    fn graph_coherence(
        sequences: &[(u64, &[AbstractOperation])],
        predecessors: &[(u64, &[u64])],
        dominators: &[(u64, &[u64])],
    ) -> Option<(BlockId, usize, V)> {
        let blocks = sequences
            .iter()
            .map(|(raw, operations)| (block(*raw), operations.iter().collect::<Vec<_>>()))
            .collect();
        let predecessors = predecessors
            .iter()
            .map(|(raw, incoming)| {
                (
                    block(*raw),
                    incoming.iter().map(|raw| block(*raw)).collect(),
                )
            })
            .collect();
        let dominators = dominators
            .iter()
            .map(|(raw, dominating)| {
                (
                    block(*raw),
                    dominating.iter().map(|raw| block(*raw)).collect(),
                )
            })
            .collect();
        happens_before_atomic_coherence_violation(&blocks, &predecessors, &dominators)
    }

    #[test]
    fn load_reads_the_modification_order_latest_write() {
        let location = place(1);
        let first = atomic(10, store(location, 11), None);
        let second = atomic(12, store(location, 13), None);
        let observed = atomic(
            14,
            load(location, 15),
            Some(W::Write {
                operation: operation(12),
            }),
        );
        assert_eq!(coherence(&[first.clone(), second, observed]), None);
        let stale = atomic(
            14,
            load(location, 15),
            Some(W::Write {
                operation: operation(10),
            }),
        );
        assert_eq!(
            coherence(&[first, atomic(12, store(location, 13), None), stale]),
            Some((
                2,
                V::ObservedWriteOverwritten {
                    claimed: operation(10),
                }
            )),
            "a write superseded in the modification order cannot be observed"
        );
    }

    #[test]
    fn observers_read_initial_residency_only_before_any_write() {
        let location = place(1);
        let untouched = atomic(10, load(location, 11), Some(W::InitialResidency));
        assert_eq!(coherence(&[untouched]), None);
        let other_place = atomic(10, load(place(2), 11), Some(W::InitialResidency));
        let written_elsewhere = atomic(12, store(place(1), 13), None);
        assert_eq!(
            coherence(&[written_elsewhere, other_place]),
            None,
            "a write to a different place leaves this place's residency initial"
        );
        let written = atomic(10, store(location, 11), None);
        let late_initial = atomic(12, load(location, 13), Some(W::InitialResidency));
        assert_eq!(
            coherence(&[written, late_initial]),
            Some((1, V::InitialResidencyAfterWrite)),
            "an earlier write to the place displaces initial residency"
        );
    }

    #[test]
    fn read_modify_write_chains_through_the_order_it_joins() {
        let location = place(1);
        let stored = atomic(10, store(location, 11), None);
        let fetched = atomic(
            12,
            E::ReadModifyWrite {
                place: location,
                operation: AbstractAtomicReadModifyWrite::FetchAdd,
                ordering: O::ReceivePublish,
                operand: value(13),
                prior: scalar_result(14),
            },
            Some(W::Write {
                operation: operation(10),
            }),
        );
        let observed = atomic(
            15,
            load(location, 16),
            Some(W::Write {
                operation: operation(12),
            }),
        );
        assert_eq!(coherence(&[stored, fetched, observed]), None);
    }

    #[test]
    fn every_observing_kind_carries_a_checked_edge() {
        let location = place(1);
        let stored = atomic(10, store(location, 11), None);
        for observer in [
            E::Swap {
                place: location,
                ordering: O::ReceivePublish,
                value: value(13),
                prior: scalar_result(14),
            },
            E::CompareExchange {
                place: location,
                success: O::ReceivePublish,
                failure: O::Receive,
                expected: value(13),
                replacement: value(15),
                observed: scalar_result(16),
            },
            E::CompareExchangeOnce {
                place: location,
                success: O::ReceivePublish,
                failure: O::Receive,
                expected: value(13),
                replacement: value(15),
                outcome: structural_outcome(16),
                custody: AtomicCompareExchangeOnceResultCustody::CANONICAL,
            },
        ] {
            let sequence = [
                stored.clone(),
                atomic(
                    12,
                    observer,
                    Some(W::Write {
                        operation: operation(10),
                    }),
                ),
            ];
            assert_eq!(
                coherence(&sequence),
                None,
                "each observing kind reads the latest write: {:?}",
                sequence[1]
            );
        }
    }

    #[test]
    fn fences_and_non_atomic_operations_do_not_disturb_the_order() {
        let location = place(1);
        let stored = atomic(10, store(location, 11), None);
        let fence = atomic(
            12,
            E::Fence {
                ordering: AbstractAtomicFenceOrdering::ReceivePublish,
            },
            None,
        );
        let other = return_unit(20);
        let observed = atomic(
            14,
            load(location, 15),
            Some(W::Write {
                operation: operation(10),
            }),
        );
        assert_eq!(
            coherence(&[stored, fence, other, observed]),
            None,
            "neither fences nor non-atomic operations join the modification order"
        );
    }

    #[test]
    fn refused_orderings_name_their_exact_failure() {
        let location = place(1);
        let stored = atomic(10, store(location, 11), None);
        let load_unchecked = atomic(12, load(location, 13), None);
        assert_eq!(
            coherence(&[stored.clone(), load_unchecked]),
            Some((1, V::MissingWitness)),
            "an observing event without a witness refuses"
        );
        let store_witness = atomic(10, store(location, 11), Some(W::InitialResidency));
        assert_eq!(
            coherence(&[store_witness]),
            Some((0, V::NonObservingWitness)),
            "a store observes nothing and cannot carry an edge"
        );
        let fence_witness = atomic(
            10,
            E::Fence {
                ordering: AbstractAtomicFenceOrdering::Receive,
            },
            Some(W::InitialResidency),
        );
        assert_eq!(
            coherence(&[fence_witness]),
            Some((0, V::NonObservingWitness)),
            "a fence observes nothing and cannot carry an edge"
        );
        let dangling = atomic(
            12,
            load(location, 13),
            Some(W::Write {
                operation: operation(99),
            }),
        );
        assert_eq!(
            coherence(&[stored.clone(), dangling]),
            Some((
                1,
                V::UnresolvedObservedWrite {
                    claimed: operation(99),
                }
            )),
            "a witness naming nothing in the sequence refuses"
        );
        let later_write = atomic(14, store(location, 15), None);
        let backwards = atomic(
            12,
            load(location, 13),
            Some(W::Write {
                operation: operation(14),
            }),
        );
        assert_eq!(
            coherence(&[stored.clone(), backwards, later_write]),
            Some((
                1,
                V::ObservedWriteNotHappensBefore {
                    claimed: operation(14),
                }
            )),
            "a witness cannot name a write that follows the observation"
        );
        let wrong_place = atomic(
            12,
            load(place(2), 13),
            Some(W::Write {
                operation: operation(10),
            }),
        );
        assert_eq!(
            coherence(&[stored.clone(), wrong_place]),
            Some((
                1,
                V::WriteOutsideModificationOrder {
                    claimed: operation(10),
                }
            )),
            "a witness must name a write in the observer's own modification order"
        );
        let first_load = atomic(10, load(location, 11), Some(W::InitialResidency));
        let second_load = atomic(
            12,
            load(location, 13),
            Some(W::Write {
                operation: operation(10),
            }),
        );
        assert_eq!(
            coherence(&[first_load, second_load]),
            Some((
                1,
                V::WriteOutsideModificationOrder {
                    claimed: operation(10),
                }
            )),
            "a load writes nothing and cannot stand in a modification order"
        );
    }

    #[test]
    fn shared_operation_identities_make_a_witness_ambiguous() {
        let location = place(1);
        let first = atomic(10, store(location, 11), None);
        let shared = atomic(10, store(location, 13), None);
        let observed = atomic(
            14,
            load(location, 15),
            Some(W::Write {
                operation: operation(10),
            }),
        );
        assert_eq!(
            coherence(&[first, shared, observed]),
            Some((
                2,
                V::UnresolvedObservedWrite {
                    claimed: operation(10),
                }
            )),
            "an identity standing under two events resolves to neither"
        );
    }

    #[test]
    fn a_witness_cannot_name_a_non_atomic_operation() {
        let location = place(1);
        let call = AbstractOperation::CallUnit {
            psi_operation: operation(10),
            callee: MachineId::new(20).expect("test machine identities are nonzero"),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        };
        let observed = atomic(
            12,
            load(location, 13),
            Some(W::Write {
                operation: operation(10),
            }),
        );
        assert_eq!(
            coherence(&[call, observed]),
            Some((
                1,
                V::UnresolvedObservedWrite {
                    claimed: operation(10),
                }
            )),
            "a non-atomic operation joins no modification order even though it carries an identity"
        );
    }

    #[test]
    fn load_and_store_recheck_their_admitted_orderings() {
        for ordering in [
            O::NoOrdering,
            O::Receive,
            O::Publish,
            O::ReceivePublish,
            O::GlobalOrder,
        ] {
            let load = AbstractAtomicEvent::Load {
                place: place(1),
                ordering,
                result: scalar_result(2),
            };
            assert_eq!(
                load.ordering_is_legal(),
                ordering.valid_for_load(),
                "load ordering {ordering:?} must replay the source legality matrix"
            );
            let store = AbstractAtomicEvent::Store {
                place: place(1),
                ordering,
                value: value(3),
            };
            assert_eq!(
                store.ordering_is_legal(),
                ordering.valid_for_store(),
                "store ordering {ordering:?} must replay the source legality matrix"
            );
        }
    }

    #[test]
    fn read_modify_write_and_swap_admit_every_ordering() {
        for ordering in [
            O::NoOrdering,
            O::Receive,
            O::Publish,
            O::ReceivePublish,
            O::GlobalOrder,
        ] {
            for operation in [
                AbstractAtomicReadModifyWrite::FetchAdd,
                AbstractAtomicReadModifyWrite::FetchSub,
                AbstractAtomicReadModifyWrite::FetchXor,
                AbstractAtomicReadModifyWrite::FetchOr,
                AbstractAtomicReadModifyWrite::FetchAnd,
            ] {
                let event = AbstractAtomicEvent::ReadModifyWrite {
                    place: place(1),
                    operation,
                    ordering,
                    operand: value(2),
                    prior: scalar_result(3),
                };
                assert!(
                    event.ordering_is_legal(),
                    "read-modify-write success admits all five orderings"
                );
            }
            let swap = AbstractAtomicEvent::Swap {
                place: place(1),
                ordering,
                value: value(4),
                prior: scalar_result(5),
            };
            assert!(
                swap.ordering_is_legal(),
                "swap success admits all five orderings"
            );
        }
    }

    #[test]
    fn compare_exchange_failure_cannot_publish_or_exceed_success() {
        for success in [
            O::NoOrdering,
            O::Receive,
            O::Publish,
            O::ReceivePublish,
            O::GlobalOrder,
        ] {
            for failure in [
                O::NoOrdering,
                O::Receive,
                O::Publish,
                O::ReceivePublish,
                O::GlobalOrder,
            ] {
                let event = AbstractAtomicEvent::CompareExchange {
                    place: place(1),
                    success,
                    failure,
                    expected: value(2),
                    replacement: value(3),
                    observed: scalar_result(4),
                };
                assert_eq!(
                    event.ordering_is_legal(),
                    failure.valid_compare_exchange_failure(success),
                    "failure {failure:?} under success {success:?} must replay the source legality matrix"
                );
            }
        }
    }

    #[test]
    fn fence_ordering_is_legal_by_construction_and_place_free() {
        for ordering in [
            AbstractAtomicFenceOrdering::Receive,
            AbstractAtomicFenceOrdering::Publish,
            AbstractAtomicFenceOrdering::ReceivePublish,
        ] {
            let event = AbstractAtomicEvent::Fence { ordering };
            assert!(event.ordering_is_legal());
            assert_eq!(event.place(), None);
            assert_eq!(event.ordering_plan(), None);
            assert!(event.custody_is_consistent());
        }
    }

    #[test]
    fn ordering_plan_reconstructs_the_retained_plan() {
        let load = AbstractAtomicEvent::Load {
            place: place(1),
            ordering: O::GlobalOrder,
            result: scalar_result(2),
        };
        assert_eq!(
            load.ordering_plan(),
            Some(AtomicOrderingPlan::Load(O::GlobalOrder))
        );
        let once = AbstractAtomicEvent::CompareExchangeOnce {
            place: place(1),
            success: O::GlobalOrder,
            failure: O::Receive,
            expected: value(2),
            replacement: value(3),
            outcome: structural_outcome(6),
            custody: AtomicCompareExchangeOnceResultCustody::CANONICAL,
        };
        assert_eq!(
            once.ordering_plan(),
            Some(AtomicOrderingPlan::CompareExchangeOnce {
                success: O::GlobalOrder,
                failure: O::Receive,
            })
        );
    }

    #[test]
    fn a_witness_resolves_across_blocks_under_happens_before() {
        let location = place(1);
        let stored = atomic(10, store(location, 11), None);
        let observed = atomic(
            13,
            load(location, 14),
            Some(W::Write {
                operation: operation(10),
            }),
        );
        assert_eq!(
            graph_coherence(
                &[
                    (1, &[stored, return_unit(15)]),
                    (2, &[observed, return_unit(16)]),
                ],
                &[(2, &[1])],
                &[(1, &[1]), (2, &[1, 2])],
            ),
            None,
            "a dominating block's write happens before the observation"
        );
    }

    #[test]
    fn a_witness_must_happen_before_its_observation() {
        let location = place(1);
        let stored = atomic(10, store(location, 11), None);
        let observed = atomic(
            14,
            load(location, 15),
            Some(W::Write {
                operation: operation(10),
            }),
        );
        // A write on one branch of a diamond reaches the join's
        // observation on only some paths.
        assert_eq!(
            graph_coherence(
                &[
                    (1, &[return_unit(20)]),
                    (2, &[stored.clone(), return_unit(21)]),
                    (3, &[return_unit(22)]),
                    (4, &[observed.clone(), return_unit(23)]),
                ],
                &[(2, &[1]), (3, &[1]), (4, &[2, 3])],
                &[(1, &[1]), (2, &[1, 2]), (3, &[1, 3]), (4, &[1, 4])],
            ),
            Some((
                block(4),
                0,
                V::ObservedWriteNotHappensBefore {
                    claimed: operation(10),
                }
            )),
            "a sibling-branch write happens before no join observation"
        );
        // A write in the observer's successor cannot precede it.
        assert_eq!(
            graph_coherence(
                &[
                    (1, &[observed.clone(), return_unit(24)]),
                    (2, &[stored.clone(), return_unit(25)]),
                ],
                &[(2, &[1])],
                &[(1, &[1]), (2, &[1, 2])],
            ),
            Some((
                block(1),
                0,
                V::ObservedWriteNotHappensBefore {
                    claimed: operation(10),
                }
            )),
            "a successor write never happens before the observation"
        );
        // The same shape refuses a still-initial claim: the write may
        // precede on the branch that runs it.
        let initial = atomic(14, load(location, 15), Some(W::InitialResidency));
        assert_eq!(
            graph_coherence(
                &[
                    (1, &[return_unit(20)]),
                    (2, &[stored, return_unit(21)]),
                    (3, &[return_unit(22)]),
                    (4, &[initial, return_unit(23)]),
                ],
                &[(2, &[1]), (3, &[1]), (4, &[2, 3])],
                &[(1, &[1]), (2, &[1, 2]), (3, &[1, 3]), (4, &[1, 4])],
            ),
            Some((block(4), 0, V::InitialResidencyAfterWrite)),
            "a branch-scoped write displaces initial residency on its path"
        );
    }

    #[test]
    fn only_the_latest_write_on_every_path_may_be_observed() {
        let location = place(1);
        let first = atomic(10, store(location, 11), None);
        let second = atomic(12, store(location, 13), None);
        let stale = atomic(
            14,
            load(location, 15),
            Some(W::Write {
                operation: operation(10),
            }),
        );
        let graph = |observed: &[AbstractOperation]| {
            graph_coherence(
                &[
                    (1, &[first.clone(), return_unit(20)]),
                    (2, &[second.clone(), return_unit(21)]),
                    (3, &[observed[0].clone(), return_unit(22)]),
                ],
                &[(2, &[1]), (3, &[2])],
                &[(1, &[1]), (2, &[1, 2]), (3, &[1, 2, 3])],
            )
        };
        assert_eq!(
            graph(&[stale]),
            Some((
                block(3),
                0,
                V::ObservedWriteOverwritten {
                    claimed: operation(10),
                }
            )),
            "a predecessor-chain overwrite makes the earlier claim stale"
        );
        let coherent = atomic(
            14,
            load(location, 15),
            Some(W::Write {
                operation: operation(12),
            }),
        );
        assert_eq!(graph(&[coherent]), None);
    }

    #[test]
    fn branch_writes_converging_have_no_unique_latest() {
        let location = place(1);
        let dominating = atomic(8, store(location, 9), None);
        let left = atomic(10, store(location, 11), None);
        let right = atomic(12, store(location, 13), None);
        let join = |witness: Option<W>| atomic(14, load(location, 15), witness);
        let graph = |observed: &[AbstractOperation]| {
            graph_coherence(
                &[
                    (1, &[dominating.clone(), return_unit(20)]),
                    (2, &[left.clone(), return_unit(21)]),
                    (3, &[right.clone(), return_unit(22)]),
                    (4, &[observed[0].clone(), return_unit(23)]),
                ],
                &[(2, &[1]), (3, &[1]), (4, &[2, 3])],
                &[(1, &[1]), (2, &[1, 2]), (3, &[1, 3]), (4, &[1, 4])],
            )
        };
        for (claimed, violation) in [
            (
                operation(10),
                V::ObservedWriteNotHappensBefore {
                    claimed: operation(10),
                },
            ),
            (
                operation(12),
                V::ObservedWriteNotHappensBefore {
                    claimed: operation(12),
                },
            ),
            (
                operation(8),
                V::ObservedWriteOverwritten {
                    claimed: operation(8),
                },
            ),
        ] {
            assert_eq!(
                graph(&[join(Some(W::Write { operation: claimed }))]),
                Some((block(4), 0, violation)),
                "witness naming {claimed:?} must refuse"
            );
        }
        assert_eq!(
            graph(&[join(Some(W::InitialResidency))]),
            Some((block(4), 0, V::InitialResidencyAfterWrite)),
            "every path wrote the place before the join"
        );
    }

    #[test]
    fn a_fence_neither_writes_nor_disturbs_the_order_across_blocks() {
        let location = place(1);
        let stored = atomic(10, store(location, 11), None);
        let fence = atomic(
            12,
            E::Fence {
                ordering: AbstractAtomicFenceOrdering::Publish,
            },
            None,
        );
        let observed = atomic(
            14,
            load(location, 15),
            Some(W::Write {
                operation: operation(10),
            }),
        );
        assert_eq!(
            graph_coherence(
                &[
                    (1, &[stored.clone(), return_unit(20)]),
                    (2, &[fence, observed, return_unit(21)]),
                ],
                &[(2, &[1])],
                &[(1, &[1]), (2, &[1, 2])],
            ),
            None,
            "a sequenced fence leaves the claimed write latest"
        );
        let claims_fence = atomic(
            14,
            load(location, 15),
            Some(W::Write {
                operation: operation(12),
            }),
        );
        assert_eq!(
            graph_coherence(
                &[
                    (1, &[stored, return_unit(20)]),
                    (
                        2,
                        &[
                            atomic(
                                12,
                                E::Fence {
                                    ordering: AbstractAtomicFenceOrdering::Receive,
                                },
                                None,
                            ),
                            claims_fence,
                            return_unit(21),
                        ],
                    ),
                ],
                &[(2, &[1])],
                &[(1, &[1]), (2, &[1, 2])],
            ),
            Some((
                block(2),
                1,
                V::WriteOutsideModificationOrder {
                    claimed: operation(12),
                }
            )),
            "a fence writes nothing and cannot stand in a modification order"
        );
    }

    #[test]
    fn single_attempt_requires_canonical_three_case_custody() {
        let legal = AbstractAtomicEvent::CompareExchangeOnce {
            place: place(1),
            success: O::ReceivePublish,
            failure: O::Receive,
            expected: value(2),
            replacement: value(3),
            outcome: structural_outcome(6),
            custody: AtomicCompareExchangeOnceResultCustody::CANONICAL,
        };
        assert!(legal.custody_is_consistent());
        let substituted = AbstractAtomicEvent::CompareExchangeOnce {
            place: place(1),
            success: O::ReceivePublish,
            failure: O::Receive,
            expected: value(2),
            replacement: value(3),
            outcome: structural_outcome(6),
            custody: AtomicCompareExchangeOnceResultCustody {
                operation: language_core::atomic::AtomicObservingCompareExchangeOperation::Decisive,
                ..AtomicCompareExchangeOnceResultCustody::CANONICAL
            },
        };
        assert!(
            !substituted.custody_is_consistent(),
            "a decisive custody identity cannot substitute for single-attempt custody"
        );
    }
}
