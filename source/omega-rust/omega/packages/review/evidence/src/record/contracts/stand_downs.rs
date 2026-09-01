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
