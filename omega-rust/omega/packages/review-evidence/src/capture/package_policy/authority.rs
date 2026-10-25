//! Compiler-classified danger and ceiling slack over normalized callable facts.

use crate::capture::PackageReviewInput;
use crate::capture::authority::dangerous_authority_class;
use crate::capture::semantics::declarations::nominal_identity;
use crate::record::{
    PackagePolicyCallable, PackagePolicyCallables, PackageReviewDangerousAuthority,
    PackageReviewDangerousAuthoritySlack, PackageReviewNominalIdentity,
};
use diagnostics::Diagnostic;

pub(super) fn registrations(
    compilation: &PackageReviewInput<'_>,
) -> Result<Vec<PackageReviewNominalIdentity>, Vec<Diagnostic>> {
    let mut services = Vec::new();
    for registration in &compilation.custody.build().config.service_audits {
        let mut selected = None;
        for definition in current_boundary_services(compilation) {
            let definition = definition?;
            let identity = compilation
                .typed
                .normalized_hermetic_symbol_identity(definition.symbol)
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "service audit identity: {reason}"
                    ))]
                })?;
            if identity == registration.identity {
                selected = Some(definition);
                break;
            }
        }
        let definition = selected.ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "audited service `{}` has no checked boundary identity",
                registration.name
            ))]
        })?;
        services.push(nominal_identity(compilation, definition.symbol)?);
    }
    services.sort();
    services.dedup();
    Ok(services)
}

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    callables: &PackagePolicyCallables,
) -> Result<
    (
        Vec<PackageReviewDangerousAuthority>,
        Vec<PackageReviewDangerousAuthoritySlack>,
    ),
    Vec<Diagnostic>,
> {
    let mut dangerous = Vec::new();
    let mut slack = Vec::new();
    for definition in current_boundary_services(compilation) {
        let definition = definition?;
        let Some(class) = dangerous_authority_class(compilation, definition)? else {
            continue;
        };
        let service = nominal_identity(compilation, definition.symbol)?;
        if callables
            .callables()
            .iter()
            .any(|callable| exposes(callable, &service))
        {
            dangerous.push(PackageReviewDangerousAuthority {
                class,
                service: service.clone(),
            });
        }
        for callable in callables.callables() {
            let Some(realized) = callable.checked_service_reach().realized() else {
                continue;
            };
            if callable
                .declared_service_reach()
                .is_some_and(|ceiling| ceiling.contains(&service))
                && !realized.contains(&service)
            {
                slack.push(PackageReviewDangerousAuthoritySlack {
                    class,
                    callable: callable.identity().clone(),
                    service: service.clone(),
                });
            }
        }
    }
    dangerous.sort();
    dangerous.dedup();
    slack.sort();
    slack.dedup();
    Ok((dangerous, slack))
}

/// Service IDs retain historical backing across source activations. Current
/// boundary declarations, not interner capacity, define this review's surface.
/// A missing row for a current declaration is still lost checking custody.
fn current_boundary_services<'program>(
    compilation: &'program PackageReviewInput<'_>,
) -> impl Iterator<Item = Result<&'program language_semantics::ServiceReachDefinition, Vec<Diagnostic>>>
{
    let services = &compilation.facts.service_reaches.services;
    compilation
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .map(|definition| {
            services
                .id_for_symbol(definition.symbol)
                .and_then(|identity| services.definition(identity))
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "current boundary trait `{}` has no checked service identity",
                        compilation
                            .typed
                            .symbols
                            .display_path(definition.symbol, "::"),
                    ))]
                })
        })
}

fn exposes(callable: &PackagePolicyCallable, service: &PackageReviewNominalIdentity) -> bool {
    callable
        .published_service_reach()
        .is_some_and(|row| row.contains(service))
        || callable
            .checked_service_reach()
            .realized()
            .is_some_and(|row| row.contains(service))
        || callable
            .checked_service_reach()
            .concrete()
            .is_some_and(|row| row.contains(service))
        || callable
            .unresolved_installation_reaches()
            .iter()
            .any(|row| row.upper_bound().contains(service))
        || callable
            .declared_synchronous_invocations()
            .is_some_and(|row| {
                row.iter()
                    .any(|invocation| invocation.service() == Some(service))
            })
        || callable
            .realized_synchronous_invocations()
            .iter()
            .any(|invocation| invocation.service() == Some(service))
}
