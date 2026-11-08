//! Exact progress-profile subjects and owner-authored establishment routes.
use super::Diagnostic;
use crate::capture::PackageReviewInput;
use crate::capture::nominal_identity;
use crate::capture::semantics::declarations::{
    premise_projections, progress_profile, project_service_progress_routes,
};
use crate::capture::semantics::services::rejected;
use crate::record::PackagePolicyServiceProgressPremise;
use effects::provider_plan::ServiceProgressSubject;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    guarantee: &language_semantics::TerminationGuarantee,
    parameters: &[typed_trees::signature::StateParameter],
) -> Result<Vec<PackagePolicyServiceProgressPremise>, Vec<Diagnostic>> {
    let language_semantics::TerminationGuarantee::Terminates { premises } = guarantee else {
        return Ok(Vec::new());
    };
    let mut projected = Vec::new();
    for premise in premises {
        let profile = progress_profile(compilation, premise.profile, "progress premise")?;
        let subjects = parameters
            .iter()
            .filter(|parameter| parameter.symbol == premise.subject.root)
            .collect::<Vec<_>>();
        let [subject] = subjects.as_slice() else {
            return Err(rejected(
                "progress subject is not one exact requirement parameter",
            ));
        };
        let ordinal = typed_trees::signature::state_parameter_position(parameters, subject.symbol)
            .ok_or_else(|| rejected("progress subject has no source parameter ordinal"))?;
        let subject = ServiceProgressSubject::Parameter(ordinal);
        let establishment_routes = project_service_progress_routes(
            compilation,
            profile,
            "selected provider progress establishment",
        )?;
        projected.push(PackagePolicyServiceProgressPremise {
            profile: nominal_identity(compilation, profile.symbol)?,
            subject,
            subject_projections: premise_projections(compilation, premise)?,
            establishment_routes,
        });
    }
    projected.sort();
    Ok(projected)
}
