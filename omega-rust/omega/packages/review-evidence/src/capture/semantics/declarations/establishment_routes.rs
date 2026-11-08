/*
We project the requirement routes that establish a progress profile into the
policy rows every consumer publishes.

A progress profile establishes only through requirement routes: an
exact-machine route is ineligible for an admitted boundary progress profile, so
it is filtered here once instead of in each consumer. Checked requirements and
boundary requirements stay distinct establishment vocabularies rather than a
shared mode.
*/

use super::{nominal_identity, trait_requirement_identity_from_symbols};
use crate::capture::SourceDeclarationInput;
use crate::record::PackagePolicyServiceProgressRoute;
use diagnostics::Diagnostic;
use effects::provider_plan::ServiceProgressEstablishmentRouteKind;
use language_semantics::DomainEstablishmentRoute;
use typed_trees::domain::DomainDefinition;

pub(crate) fn project_service_progress_routes(
    compilation: &SourceDeclarationInput<'_>,
    profile: &DomainDefinition,
    requirement_context: &str,
) -> Result<Vec<PackagePolicyServiceProgressRoute>, Vec<Diagnostic>> {
    let mut routes = profile
        .establishment_routes
        .iter()
        .filter(|route| !matches!(route, DomainEstablishmentRoute::ExactMachine { .. }))
        .map(|route| {
            Ok(PackagePolicyServiceProgressRoute {
                kind: match route {
                    DomainEstablishmentRoute::CheckedRequirement { .. } => {
                        ServiceProgressEstablishmentRouteKind::CheckedRequirement
                    }
                    DomainEstablishmentRoute::BoundaryRequirement { .. } => {
                        ServiceProgressEstablishmentRouteKind::BoundaryRequirement
                    }
                    DomainEstablishmentRoute::ExactMachine { .. } => {
                        unreachable!("exact-machine routes are filtered above")
                    }
                },
                requirement_owner: nominal_identity(compilation, route.source_symbol())?,
                requirement: trait_requirement_identity_from_symbols(
                    compilation,
                    route.source_symbol(),
                    route.requirement_symbol(),
                    requirement_context,
                )?,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    routes.sort();
    routes.dedup();
    Ok(routes)
}
