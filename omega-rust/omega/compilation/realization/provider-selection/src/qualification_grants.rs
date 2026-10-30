//! Check that qualification requirements have unambiguous selected grants.
//!
//! Source facts retain the exact boundary owner and requirement. Realization
//! checks their association with granted plans without stamping a target's
//! report identity into the checked program. An absent grant remains absent;
//! downstream execution admission owns whether that authority is required.

pub fn validate_qualification_grants<'issuer>(
    issuers: impl IntoIterator<Item = &'issuer language_semantics::BoundaryQualificationIssuer>,
    facts: &effects::SelectedProviderPlanFacts,
    provider_grants: &[effects::provider_grants::ResolvedSelectedProviderGrant],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let mut granted_plans = Vec::new();
    for grant in provider_grants {
        let exact_selected_matches = facts
            .plans()
            .iter()
            .filter(|plan| grant.replays_selected_plan(plan))
            .count();
        if exact_selected_matches != 1 {
            return Err(vec![diagnostics::Diagnostic::error(format!(
                "provider grant `{}` replays against {exact_selected_matches} exact selected provider plans",
                grant.selector,
            ))]);
        }
        if !granted_plans.iter().any(
            |retained: &&effects::provider_grants::ResolvedSelectedProviderGrant| {
                retained.selected_plan == grant.selected_plan
                    && retained.selected_plan_digest == grant.selected_plan_digest
            },
        ) {
            granted_plans.push(grant);
        }
    }
    let mut receipt_diagnostics = Vec::new();
    for issuer in issuers {
        let requirement_identity = &issuer.requirement_identity;
        let matches = granted_plans
            .iter()
            .filter(|grant| {
                grant.selected_plan.schema.methods.iter().any(|method| {
                    grant
                        .selected_plan
                        .schema
                        .identity_text(method.requirement_identity)
                        == requirement_identity.as_str()
                })
            })
            .collect::<Vec<_>>();
        // A shared requirement identity — an inherited requirement carried
        // by several granted plans — still has one grant owner: the
        // plan whose schema is the requirement's own boundary trait owns the
        // declared evidence. When that owner slot is not granted the
        // unscoped matches keep their fail-closed behavior rather than
        // attributing authority to an inheriting plan by roster order.
        let owner_matches = matches
            .iter()
            .copied()
            .filter(|grant| {
                issuer.is_product_owner
                    && grant
                        .selected_plan
                        .schema
                        .identity_text(grant.selected_plan.schema.trait_name)
                        == issuer.boundary_path
                    && grant.selected_plan.schema.trait_package_identity == issuer.boundary_package
            })
            .collect::<Vec<_>>();
        let matches = if owner_matches.is_empty() {
            matches
        } else {
            owner_matches
        };
        match matches.as_slice() {
            [] | [_] => {}
            _ => receipt_diagnostics.push(diagnostics::Diagnostic::error(format!(
                "admitted qualification requirement `{requirement_identity}` matches {} granted selected provider plans",
                matches.len()
            ))),
        }
    }
    if !receipt_diagnostics.is_empty() {
        return Err(receipt_diagnostics);
    }
    Ok(())
}
