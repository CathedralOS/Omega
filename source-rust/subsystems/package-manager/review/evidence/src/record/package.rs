// Package-level review records: boundary applications, callables, providers,
// and terminal permission policies.

mod boundary_policy;
pub(in crate::record) mod callable_policy;

use super::PackageReviewServiceIdentity;
pub(crate) use boundary_policy::PackagePolicyBoundaryApplicationDemand;
pub use boundary_policy::{
    PackagePolicyBoundaryApplicationRealization, PackagePolicyBoundaryApplications,
    PackagePolicyBoundaryRealization,
};
pub(crate) use callable_policy::PackagePolicyCallableConformance;
pub use callable_policy::{
    PackagePolicyCallable, PackagePolicyCallableRole, PackagePolicyCallables,
    PackagePolicyServiceReachDependency,
};
pub use provider_policy::{
    PackagePolicyProviderBinding, PackagePolicyProviderEvaluatedSyscall, PackagePolicyProviderPlan,
    PackagePolicyProviderRow, PackagePolicySelectedProviders, PackagePolicyServiceAuthority,
    PackagePolicyServiceMethod, PackagePolicyServiceSignature,
};
pub(crate) use provider_policy::{
    PackagePolicyProviderFamily, PackagePolicyProviderFamilyCoordinate,
    PackagePolicyServiceProgressPremise, PackagePolicyServiceProgressRoute,
    validate_native_service_method,
};
pub use providers::{
    PackageReviewBoundaryApplication, PackageReviewCompilerIntrinsicExecution,
    PackageReviewProviderGrantSelectorKind, PackageReviewSelectedInstallationReach,
};
pub(crate) use providers::{
    PackageReviewBoundaryApplicationArgument, PackageReviewProviderFamilyCoverage,
    PackageReviewProviderSelectionAuthority, PackageReviewSymbolicBoundaryApplicationArgument,
};
pub use terminal_permissions::{
    PackagePolicyTerminalPermission, PackagePolicyTerminalPermissions, PackagePolicyTerminalService,
};
#[allow(clippy::module_inception)]
mod provider_policy;
mod providers;
mod terminal_permissions;

// Checked implementation body service-reach state (`PackageReviewCheckedServiceReach`).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewCheckedServiceReach {
    NoCheckedBody,
    CheckedBody {
        realized: Vec<PackageReviewServiceIdentity>,
        concrete: Vec<PackageReviewServiceIdentity>,
    },
}

impl PackageReviewCheckedServiceReach {
    pub fn realized(&self) -> Option<&[PackageReviewServiceIdentity]> {
        match self {
            Self::NoCheckedBody => None,
            Self::CheckedBody { realized, .. } => Some(realized),
        }
    }

    pub fn concrete(&self) -> Option<&[PackageReviewServiceIdentity]> {
        match self {
            Self::NoCheckedBody => None,
            Self::CheckedBody { concrete, .. } => Some(concrete),
        }
    }
}
