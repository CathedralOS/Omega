//! Join selected rows to Psi's checked declaration/application inventory.
//! The caller supplies one declaration roster or one exact application's body
//! roster. This owner never reconstructs source conformance or specialization.

use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use language_semantics::BoundaryBodySelection;

pub struct CheckedBodyCandidate<'candidate, Body> {
    pub body: Body,
    pub declaration: &'candidate BoundaryBodySelection,
    pub target: Option<&'candidate str>,
    pub is_product: bool,
}

/// Select exactly one retained declaration or one checked application body.
/// A generic declaration is eligible policy, never a substitute for the
/// distinct closed-body roster a concrete application must supply.
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
    let ProviderBinding::CheckedAdapter {
        machine_identity,
        machine_package_identity,
    } = &row.binding
    else {
        return Err(Diagnostic::error(format!(
            "ProviderPlan `{}` is not a checked-body selection",
            plan.name
        )));
    };
    if *machine_package_identity != plan.origin_package_identity {
        return Err(Diagnostic::error(
            "selected checked body belongs to a different package",
        ));
    }
    let mut matching = candidates.into_iter().filter(|candidate| {
        let declaration = candidate.declaration;
        candidate.is_product
            && declaration.is_product
            && declaration.requirement == plan.schema.identity_text(plan.schema.trait_name)
            && declaration.requirement_package == plan.schema.trait_package_identity
            && declaration.requirement == plan.identities.text(row.requirement_identity)
            && declaration.callable == plan.identity_text(*machine_identity)
            && declaration.package == *machine_package_identity
            && declaration.provider == plan.provider_type
            && declaration.provider_package == plan.provider_type_package_identity
            && declaration.provider_application.as_deref() == plan.application_identity()
            && declaration.requirement_lifetime_partition == row.requirement_lifetime_partition
            && candidate.target.is_none_or(|target| target == plan.target)
    });
    match (matching.next(), matching.next()) {
        (Some(candidate), None) => Ok(candidate.body),
        _ => Err(Diagnostic::error(format!(
            "selected checked boundary-operator ProviderPlan `{}` has no unique exact checked candidate",
            plan.name,
        ))),
    }
}
