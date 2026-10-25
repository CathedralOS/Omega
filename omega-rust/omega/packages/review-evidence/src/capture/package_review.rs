//! Assemble one package's review evidence at the compiler boundary.
//!
//! The caller supplies its remaining canonical policy budget. Enforce it before
//! binding nomination, preserving the review's resource failure order. Root-wide
//! accounting, consumer scoping and cache/publication policy remain outside.

use crate::encoding::PackageReviewEncodingError;
use crate::record::PackageReviewEvidence;
use crate::{SemanticBindingProjectionError, project_checked_package_policy};
use compilation_products::CheckedCompilation;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

#[derive(Debug)]
pub enum PackageReviewProjectionError {
    SourceConsumptionMissing,
    Projection(Vec<Diagnostic>),
    IdentityMismatch,
    Encoding(PackageReviewEncodingError),
    PolicyBudget,
    SemanticBinding(SemanticBindingProjectionError),
}

/// Project exact checked findings without granting consumer permissions.
pub fn project_package_review(
    checked: &CheckedCompilation,
    package: PackageKeyIdentity,
    target: TargetProfile,
    maximum_policy_canonical_bytes: usize,
) -> Result<PackageReviewEvidence, PackageReviewProjectionError> {
    use PackageReviewProjectionError as Error;
    let source_consumption_commitment = checked
        .sources()
        .source_consumption_commitment()
        .ok_or(Error::SourceConsumptionMissing)?;
    let selected_build_machine_identity = checked
        .build_admission()
        .selected_build_machine_identity
        .clone()
        .ok_or_else(|| {
            Error::Projection(vec![Diagnostic::error(
                "package review requires one exact selected build-machine identity",
            )])
        })?;
    let generated_source_bundle = checked
        .package_generated_source_bundle()
        .map_err(|error| Error::Projection(vec![Diagnostic::error(error)]))?;
    if generated_source_bundle.package() != package
        || generated_source_bundle.source_consumption_commitment() != source_consumption_commitment
    {
        return Err(Error::IdentityMismatch);
    }
    let policy =
        project_checked_package_policy(checked, target, package).map_err(Error::Projection)?;
    if policy.package() != package || policy.target() != target {
        return Err(Error::IdentityMismatch);
    }
    let policy_canonical_bytes = policy.canonical_bytes().map_err(Error::Encoding)?.len();
    if policy_canonical_bytes > maximum_policy_canonical_bytes {
        return Err(Error::PolicyBudget);
    }
    let service_binding_candidates =
        super::semantic_bindings::candidate_service_bindings(checked, package)
            .map_err(Error::SemanticBinding)?;
    let hosted_exit_binding_candidates =
        super::semantic_bindings::candidate_hosted_exit_bindings(checked)
            .map_err(Error::SemanticBinding)?;
    Ok(PackageReviewEvidence {
        source_consumption_commitment,
        selected_build_machine_identity,
        build_evaluation_usage: checked.build().evaluation_usage,
        build_observation_summary: checked.build().observation_summary.clone(),
        restricted_build_requests: checked.build_admission().restricted_build_requests.clone(),
        semantic_bindings: checked
            .execution()
            .accepted_semantic_bindings()
            .cloned()
            .collect(),
        service_binding_candidates,
        hosted_exit_binding_candidates,
        generated_source_bundle,
        policy,
        policy_canonical_bytes,
    })
}
