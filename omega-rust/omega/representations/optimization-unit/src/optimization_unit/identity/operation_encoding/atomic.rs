//! Atomic memory events retain their normalized event kind, exact place,
//! operand identity, observed results, and proof-static ordering. Ordering
//! and single-attempt custody are encoded as data so identity changes when
//! either changes; legality remains the event's own recheck.

use super::super::{encode_abstract_result, encode_structural_operation_result};
use super::{AbstractOperation, CanonicalBytes};
use abstract_operations::{
    AbstractAtomicFenceOrdering, AbstractAtomicReadModifyWrite, AtomicReadsFrom,
};
use language_core::atomic::{
    AtomicCompareExchangeOutcomeIdentity, AtomicObservingCompareExchangeOperation,
    AtomicObservingCompareExchangeResultShape, MemoryOrdering,
};

pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    let AbstractOperation::AtomicEvent {
        psi_operation,
        event,
        reads_from,
    } = operation
    else {
        unreachable!("operation family routing admitted a non-atomic operation")
    };
    use abstract_operations::AbstractAtomicEvent as E;
    bytes.u8(72);
    bytes.id(*psi_operation);
    match event {
        E::Load {
            place,
            ordering,
            result,
        } => {
            bytes.u8(1);
            bytes.id(*place);
            encode_memory_ordering(bytes, *ordering);
            encode_abstract_result(bytes, *result);
        }
        E::Store {
            place,
            ordering,
            value,
        } => {
            bytes.u8(2);
            bytes.id(*place);
            encode_memory_ordering(bytes, *ordering);
            bytes.id(*value);
        }
        E::ReadModifyWrite {
            place,
            operation,
            ordering,
            operand,
            prior,
        } => {
            bytes.u8(3);
            bytes.id(*place);
            bytes.u8(match operation {
                AbstractAtomicReadModifyWrite::FetchAdd => 1,
                AbstractAtomicReadModifyWrite::FetchSub => 2,
                AbstractAtomicReadModifyWrite::FetchXor => 3,
                AbstractAtomicReadModifyWrite::FetchOr => 4,
                AbstractAtomicReadModifyWrite::FetchAnd => 5,
            });
            encode_memory_ordering(bytes, *ordering);
            bytes.id(*operand);
            encode_abstract_result(bytes, *prior);
        }
        E::Swap {
            place,
            ordering,
            value,
            prior,
        } => {
            bytes.u8(4);
            bytes.id(*place);
            encode_memory_ordering(bytes, *ordering);
            bytes.id(*value);
            encode_abstract_result(bytes, *prior);
        }
        E::CompareExchange {
            place,
            success,
            failure,
            expected,
            replacement,
            observed,
        } => {
            bytes.u8(5);
            bytes.id(*place);
            encode_memory_ordering(bytes, *success);
            encode_memory_ordering(bytes, *failure);
            bytes.id(*expected);
            bytes.id(*replacement);
            encode_abstract_result(bytes, *observed);
        }
        E::CompareExchangeOnce {
            place,
            success,
            failure,
            expected,
            replacement,
            outcome,
            custody,
        } => {
            bytes.u8(6);
            bytes.id(*place);
            encode_memory_ordering(bytes, *success);
            encode_memory_ordering(bytes, *failure);
            bytes.id(*expected);
            bytes.id(*replacement);
            encode_structural_operation_result(bytes, outcome);
            // The custody identity is deliberately redundant with the
            // event kind; encode all three axes so a non-canonical
            // substitution changes the operation identity.
            bytes.u8(match custody.operation {
                AtomicObservingCompareExchangeOperation::Decisive => 1,
                AtomicObservingCompareExchangeOperation::SingleAttempt => 2,
            });
            bytes.u8(match custody.result_shape {
                AtomicObservingCompareExchangeResultShape::ExchangedOrMismatchedObserved => 1,
                AtomicObservingCompareExchangeResultShape::ExchangedOrMismatchedOrUncommittedObserved => 2,
            });
            bytes.u8(match custody.outcome_identity {
                AtomicCompareExchangeOutcomeIdentity::AtomicCompareExchangeOutcome => 1,
                AtomicCompareExchangeOutcomeIdentity::AtomicCompareExchangeOnceOutcome => 2,
                AtomicCompareExchangeOutcomeIdentity::AtomicTryExchangeOutcome => 3,
                AtomicCompareExchangeOutcomeIdentity::AtomicTryExchangeOnceOutcome => 4,
            });
        }
        E::Fence { ordering } => {
            bytes.u8(7);
            bytes.u8(match ordering {
                AbstractAtomicFenceOrdering::Receive => 1,
                AbstractAtomicFenceOrdering::Publish => 2,
                AbstractAtomicFenceOrdering::ReceivePublish => 3,
            });
        }
    }
    // The retained reads-from edge is coherence evidence like the retained
    // ordering: identity changes when the claimed edge changes.
    match reads_from {
        None => bytes.u8(1),
        Some(AtomicReadsFrom::InitialResidency) => bytes.u8(2),
        Some(AtomicReadsFrom::Write { operation }) => {
            bytes.u8(3);
            bytes.id(*operation);
        }
    }
}

fn encode_memory_ordering(bytes: &mut CanonicalBytes, ordering: MemoryOrdering) {
    bytes.u8(match ordering {
        MemoryOrdering::NoOrdering => 1,
        MemoryOrdering::Receive => 2,
        MemoryOrdering::Publish => 3,
        MemoryOrdering::ReceivePublish => 4,
        MemoryOrdering::GlobalOrder => 5,
    });
}
