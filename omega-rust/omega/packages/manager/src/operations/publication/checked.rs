//! Join accepted project bytes, staged edits, and complete target decisions.

mod error;
use PublishReviewedPackageChangeError as Error;
pub use error::PublishReviewedPackageChangeError;

use super::PackageFileTransaction;
use crate::declarations::BuildFileReplacement;
use crate::lock::{PackageLock, PackageLockRecoveryLimits};
use crate::operations::PackageChangeReview;
use crate::resolution::graph::PackageRootSourceRequest;
use crate::review::{
    PackagePolicyChangeLimits, PackagePolicyResolution, compare_package_policy_changes,
};
use omega_package_source::local::staging::StagedLocalSnapshot;
use omega_package_source::{SourceContentDigest, SourceLineage};
use sha2::{Digest, Sha256};

/// Publish the exact reviewed declaration and all retained target sections.
/// No target is silently dropped. Missing acceptance means fresh review; an
/// edited or mismatched baseline must be reviewed again before publication.
///
/// The transaction journal records an accepted intent, not proof of an audit.
/// A pending-publication error may follow a partial write; recover that intent
/// under the project mutex before loading the next accepted pair.
pub fn publish_reviewed_package_change(
    transaction: &mut PackageFileTransaction,
    replacement: &BuildFileReplacement,
    stage: &StagedLocalSnapshot,
    reviews: &[(&PackageChangeReview, &PackagePolicyResolution)],
    accepted_lock: Option<&str>,
) -> Result<PackageLock, Error> {
    if transaction.project_root() != stage.canonical_live_root()
        || stage.replacement_path().as_str() != "build.omg"
        || stage.expected_sha256() != replacement.expected_sha256()
        || stage.replacement_sha256()
            != &<[u8; 32]>::from(Sha256::digest(replacement.replacement_source().as_bytes()))
    {
        return Err(Error::Association("edit, stage, and project root differ"));
    }
    let planned_root = replacement
        .build_path()
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    if planned_root.canonicalize().ok().as_deref() != Some(stage.canonical_live_root()) {
        return Err(Error::Association("edit plan belongs to another project"));
    }
    let (before_build, before_lock) = transaction.read_pair().map_err(Error::Publication)?;
    if before_lock.as_deref() != accepted_lock.map(str::as_bytes) {
        return Err(Error::Association("accepted lock changed during review"));
    }
    let actual: [u8; 32] = Sha256::digest(&before_build).into();
    if &actual != replacement.expected_sha256() {
        return Err(Error::Association("build.omg changed during review"));
    }
    let accepted = accepted_lock
        .map(|text| PackageLock::recover_text(text, PackageLockRecoveryLimits::default()))
        .transpose()
        .map_err(Error::Lock)?;
    if reviews.is_empty() || reviews.len() > PackageLockRecoveryLimits::default().maximum_targets {
        return Err(Error::Association(
            "publication requires a bounded nonempty target set",
        ));
    }
    if accepted.as_ref().is_some_and(|lock| {
        lock.targets().iter().any(|target| {
            !reviews
                .iter()
                .any(|(review, _)| review.target() == target.target())
        })
    }) {
        return Err(Error::Association(
            "publication would drop an accepted target without review",
        ));
    }
    let mut targets = Vec::new();
    targets
        .try_reserve_exact(reviews.len())
        .map_err(|_| Error::Association("target allocation failed"))?;
    for (review, resolution) in reviews {
        verify_staged_review(stage, review)?;
        let closure = review.source_closure().for_exact_target(review.target());
        let comparison = compare_package_policy_changes(
            accepted
                .as_ref()
                .and_then(|lock| lock.target(review.target())),
            review.reviews(),
            &closure,
            PackagePolicyChangeLimits::default(),
        )
        .map_err(Error::Comparison)?;
        if comparison.fingerprint() != review.changes().fingerprint() {
            return Err(Error::Association(
                "review used a different acceptance baseline",
            ));
        }
        targets.push(
            review
                .propose_lock_target(resolution)
                .map_err(Error::Review)?,
        );
    }
    let proposed = PackageLock::from_targets(targets).map_err(Error::Lock)?;
    let text = proposed.canonical_text().map_err(Error::Lock)?;
    verify_live_dependencies(reviews[0].0)?;
    stage
        .verify_live_source_unchanged()
        .map_err(Error::Source)?;
    transaction
        .publish(
            &before_build,
            replacement.replacement_source().as_bytes(),
            before_lock.as_deref(),
            text.as_bytes(),
        )
        .map_err(Error::Publication)?;
    Ok(proposed)
}

fn verify_live_dependencies(review: &PackageChangeReview) -> Result<(), Error> {
    let closure = review.source_closure();
    for custody in closure.custodies() {
        if custody.key() == closure.graph().root() {
            continue;
        }
        let SourceLineage::ExternalLocal(lineage) = custody.key().source_lineage() else {
            continue;
        };
        let path = lineage.canonical_absolute_path();
        let current = omega_package_source::local::operations::resolve_local_source(
            path,
            custody.source_limits(),
        )
        .map_err(Error::Source)?;
        if custody.materialization().content()
            != &SourceContentDigest::derive(current.content_identity.as_bytes())
        {
            return Err(Error::Source(
                omega_package_source::SourceResolveError::LocalSourceChanged {
                    path: path.to_path_buf(),
                },
            ));
        }
    }
    Ok(())
}

fn verify_staged_review(
    stage: &StagedLocalSnapshot,
    review: &PackageChangeReview,
) -> Result<(), Error> {
    let closure = review.source_closure();
    let Some(root) = closure.custody(closure.graph().root()) else {
        return Err(Error::Association("review has no root source"));
    };
    let PackageRootSourceRequest::ExternalLocal {
        requested_root,
        source_context,
    } = closure.source_requests().root().request()
    else {
        return Err(Error::Association(
            "review root is not the staged local project",
        ));
    };
    let SourceLineage::ExternalLocal(lineage) = root.key().source_lineage() else {
        return Err(Error::Association(
            "review root has a different source lineage",
        ));
    };
    if requested_root != stage.requested_root()
        || lineage.canonical_absolute_path() != stage.canonical_live_root()
        || lineage.source_context() != source_context
        || root.materialization().content()
            != &SourceContentDigest::derive(stage.normalized().content_identity.as_bytes())
    {
        return Err(Error::Association(
            "review root differs from the proposed source",
        ));
    }
    Ok(())
}
