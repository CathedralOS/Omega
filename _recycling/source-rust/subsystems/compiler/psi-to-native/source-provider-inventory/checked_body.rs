use declaration_contracts::CheckedBodyCandidate;
use diagnostics::Diagnostic;
use provider_plans::ProviderPlan;

pub fn select_checked_body<'candidate, Body>(
    plan: &ProviderPlan,
    candidates: impl IntoIterator<Item = CheckedBodyCandidate<'candidate, Body>>,
) -> Result<Body, Diagnostic> {
    let [row] = plan.rows.as_slice() else {
        return Err(Diagnostic::error(format!(
            "checked boundary-operator ProviderPlan `{}` requires one realization row",
            plan.name,
        )));
    };
    let selection = CheckedBodySelection::new(plan, row)?;
    let mut matching = candidates.into_iter().filter(|candidate| {
        let declaration = candidate.declaration;
        candidate.is_product
            && declaration.is_product
            && declaration.requirement
                == plan
                    .schema
                    .identity_text(plan.schema.declaration.trait_name)
            && declaration.requirement_package == plan.schema.declaration.trait_package_identity
            && declaration.requirement == plan.identities.text(row.requirement_identity)
            && selection.matches(&declaration.callable, declaration.package, candidate.target)
            && declaration.provider == plan.provider_type
            && declaration.provider_package == plan.provider_type_package_identity
            && declaration.provider_application.as_deref() == plan.application_identity()
            && declaration.requirement_lifetime_partition == row.requirement_lifetime_partition
    });
    match (matching.next(), matching.next()) {
        (Some(candidate), None) => Ok(candidate.body),
        _ => Err(Diagnostic::error(format!(
            "selected checked boundary-operator ProviderPlan `{}` has no unique exact checked candidate",
            plan.name,
        ))),
    }
}
