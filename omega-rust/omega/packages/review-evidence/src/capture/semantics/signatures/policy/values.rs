//! Complete behavioral values shared by public declaration signatures.

use super::rejected;
use crate::capture::PackageReviewInput;
use crate::capture::semantics::declarations::{
    nominal_identity, progress_profile, project_service_progress_routes,
};
use crate::capture::semantics::facts::exactly_one;
use crate::record::{
    PackagePolicyCrashGuard, PackagePolicyCrashRoute, PackagePolicyProgressPremise,
    PackagePolicyTermination, PackageReviewCrashRoute, PackageReviewCrashRouteGuard,
    PackageReviewTermination,
};
use diagnostics::Diagnostic;

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
    compilation: &PackageReviewInput<'_>,
    guarantee: &language_semantics::TerminationGuarantee,
    projected: PackageReviewTermination,
) -> Result<PackagePolicyTermination, Vec<Diagnostic>> {
    match (guarantee, projected) {
        (
            language_semantics::TerminationGuarantee::NoGuarantee,
            PackageReviewTermination::NoGuarantee,
        ) => Ok(PackagePolicyTermination::NoGuarantee),
        (
            language_semantics::TerminationGuarantee::Terminates { premises },
            PackageReviewTermination::Terminates {
                premises: projected,
            },
        ) => {
            let mut profiles = Vec::new();
            for premise in premises {
                let profile =
                    progress_profile(compilation, premise.profile, "public policy termination")?;
                let identity = nominal_identity(compilation, profile.symbol)?;
                if !profiles.iter().any(|(prior, _)| *prior == identity) {
                    profiles.push((identity, profile));
                }
            }
            let mut projected = projected
                .into_iter()
                .map(|premise| {
                    let (_, profile) = exactly_one(
                        profiles
                            .iter()
                            .filter(|(identity, _)| *identity == premise.profile),
                        "public policy termination",
                        "projected progress profile",
                    )?;
                    let establishment_routes = project_service_progress_routes(
                        compilation,
                        profile,
                        "public policy progress establishment",
                    )?;
                    Ok(PackagePolicyProgressPremise {
                        profile: premise.profile,
                        subject: premise.subject,
                        projections: premise.projections,
                        establishment_routes,
                    })
                })
                .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
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
