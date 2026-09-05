//! Complete behavioral values shared by public declaration signatures.

use super::rejected;
use crate::capture::semantics::declarations::{
    nominal_identity, trait_requirement_identity_from_symbols,
};
use crate::capture::semantics::facts::exactly_one;
use crate::record::*;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

pub(crate) fn crashes(
    routes: Vec<PackageReviewCrashRoute>,
) -> Result<Vec<PackagePolicyCrashRoute>, Vec<Diagnostic>> {
    routes
        .into_iter()
        .map(|route| {
            Ok(PackagePolicyCrashRoute {
                cause: route.cause,
                alternative_guards: route
                    .alternative_guards
                    .into_iter()
                    .map(|guard| match guard {
                        PackageReviewCrashRouteGuard::Truth => Ok(PackagePolicyCrashGuard::Truth),
                        PackageReviewCrashRouteGuard::Expression(expression) => {
                            Ok(PackagePolicyCrashGuard::Expression(expression))
                        }
                        PackageReviewCrashRouteGuard::Predicate(_) => Err(rejected(
                            "a policy crash route retains an unowned predicate",
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            })
        })
        .collect()
}

/// The legacy projection has already checked the public profile and relative
/// subject. Rejoin that profile to its exact source declaration before adding
/// the full establishment routes absent from the old review record.
pub(crate) fn termination(
    compilation: &CheckedCompilation,
    guarantee: &psi_language_semantics::TerminationGuarantee,
    projected: PackageReviewTermination,
) -> Result<PackagePolicyTermination, Vec<Diagnostic>> {
    match (guarantee, projected) {
        (
            psi_language_semantics::TerminationGuarantee::NoGuarantee,
            PackageReviewTermination::NoGuarantee,
        ) => Ok(PackagePolicyTermination::NoGuarantee),
        (
            psi_language_semantics::TerminationGuarantee::Terminates { premises },
            PackageReviewTermination::Terminates {
                premises: projected,
            },
        ) => {
            let mut profiles = Vec::new();
            for premise in premises {
                let profile = exactly_one(
                    compilation
                        .domain_definitions()
                        .iter()
                        .filter(|domain| domain.semantic_id == premise.profile),
                    "public policy termination",
                    "progress profile",
                )?;
                let identity = nominal_identity(compilation, profile.symbol)?;
                if !profiles.iter().any(|(prior, _)| *prior == identity) {
                    profiles.push((identity, profile));
                }
            }
            let mut projected = projected.into_iter().map(|premise| {
                let (_, profile) = exactly_one(profiles.iter().filter(|(identity, _)| *identity == premise.profile), "public policy termination", "projected progress profile")?;
                let mut establishment_routes = profile.establishment_routes.iter().map(|route| {
                    Ok(PackagePolicyServiceProgressRoute {
                        kind: match route {
                            psi_language_semantics::DomainEstablishmentRoute::CheckedRequirement { .. } => omega_effects::provider_plan::ServiceProgressEstablishmentRouteKind::CheckedRequirement,
                            psi_language_semantics::DomainEstablishmentRoute::BoundaryRequirement { .. } => omega_effects::provider_plan::ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                        },
                        requirement_owner: nominal_identity(compilation, route.source_symbol())?,
                        requirement: trait_requirement_identity_from_symbols(compilation, route.source_symbol(), route.requirement_symbol(), "public policy progress establishment")?,
                    })
                }).collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
                establishment_routes.sort();
                establishment_routes.dedup();
                Ok(PackagePolicyProgressPremise {
                    profile: premise.profile,
                    subject: premise.subject,
                    projections: premise.projections,
                    establishment_routes,
                })
            }).collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
            projected.sort();
            projected.dedup();
            Ok(PackagePolicyTermination::Terminates {
                premises: projected,
            })
        }
        _ => Err(rejected(
            "termination projection differs from its checked guarantee",
        )),
    }
}
