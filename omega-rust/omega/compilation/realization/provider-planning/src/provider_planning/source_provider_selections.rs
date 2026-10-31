//! Source selection adapters for early and reference consumers.
//! Complete Product boundary policy belongs to provider-selection; these
//! adapters only locate source uses or borrow an exact checked body payload.

use crate::provider_planning::trait_name_text;
use checked_trees::typed_trees::TypedTrees;
use effects::provider_plan::{ProviderBinding, ProviderPlan};

/// The selected ProviderPlan serving one checked use of a boundary operator
/// or receiver-free top-level requirement, with its index in `plans`.
/// Checked uses carry no selection: each target joins its own selected plans
/// here by requirement identity (slot and declaring package) in the caller's
/// dependency purpose, after
/// `bind_selected_provider_plan_facts` rejected ambiguous or malformed plans.
/// A use inside a pruned target-sibling body has no provider, and a checked
/// adapter never serves a generic template's use (its concrete clones are
/// served instead).
pub fn selected_use_plan<'a>(
    typed: &checked_trees::typed_trees::TypedTrees,
    scope: source::DependencyScope,
    plans: &'a [ProviderPlan],
    requirement: symbols::SymbolHandle,
    origin: checked_trees::CheckedValueOrigin,
) -> Option<(usize, &'a ProviderPlan)> {
    if let checked_trees::CheckedValueOrigin::StateStatement { machine_symbol, .. } = origin
        && !typed.machine(machine_symbol).is_some()
    {
        return None;
    }
    let (index, plan) = selected_requirement_plan(
        crate::ProviderPlanningContext::new(typed, scope),
        plans,
        requirement,
    )?;
    let is_operator = typed.operator_definition_by_symbol(requirement).is_some();
    if is_operator
        && matches!(
            plan.rows.as_slice(),
            [row] if matches!(row.binding, ProviderBinding::CheckedAdapter { .. })
        )
        && use_site_is_generic_template(typed, origin)
    {
        return None;
    }
    Some((index, plan))
}

/// The one selected ProviderPlan whose schema binds `requirement` — a boundary
/// operator, trait-method or top-level requirement — in its exact
/// declaring package and dependency purpose, with its index in `plans`.
pub fn selected_requirement_plan<'a>(
    context: crate::ProviderPlanningContext<'_>,
    plans: &'a [ProviderPlan],
    requirement: symbols::SymbolHandle,
) -> Option<(usize, &'a ProviderPlan)> {
    if !context.contains(requirement) {
        return None;
    }
    let typed = context.typed;
    if let Some((owner, method)) = typed.trait_machine_signature(requirement) {
        let identity = typed.trait_requirement_callable_identity_text(owner, method);
        if identity.is_empty() {
            return None;
        }
        let mut matching = plans.iter().enumerate().filter(|(_, plan)| {
            crate::service_schema::schema_binds_exact_boundary_trait(context, &plan.schema, owner)
                && plan
                    .schema
                    .declaration
                    .methods
                    .iter()
                    .filter(|candidate| {
                        plan.schema.identity_text(candidate.requirement_identity)
                            == identity.as_str()
                    })
                    .count()
                    == 1
        });
        let selected = matching.next()?;
        return matching.next().is_none().then_some(selected);
    }
    let mut matching: Vec<(usize, &'a ProviderPlan)> = match typed
        .operator_definition_by_symbol(requirement)
    {
        Some(operator) => {
            let slot = checked_trees::typed_trees::operator::boundary_operator_requirement_identity(
                typed, operator,
            );
            let package = typed.symbols.symbol_package_identity(operator.symbol);
            plans
                .iter()
                .enumerate()
                .filter(|(_, plan)| {
                    trait_name_text(&plan.schema) == slot
                        && plan.schema.declaration.trait_package_identity == package
                })
                .collect()
        }
        None => {
            let requirement = crate::IntrinsicRequirement::by_symbol(typed, requirement)?;
            if requirement.kind != crate::IntrinsicRequirementKind::TopLevelRequirement {
                return None;
            }
            plans
                .iter()
                .enumerate()
                .filter(|(_, plan)| requirement.schema_binds(context, &plan.schema))
                .collect()
        }
    };
    match matching.len() {
        1 => matching.pop(),
        _ => None,
    }
}

fn use_site_is_generic_template(
    typed: &checked_trees::typed_trees::TypedTrees,
    origin: checked_trees::CheckedValueOrigin,
) -> bool {
    let checked_trees::CheckedValueOrigin::StateStatement { machine_symbol, .. } = origin else {
        return false;
    };
    typed
        .machine(machine_symbol)
        .is_some_and(|machine| !typed.machine_type_parameters(machine).is_empty())
}

pub(crate) fn provider_type_package_identity(
    typed: &TypedTrees,
    machine: &checked_trees::typed_trees::machine::Machine,
) -> Option<semantic_vocabulary::PackageKeyIdentity> {
    provider_type_symbol(typed, machine)
        .and_then(|symbol| typed.symbols.symbol_package_identity(symbol))
}

pub(crate) fn provider_type_symbol(
    typed: &TypedTrees,
    machine: &checked_trees::typed_trees::machine::Machine,
) -> Option<symbols::SymbolHandle> {
    let attached_data = machine.attached_data.as_ref()?;
    let mut owners = typed.data_definitions().iter().filter(|definition| {
        definition.symbol == machine.attached_data_symbol && definition.name == *attached_data
    });
    let owner = owners.next()?;
    owners.next().is_none().then_some(owner.symbol)
}

/// Project the exact demand's already checked bodies into source-free choice.
/// Neither admission nor checked dispatch may reconstruct this roster from
/// typed specialization or satisfaction rows.
pub fn selected_checked_operator_body<'checked>(
    facts: &'checked checked_trees::CheckFacts,
    application: &checked_trees::CheckedBoundaryOperatorApplicationDemand,
    plan: &ProviderPlan,
) -> Result<&'checked checked_trees::CheckedBoundaryBodyCandidate, diagnostics::Diagnostic> {
    let candidates = facts
        .operators
        .project_body_candidates(application, |body| body)
        .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))?;
    provider_selection::select_checked_body(plan, candidates)
}
