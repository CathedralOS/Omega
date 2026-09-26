//! Receipt-free D29 demand and selected application relationships.

mod validation;

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
    pub fn demands(&self) -> &[PackagePolicyBoundaryApplicationDemand] {
        &self.demands
    }
    pub fn realizations(&self) -> &[PackagePolicyBoundaryApplicationRealization] {
        &self.realizations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyBoundaryApplicationDemand {
    pub(crate) operator_coordinate: PackageReviewOperatorCoordinate,
    pub(crate) producer_callable: PackageReviewNominalIdentity,
    pub(crate) arguments: Vec<PackageReviewSymbolicBoundaryApplicationArgument>,
}

impl PackagePolicyBoundaryApplicationDemand {
    pub const fn operator_coordinate(&self) -> &PackageReviewOperatorCoordinate {
        &self.operator_coordinate
    }
    pub const fn producer_callable(&self) -> &PackageReviewNominalIdentity {
        &self.producer_callable
    }
    pub fn arguments(&self) -> &[PackageReviewSymbolicBoundaryApplicationArgument] {
        &self.arguments
    }
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
    pub const fn operator_coordinate(&self) -> &PackageReviewOperatorCoordinate {
        &self.operator_coordinate
    }
    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }
    pub const fn application(&self) -> &PackageReviewBoundaryApplication {
        &self.application
    }
    pub const fn selected_plan_index(&self) -> u32 {
        self.selected_plan_index
    }
    pub const fn realization(&self) -> &PackagePolicyBoundaryRealization {
        &self.realization
    }
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
