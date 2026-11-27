//! Match authored provider applications to the exact plan selected for the target.
use diagnostics::Diagnostic;
use build_providers::plans::ProviderPlan;
use build_providers::selections::SelectedProviderPlanFacts;
use symbols::root_grants::{AuthoredRootGrant, RootGrantSubject};

fn application_key(plan: &ProviderPlan) -> (&str, &str) {
    (
        &plan.provider_type,
        plan.identities.text(plan.provider_application),
    )
}

struct ProviderGrant<'grant> {
    index: usize,
    application: Option<&'grant str>,
}

/// Resolve a Build containing only provider grants against its selected target plans.
pub fn selected_provider_grants<'grant>(
    selected: &SelectedProviderPlanFacts,
    authored: &'grant [AuthoredRootGrant],
) -> Result<Vec<(usize, Option<&'grant str>)>, Diagnostic> {
    let matched = resolve_provider_grants(selected, authored)?;
    if matched.len() != authored.len() {
        return Err(Diagnostic::error(
            "a grant for a boundary requirement or an accepted machine requires its own admission check",
        ));
    }
    Ok(matched
        .into_iter()
        .map(|grant| (grant.index, grant.application))
        .collect())
}

fn resolve_provider_grants<'grant>(
    selected: &SelectedProviderPlanFacts,
    authored_grants: &'grant [AuthoredRootGrant],
) -> Result<Vec<ProviderGrant<'grant>>, Diagnostic> {
    if authored_grants.is_empty() {
        return Ok(Vec::new());
    }
    let mut selected_rows = selected.plans().iter().enumerate().collect::<Vec<_>>();
    selected_rows.sort_unstable_by(|(_, left), (_, right)| {
        application_key(left).cmp(&application_key(right))
    });
    let mut resolved_authored = Vec::new();
    for authored in authored_grants {
        let RootGrantSubject::Provider {
            declaration,
            application,
        } = &authored.subject
        else {
            continue;
        };
        let key = (declaration.as_str(), application.as_deref().unwrap_or(""));
        let first = selected_rows.partition_point(|(_, plan)| application_key(plan) < key);
        let last = selected_rows.partition_point(|(_, plan)| application_key(plan) <= key);
        let [(index, plan)] = &selected_rows[first..last] else {
            return Err(Diagnostic::error(format!(
                "root grant `{}` has {} selected plans for its exact provider declaration and application",
                authored.selector,
                last - first,
            )));
        };
        if plan.provider_type_package_identity != authored.subject_package {
            return Err(Diagnostic::error(format!(
                "root grant `{}` changes its provider package identity",
                authored.selector
            ))
            .with_source_span(authored.source_span));
        }
        resolved_authored.push(ProviderGrant {
            index: *index,
            application: application.as_deref(),
        });
    }
    Ok(resolved_authored)
}
