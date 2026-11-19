// Package-level review records: boundary applications, callables, providers,
// and terminal permission policies.

mod boundary_policy;
pub(in crate::record) mod callable_policy;

pub(crate) use boundary_policy::{
    PackagePolicyBoundaryApplicationDemand, PackagePolicyBoundaryApplicationRealization,
    PackagePolicyBoundaryApplications, PackagePolicyBoundaryRealization,
};
mod callables;
pub use callable_policy::PackagePolicyCallable;
pub(crate) use callable_policy::{
    PackagePolicyCallableConformance, PackagePolicyCallableRole, PackagePolicyCallables,
    PackagePolicyServiceReachDependency,
};
#[allow(clippy::module_inception)]
mod provider_policy;
pub(crate) use provider_policy::validate_native_service_method;
mod providers;
mod terminal_permissions;

pub use terminal_permissions::PackagePolicyTerminalPermissions;
pub(crate) use terminal_permissions::{
    PackagePolicyTerminalPermission, PackagePolicyTerminalService,
};

pub(crate) use callables::PackageReviewCheckedServiceReach;
pub(crate) use provider_policy::{
    PackagePolicyProviderBinding, PackagePolicyProviderEvaluatedSyscall,
    PackagePolicyProviderFamily, PackagePolicyProviderFamilyCoordinate, PackagePolicyProviderPlan,
    PackagePolicyProviderRow, PackagePolicyServiceAuthority, PackagePolicyServiceProgressPremise,
    PackagePolicyServiceProgressRoute, PackagePolicyServiceSignature,
};
pub use provider_policy::{PackagePolicySelectedProviders, PackagePolicyServiceMethod};

pub(crate) use providers::{
    PackageReviewBoundaryApplication, PackageReviewBoundaryApplicationArgument,
    PackageReviewCompilerIntrinsicExecution, PackageReviewProviderFamilyCoverage,
    PackageReviewProviderGrantSelectorKind, PackageReviewProviderSelectionAuthority,
    PackageReviewSelectedInstallationReach, PackageReviewSymbolicBoundaryApplicationArgument,
};
