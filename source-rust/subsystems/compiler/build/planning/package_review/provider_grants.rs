//! Record the Build's acceptance on the exact provider plan selected for this target.
use super::declarations::unavailable;
use diagnostics::Diagnostic;
use review_evidence::record::PackageReviewProviderGrantSelectorKind;
use symbols::root_grants::AuthoredRootGrant;

pub(super) fn by_plan(
    selected: &compiler::SelectedProviderResult,
    authored: &[AuthoredRootGrant],
) -> Result<Vec<Vec<PackageReviewProviderGrantSelectorKind>>, Vec<Diagnostic>> {
    let matched = selected
        .trust_evidence()
        .provider_grant_plans(authored)
        .map_err(|error| vec![error])?;
    if matched.len() != authored.len() {
        return Err(unavailable(
            "a root grant for a boundary requirement or an accepted machine",
        ));
    }
    let mut grants = vec![Vec::new(); selected.plans().plans().len()];
    for (index, application) in matched {
        grants[index].push(PackageReviewProviderGrantSelectorKind::ProviderApplication(
            application.map(str::to_owned),
        ));
    }
    for plan in &mut grants {
        plan.sort();
        plan.dedup();
    }
    Ok(grants)
}
