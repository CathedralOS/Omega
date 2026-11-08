use crate::record::PackageReviewContractExpression;
use crate::record::PackageReviewContractKind;
use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewPropositionApplication;
use crate::record::PackageReviewPropositionParameterApplication;
use crate::record::accessors::field_projection_accessors;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractFact {
    Expression(PackageReviewContractExpression),
    Membership {
        value: PackageReviewContractExpression,
        domain: PackageReviewNominalIdentity,
    },
    /// Closed compiler permission fact; no nominal value-domain owner exists.
    /// This describes the checked contract and grants no runtime authority.
    CarryMembership {
        value: PackageReviewContractExpression,
        permission: language_semantics::CarryPermission,
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
    field_projection_accessors!(borrowed result_data: &PackageReviewNominalIdentity);
    field_projection_accessors!(borrowed result_case: &PackageReviewNominalIdentity);
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
    field_projection_accessors!(copied kind: PackageReviewContractKind);
    field_projection_accessors!(optional result_case: Option<&PackageReviewResultCaseIdentity>);

    pub fn binding(&self) -> Option<&str> {
        self.binding.as_deref()
    }

    field_projection_accessors!(copied evidence_lane_position: Option<u32>);
    field_projection_accessors!(borrowed fact: &PackageReviewContractFact);
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
