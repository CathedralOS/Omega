//! Reacquire one retained target's exact source graph without replaying policy.

use crate::lock::PackageLock;
use crate::resolution::graph::{
    CanonicalSourceClosureSubjectLimits, PackageRootSourceRequest, PackageSourceClosureLimits,
    ResolveLockedPackageClosureError, ResolvedPackageSourceClosure,
    resolve_locked_package_source_closure_with_storage,
};
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::{LocalSourceLimits, SourceResolverStorage};
use std::fmt;
use target::TargetProfile;

/// Acquisition permission and current resource ceilings, not accepted policy.
#[derive(Debug, Clone, Copy)]
pub struct LockedSourceRecoveryOptions {
    pub git_acquisition: GitExactRevisionAcquisition,
    pub source_limits: LocalSourceLimits,
    pub closure_limits: PackageSourceClosureLimits,
    pub subject_limits: CanonicalSourceClosureSubjectLimits,
}

impl Default for LockedSourceRecoveryOptions {
    fn default() -> Self {
        Self {
            git_acquisition: GitExactRevisionAcquisition::Offline,
            source_limits: LocalSourceLimits::default(),
            closure_limits: PackageSourceClosureLimits::default(),
            subject_limits: CanonicalSourceClosureSubjectLimits::default(),
        }
    }
}

#[derive(Debug)]
pub enum RecoverLockedSourcesError {
    MissingTarget { target: TargetProfile },
    Resolution(ResolveLockedPackageClosureError),
}

impl fmt::Display for RecoverLockedSourcesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTarget { target } => write!(
                formatter,
                "accepted lock has no source section for target {}; select a recorded target or explicitly review the required target without updating source pins",
                target.identity().as_str(),
            ),
            Self::Resolution(error) => {
                write!(formatter, "cannot recover exact locked sources: {error}")
            }
        }
    }
}

impl std::error::Error for RecoverLockedSourcesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MissingTarget { .. } => None,
            Self::Resolution(error) => Some(error),
        }
    }
}

/// Select the retained target before any storage verification or acquisition,
/// then rebuild its graph from freshly verified sources and declarations.
///
/// The caller supplies the typed root request so platform-encoded retained path
/// bytes never become filesystem authority through unchecked decoding. Its exact
/// spelling must match the lock. Git defaults to offline acquisition; allowing
/// a fetch permits only the recorded revision, never selector refresh. Local
/// sources must still be available for recapture and must match the recorded
/// content, lineage, role, navigation, and dependency projection.
///
/// The borrowed lock remains readable on every failure. Neither its accepted
/// baselines nor historical choices become fresh compiler review. The returned
/// closure can enter ordinary compiler checking through existing package inputs.
pub fn recover_locked_sources(
    lock: &PackageLock,
    target: TargetProfile,
    root_request: &PackageRootSourceRequest,
    storage: &SourceResolverStorage,
    options: LockedSourceRecoveryOptions,
) -> Result<ResolvedPackageSourceClosure, RecoverLockedSourcesError> {
    let retained = lock
        .target(target)
        .ok_or(RecoverLockedSourcesError::MissingTarget { target })?;
    resolve_locked_package_source_closure_with_storage(
        retained.source(),
        root_request,
        options.git_acquisition,
        storage,
        options.source_limits,
        options.closure_limits,
        options.subject_limits,
    )
    .map_err(RecoverLockedSourcesError::Resolution)
}
