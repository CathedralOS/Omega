//! Receipt-free D29 demand and selected application relationships.

mod validation;

use crate::record::accessors::field_projection_accessors;
use crate::record::{
    PackageReviewBoundaryApplication, PackageReviewCompilerIntrinsicExecution,
    PackageReviewNominalIdentity, PackageReviewOperatorCoordinate,
    PackageReviewSymbolicBoundaryApplicationArgument,
};

/// Inert application relationships; no coverage, execution, or admission claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyBoundaryApplications {
    pub(crate) demands: Vec<PackagePolicyBoundaryApplicationDemand>,
    pub(crate) realizations: Vec<PackagePolicyBoundaryApplicationRealization>,
}

impl PackagePolicyBoundaryApplications {
    field_projection_accessors!(sliced demands: &[PackagePolicyBoundaryApplicationDemand]);
    field_projection_accessors!(sliced realizations: &[PackagePolicyBoundaryApplicationRealization]);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyBoundaryApplicationDemand {
    pub(crate) operator_coordinate: PackageReviewOperatorCoordinate,
    pub(crate) producer_callable: PackageReviewNominalIdentity,
    pub(crate) arguments: Vec<PackageReviewSymbolicBoundaryApplicationArgument>,
}

impl PackagePolicyBoundaryApplicationDemand {
    field_projection_accessors!(borrowed operator_coordinate: &PackageReviewOperatorCoordinate);
    field_projection_accessors!(borrowed producer_callable: &PackageReviewNominalIdentity);
    field_projection_accessors!(sliced arguments: &[PackageReviewSymbolicBoundaryApplicationArgument]);
}

/// Selected provider position is in the normalized aggregate's canonical plan order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyBoundaryApplicationRealization {
    pub(crate) operator_coordinate: PackageReviewOperatorCoordinate,
    pub(crate) requirement_identity: String,
    pub(crate) application: PackageReviewBoundaryApplication,
    pub(crate) selected_plan_index: u32,
    pub(crate) realization: PackagePolicyBoundaryRealization,
}

impl PackagePolicyBoundaryApplicationRealization {
    field_projection_accessors!(borrowed operator_coordinate: &PackageReviewOperatorCoordinate);
    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }
    field_projection_accessors!(borrowed application: &PackageReviewBoundaryApplication);
    field_projection_accessors!(copied selected_plan_index: u32);
    field_projection_accessors!(borrowed realization: &PackagePolicyBoundaryRealization);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackagePolicyBoundaryRealization {
    NongenericCheckedBody {
        declaration: PackageReviewNominalIdentity,
        realization: PackageReviewNominalIdentity,
    },
    SpecializedCheckedBody {
        declaration: PackageReviewNominalIdentity,
        template: PackageReviewNominalIdentity,
    },
    ExactCompilerIntrinsic {
        execution: PackageReviewCompilerIntrinsicExecution,
    },
}
