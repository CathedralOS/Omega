use super::{PackageReviewCallableContract, PackageReviewNominalIdentity};

/// Closed compiler-owned reason that one exact contract obligation remains
/// open for a later discharge route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractEntailmentOpenReason {
    UnsupportedEnsuresFact,
    UnrecognizedInductiveBody,
    OutsideEntailmentLanguage,
}

/// One source-handle-free contract-entailment obligation retained by package
/// review. This is an open obligation, never evidence of discharge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewContractEntailmentOpenObligation {
    pub(crate) callable: PackageReviewNominalIdentity,
    pub(crate) contract_position: u32,
    pub(crate) fact_position: u32,
    pub(crate) machine_contract_commitment: [u8; 32],
    pub(crate) goal: PackageReviewCallableContract,
    pub(crate) reason: PackageReviewContractEntailmentOpenReason,
}

impl PackageReviewContractEntailmentOpenObligation {
    pub const fn callable(&self) -> &PackageReviewNominalIdentity {
        &self.callable
    }

    pub const fn contract_position(&self) -> u32 {
        self.contract_position
    }

    pub const fn fact_position(&self) -> u32 {
        self.fact_position
    }

    pub const fn machine_contract_commitment(&self) -> [u8; 32] {
        self.machine_contract_commitment
    }

    pub const fn goal(&self) -> &PackageReviewCallableContract {
        &self.goal
    }

    pub const fn reason(&self) -> PackageReviewContractEntailmentOpenReason {
        self.reason
    }
}

/// Source-handle-free evidence that one exact open contract-entailment
/// obligation was discharged by citing an authored assumption.
///
/// The package row retains the complete canonical kernel question and the
/// deterministic assumption selection. It grants no package admission or
/// accepted-lock authority.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewContractEntailmentAssumptionDischarge {
    pub(crate) obligation: PackageReviewContractEntailmentOpenObligation,
    pub(crate) assumptions: Vec<semantic_vocabulary::Proposition>,
    pub(crate) goal: semantic_vocabulary::Proposition,
    pub(crate) selected_assumption_position: u32,
}

impl PackageReviewContractEntailmentAssumptionDischarge {
    pub const fn obligation(&self) -> &PackageReviewContractEntailmentOpenObligation {
        &self.obligation
    }

    pub fn assumptions(&self) -> &[semantic_vocabulary::Proposition] {
        &self.assumptions
    }

    pub const fn goal(&self) -> &semantic_vocabulary::Proposition {
        &self.goal
    }

    pub const fn selected_assumption_position(&self) -> u32 {
        self.selected_assumption_position
    }
}

/// One normal exit covered by the compiler's live membership judgment.
/// State identity is canonical; no arena handle crosses package persistence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewContractEntailmentCoveredExit {
    pub(crate) state: PackageReviewNominalIdentity,
    pub(crate) statement_index: u32,
}

impl PackageReviewContractEntailmentCoveredExit {
    pub const fn state(&self) -> &PackageReviewNominalIdentity {
        &self.state
    }
    pub const fn statement_index(&self) -> u32 {
        self.statement_index
    }
}

/// Exact checked-body membership coverage. This row is inert until fresh
/// compilation custody reproduces its goal and complete normal-exit roster.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewContractEntailmentExitMembershipDischarge {
    pub(crate) obligation: PackageReviewContractEntailmentOpenObligation,
    pub(crate) covered_exits: Vec<PackageReviewContractEntailmentCoveredExit>,
}

impl PackageReviewContractEntailmentExitMembershipDischarge {
    pub const fn obligation(&self) -> &PackageReviewContractEntailmentOpenObligation {
        &self.obligation
    }
    pub fn covered_exits(&self) -> &[PackageReviewContractEntailmentCoveredExit] {
        &self.covered_exits
    }
}
