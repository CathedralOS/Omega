//! Normalized atomic memory events on one exact primitive leaf.
//!
//! # Why one operation carries the whole event
//!
//! The concurrency contract (`wiki/spec/language/concurrency.md`) makes each
//! atomic operation one indivisible event: a swap or fetch returns the prior
//! value the instruction itself observed, never a separately loaded one, and
//! a compare-exchange keeps its success and read-only failure orderings as
//! two distinct commitments. Terminal therefore carries every serial atomic
//! access as one `OperationKind::AtomicAccess` whose [`AtomicAccessEvent`]
//! names the exact operation, its operands, and its orderings, addressed by
//! the same `(place, path, field)` location a `StructuralScalarFieldStore`
//! writes and an `IntegerStructuralField` reads: atomic cells are scalar
//! fields of records, reached from a structural root.
//!
//! Rejected alternatives:
//!
//! - lowering the source carrier's arithmetic model (`prior + operand`,
//!   `prior + (prior == expected) * (replacement - prior)`) into a
//!   `PrimitiveScalarRead`, ordinary arithmetic and a `WriteOnlyPrimitiveStore`
//!   would split one event into a racing load and store, erase the ordering,
//!   and let optimization reorder the halves. No later stage could recover
//!   the instruction-observed prior from that spelling;
//! - one operation kind per event (`AtomicLoad`, `AtomicStore`, …) would
//!   repeat the place, path, and access validation six times. The event sum
//!   keeps that shared custody in one row while every consumer still matches
//!   the event exhaustively.
//!
//! # What the event retains and what checks it
//!
//! - The location is the root, a *static* carrier path (fields, then at most
//!   one literal element), and one relevant scalar field. An atomic event
//!   joins one location's modification order, so a runtime-selected element
//!   (which names no single location) is refused by verification rather than
//!   carried. A bounded-integer field is refused too: an event cannot carry
//!   the range obligation a store into it owes.
//! - The ordering fields are the source-admitted proof-static orderings;
//!   [`AtomicAccessEvent::ordering_is_legal`] replays the source legality
//!   matrix so a load cannot publish, a store cannot receive, and a
//!   compare-exchange failure cannot publish or exceed its success.
//! - The operation result is the observed prior for every observing event
//!   (load, read-modify-write, swap, decisive compare-exchange) and Unit for a
//!   store. A decisive compare-exchange's outcome is therefore
//!   `observed == expected`; the single-attempt form with its `Uncommitted`
//!   case has no Terminal producer yet and stays unrepresentable here rather
//!   than erased into this scalar carrier.
//! - Access custody is the root's own: an observing event needs readable
//!   authority, a modifying event writable authority, and an event that does
//!   both needs both. Terminal structural types do not yet record which
//!   primitive leaves are atomic cells, so a modifying event through a shared
//!   borrow is refused; admitting the shared-receiver form needs that cell
//!   identity first.
//!
//! A fence has no Terminal producer yet. It will be a separate place-free
//! event, not an `AtomicAccess` with an empty place.

use language_core::atomic::AtomicOrderingPlan;
/// The source ordering vocabulary is the Terminal vocabulary: re-exported so
/// Terminal consumers name one ordering type without a second definition.
pub use language_core::atomic::MemoryOrdering;
use semantic_vocabulary::ValueId;

/// The fetch-family arithmetic one read-modify-write applies to the
/// resident. The resident becomes `prior op operand`, reduced modulo the
/// leaf's width; the operation result is `prior`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicReadModifyWrite {
    FetchAdd,
    FetchSub,
    FetchAnd,
    FetchOr,
    FetchXor,
}

/// One normalized atomic event and its operands. Every operand has the
/// leaf's exact scalar type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicAccessEvent {
    /// Observe the resident; the result is the observed value.
    Load { ordering: MemoryOrdering },
    /// Replace the resident with `value`; the result is Unit.
    Store {
        value: ValueId,
        ordering: MemoryOrdering,
    },
    /// Replace the resident with `prior op operand`; the result is the
    /// instruction-observed `prior`.
    ReadModifyWrite {
        operation: AtomicReadModifyWrite,
        operand: ValueId,
        ordering: MemoryOrdering,
    },
    /// Replace the resident with `value`; the result is the displaced prior.
    Swap {
        value: ValueId,
        ordering: MemoryOrdering,
    },
    /// Decisive observing compare-exchange: when the observed resident equals
    /// `expected` it becomes `replacement` under `success`; otherwise it is
    /// unchanged and the read-only failure applies `failure`. The result is
    /// the observed resident either way.
    CompareExchange {
        expected: ValueId,
        replacement: ValueId,
        success: MemoryOrdering,
        failure: MemoryOrdering,
    },
}

impl AtomicAccessEvent {
    /// The proof-static ordering plan this event retains.
    pub const fn ordering_plan(self) -> AtomicOrderingPlan {
        match self {
            Self::Load { ordering } => AtomicOrderingPlan::Load(ordering),
            Self::Store { ordering, .. } => AtomicOrderingPlan::Store(ordering),
            Self::ReadModifyWrite { ordering, .. } => AtomicOrderingPlan::ReadModifyWrite(ordering),
            Self::Swap { ordering, .. } => AtomicOrderingPlan::Swap(ordering),
            Self::CompareExchange {
                success, failure, ..
            } => AtomicOrderingPlan::CompareExchange { success, failure },
        }
    }

    /// Replay the source legality matrix instead of trusting the producer.
    pub const fn ordering_is_legal(self) -> bool {
        match self {
            Self::Load { ordering } => ordering.valid_for_load(),
            Self::Store { ordering, .. } => ordering.valid_for_store(),
            Self::ReadModifyWrite { .. } | Self::Swap { .. } => true,
            Self::CompareExchange {
                success, failure, ..
            } => failure.valid_compare_exchange_failure(success),
        }
    }

    /// Whether the event observes the resident and therefore defines a
    /// scalar result. Only a store observes nothing.
    pub const fn observes_resident(self) -> bool {
        !matches!(self, Self::Store { .. })
    }

    /// Whether the event may replace the resident. A compare-exchange counts
    /// even though its failure leaves the resident unchanged: its authority
    /// must cover the success path.
    pub const fn modifies_resident(self) -> bool {
        !matches!(self, Self::Load { .. })
    }

    /// The operands in evaluation order.
    pub fn operands(self) -> Vec<ValueId> {
        match self {
            Self::Load { .. } => Vec::new(),
            Self::Store { value, .. } | Self::Swap { value, .. } => vec![value],
            Self::ReadModifyWrite { operand, .. } => vec![operand],
            Self::CompareExchange {
                expected,
                replacement,
                ..
            } => vec![expected, replacement],
        }
    }

    /// Rewrite every operand through `map`, preserving the event.
    pub fn map_operands(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
        match self {
            Self::Load { .. } => {}
            Self::Store { value, .. } | Self::Swap { value, .. } => *value = map(*value),
            Self::ReadModifyWrite { operand, .. } => *operand = map(*operand),
            Self::CompareExchange {
                expected,
                replacement,
                ..
            } => {
                *expected = map(*expected);
                *replacement = map(*replacement);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AtomicAccessEvent, AtomicReadModifyWrite, MemoryOrdering, ValueId};

    fn value(index: u64) -> ValueId {
        ValueId::new(index).expect("nonzero value identity")
    }

    #[test]
    fn legality_replays_the_source_ordering_matrix() {
        assert!(
            AtomicAccessEvent::Load {
                ordering: MemoryOrdering::Receive
            }
            .ordering_is_legal()
        );
        assert!(
            !AtomicAccessEvent::Load {
                ordering: MemoryOrdering::Publish
            }
            .ordering_is_legal()
        );
        assert!(
            !AtomicAccessEvent::Store {
                value: value(1),
                ordering: MemoryOrdering::ReceivePublish,
            }
            .ordering_is_legal()
        );
        assert!(
            AtomicAccessEvent::CompareExchange {
                expected: value(1),
                replacement: value(2),
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
            }
            .ordering_is_legal()
        );
        assert!(
            !AtomicAccessEvent::CompareExchange {
                expected: value(1),
                replacement: value(2),
                success: MemoryOrdering::Receive,
                failure: MemoryOrdering::GlobalOrder,
            }
            .ordering_is_legal()
        );
    }

    #[test]
    fn operands_and_result_shape_follow_the_event() {
        let fetch = AtomicAccessEvent::ReadModifyWrite {
            operation: AtomicReadModifyWrite::FetchXor,
            operand: value(3),
            ordering: MemoryOrdering::NoOrdering,
        };
        assert_eq!(fetch.operands(), vec![value(3)]);
        assert!(fetch.observes_resident() && fetch.modifies_resident());
        let store = AtomicAccessEvent::Store {
            value: value(4),
            ordering: MemoryOrdering::GlobalOrder,
        };
        assert!(!store.observes_resident() && store.modifies_resident());
        let mut exchange = AtomicAccessEvent::CompareExchange {
            expected: value(5),
            replacement: value(6),
            success: MemoryOrdering::GlobalOrder,
            failure: MemoryOrdering::GlobalOrder,
        };
        exchange.map_operands(&mut |operand| value(operand.get() + 10));
        assert_eq!(exchange.operands(), vec![value(15), value(16)]);
    }
}
