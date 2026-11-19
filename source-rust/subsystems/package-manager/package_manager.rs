//! Prepare package sources and admit project decisions. Build executes the requested
//! compilations and returns their evidence to the transaction that requested it.

#![forbid(unsafe_code)]

pub mod dependencies;
pub mod lockfile;
pub mod review;
pub mod transaction;

pub use dependencies::declarations;
pub use lockfile as lock;
pub use transaction as package_manager;
pub use transaction::{
    PackageCommand, PackageCommandError, PackageCommandKind, PackageCommandOptions,
    PackageCommandOutcome, PackageCommandPreparation, PackageCommandReview,
    PackageCommandReviewRequest, PackageCommandStatus, PackageCommandTransaction,
    PreparedPackageCommand, discard_package_review,
};

pub(crate) use lockfile::hex::{Hex, hex_digest};
pub(crate) use review::build_session;
pub use review::build_session::{PackageBuildSession, PackageSourceBindings};
pub(crate) use wire::{codec_bytes, digest_encoder};

/// Source selection and graph queries used by Build and review.
pub mod resolution {
    pub(crate) use crate::dependencies::PackageCompilationScope;
    pub use crate::dependencies::{graph, source};
}

/// Existing Build and CLI callers import these operations together. Each operation
/// executes in the subsystem that owns the associated state.
pub mod operations {
    pub use crate::dependencies::{
        AcceptedDeployment, LocalProjectPreparation, PrepareLocalProjectError,
        PreparedLocalProject, ReviewedHostPackages, begin_local_project_preparation,
    };
    pub use crate::review::inspect_source::{SourceAdapter, inspect_package_source_locator};
    pub use crate::review::{
        PackageInspectionError, PackageInspectionOptions, PackageInspectionOutcome,
        PackageInspectionSources, PackageInspectionTransaction, PreparedPackageInspection,
        inspect_source,
    };
}

macro_rules! bounded_limits {
    ($visibility:vis fn; $($field:ident),+ $(,)?) => {
        $visibility fn bounded(self) -> Self {
            let ceiling = Self::default();
            Self { $($field: self.$field.min(ceiling.$field),)+ }
        }
    };
}
pub(crate) use bounded_limits;

macro_rules! reserve {
    ($buffer:expr, $count:expr, $error:ty) => {
        $buffer
            .try_reserve_exact($count)
            .map_err(|_| <$error>::AllocationFailed)?
    };
}
pub(crate) use reserve;
