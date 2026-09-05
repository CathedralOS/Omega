//! Exact progress-profile subjects and owner-authored establishment routes.

use super::*;
use crate::capture::semantics::declarations::trait_requirement_identity_from_symbols;
use crate::record::{PackagePolicyServiceProgressPremise, PackagePolicyServiceProgressRoute};
use omega_effects::provider_plan::{ServiceProgressEstablishmentRouteKind, ServiceProgressSubject};

pub(super) fn project(
    compilation: &CheckedCompilation,
    guarantee: &psi_language_semantics::TerminationGuarantee,
    parameters: &[psi_typed_trees::signature::StateParameter],
) -> Result<Vec<PackagePolicyServiceProgressPremise>, Vec<Diagnostic>> {
    let psi_language_semantics::TerminationGuarantee::Terminates { premises } = guarantee else {
        return Ok(Vec::new());
    };
    let mut projected = Vec::new();
    for premise in premises {
        let profiles = compilation
            .domain_definitions()
            .iter()
            .filter(|domain| domain.semantic_id == premise.profile)
            .collect::<Vec<_>>();
        let [profile] = profiles.as_slice() else {
            return Err(rejected(
                "progress premise has no unique exact profile declaration",
            ));
        };
        if profile.classification
            != Some(psi_language_semantics::DomainClassification::ProgressProfile)
        {
            return Err(rejected(
                "progress premise names another domain classification",
            ));
        }
        let subjects = parameters
            .iter()
            .filter(|parameter| parameter.symbol == premise.subject.root)
            .collect::<Vec<_>>();
        let [subject] = subjects.as_slice() else {
            return Err(rejected(
                "progress subject is not one exact requirement parameter",
            ));
        };
        let subject = if subject.is_self {
            ServiceProgressSubject::ProviderReceiver
        } else {
            let ordinal = parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .position(|parameter| parameter.symbol == subject.symbol)
                .ok_or_else(|| rejected("progress subject has no source parameter ordinal"))?;
            ServiceProgressSubject::Parameter(ordinal)
        };
        let mut establishment_routes = profile
            .establishment_routes
            .iter()
            .map(|route| {
                Ok(PackagePolicyServiceProgressRoute {
                    kind: match route {
                        psi_language_semantics::DomainEstablishmentRoute::CheckedRequirement {
                            ..
                        } => ServiceProgressEstablishmentRouteKind::CheckedRequirement,
                        psi_language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                            ..
                        } => ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                    },
                    requirement_owner: nominal_identity(compilation, route.source_symbol())?,
                    requirement: trait_requirement_identity_from_symbols(
                        compilation,
                        route.source_symbol(),
                        route.requirement_symbol(),
                        "selected provider progress establishment",
                    )?,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        establishment_routes.sort();
        establishment_routes.dedup();
        projected.push(PackagePolicyServiceProgressPremise {
            profile: nominal_identity(compilation, profile.symbol)?,
            subject,
            subject_projections: premise
                .subject
                .projections
                .iter()
                .map(|symbol| nominal_identity(compilation, *symbol))
                .collect::<Result<Vec<_>, _>>()?,
            establishment_routes,
        });
    }
    projected.sort();
    Ok(projected)
}
