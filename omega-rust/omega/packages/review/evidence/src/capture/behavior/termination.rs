use super::super::contracts::expressions::names::portable_parameter_position;
use super::super::semantics::declarations::nominal_identity;
use crate::record::{
    PackageReviewProgressPremise, PackageReviewProgressSubject, PackageReviewTermination,
};
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(crate) fn project_termination(
    compilation: &CheckedCompilation,
    guarantee: &language_semantics::TerminationGuarantee,
) -> Result<PackageReviewTermination, Vec<Diagnostic>> {
    project_termination_with_subject(compilation, guarantee, |root| {
        nominal_identity(compilation, root).map(PackageReviewProgressSubject::Declaration)
    })
}

pub(crate) fn project_trait_requirement_termination(
    compilation: &CheckedCompilation,
    requirement: &typed_trees::signature::StateSignature,
) -> Result<PackageReviewTermination, Vec<Diagnostic>> {
    let parameters = compilation.state_signature_parameters(requirement);
    if let language_semantics::TerminationGuarantee::Terminates { premises } =
        &requirement.termination_guarantee
    {
        for premise in premises {
            let profile = compilation
                .domain_definitions()
                .iter()
                .find(|domain| domain.semantic_id == premise.profile)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "public trait requirement `{}` has an unknown termination profile",
                        requirement.name
                    ))]
                })?;
            if !profile.is_public {
                return Err(vec![Diagnostic::error(format!(
                    "public trait requirement `{}` exposes non-public progress profile `{}`",
                    requirement.name, profile.name
                ))]);
            }
        }
    }
    project_termination_with_subject(compilation, &requirement.termination_guarantee, |root| {
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.symbol == root)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "public trait requirement `{}` has a termination premise outside its parameter telescope",
                    requirement.name
                ))]
            })?;
        if parameter.is_self {
            return Ok(PackageReviewProgressSubject::Receiver);
        }
        let position = parameters
            .iter()
            .filter(|candidate| !candidate.is_self)
            .position(|candidate| candidate.symbol == root)
            .expect("matched non-self requirement parameter must have an ordinal");
        let position = u32::try_from(position).map_err(|_| {
            vec![Diagnostic::error(format!(
                "public trait requirement `{}` has too many parameters for portable review evidence",
                requirement.name
            ))]
        })?;
        Ok(PackageReviewProgressSubject::Parameter(position))
    })
}

pub(crate) fn project_machine_parameter_termination(
    compilation: &CheckedCompilation,
    signature: &typed_trees::signature::StateSignature,
    declaration_path: &str,
) -> Result<PackageReviewTermination, Vec<Diagnostic>> {
    let parameters = compilation.state_signature_parameters(signature);
    if let language_semantics::TerminationGuarantee::Terminates { premises } =
        &signature.termination_guarantee
    {
        for premise in premises {
            let profile = compilation
                .domain_definitions()
                .iter()
                .find(|domain| domain.semantic_id == premise.profile)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "public static-machine parameter on `{declaration_path}` has an unknown termination profile",
                    ))]
                })?;
            if !profile.is_public {
                return Err(vec![Diagnostic::error(format!(
                    "public static-machine parameter on `{declaration_path}` exposes non-public progress profile `{}`",
                    profile.name,
                ))]);
            }
        }
    }
    project_termination_with_subject(compilation, &signature.termination_guarantee, |root| {
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.symbol == root)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "public static-machine parameter on `{declaration_path}` has a termination premise outside its parameter telescope",
                ))]
            })?;
        if parameter.is_self {
            return Ok(PackageReviewProgressSubject::Receiver);
        }
        let position = parameters
            .iter()
            .filter(|candidate| !candidate.is_self)
            .position(|candidate| candidate.symbol == root)
            .expect("matched non-self machine-parameter contract parameter must have an ordinal");
        Ok(PackageReviewProgressSubject::Parameter(
            portable_parameter_position(position)?,
        ))
    })
}

fn project_termination_with_subject(
    compilation: &CheckedCompilation,
    guarantee: &language_semantics::TerminationGuarantee,
    mut project_subject: impl FnMut(
        SymbolHandle,
    ) -> Result<PackageReviewProgressSubject, Vec<Diagnostic>>,
) -> Result<PackageReviewTermination, Vec<Diagnostic>> {
    let language_semantics::TerminationGuarantee::Terminates { premises } = guarantee else {
        return Ok(PackageReviewTermination::NoGuarantee);
    };
    let mut projected = premises
        .iter()
        .map(|premise| {
            let profile = compilation
                .domain_definitions()
                .iter()
                .find(|domain| domain.semantic_id == premise.profile)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "package review termination premise has an unknown progress-profile identity",
                    )]
                })?;
            if profile.classification
                != Some(language_semantics::DomainClassification::ProgressProfile)
            {
                return Err(vec![Diagnostic::error(
                    "package review termination premise does not name a closed progress-profile domain",
                )]);
            }
            let profile = nominal_identity(compilation, profile.symbol)?;
            let subject = project_subject(premise.subject.root)?;
            let projections = premise
                .subject
                .projections
                .iter()
                .map(|projection| nominal_identity(compilation, *projection))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PackageReviewProgressPremise {
                profile,
                subject,
                projections,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    projected.sort();
    projected.dedup();
    Ok(PackageReviewTermination::Terminates {
        premises: projected,
    })
}
