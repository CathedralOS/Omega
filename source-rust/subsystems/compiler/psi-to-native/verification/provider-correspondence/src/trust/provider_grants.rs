//! Match authored provider applications to the exact plan selected for the target.
use diagnostics::Diagnostic;
use provider_plans::ProviderPlan;
use selected_provider_plans::SelectedProviderPlanFacts;
use symbols::root_grants::{AuthoredRootGrant, RootGrantSubject};

fn application_key(plan: &ProviderPlan) -> (&str, &str) {
    (
        &plan.provider_type,
        plan.identities.text(plan.provider_application),
    )
}

struct ProviderGrant<'grant> {
    index: usize,
    authored: &'grant AuthoredRootGrant,
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

pub(super) fn match_provider_grants<'plan>(
    candidates: &[&ProviderPlan],
    selected: &'plan SelectedProviderPlanFacts,
    authored: &[AuthoredRootGrant],
) -> Result<Vec<&'plan ProviderPlan>, Diagnostic> {
    let matched = resolve_provider_grants(selected, authored)?;
    if matched.is_empty() {
        return Ok(Vec::new());
    }
    let mut candidates = candidates.to_vec();
    candidates.sort_unstable_by(|left, right| application_key(left).cmp(&application_key(right)));
    matched.into_iter().map(|grant| {
        let plan = &selected.plans()[grant.index];
        let key = application_key(plan);
        let first = candidates.partition_point(|row| application_key(row) < key);
        let last = candidates.partition_point(|row| application_key(row) <= key);
        if candidates[first..last].iter().filter(|candidate| **candidate == plan).count() != 1 {
            return Err(Diagnostic::error(format!(
                "root grant `{}` has no unique exact candidate for its selected provider application",
                grant.authored.selector,
            )));
        }
        Ok(plan)
    }).collect()
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
            authored,
            application: application.as_deref(),
        });
    }
    Ok(resolved_authored)
}

pub(super) fn commitment(plan: &ProviderPlan) -> String {
    format!(
        "provider application: {} [{}] for {} on {}",
        plan.provider_type,
        plan.identities.text(plan.provider_application),
        plan.schema
            .root_application_identity()
            .unwrap_or_else(|| plan
                .schema
                .identity_text(plan.schema.declaration.trait_name)),
        plan.target,
    )
}
