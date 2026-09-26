//! Atomic memory events retain their normalized event kind, exact location
//! (root place, static carrier path, scalar field),
//! operand identity, observed results, and proof-static ordering. Ordering
//! and single-attempt custody are encoded as data so identity changes when
//! either changes; legality remains the event's own recheck.

use crate::optimization_unit::identity::carrier_encoding::{
    encode_abstract_result, encode_structural_path_segment,
};
use crate::optimization_unit::identity::structural_encoding::encode_structural_operation_result;

use super::{AbstractOperation, CanonicalBytes};
use crate::abstract_operations::atomic::AbstractAtomicLocation;
use crate::abstract_operations::{
    AbstractAtomicFenceOrdering, AbstractAtomicReadModifyWrite, AtomicModificationAfter,
    AtomicReadsFrom,
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
        modification_after,
    } = operation
    else {
        unreachable!("operation family routing admitted a non-atomic operation")
    };
    use crate::abstract_operations::AbstractAtomicEvent as E;
    bytes.u8(72);
    bytes.id(*psi_operation);
    match event {
        E::Load {
            location,
            ordering,
            result,
        } => {
            bytes.u8(1);
            encode_location(bytes, location);
            encode_memory_ordering(bytes, *ordering);
            encode_abstract_result(bytes, *result);
        }
        E::Store {
            location,
            ordering,
            value,
        } => {
            bytes.u8(2);
            encode_location(bytes, location);
            encode_memory_ordering(bytes, *ordering);
            bytes.id(*value);
        }
        E::ReadModifyWrite {
            location,
            operation,
            ordering,
            operand,
            prior,
        } => {
            bytes.u8(3);
            encode_location(bytes, location);
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
            location,
            ordering,
            value,
            prior,
        } => {
            bytes.u8(4);
            encode_location(bytes, location);
            encode_memory_ordering(bytes, *ordering);
            bytes.id(*value);
            encode_abstract_result(bytes, *prior);
        }
        E::CompareExchange {
            location,
            success,
            failure,
            expected,
            replacement,
            observed,
        } => {
            bytes.u8(5);
            encode_location(bytes, location);
            encode_memory_ordering(bytes, *success);
            encode_memory_ordering(bytes, *failure);
            bytes.id(*expected);
            bytes.id(*replacement);
            encode_abstract_result(bytes, *observed);
        }
        E::CompareExchangeOnce {
            location,
            success,
            failure,
            expected,
            replacement,
            outcome,
            custody,
        } => {
            bytes.u8(6);
            encode_location(bytes, location);
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
    // The retained ordering-relation edges are coherence evidence like the
    // retained ordering: identity changes when either claimed edge changes.
    match reads_from {
        None => bytes.u8(1),
        Some(AtomicReadsFrom::InitialResidency) => bytes.u8(2),
        Some(AtomicReadsFrom::Write { operation }) => {
            bytes.u8(3);
            bytes.id(*operation);
        }
    }
    match modification_after {
        None => bytes.u8(1),
        Some(AtomicModificationAfter::InitialResidency) => bytes.u8(2),
        Some(AtomicModificationAfter::Write { operation }) => {
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

/// The exact location: root place, every carrier path segment, and the
/// scalar field, so a substituted field or element changes identity.
fn encode_location(bytes: &mut CanonicalBytes, location: &AbstractAtomicLocation) {
    bytes.id(location.root);
    bytes.slice(&location.path, encode_structural_path_segment);
    bytes.id(location.field);
}
