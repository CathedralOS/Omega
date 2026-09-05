//! Compiler-classified danger and ceiling slack over normalized callable facts.

use crate::capture::authority::dangerous_authority_class;
use crate::capture::semantics::declarations::nominal_identity;
use crate::record::*;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;

pub(super) fn project(
    compilation: &CheckedCompilation,
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
    for definition in compilation.facts.service_reaches.services.definitions() {
        let Some(class) = dangerous_authority_class(compilation, definition) else {
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

fn exposes(callable: &PackagePolicyCallable, service: &PackageReviewNominalIdentity) -> bool {
    callable
        .declared_service_reach()
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
