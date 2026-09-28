//! Root-grant meanings shared within one checked trust reconstruction.
//!
//! Provider selectors resolve as a complete pass before nonprovider selectors.
//! Obligation construction retains each nonprovider meaning in authored order,
//! after its existing machine and template checks. Report reconstruction may
//! borrow that result only within the same immutable checked admission call.
//! Independent report callers reconstruct from their own inputs.

use crate::{
    NonProviderTrustGrant, ResolvedSelectedProviderGrant, resolve_non_provider_trust_grant,
};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

pub(crate) struct ResolvedRootGrants<'a> {
    pub(crate) providers: Vec<ResolvedSelectedProviderGrant>,
    pub(crate) non_providers: Vec<(&'a str, NonProviderTrustGrant)>,
}

/// Package claims require package-level review; individual accepted-machine
/// grants cannot substitute for it. The early typed checking gate reconstructs
/// its own provider meanings, independent of later sealed checked admission.
pub fn reject_package_non_provider_grants(
    typed: &TypedTrees,
    root_grants: &[String],
    provider_plans: &[effects::provider_plan::ProviderPlan],
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    let provider_grants = crate::resolve_selected_provider_grants(
        provider_plans,
        selected_provider_plans,
        root_grants,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    reject_package_grants_with_providers(typed, root_grants, &provider_grants)
}

pub(crate) fn reject_package_grants_with_providers(
    typed: &TypedTrees,
    root_grants: &[String],
    provider_grants: &[ResolvedSelectedProviderGrant],
) -> Result<(), Vec<Diagnostic>> {
    for grant in root_grants {
        if provider_grants
            .iter()
            .any(|provider_grant| provider_grant.selector == *grant)
        {
            continue;
        }
        match resolve_non_provider_trust_grant(typed, grant)
            .map_err(|diagnostic| vec![diagnostic])?
        {
            NonProviderTrustGrant::AcceptedMachine(_) => {
                return Err(vec![Diagnostic::error(format!(
                    "package-aware compilation cannot admit individual accepted machine `{grant}`; package claims require complete package-level review",
                ))]);
            }
        }
    }
    Ok(())
}
