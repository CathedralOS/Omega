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
    pub(crate) assumptions: Vec<psi_core::Proposition>,
    pub(crate) goal: psi_core::Proposition,
    pub(crate) selected_assumption_position: u32,
}

impl PackageReviewContractEntailmentAssumptionDischarge {
    pub const fn obligation(&self) -> &PackageReviewContractEntailmentOpenObligation {
        &self.obligation
    }

    pub fn assumptions(&self) -> &[psi_core::Proposition] {
        &self.assumptions
    }

    pub const fn goal(&self) -> &psi_core::Proposition {
        &self.goal
    }

    pub const fn selected_assumption_position(&self) -> u32 {
        self.selected_assumption_position
    }
}
