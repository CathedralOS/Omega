//! Compiler-owned atomic memory-order vocabulary and operation legality.
//!
//! These names are semantic commitments, not arbitrary identifiers that an
//! atomic desugar may discard. Target lowering may implement a request with a
//! stronger instruction, but source admission first rejects orderings that the
//! operation cannot mean.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryOrdering {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

impl MemoryOrdering {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Relaxed" => Some(Self::Relaxed),
            "Acquire" => Some(Self::Acquire),
            "Release" => Some(Self::Release),
            "AcqRel" => Some(Self::AcqRel),
            "SeqCst" => Some(Self::SeqCst),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Relaxed => "Relaxed",
            Self::Acquire => "Acquire",
            Self::Release => "Release",
            Self::AcqRel => "AcqRel",
            Self::SeqCst => "SeqCst",
        }
    }

    pub const fn valid_for_load(self) -> bool {
        matches!(self, Self::Relaxed | Self::Acquire | Self::SeqCst)
    }

    pub const fn valid_for_store(self) -> bool {
        matches!(self, Self::Relaxed | Self::Release | Self::SeqCst)
    }

    /// Compare-exchange failure performs only a load: it cannot release, and
    /// it cannot demand synchronization stronger than the success ordering.
    pub const fn valid_compare_exchange_failure(self, success: Self) -> bool {
        match success {
            Self::Relaxed => matches!(self, Self::Relaxed),
            Self::Acquire => matches!(self, Self::Relaxed | Self::Acquire),
            Self::Release => matches!(self, Self::Relaxed),
            Self::AcqRel => matches!(self, Self::Relaxed | Self::Acquire),
            Self::SeqCst => matches!(self, Self::Relaxed | Self::Acquire | Self::SeqCst),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MemoryOrdering as O;

    #[test]
    fn operation_legality_matches_the_c11_matrix() {
        assert!(O::Relaxed.valid_for_load());
        assert!(O::Acquire.valid_for_load());
        assert!(O::SeqCst.valid_for_load());
        assert!(!O::Release.valid_for_load());
        assert!(!O::AcqRel.valid_for_load());

        assert!(O::Relaxed.valid_for_store());
        assert!(O::Release.valid_for_store());
        assert!(O::SeqCst.valid_for_store());
        assert!(!O::Acquire.valid_for_store());
        assert!(!O::AcqRel.valid_for_store());

        assert!(O::Acquire.valid_compare_exchange_failure(O::AcqRel));
        assert!(O::SeqCst.valid_compare_exchange_failure(O::SeqCst));
        assert!(!O::Release.valid_compare_exchange_failure(O::SeqCst));
        assert!(!O::SeqCst.valid_compare_exchange_failure(O::Acquire));
        assert!(!O::Acquire.valid_compare_exchange_failure(O::Release));
    }
}
