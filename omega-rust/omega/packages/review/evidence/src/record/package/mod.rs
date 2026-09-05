mod callable_policy;
mod callables;
pub use callable_policy::{
    PackagePolicyCallable, PackagePolicyCallableConformance, PackagePolicyCallableRole,
    PackagePolicyCallables,
};
#[allow(clippy::module_inception)]
// This file groups record modules; `package` holds the package record itself.
mod package;
mod provider_policy;
mod providers;
mod source;
mod terminal_permissions;
#[cfg(test)]
pub(crate) use terminal_permissions::write_service_parameter_identity;

pub use terminal_permissions::{
    PackagePolicyTerminalPermission, PackagePolicyTerminalPermissions, PackagePolicyTerminalService,
};

pub use callables::{CheckedPackageCallableReview, PackageReviewCheckedServiceReach};
pub use package::CheckedPackageReviewProjection;
pub use provider_policy::{
    PackagePolicyProviderBinding, PackagePolicyProviderEvaluatedSyscall,
    PackagePolicyProviderFamily, PackagePolicyProviderFamilyCoordinate, PackagePolicyProviderPlan,
    PackagePolicyProviderRow, PackagePolicySelectedProviders, PackagePolicyServiceAuthority,
    PackagePolicyServiceMethod, PackagePolicyServiceProgressPremise,
    PackagePolicyServiceProgressRoute, PackagePolicyServiceSignature,
};
pub use providers::{
    CheckedPackageBoundaryApplicationDemandReview,
    CheckedPackageBoundaryApplicationRealizationReview,
    CheckedPackageProviderFamilyCoordinateReview, CheckedPackageProviderFamilyReview,
    CheckedPackageProviderReview, CheckedPackageProviderRowIdentity,
    PackageReviewBoundaryApplication, PackageReviewBoundaryApplicationArgument,
    PackageReviewBoundaryApplicationRealization, PackageReviewBoundaryApplicationRealizationRole,
    PackageReviewCompilerIntrinsicExecution, PackageReviewProviderFamilyCoverage,
    PackageReviewProviderGrantSelectorKind, PackageReviewProviderSelectionAuthority,
    PackageReviewSelectedInstallationReach, PackageReviewSelectedProviderGrant,
    PackageReviewSymbolicBoundaryApplicationArgument,
};
pub(crate) use source::PackageReviewCanonicalRowSources;
