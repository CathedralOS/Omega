//! Fresh compiler findings for a recovered graph, beside its accepted policy.

use super::{LockedSourceRecoveryOptions, RecoverLockedSourcesError, recover_locked_sources};
use crate::declarations::PackageKey;
use crate::lock::{PackageLock, PackageLockTarget};
use crate::resolution::graph::{PackageRootSourceRequest, ResolvedPackageSourceClosure};
use crate::review::{
    CompileResolvedPackageReviewsError, CompilerIssuedPackageReviewSet,
    LockedPolicyComparisonError, compare_locked_package_policies,
    compile_resolved_package_candidate_reviews,
};
use package_source::SourceResolverStorage;
use std::fmt;
use std::path::Path;
use target::TargetProfile;

/// The accepted section remains borrowed history; reviews belong to this fresh
/// compiler run. Policy equality neither records a decision nor admits source.
#[derive(Debug)]
pub struct CheckedLockedSources<'lock> {
    accepted: &'lock PackageLockTarget,
    source_closure: ResolvedPackageSourceClosure,
    reviews: CompilerIssuedPackageReviewSet,
    changed_policies: Vec<PackageKey>,
}

impl<'lock> CheckedLockedSources<'lock> {
    pub const fn accepted(&self) -> &'lock PackageLockTarget {
        self.accepted
    }

    pub const fn source_closure(&self) -> &ResolvedPackageSourceClosure {
        &self.source_closure
    }

    pub const fn reviews(&self) -> &CompilerIssuedPackageReviewSet {
        &self.reviews
    }

    /// Exact packages whose complete normalized policy differs, in the lock's
    /// canonical source order. This is not a row-level conflict or decision set.
    pub fn changed_policies(&self) -> &[PackageKey] {
        &self.changed_policies
    }

    pub fn into_parts(
        self,
    ) -> (
        &'lock PackageLockTarget,
        ResolvedPackageSourceClosure,
        CompilerIssuedPackageReviewSet,
        Vec<PackageKey>,
    ) {
        (
            self.accepted,
            self.source_closure,
            self.reviews,
            self.changed_policies,
        )
    }
}

#[derive(Debug)]
pub enum CheckLockedSourcesError {
    Recovery(RecoverLockedSourcesError),
    Compilation(CompileResolvedPackageReviewsError),
    Comparison(LockedPolicyComparisonError),
}

impl fmt::Display for CheckLockedSourcesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recovery(error) => error.fmt(formatter),
            Self::Compilation(error) => {
                write!(
                    formatter,
                    "fresh checking of locked sources failed: {error}"
                )
            }
            Self::Comparison(error) => {
                write!(
                    formatter,
                    "cannot compare accepted and fresh package policy: {error}"
                )
            }
        }
    }
}

impl std::error::Error for CheckLockedSourcesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Recovery(error) => Some(error),
            Self::Compilation(error) => Some(error),
            Self::Comparison(error) => Some(error),
        }
    }
}

/// Reacquire exact pins and check the complete graph with the current compiler.
///
/// No retained compiler analysis is reused. The ordinary candidate entrance
/// retains its preliminary semantic-binding discovery and final checking where
/// needed, including generated-source handoffs. No additional review replay,
/// admission, certificate, or native production is required by this operation.
///
/// An unchanged policy is only equality with the project's accepted baseline.
/// Differences are reported without updating the lock or authorizing a changed
/// policy. Missing or corrupt old source remains a recovery error; this route
/// never chooses a newer revision as fallback. A standalone candidate review
/// must be explicitly supplied through the separate install/update workflow.
pub fn check_locked_sources<'lock>(
    lock: &'lock PackageLock,
    target: TargetProfile,
    root_request: &PackageRootSourceRequest,
    storage: &SourceResolverStorage,
    options: LockedSourceRecoveryOptions,
    build_root: &Path,
) -> Result<CheckedLockedSources<'lock>, CheckLockedSourcesError> {
    let source_closure = recover_locked_sources(lock, target, root_request, storage, options)
        .map_err(CheckLockedSourcesError::Recovery)?;
    let accepted = lock
        .target(target)
        .expect("successful locked recovery selected this exact retained target");
    let reviews = compile_resolved_package_candidate_reviews(
        &source_closure.for_exact_target(target),
        build_root,
    )
    .map_err(CheckLockedSourcesError::Compilation)?;
    let changed_policies = compare_locked_package_policies(accepted, &reviews)
        .map_err(CheckLockedSourcesError::Comparison)?;
    Ok(CheckedLockedSources {
        accepted,
        source_closure,
        reviews,
        changed_policies,
    })
}
