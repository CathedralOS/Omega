use super::super::semantics::declarations::{
    nominal_identity, premise_projections, premise_subject, progress_profile,
};
use crate::capture::PackageReviewInput;
use crate::record::{PackageReviewProgressPremise, PackageReviewTermination};
use diagnostics::Diagnostic;
use typed_trees::signature::StateSignature;

pub(crate) fn project_trait_requirement_termination(
    compilation: &PackageReviewInput<'_>,
    requirement: &StateSignature,
) -> Result<PackageReviewTermination, Vec<Diagnostic>> {
    project_public_signature_termination(
        compilation,
        requirement,
        &format!("public trait requirement `{}`", requirement.name),
    )
}

pub(crate) fn project_machine_parameter_termination(
    compilation: &PackageReviewInput<'_>,
    signature: &StateSignature,
    declaration_path: &str,
) -> Result<PackageReviewTermination, Vec<Diagnostic>> {
    project_public_signature_termination(
        compilation,
        signature,
        &format!("public static-machine parameter on `{declaration_path}`"),
    )
}

/// A public signature's termination guarantee, certified for review evidence:
/// every premise names a public closed progress profile and a subject the
/// public API can carry. Both public signature kinds reach review evidence
/// through this one projection, so their certification cannot drift.
fn project_public_signature_termination(
    compilation: &PackageReviewInput<'_>,
    signature: &StateSignature,
    subject: &str,
) -> Result<PackageReviewTermination, Vec<Diagnostic>> {
    let language_semantics::TerminationGuarantee::Terminates { premises } =
        &signature.termination_guarantee
    else {
        return Ok(PackageReviewTermination::NoGuarantee);
    };
    let parameters = compilation.state_signature_parameters(signature);
    let mut projected = Vec::new();
    for premise in premises {
        let profile = progress_profile(compilation, premise.profile, subject)?;
        if !profile.is_public {
            return Err(vec![Diagnostic::error(format!(
                "{subject} exposes non-public progress profile `{}`",
                profile.name
            ))]);
        }
        projected.push(PackageReviewProgressPremise {
            profile: nominal_identity(compilation, profile.symbol)?,
            subject: premise_subject(compilation, parameters, premise.subject.root, subject)?,
            projections: premise_projections(compilation, premise)?,
        });
    }
    projected.sort();
    projected.dedup();
    Ok(PackageReviewTermination::Terminates {
        premises: projected,
    })
}
