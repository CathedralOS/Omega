//! Complete normalized policy deltas; neither decisions nor publication.

mod assembly;
mod error;
mod fingerprints;
mod limits;
mod merge;
mod model;
mod paths;
mod projection;

pub use error::PackagePolicyChangeError;
pub use limits::PackagePolicyChangeLimits;
pub use model::{
    PackagePolicyChangeFingerprint, PackagePolicyChangeKind, PackagePolicyChangeSet,
    PackagePolicyDependencyPath, PackagePolicyDependencyPathStep, PackagePolicyPackageChange,
    PackagePolicyRowChange,
};

use crate::declarations::BuildDeclarationKind;
use crate::lock::PackageLockTarget;
use crate::resolution::graph::{CanonicalSourceClosureSubject, ExactTargetPackageSourceClosure};
use crate::review::{
    CompilerIssuedPackageReviewSet, ReviewOnlyRootRoleChange, ReviewOnlyRootRoleContract,
};
use limits::Budget;
use sha2::Digest;

/// Compare one complete fresh candidate against retained policy or an explicit
/// empty initial baseline. Old content is never reacquired or compiler-replayed.
/// Removed packages remain present with no candidate resolution or path.
///
/// Candidate verification and obligation discharge remain independent. This
/// function does not reinterpret historical decisions or authorize a transaction.
pub fn compare_package_policy_changes(
    accepted: Option<&PackageLockTarget>,
    candidate: &CompilerIssuedPackageReviewSet,
    candidate_sources: &ExactTargetPackageSourceClosure<'_>,
    limits: PackagePolicyChangeLimits,
) -> Result<PackagePolicyChangeSet, PackagePolicyChangeError> {
    let mut budget = Budget::new(limits);
    let reviews = projection::candidate(candidate, candidate_sources, &mut budget)?;
    if accepted.is_some_and(|old| old.target() != candidate_sources.target_profile()) {
        return Err(PackagePolicyChangeError::TargetMismatch);
    }
    let source =
        CanonicalSourceClosureSubject::from_resolved(candidate_sources, budget.subject_limits())
            .map_err(PackagePolicyChangeError::SourceSubject)?;
    budget.context(source.canonical_bytes().len())?;
    if let Some(old) = accepted {
        budget.context(old.source().canonical_bytes().len())?;
    }
    let mut context = fingerprints::context(accepted, &source);
    let mut packages = assembly::packages(accepted, &source, &reviews, &mut budget, &mut context)?;
    let fingerprint = PackagePolicyChangeFingerprint(context.finalize().into());
    for package in &mut packages {
        fingerprints::finish_package(fingerprint, package);
    }
    let root_changed = accepted.is_some_and(|old| {
        old.source().root().selected().key() != source.root().selected().key()
            || old.source().root_role() != source.root_role()
    });
    let source_subject_changed =
        accepted.is_some_and(|old| !old.source().same_source_graph(&source));
    let root_role_change = accepted
        .map(|old| role_change(old.source(), &source, &mut budget))
        .transpose()?
        .flatten();
    Ok(PackagePolicyChangeSet {
        baseline_source_subject: accepted.map(|old| old.source().fingerprint().clone()),
        candidate_source_subject: source.fingerprint().clone(),
        fingerprint,
        root_changed,
        source_subject_changed,
        root_role_change,
        packages,
    })
}

fn role_change(
    old: &CanonicalSourceClosureSubject,
    new: &CanonicalSourceClosureSubject,
    budget: &mut Budget,
) -> Result<Option<ReviewOnlyRootRoleChange>, PackagePolicyChangeError> {
    if old.root().selected().key() != new.root().selected().key() {
        return Ok(None);
    }
    let broken_contract = match (old.root_role(), new.root_role()) {
        (BuildDeclarationKind::Package, BuildDeclarationKind::Application) => {
            ReviewOnlyRootRoleContract::DependencyCompatibility
        }
        (BuildDeclarationKind::Application, BuildDeclarationKind::Package) => {
            ReviewOnlyRootRoleContract::ApplicationActivation
        }
        _ => return Ok(None),
    };
    budget.key(old.root().selected().key())?;
    Ok(Some(ReviewOnlyRootRoleChange {
        root: old.root().selected().key().clone(),
        baseline_role: old.root_role(),
        candidate_role: new.root_role(),
        broken_contract,
    }))
}
