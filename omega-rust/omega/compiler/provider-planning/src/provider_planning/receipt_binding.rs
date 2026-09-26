//! Bind admitted qualification receipts to exact granted provider requirements.

pub(super) fn plan_admitted_receipt_updates(
    checked: &checked_trees::CheckedTrees,
    facts: &effects::SelectedProviderPlanFacts,
    provider_grants: &[trust_model::ResolvedSelectedProviderGrant],
) -> Result<Vec<(facts::FactHandle, u64)>, Vec<diagnostics::Diagnostic>> {
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
        if !granted_plans
            .iter()
            .any(|retained: &&trust_model::ResolvedSelectedProviderGrant| {
                retained.selected_plan == grant.selected_plan
                    && retained.selected_plan_digest == grant.selected_plan_digest
            })
        {
            granted_plans.push(grant);
        }
    }
    let mut receipt_updates = Vec::new();
    let mut receipt_diagnostics = Vec::new();
    let traits = checked.typed.traits();
    if traits.len() != checked.typed.roots.traits.count() as usize {
        receipt_diagnostics.push(diagnostics::Diagnostic::error(
            "admitted qualification receipt binding has an invalid typed trait span",
        ));
    }
    for definition in traits {
        if checked.typed.trait_machine_signatures(definition).len()
            != definition.machines.count() as usize
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
    for (handle, fact) in checked.facts.semantic.facts.iter().filter(|(_, fact)| {
        fact.evidence.origin == language_semantics::QualificationEvidenceOrigin::AdmittedReceipt
            && fact.evidence.receipt_identity == 0
    }) {
        let owners = checked
            .typed
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
        let requirement_owners = checked
            .typed
            .traits()
            .iter()
            .flat_map(|candidate_owner| {
                checked
                    .typed
                    .trait_machine_signatures(candidate_owner)
                    .iter()
                    .filter(move |requirement| {
                        requirement.symbol == fact.evidence.requirement_symbol
                    })
                    .map(move |requirement| (candidate_owner, requirement))
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
        let requirement_identity = checked
            .typed
            .normalized_trait_requirement_overload_identity(owner, requirement)
            .identity();
        let matches = granted_plans
            .iter()
            .filter(|grant| {
                grant
                    .selected_plan
                    .schema
                    .methods
                    .iter()
                    .any(|method| method.requirement_identity == requirement_identity)
            })
            .collect::<Vec<_>>();
        // A shared requirement identity — an inherited requirement carried
        // by several granted plans — still binds exactly one receipt: the
        // plan whose schema is the requirement's own boundary trait owns the
        // declared evidence. When that owner slot is not granted the
        // unscoped matches keep their fail-closed behavior rather than
        // attributing the receipt to an inheriting plan by roster order.
        let owner_matches = matches
            .iter()
            .copied()
            .filter(|grant| {
                crate::service_schema::schema_binds_exact_boundary_trait(
                    &checked.typed,
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
            [] => {}
            [grant] => {
                receipt_updates.push((handle, grant.selected_plan_report_identity));
            }
            _ => receipt_diagnostics.push(diagnostics::Diagnostic::error(format!(
                "admitted qualification requirement `{requirement_identity}` matches {} granted selected provider plans",
                matches.len()
            ))),
        }
    }
    if !receipt_diagnostics.is_empty() {
        return Err(receipt_diagnostics);
    }
    Ok(receipt_updates)
}
