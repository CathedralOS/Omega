//! Join selected providers to immutable checked Psi.
//! Source preparation, opaque property admission and template capture happen
//! once at the source owner, before realization selects a provider.

use checked_trees::CheckedTrees as CheckedProgram;
use diagnostics::Diagnostic;
use std::sync::Arc;

pub struct CheckedProviderProgram {
    pub program: Arc<CheckedProgram>,
    pub selected_provider_plan_facts: effects::SelectedProviderPlanFacts,
    pub selected_provider_grants: Vec<trust_model::ResolvedAuthoredSelectedProviderGrant>,
    pub callback_placements:
        Vec<provider_planning::calling_policy_plans::SelectedCallbackPlacement>,
    pub accepted_template_classifications: trust_model::AcceptedTemplateClassifications,
}

/// Realization choices joined to a completed source judgment.
pub struct ProviderBindingRequest<'a> {
    pub package_inputs: Option<&'a package_compilation::PackageCompilationInputs>,
    pub boundary_calling_plan_realizations:
        &'a [provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    pub provider_plans: &'a [effects::provider_plan::ProviderPlan],
    pub selected_provider_plan_facts: effects::SelectedProviderPlanFacts,
    pub root_grants: &'a [String],
    pub authored_root_grants: &'a [trust_model::AuthoredRootGrant],
}

/// Validate target grants and callback placements without rewriting the shared
/// program. Package authority admits whole packages, not standalone machine grants.
pub fn bind_checked_providers(
    program: Arc<CheckedProgram>,
    accepted_template_classifications: trust_model::AcceptedTemplateClassifications,
    settlement: ProviderBindingRequest<'_>,
) -> Result<CheckedProviderProgram, Vec<Diagnostic>> {
    if settlement.package_inputs.is_some() {
        trust_model::reject_package_non_provider_grants(
            &program.typed,
            settlement.root_grants,
            settlement.provider_plans,
            &settlement.selected_provider_plan_facts,
        )?;
    }
    let binding = provider_planning::bind_selected_provider_plan_facts(
        &program,
        &program.facts,
        settlement.provider_plans,
        settlement.selected_provider_plan_facts,
        settlement.root_grants,
        settlement.authored_root_grants,
    )?;
    let (selected_provider_plan_facts, selected_provider_grants) = binding.into_parts();
    let callback_placements =
        provider_planning::calling_policy_plans::validate_nominal_callback_placement_bindings(
            &program.typed,
            &selected_provider_plan_facts,
            &program.facts,
            settlement.boundary_calling_plan_realizations,
        )?;
    Ok(CheckedProviderProgram {
        program,
        selected_provider_plan_facts,
        selected_provider_grants,
        callback_placements,
        accepted_template_classifications,
    })
}
