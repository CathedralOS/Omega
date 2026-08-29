use super::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractFact {
    Expression(PackageReviewContractExpression),
    Membership {
        value: PackageReviewContractExpression,
        domain: PackageReviewNominalIdentity,
    },
    Proposition(PackageReviewPropositionApplication),
    PropositionParameter(PackageReviewPropositionParameterApplication),
}

/// Exact nominal result-arm coordinate guarding one outcome-specific
/// guarantee. The coordinate is absent for unconditional `requires` and
/// `ensures` rows.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewResultCaseIdentity {
    pub(crate) result_data: PackageReviewNominalIdentity,
    pub(crate) result_case: PackageReviewNominalIdentity,
}

impl PackageReviewResultCaseIdentity {
    pub const fn result_data(&self) -> &PackageReviewNominalIdentity {
        &self.result_data
    }

    pub const fn result_case(&self) -> &PackageReviewNominalIdentity {
        &self.result_case
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewCallableContract {
    pub(crate) kind: PackageReviewContractKind,
    pub(crate) result_case: Option<PackageReviewResultCaseIdentity>,
    pub(crate) binding: Option<String>,
    pub(crate) evidence_lane_position: Option<u32>,
    pub(crate) fact: PackageReviewContractFact,
}

impl PackageReviewCallableContract {
    pub const fn kind(&self) -> PackageReviewContractKind {
        self.kind
    }

    pub const fn result_case(&self) -> Option<&PackageReviewResultCaseIdentity> {
        self.result_case.as_ref()
    }

    pub fn binding(&self) -> Option<&str> {
        self.binding.as_deref()
    }

    pub const fn evidence_lane_position(&self) -> Option<u32> {
        self.evidence_lane_position
    }

    pub const fn fact(&self) -> &PackageReviewContractFact {
        &self.fact
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewSynchronousInvocation {
    Parameter(u32),
    Service(PackageReviewNominalIdentity),
}

impl PackageReviewSynchronousInvocation {
    pub const fn parameter(&self) -> Option<u32> {
        match self {
            Self::Parameter(position) => Some(*position),
            Self::Service(_) => None,
        }
    }

    pub const fn service(&self) -> Option<&PackageReviewNominalIdentity> {
        match self {
            Self::Parameter(_) => None,
            Self::Service(service) => Some(service),
        }
    }
}
