//! Package-level review records: the package itself, its source, callables
//! and providers, and the boundary, callable, provider and terminal
//! permission policies it publishes.

mod boundary_policy;
pub(in crate::record) mod callable_policy;
pub use boundary_policy::{
    PackagePolicyBoundaryApplicationDemand, PackagePolicyBoundaryApplicationRealization,
    PackagePolicyBoundaryApplications, PackagePolicyBoundaryRealization,
};
mod callables;
pub use callable_policy::{
    PackagePolicyCallable, PackagePolicyCallableConformance, PackagePolicyCallableRole,
    PackagePolicyCallables, PackagePolicyServiceReachDependency,
};
#[allow(clippy::module_inception)]
// This file groups record modules; `package` holds the package record itself.
mod provider_policy;
mod providers;
mod terminal_permissions;

pub use terminal_permissions::{
    PackagePolicyTerminalPermission, PackagePolicyTerminalPermissions, PackagePolicyTerminalService,
};

pub use callables::PackageReviewCheckedServiceReach;
pub use provider_policy::{
    PackagePolicyProviderBinding, PackagePolicyProviderEvaluatedSyscall,
    PackagePolicyProviderFamily, PackagePolicyProviderFamilyCoordinate, PackagePolicyProviderPlan,
    PackagePolicyProviderRow, PackagePolicySelectedProviders, PackagePolicyServiceAuthority,
    PackagePolicyServiceMethod, PackagePolicyServiceProgressPremise,
    PackagePolicyServiceProgressRoute, PackagePolicyServiceSignature,
};
pub use providers::{
    CheckedPackageBoundaryApplicationRealizationReview, PackageReviewBoundaryApplication,
    PackageReviewBoundaryApplicationArgument, PackageReviewBoundaryApplicationRealization,
    PackageReviewCompilerIntrinsicExecution, PackageReviewProviderFamilyCoverage,
    PackageReviewProviderGrantSelectorKind, PackageReviewProviderSelectionAuthority,
    PackageReviewSelectedInstallationReach, PackageReviewSymbolicBoundaryApplicationArgument,
};
