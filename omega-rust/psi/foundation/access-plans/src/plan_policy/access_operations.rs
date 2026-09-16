//! The borrow polarities and access operations a placed field can be asked
//! to perform.

use language_core::atomic::{AtomicOrderingPlan, MemoryOrdering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BorrowPolarity {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessOperation {
    Read,
    /// One destructive external read. It consumes the device event through an
    /// exclusive current borrow and never derives ordinary readability.
    Take,
    Write,
    /// Ordinary stable compound mutation. Legality is derived from stable
    /// read+write permission and an exclusive borrow; it is never an external
    /// primitive permission.
    CompoundMutation,
    /// One atomic operation carrying the exact source-selected ordering plan.
    ///
    /// The exact operation family remains distinct while its ordering converts
    /// to the shared `AtomicOrderingPlan` carried by the compiler pipeline.
    Atomic(AtomicAccessOperation),
}

impl AccessOperation {
    /// The borrow polarity the operation's sealed requirement declares for
    /// its receiver. Ordinary writes, takes, and compound mutation need an
    /// exclusive receiver; reads and every atomic family are shared
    /// (`wiki/spec/language/concurrency.md`: "All receivers are shared").
    /// The custody rule that authorizes an operation through a borrow reads
    /// this rather than deciding per type name.
    pub const fn receiver_polarity(self) -> BorrowPolarity {
        match self {
            Self::Read => BorrowPolarity::Shared,
            Self::Atomic(operation) => operation.receiver_polarity(),
            Self::Take | Self::Write | Self::CompoundMutation => BorrowPolarity::Exclusive,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicAccessOperation {
    Load(MemoryOrdering),
    Store(MemoryOrdering),
    FetchAdd(MemoryOrdering),
    FetchSub(MemoryOrdering),
    FetchXor(MemoryOrdering),
    FetchOr(MemoryOrdering),
    FetchAnd(MemoryOrdering),
    Swap(MemoryOrdering),
    CompareExchange {
        success: MemoryOrdering,
        failure: MemoryOrdering,
    },
    CompareExchangeOnce {
        success: MemoryOrdering,
        failure: MemoryOrdering,
    },
}

impl AtomicAccessOperation {
    /// Every sealed atomic requirement takes a shared receiver: the hardware
    /// operation serializes the cell itself, so an exclusive borrow of the
    /// enclosing record adds no permission and a shared one withholds none.
    /// This is one fact for all families, including the mutating ones.
    pub const fn receiver_polarity(self) -> BorrowPolarity {
        BorrowPolarity::Shared
    }

    pub const fn ordering_plan(self) -> AtomicOrderingPlan {
        match self {
            Self::Load(ordering) => AtomicOrderingPlan::Load(ordering),
            Self::Store(ordering) => AtomicOrderingPlan::Store(ordering),
            Self::FetchAdd(ordering)
            | Self::FetchSub(ordering)
            | Self::FetchXor(ordering)
            | Self::FetchOr(ordering)
            | Self::FetchAnd(ordering) => AtomicOrderingPlan::ReadModifyWrite(ordering),
            Self::Swap(ordering) => AtomicOrderingPlan::Swap(ordering),
            Self::CompareExchange { success, failure } => {
                AtomicOrderingPlan::CompareExchange { success, failure }
            }
            Self::CompareExchangeOnce { success, failure } => {
                AtomicOrderingPlan::CompareExchangeOnce { success, failure }
            }
        }
    }
}
