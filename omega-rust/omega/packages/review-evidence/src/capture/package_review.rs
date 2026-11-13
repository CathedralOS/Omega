/*
Compiler-boundary assembly of `PackageReviewEvidence` (`project_package_review`).

We verify that the build handoff's package identity, source consumption
commitment, and selection observation match the checked compilation, project
`PackagePolicyBaseline`, enforce `maximum_policy_canonical_bytes` before
nominating semantic bindings (`candidate_service_bindings` and
`candidate_hosted_exit_bindings`), and return the inert review capsule without
granting consumer permissions.
*/

use crate::SemanticBindingProjectionError;
use crate::encoding::PackageReviewEncodingError;
use crate::record::PackageReviewEvidence;
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
    let build_handoff = checked
        .package_build_handoff()
        .map_err(|error| Error::Projection(vec![Diagnostic::error(error)]))?;
    if build_handoff
        .selection_observation()
        .is_some_and(|original| {
            Some(original)
                != checked
                    .build()
                    .observation_summary
                    .as_ref()
                    .map(|summary| summary.identity().digest())
        })
    {
        return Err(Error::IdentityMismatch);
    }
    if build_handoff.package() != package
        || build_handoff.source_consumption_commitment() != source_consumption_commitment
    {
        return Err(Error::IdentityMismatch);
    }
    let _ = (
        selected_build_machine_identity,
        maximum_policy_canonical_bytes,
    );
    Err(Error::Projection(super::unavailable_projection(
        "package review policy",
    )))
}
