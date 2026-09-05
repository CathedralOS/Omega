use super::semantics::declarations::nominal_identity;
use crate::capture::source::{
    ProjectedDangerousAuthorityRow, ProjectedDangerousAuthoritySlackRow, ProjectedReviewRow,
};
use crate::record::{
    CheckedPackageCallableReview, PackageReviewDangerousAuthority,
    PackageReviewDangerousAuthorityClass, PackageReviewDangerousAuthoritySlack,
    PackageReviewNominalIdentity, PackageReviewSynchronousInvocation,
};
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use std::collections::BTreeSet;

pub(crate) fn project_dangerous_authorities(
    compilation: &CheckedCompilation,
    callables: &[ProjectedReviewRow<CheckedPackageCallableReview>],
) -> Result<Vec<ProjectedDangerousAuthorityRow>, Vec<Diagnostic>> {
    let mut exposed_services = BTreeSet::new();
    for callable in callables.iter().map(|projected| &projected.row) {
        if let Some(services) = callable.declared_service_reach() {
            exposed_services.extend(services.iter().cloned());
        }
        if let Some(services) = callable.checked_service_reach().realized() {
            exposed_services.extend(services.iter().cloned());
        }
        if let Some(services) = callable.checked_service_reach().concrete() {
            exposed_services.extend(services.iter().cloned());
        }
        for reach in callable.unresolved_installation_reaches() {
            exposed_services.extend(reach.upper_bound().iter().cloned());
        }
        if let Some(invocations) = callable.declared_synchronous_invocations() {
            exposed_services.extend(
                invocations
                    .iter()
                    .filter_map(PackageReviewSynchronousInvocation::service)
                    .cloned(),
            );
        }
        exposed_services.extend(
            callable
                .realized_synchronous_invocations()
                .iter()
                .filter_map(PackageReviewSynchronousInvocation::service)
                .cloned(),
        );
    }

    let mut rows = Vec::new();
    for definition in compilation.facts.service_reaches.services.definitions() {
        let service = nominal_identity(compilation, definition.symbol)?;
        if !exposed_services.contains(&service) {
            continue;
        }
        let Some(class) = dangerous_authority_class(compilation, definition) else {
            continue;
        };
        let exposures = callables
            .iter()
            .filter(|callable| callable_exposes_service(&callable.row, &service))
            .map(|callable| callable.declaration)
            .collect();
        rows.push(ProjectedDangerousAuthorityRow {
            row: PackageReviewDangerousAuthority { class, service },
            declaration: definition.symbol,
            exposures,
        });
    }
    rows.sort_by(|left, right| left.row.cmp(&right.row));
    rows.dedup_by(|left, right| {
        left.row == right.row
            && left.declaration == right.declaration
            && left.exposures == right.exposures
    });
    Ok(rows)
}

pub(crate) fn project_dangerous_authority_slack(
    compilation: &CheckedCompilation,
    callables: &[ProjectedReviewRow<CheckedPackageCallableReview>],
) -> Result<Vec<ProjectedDangerousAuthoritySlackRow>, Vec<Diagnostic>> {
    let mut catalog = Vec::new();
    for definition in compilation.facts.service_reaches.services.definitions() {
        let Some(class) = dangerous_authority_class(compilation, definition) else {
            continue;
        };
        catalog.push((
            nominal_identity(compilation, definition.symbol)?,
            class,
            definition.symbol,
        ));
    }
    catalog.sort_by(|left, right| left.0.cmp(&right.0));

    let mut rows = Vec::new();
    for callable in callables {
        let Some(realized) = callable.row.checked_service_reach().realized() else {
            continue;
        };
        let Some(declared) = callable.row.declared_service_reach() else {
            continue;
        };
        for service in declared {
            if realized.contains(service) {
                continue;
            }
            let Ok(index) = catalog.binary_search_by(|entry| entry.0.cmp(service)) else {
                continue;
            };
            let (_, class, authority_declaration) = &catalog[index];
            rows.push(ProjectedDangerousAuthoritySlackRow {
                row: PackageReviewDangerousAuthoritySlack {
                    class: *class,
                    callable: callable.row.identity.clone(),
                    service: service.clone(),
                },
                authority_declaration: *authority_declaration,
                callable_declaration: callable.declaration,
            });
        }
    }
    rows.sort_by(|left, right| left.row.cmp(&right.row));
    rows.dedup_by(|left, right| {
        left.row == right.row
            && left.authority_declaration == right.authority_declaration
            && left.callable_declaration == right.callable_declaration
    });
    Ok(rows)
}

pub(crate) fn callable_exposes_service(
    callable: &CheckedPackageCallableReview,
    service: &PackageReviewNominalIdentity,
) -> bool {
    callable
        .declared_service_reach()
        .is_some_and(|services| services.contains(service))
        || callable
            .checked_service_reach()
            .realized()
            .is_some_and(|services| services.contains(service))
        || callable
            .checked_service_reach()
            .concrete()
            .is_some_and(|services| services.contains(service))
        || callable
            .unresolved_installation_reaches()
            .iter()
            .any(|reach| reach.upper_bound().contains(service))
        || callable
            .declared_synchronous_invocations()
            .is_some_and(|invocations| {
                invocations
                    .iter()
                    .any(|invocation| invocation.service() == Some(service))
            })
        || callable
            .realized_synchronous_invocations()
            .iter()
            .any(|invocation| invocation.service() == Some(service))
}

/// Transitional compiler-owned risk classification for exact semantic bindings.
///
/// Physical terminal authority is classified independently from exact exercised
/// mechanisms during realization. Package review retains only the broad legacy
/// summaries backed by compiler-resolved Console and FilesystemHost bindings;
/// service names, source paths, and toolchain provenance cannot manufacture a
/// physical authority class.
pub(crate) fn dangerous_authority_class(
    compilation: &CheckedCompilation,
    definition: &language_semantics::ServiceReachDefinition,
) -> Option<PackageReviewDangerousAuthorityClass> {
    if compilation
        .resolved_semantic_binding(
            package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        )
        .is_some_and(|binding| binding.declaration_symbol() == definition.symbol)
    {
        return Some(PackageReviewDangerousAuthorityClass::Process);
    }
    if compilation
        .resolved_semantic_binding(
            package_compilation::AcceptedSemanticBindingRole::FilesystemHostService,
        )
        .is_some_and(|binding| binding.declaration_symbol() == definition.symbol)
    {
        return Some(PackageReviewDangerousAuthorityClass::Filesystem);
    }

    None
}
