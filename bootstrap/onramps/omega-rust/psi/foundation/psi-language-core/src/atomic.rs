//! Compiler-owned atomic memory-order vocabulary and operation legality.
//!
//! These names are semantic commitments, not arbitrary identifiers that an
//! atomic desugar may discard. Target lowering may implement a request with a
//! stronger instruction, but source admission first rejects orderings that the
//! operation cannot mean.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryOrdering {
    NoOrdering,
    Receive,
    Publish,
    ReceivePublish,
    GlobalOrder,
}

/// The operation-specific ordering commitment attached to an atomic source
/// expression. This survives compiler-authored carrier lowering so later
/// phases do not have to rediscover semantics from an expression shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicOrderingPlan {
    Load(MemoryOrdering),
    Store(MemoryOrdering),
    ReadModifyWrite(MemoryOrdering),
    Swap(MemoryOrdering),
    CompareExchange {
        success: MemoryOrdering,
        failure: MemoryOrdering,
    },
}

impl AtomicOrderingPlan {
    pub const fn success(self) -> MemoryOrdering {
        match self {
            Self::Load(ordering)
            | Self::Store(ordering)
            | Self::ReadModifyWrite(ordering)
            | Self::Swap(ordering) => ordering,
            Self::CompareExchange { success, .. } => success,
        }
    }

    pub const fn failure(self) -> Option<MemoryOrdering> {
        match self {
            Self::CompareExchange { failure, .. } => Some(failure),
            _ => None,
        }
    }
}

impl MemoryOrdering {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "NoOrdering" => Some(Self::NoOrdering),
            "Receive" => Some(Self::Receive),
            "Publish" => Some(Self::Publish),
            "ReceivePublish" => Some(Self::ReceivePublish),
            "GlobalOrder" => Some(Self::GlobalOrder),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::NoOrdering => "NoOrdering",
            Self::Receive => "Receive",
            Self::Publish => "Publish",
            Self::ReceivePublish => "ReceivePublish",
            Self::GlobalOrder => "GlobalOrder",
        }
    }

    pub const fn valid_for_load(self) -> bool {
        matches!(self, Self::NoOrdering | Self::Receive | Self::GlobalOrder)
    }

    pub const fn valid_for_store(self) -> bool {
        matches!(self, Self::NoOrdering | Self::Publish | Self::GlobalOrder)
    }

    /// Compare-exchange failure performs only a load: it cannot publish, and
    /// it cannot demand synchronization stronger than the success ordering.
    pub const fn valid_compare_exchange_failure(self, success: Self) -> bool {
        match success {
            Self::NoOrdering => matches!(self, Self::NoOrdering),
            Self::Receive => matches!(self, Self::NoOrdering | Self::Receive),
            Self::Publish => matches!(self, Self::NoOrdering),
            Self::ReceivePublish => matches!(self, Self::NoOrdering | Self::Receive),
            Self::GlobalOrder => {
                matches!(self, Self::NoOrdering | Self::Receive | Self::GlobalOrder)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MemoryOrdering as O;

    #[test]
    fn operation_legality_matches_the_c11_matrix() {
        assert!(O::NoOrdering.valid_for_load());
        assert!(O::Receive.valid_for_load());
        assert!(O::GlobalOrder.valid_for_load());
        assert!(!O::Publish.valid_for_load());
        assert!(!O::ReceivePublish.valid_for_load());

        assert!(O::NoOrdering.valid_for_store());
        assert!(O::Publish.valid_for_store());
        assert!(O::GlobalOrder.valid_for_store());
        assert!(!O::Receive.valid_for_store());
        assert!(!O::ReceivePublish.valid_for_store());

        assert!(O::Receive.valid_compare_exchange_failure(O::ReceivePublish));
        assert!(O::GlobalOrder.valid_compare_exchange_failure(O::GlobalOrder));
        assert!(!O::Publish.valid_compare_exchange_failure(O::GlobalOrder));
        assert!(!O::GlobalOrder.valid_compare_exchange_failure(O::Receive));
        assert!(!O::Receive.valid_compare_exchange_failure(O::Publish));
    }

    #[test]
    fn operation_plan_keeps_compare_exchange_axes_separate() {
        let plan = super::AtomicOrderingPlan::CompareExchange {
            success: O::ReceivePublish,
            failure: O::Receive,
        };
        assert_eq!(plan.success(), O::ReceivePublish);
        assert_eq!(plan.failure(), Some(O::Receive));
        assert_eq!(
            super::AtomicOrderingPlan::Load(O::GlobalOrder).failure(),
            None
        );
    }
}
