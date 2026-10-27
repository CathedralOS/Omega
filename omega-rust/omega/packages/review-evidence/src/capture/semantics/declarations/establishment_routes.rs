//! The establishment routes a progress profile's owner authored, projected
//! into the policy rows every consumer publishes.

use super::{nominal_identity, trait_requirement_identity_from_symbols};
use crate::capture::PackageReviewInput;
use crate::record::PackagePolicyServiceProgressRoute;
use diagnostics::Diagnostic;
use effects::provider_plan::ServiceProgressEstablishmentRouteKind;
use language_semantics::DomainEstablishmentRoute;
use typed_trees::domain::DomainDefinition;

/// Project the requirement routes that establish a progress profile.
///
/// A progress profile establishes only through requirement routes: an
/// exact-machine route is ineligible for an admitted boundary progress
/// profile, so it is filtered here once instead of in each consumer. The
/// route kinds stay distinct — a checked requirement and a boundary
/// requirement are different establishment vocabularies, not a shared mode.
/// `requirement_context` names the consuming policy in a rejected route.
pub(crate) fn project_service_progress_routes(
    compilation: &PackageReviewInput<'_>,
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
