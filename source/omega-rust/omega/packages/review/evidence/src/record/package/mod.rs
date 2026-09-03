mod callables;
#[allow(clippy::module_inception)]
// This file groups record modules; `package` holds the package record itself.
mod package;
mod providers;
mod source;

pub use callables::{CheckedPackageCallableReview, PackageReviewCheckedServiceReach};
pub use package::CheckedPackageReviewProjection;
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
