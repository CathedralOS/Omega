//! Match authored provider applications to the exact plan selected for the target.
use diagnostics::Diagnostic;
use provider_plans::ProviderPlan;
use selected_provider_plans::SelectedProviderPlanFacts;
use symbols::root_grants::{AuthoredRootGrant, RootGrantSubject};

pub(super) fn selected_provider_grants<'plan>(
    candidates: &[&ProviderPlan],
    selected: &'plan SelectedProviderPlanFacts,
    authored_grants: &[AuthoredRootGrant],
) -> Result<Vec<&'plan ProviderPlan>, Diagnostic> {
    fn application_key(plan: &ProviderPlan) -> (&str, &str) {
        (
            &plan.provider_type,
            plan.identities.text(plan.provider_application),
        )
    }
    let mut candidate_rows = candidates.to_vec();
    candidate_rows
        .sort_unstable_by(|left, right| application_key(left).cmp(&application_key(right)));
    let mut selected_rows = selected.plans().iter().collect::<Vec<_>>();
    selected_rows
        .sort_unstable_by(|left, right| application_key(left).cmp(&application_key(right)));
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
        let first = selected_rows.partition_point(|plan| application_key(plan) < key);
        let last = selected_rows.partition_point(|plan| application_key(plan) <= key);
        let [plan] = &selected_rows[first..last] else {
            return Err(Diagnostic::error(format!(
                "root grant `{}` has {} selected plans for its exact provider declaration and application",
                authored.selector,
                last - first,
            )));
        };
        let first = candidate_rows.partition_point(|plan| application_key(plan) < key);
        let last = candidate_rows.partition_point(|plan| application_key(plan) <= key);
        if candidate_rows[first..last]
            .iter()
            .filter(|candidate| *candidate == plan)
            .count()
            != 1
        {
            return Err(Diagnostic::error(format!(
                "root grant `{}` has no unique exact candidate for its selected provider application",
                authored.selector,
            )));
        }
        if plan.provider_type_package_identity != authored.subject_package {
            return Err(Diagnostic::error(format!(
                "root grant `{}` changes its provider package identity",
                authored.selector
            ))
            .with_source_span(authored.source_span));
        }
        resolved_authored.push(*plan);
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
