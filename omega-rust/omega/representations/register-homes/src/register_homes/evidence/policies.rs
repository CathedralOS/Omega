//! Stable policy vocabulary authenticated by allocation evidence.
//!
//! These records do not grant validation or publication authority. The owning
//! transform independently reconstructs and compares them before admission.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedPrecoloredIntervalPolicy {
    /// Every selected fixed constraint occupies exactly its authenticated
    /// liveness phase, represented as `[point, point + 1)`.
    FixedConstraintPointIntervalsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedPrecoloredSplitRequirementPolicy {
    /// Partition one-block or exact single-entry fanout source ranges only
    /// when a fixed `Use` makes the accumulated physical-view domain empty.
    FixedUseBoundaryRequirementsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedPrecoloredSegmentHomePolicy {
    /// Place the most constrained remaining domain, then its lowest viable view.
    MostConstrainedLowestCompatibleViewV1,
}

/// Stable structural policy for the first locally witnessed pressure point.
/// This is not an optimization level or a target cost model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpillChoicePolicy {
    SingleBlockFarthestEndThenHighestVregV1,
}

/// Named, deliberately bounded policy for classifying the already selected
/// pressure victim. This is not a spill, rematerialization, or cost policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecoveryClassificationPolicy {
    SelectedVictimImmediateU64EligibilityV1,
}

/// Narrow proof-preserving physical-form fold. This is not a generic constant
/// fold, instruction scheduler, rematerializer, spill policy, or opt level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LiteralFoldPolicy {
    enabled_rules: u8,
}

impl LiteralFoldPolicy {
    const EXACT_ADD_BIT: u8 = 1 << 0;
    const EXACT_SUBTRACT_BIT: u8 = 1 << 1;
    const KNOWN_BITS: u8 = Self::EXACT_ADD_BIT | Self::EXACT_SUBTRACT_BIT;

    pub const EXACT_ADD_V1: Self = Self {
        enabled_rules: Self::EXACT_ADD_BIT,
    };
    pub const EXACT_SUBTRACT_V1: Self = Self {
        enabled_rules: Self::EXACT_SUBTRACT_BIT,
    };

    pub const fn empty() -> Self {
        Self { enabled_rules: 0 }
    }

    pub const fn union(self, other: Self) -> Self {
        Self {
            enabled_rules: self.enabled_rules | other.enabled_rules,
        }
    }

    pub const fn enables_exact_add(self) -> bool {
        self.enabled_rules & Self::EXACT_ADD_BIT != 0
    }

    pub const fn enables_exact_subtract(self) -> bool {
        self.enabled_rules & Self::EXACT_SUBTRACT_BIT != 0
    }

    pub const fn canonical_bits(self) -> u8 {
        self.enabled_rules
    }

    pub const fn from_canonical_bits(bits: u8) -> Option<Self> {
        if bits == 0 || bits & !Self::KNOWN_BITS != 0 {
            None
        } else {
            Some(Self {
                enabled_rules: bits,
            })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PressureRematerializationPolicy {
    /// One reconstructed suffix value serves the sole future flexible Use.
    SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
    /// One reconstructed suffix value is inserted before the first of two or
    /// more canonical future flexible Uses and serves the complete suffix.
    SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
}

/// Exact, deliberately narrow policy for materializing entry-to-fixed-use
/// transitions. This is a stable named transformation, not an allocator mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedViewCopyPolicy {
    /// One copy immediately before each fixed leaf use.
    LeafLocalBeforeFixedUseV1,
    /// One flag-transparent copy after the entry compare and immediately
    /// before its conditional branch, shared by both return leaves.
    SharedEntryAfterCompareBeforeBranchV1,
}
