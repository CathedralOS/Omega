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

use language_core::atomic::{
    AtomicCompareExchangeOnceResultCustody, AtomicExpressionResultCustody, AtomicOrderingPlan,
    MemoryOrdering,
};
use semantic_vocabulary::{PlaceId, ValueId};
use terminal_psi::StructuralOperationResult;

use crate::AbstractResult;

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
}

#[cfg(test)]
mod tests {
    use super::{AbstractAtomicEvent, AbstractAtomicFenceOrdering, AbstractAtomicReadModifyWrite};
    use crate::AbstractResult;
    use language_core::atomic::{
        AtomicCompareExchangeOnceResultCustody, AtomicOrderingPlan, MemoryOrdering as O,
    };
    use semantic_vocabulary::{
        IntegerSign, IntegerType, PlaceId, ScalarType, StructuralTypeId, ValueId,
    };
    use terminal_psi::{StructuralMultiplicity, StructuralOperationResult};

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
