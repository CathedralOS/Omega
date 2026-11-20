use language_core::atomic::{AtomicOrderingPlan, MemoryOrdering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BorrowPolarity {
    Shared,
    Exclusive,
}

impl From<extent_contracts::LoanPolarity> for BorrowPolarity {
    fn from(polarity: extent_contracts::LoanPolarity) -> Self {
        match polarity {
            extent_contracts::LoanPolarity::Shared => BorrowPolarity::Shared,
            extent_contracts::LoanPolarity::Exclusive => BorrowPolarity::Exclusive,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessOperation {
    Read,
    Take,
    Write,
    CompoundMutation,
    Atomic(AtomicAccessOperation),
}

impl AccessOperation {
    /// Reading displaced contents and changing them are independent
    /// obligations. Atomic ordering and the receiver's loan remain
    /// separate checks; a read-modify-write needs both caller grants.
    pub const fn reads_and_writes(self) -> (bool, bool) {
        match self {
            Self::Read | Self::Atomic(AtomicAccessOperation::Load(_)) => (true, false),
            Self::Write | Self::Atomic(AtomicAccessOperation::Store(_)) => (false, true),
            Self::Take
            | Self::CompoundMutation
            | Self::Atomic(AtomicAccessOperation::FetchAdd(_))
            | Self::Atomic(AtomicAccessOperation::FetchSub(_))
            | Self::Atomic(AtomicAccessOperation::FetchXor(_))
            | Self::Atomic(AtomicAccessOperation::FetchOr(_))
            | Self::Atomic(AtomicAccessOperation::FetchAnd(_))
            | Self::Atomic(AtomicAccessOperation::Swap(_))
            | Self::Atomic(AtomicAccessOperation::CompareExchange { .. })
            | Self::Atomic(AtomicAccessOperation::CompareExchangeOnce { .. })
            | Self::Atomic(AtomicAccessOperation::TryExchangeOnce { .. }) => (true, true),
        }
    }

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
    TryExchangeOnce {
        success: MemoryOrdering,
        failure: MemoryOrdering,
    },
}

impl AtomicAccessOperation {
    pub const fn receiver_polarity(self) -> BorrowPolarity {
        BorrowPolarity::Shared
    }

    pub const fn single_attempt(
        observation: language_core::atomic::AtomicCompareExchangeObservationAxis,
        success: MemoryOrdering,
        failure: MemoryOrdering,
    ) -> Self {
        match observation {
            language_core::atomic::AtomicCompareExchangeObservationAxis::Observing => {
                Self::CompareExchangeOnce { success, failure }
            }
            language_core::atomic::AtomicCompareExchangeObservationAxis::NonObserving => {
                Self::TryExchangeOnce { success, failure }
            }
        }
    }

    pub const fn single_attempt_custody(
        self,
    ) -> Option<language_core::atomic::AtomicCompareExchangeOnceResultCustody> {
        use language_core::atomic::AtomicCompareExchangeObservationAxis as Observation;
        let observation = match self {
            Self::CompareExchangeOnce { .. } => Observation::Observing,
            Self::TryExchangeOnce { .. } => Observation::NonObserving,
            _ => return None,
        };
        Some(
            language_core::atomic::AtomicCompareExchangeOnceResultCustody::for_observation(
                observation,
            ),
        )
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
            Self::CompareExchangeOnce { success, failure }
            | Self::TryExchangeOnce { success, failure } => {
                AtomicOrderingPlan::CompareExchangeOnce { success, failure }
            }
        }
    }
}
