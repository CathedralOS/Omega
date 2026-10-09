//! Entry, quiescence, publication and retirement of one component era, as receipts
//! rather than state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComponentEraEntryReceipt {
    invocation_identity: u64,
    binding_contract_identity: String,
    entry_contract_identity: String,
    resolved_era_identity: u64,
    entry_plan_identity: String,
    entry_linearized: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComponentEraQuiescenceReceipt {
    era_identity: u64,
    binding_contract_identity: String,
    residual_lifetime_cohort_holds: usize,
    all_dispositions_complete: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComponentEraRetirementReceipt {
    retirement_identity: u64,
    era_identity: u64,
    binding_contract_identity: String,
    lifetime_cohort_released: bool,
}
/// The checkable judgment of a rejected entry replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EraEntryRejection {
    /// No era has published: there is no current record to bind.
    NoPublishedEra,
    /// The ledger's current era is absent from its own roster.
    CurrentEraAbsent,
    /// A copied coordinate diverges from the ledger's retained state, the
    /// invocation is replayed, or the era is not the open current one.
    ExactLinearizationJoin,
}
impl EraEntryRejection {
    pub const fn diagnostic(&self) -> &'static str {
        match self {
            Self::NoPublishedEra => "component binding has no published era",
            Self::CurrentEraAbsent => "current component era is absent from its ledger",
            Self::ExactLinearizationJoin => {
                "component entry receipt does not linearize exactly once into the current open era"
            }
        }
    }
}
/// The checkable judgment of a rejected quiescence replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EraQuiescenceRejection {
    /// The receipt's era is absent from the ledger's live roster.
    EraNotLive,
    /// The era is not a fully-drained Closing record, or the receipt
    /// diverges from the ledger's identities.
    ClosingCohortJoin,
}
impl EraQuiescenceRejection {
    pub const fn diagnostic(&self) -> &'static str {
        match self {
            Self::EraNotLive => "component era is not live",
            Self::ClosingCohortJoin => {
                "component era quiescence requires closing, zero entries, zero program-local root \
                 authority holds, and complete cohort disposition"
            }
        }
    }
}
/// The checkable judgment of a rejected retirement replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EraRetirementRejection {
    /// The receipt's era is absent from the ledger's live roster.
    EraNotLive,
    /// Program-local root epoch leases still pin the era.
    EpochLeasesLive,
    /// The era is not a noncurrent quiescent record, the cohort is not
    /// released, or the minted retirement identity is zero or consumed.
    QuiescentReleaseJoin,
    /// The embedded coexisting-TCB set refused the retirement; the payload
    /// is its own replay diagnostic.
    ExecutableTcbRetirement(String),
}
impl EraRetirementRejection {
    pub fn diagnostic(&self) -> &str {
        match self {
            Self::EraNotLive => "component era is not live",
            Self::EpochLeasesLive => {
                "component era retirement rejects while program-local root epoch leases remain live"
            }
            Self::QuiescentReleaseJoin => {
                "component era retirement requires a noncurrent quiescent released cohort and \
                 fresh receipt"
            }
            Self::ExecutableTcbRetirement(diagnostic) => diagnostic.as_str(),
        }
    }
}
macro_rules! recoverable_error {
    ($name:ident, $value:ident, $type:ty, $rejection:ty) => {
        #[derive(Debug)]
        pub struct $name {
            $value: $type,
            rejection: $rejection,
        }
        impl $name {
            pub fn rejection(&self) -> $rejection {
                self.rejection.clone()
            }
            pub fn diagnostic(&self) -> &str {
                self.rejection.diagnostic()
            }
            pub fn into_value(self) -> $type {
                self.$value
            }
        }
    };
}

recoverable_error!(
    EraEntryError,
    receipt,
    ComponentEraEntryReceipt,
    EraEntryRejection
);
recoverable_error!(
    EraQuiescenceError,
    receipt,
    ComponentEraQuiescenceReceipt,
    EraQuiescenceRejection
);
recoverable_error!(
    EraRetirementError,
    receipt,
    ComponentEraRetirementReceipt,
    EraRetirementRejection
);
