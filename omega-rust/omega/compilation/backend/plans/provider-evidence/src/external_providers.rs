//! Source-free joins between checked external declarations and selected plans.

use crate::CompilerIntrinsicExecutionIdentity;
use crate::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use terminal_psi::{ExternalProviderConformance, TerminalIdentityTable};

/// Declaration facts borrowed from either the source view (early Build) or
/// canonical Psi. Display coordinates are consistency checks; exact overload
/// and package identity still establish the requirement association.
pub struct RequirementPlanIdentity<'a> {
    pub overload: &'a str,
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub schema: &'a str,
    pub method: &'a str,
    pub owner: &'a str,
}

impl RequirementPlanIdentity<'_> {
    pub fn matches(
        &self,
        plan: &ProviderPlan,
        method: &crate::provider_plan::ServiceMethod,
        row: &ProviderPlanRow,
    ) -> bool {
        plan.schema.trait_package_identity == self.package
            && method.requirement_owner_package_identity == self.package
            && plan.schema.identity_text(method.requirement_identity) == self.overload
            && plan.identities.text(row.requirement_identity) == self.overload
            && plan.schema.row_binds_method(row, method, &plan.identities)
            && plan.schema.identity_text(plan.schema.trait_name) == self.schema
            && method.name == self.method
            && method.requirement_owner == self.owner
    }
}

/// Numeric provider plans contain one method and one realization. This is
/// independent of the operation's numeric meaning and its execution authority.
pub fn matches_requirement_schema(
    candidate: &ExternalProviderConformance,
    identities: &TerminalIdentityTable,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
) -> bool {
    let Some(schema) = &candidate.requirement_schema else {
        return false;
    };
    let ([method], [selected_row]) = (plan.schema.methods.as_slice(), plan.rows.as_slice()) else {
        return false;
    };
    let (Some(overload), Some(schema), Some(name), Some(owner)) = (
        identities.text(candidate.requirement_identity),
        identities.text(schema.identity),
        identities.text(schema.method),
        identities.text(schema.owner),
    ) else {
        return false;
    };
    selected_row == row
        && RequirementPlanIdentity {
            overload,
            package: candidate.requirement_package,
            schema,
            method: name,
            owner,
        }
        .matches(plan, method, row)
}

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
) -> Result<Option<&'a ExternalProviderConformance>, String> {
    let ProviderBinding::CompilerIntrinsic { machine } = &row.binding else {
        return Err("numeric provider does not select an external compiler intrinsic".into());
    };
    let mut candidates = providers.iter().filter(|candidate| {
        identities.text(candidate.requirement.identity()) == Some(requirement)
            && realization.is_none_or(|realization| {
                identities.text(candidate.realization.machine.identity()) == Some(realization)
            })
            && matches_external_selection(candidate, identities, plan, row, machine)
    });
    let candidate = candidates.next();
    if let (Some(first), Some(second)) = (candidate, candidates.next()) {
        return Err(format!(
            "selected numeric provider does not rejoin one canonical external satisfaction: \
             plan `{}`, target `{}`, requirement `{requirement}`, selected realization {realization:?}; \
             first realization {:?}, semantic declaration {:?}, target {:?}, provider {:?}, record {first:?}; \
             second realization {:?}, semantic declaration {:?}, target {:?}, provider {:?}, record {second:?}",
            plan.name,
            plan.target,
            identities.text(first.realization.machine.identity()),
            identities.text(first.realization.semantic_declaration),
            first.target,
            first
                .provider
                .as_ref()
                .and_then(|provider| identities.text(provider.declaration.identity())),
            identities.text(second.realization.machine.identity()),
            identities.text(second.realization.semantic_declaration),
            second.target,
            second
                .provider
                .as_ref()
                .and_then(|provider| identities.text(provider.declaration.identity())),
        ));
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
        && matches_requirement_schema(candidate, identities, plan, row)
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
