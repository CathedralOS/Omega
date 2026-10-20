//! Check that qualification requirements have unambiguous selected grants.
//!
//! Source facts retain the exact boundary owner and requirement. Realization
//! checks their association with granted plans without stamping a target's
//! report identity into the checked program. An absent grant remains absent;
//! downstream execution admission owns whether that authority is required.

pub(super) fn validate(
    typed: &checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
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
    let traits = typed.traits();
    if traits.len() != typed.roots.traits.count() as usize {
        receipt_diagnostics.push(diagnostics::Diagnostic::error(
            "admitted qualification receipt binding has an invalid typed trait span",
        ));
    }
    for definition in traits {
        if typed.trait_machine_signatures(definition).len() != definition.machines.count() as usize
        {
            receipt_diagnostics.push(diagnostics::Diagnostic::error(format!(
                "admitted qualification receipt binding has an invalid typed signature span for trait {:?}",
                definition.symbol,
            )));
        }
    }
    if !receipt_diagnostics.is_empty() {
        return Err(receipt_diagnostics);
    }
    for (_, fact) in check_facts.semantic.facts.iter().filter(|(_, fact)| {
        fact.evidence.origin == language_semantics::QualificationEvidenceOrigin::AdmittedReceipt
    }) {
        let owners = typed
            .traits()
            .iter()
            .filter(|definition| definition.symbol == fact.evidence.source_symbol)
            .collect::<Vec<_>>();
        let owner = match owners.as_slice() {
            [owner] if owner.is_boundary => *owner,
            [owner] => {
                receipt_diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "admitted qualification evidence source {:?} names non-boundary trait `{}`",
                    fact.evidence.source_symbol, owner.name,
                )));
                continue;
            }
            _ => {
                receipt_diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "admitted qualification evidence source {:?} resolves to {} exact typed boundary requirement owners",
                    fact.evidence.source_symbol,
                    owners.len(),
                )));
                continue;
            }
        };
        let requirement_owners = typed
            .traits()
            .iter()
            .filter_map(|candidate_owner| {
                typed
                    .trait_machine_signature_by_symbol(
                        candidate_owner,
                        fact.evidence.requirement_symbol,
                    )
                    .map(|requirement| (candidate_owner, requirement))
            })
            .collect::<Vec<_>>();
        let requirement = match requirement_owners.as_slice() {
            [(requirement_owner, requirement)] if requirement_owner.symbol == owner.symbol => {
                *requirement
            }
            [(requirement_owner, _)] => {
                receipt_diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "admitted qualification evidence requirement {:?} belongs to exact trait {:?}, not owner {:?}",
                    fact.evidence.requirement_symbol,
                    requirement_owner.symbol,
                    owner.symbol,
                )));
                continue;
            }
            _ => {
                receipt_diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "admitted qualification evidence requirement {:?} resolves to {} exact typed signatures",
                    fact.evidence.requirement_symbol,
                    requirement_owners.len(),
                )));
                continue;
            }
        };
        let requirement_identity =
            typed.trait_requirement_callable_identity_text(owner, requirement);
        let matches = granted_plans
            .iter()
            .filter(|grant| {
                grant.selected_plan.schema.methods.iter().any(|method| {
                    grant
                        .selected_plan
                        .schema
                        .identity_text(method.requirement_identity)
                        == requirement_identity
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
                crate::service_schema::schema_binds_exact_boundary_trait(
                    crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product),
                    &grant.selected_plan.schema,
                    owner,
                )
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
