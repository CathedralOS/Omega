//! Source-free joins between checked external declarations and selected plans.

use crate::CompilerIntrinsicExecutionIdentity;
use crate::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use terminal_psi::{ExternalProviderConformance, TerminalIdentityTable};

/// Select one numeric satisfaction edge using nominal requirement identity,
/// the exact callable, package owners, binding kind and deployment target.
/// Absence is not authorization: a consumer requiring numeric execution must
/// reject it. Host and placement authority are independent of this catalog.
/// A declaration review also nominates its exact realization; a portable
/// consumer without that coordinate must have one candidate in its admitted
/// catalog. Never choose a family head or a sibling merely by display name.
pub fn selected_numeric_provider<'a>(
    providers: &'a [ExternalProviderConformance],
    identities: &TerminalIdentityTable,
    requirement: &str,
    realization: Option<&str>,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
) -> Result<Option<&'a ExternalProviderConformance>, &'static str> {
    let ProviderBinding::CompilerIntrinsic { machine } = &row.binding else {
        return Err("numeric provider does not select an external compiler intrinsic");
    };
    let mut candidates = providers.iter().filter(|candidate| {
        identities.text(candidate.requirement.identity()) == Some(requirement)
            && realization.is_none_or(|realization| {
                identities.text(candidate.realization.machine.identity()) == Some(realization)
            })
            && matches_external_selection(candidate, identities, plan, row, machine)
    });
    let candidate = candidates.next();
    if candidates.next().is_some() {
        return Err(
            "selected numeric provider does not rejoin one canonical external satisfaction",
        );
    }
    if let Some(candidate) = candidate {
        candidate.validate(identities)?;
    }
    Ok(candidate)
}

pub fn matches_external_selection(
    candidate: &ExternalProviderConformance,
    identities: &TerminalIdentityTable,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    machine: &str,
) -> bool {
    candidate.numeric_intrinsic.is_some()
        && identities.text(candidate.requirement_identity)
            == Some(plan.identities.text(row.requirement_identity))
        && candidate.requirement_package == plan.schema.trait_package_identity
        && candidate
            .target
            .as_ref()
            .is_none_or(|target| *target == plan.target)
        && candidate.binding == language_semantics::ExternalBindingIdentity::CompilerIntrinsic
        && identities.text(candidate.realization.callable) == Some(machine)
        && candidate.realization_package == plan.origin_package_identity
        && match &candidate.provider {
            Some(provider) => {
                identities.text(provider.identity) == Some(plan.provider_type.as_str())
                    && provider.package == plan.provider_type_package_identity
            }
            None => plan.provider_type.is_empty() && plan.provider_type_package_identity.is_none(),
        }
}

/// Portable meaning cannot replace selected-provider custody or disagree with
/// a retained execution. Numeric execution requires its checked meaning.
pub fn reconcile_numeric_meaning(
    meaning: Option<language_semantics::numeric_intrinsics::NumericIntrinsic>,
    selected: Option<CompilerIntrinsicExecutionIdentity>,
) -> Result<(), &'static str> {
    use CompilerIntrinsicExecutionIdentity as Execution;
    match (
        meaning,
        selected,
        selected.is_some_and(Execution::is_numeric),
    ) {
        (Some(meaning), Some(execution), _)
            if meaning.signature().is_some() && Execution::from(meaning) == execution =>
        {
            Ok(())
        }
        (None, _, false) => Ok(()),
        _ => Err("numeric meaning disagrees with selected provider custody"),
    }
}
