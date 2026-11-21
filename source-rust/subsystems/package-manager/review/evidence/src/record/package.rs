// Package-level review records: boundary applications, callables, providers,
// and terminal permission policies.

mod boundary_policy;
pub(in crate::record) mod callable_policy;

pub(crate) use boundary_policy::PackagePolicyBoundaryApplicationDemand;
pub use boundary_policy::{
    PackagePolicyBoundaryApplicationRealization, PackagePolicyBoundaryApplications,
    PackagePolicyBoundaryRealization,
};
mod callables;
pub use callable_policy::PackagePolicyCallable;
pub(crate) use callable_policy::PackagePolicyCallableConformance;
pub use callable_policy::{
    PackagePolicyCallableRole, PackagePolicyCallables, PackagePolicyServiceReachDependency,
};
#[allow(clippy::module_inception)]
mod provider_policy;
pub(crate) use provider_policy::validate_native_service_method;
mod providers;
mod terminal_permissions;

pub use terminal_permissions::PackagePolicyTerminalPermissions;
pub use terminal_permissions::{PackagePolicyTerminalPermission, PackagePolicyTerminalService};

pub use callables::PackageReviewCheckedServiceReach;
pub use provider_policy::{
    PackagePolicyProviderBinding, PackagePolicyProviderEvaluatedSyscall, PackagePolicyProviderPlan,
    PackagePolicyProviderRow, PackagePolicyServiceAuthority, PackagePolicyServiceSignature,
};
pub(crate) use provider_policy::{
    PackagePolicyProviderFamily, PackagePolicyProviderFamilyCoordinate,
    PackagePolicyServiceProgressPremise, PackagePolicyServiceProgressRoute,
};
pub use provider_policy::{PackagePolicySelectedProviders, PackagePolicyServiceMethod};

pub use providers::{
    PackageReviewBoundaryApplication, PackageReviewCompilerIntrinsicExecution,
    PackageReviewProviderGrantSelectorKind, PackageReviewSelectedInstallationReach,
};
pub(crate) use providers::{
    PackageReviewBoundaryApplicationArgument, PackageReviewProviderFamilyCoverage,
    PackageReviewProviderSelectionAuthority, PackageReviewSymbolicBoundaryApplicationArgument,
};
