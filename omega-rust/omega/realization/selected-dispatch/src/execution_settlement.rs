//! Close execution dispatch, accepted host bindings and task activation for
//! one selected realization. The compiler invokes this operation; it does not
//! reconstruct its associations or decide which host contracts are admissible.

use crate::provider_checking::CheckedProviderProgram;
use checked_trees::CheckedTrees as CheckedProgram;
use diagnostics::Diagnostic;
use std::sync::Arc;

/// Checked semantics after selected execution has been settled in the exact
/// realization-owned dispatch order. This surface owns the now-closed review
/// provenance alongside every checked-phase sidecar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExecution {
    pub program: Arc<CheckedProgram>,
    pub selected_provider_plan_facts: effects::SelectedProviderPlanFacts,
    pub selected_provider_grants: Vec<trust_model::ResolvedAuthoredSelectedProviderGrant>,
    pub callback_placements: Vec<backend_plan::BoundNominalCallbackPlacement>,
    pub accepted_template_classifications: trust_model::AcceptedTemplateClassifications,
    pub contract_entailment_stand_downs: Vec<validation::ContractEntailmentStandDown>,
    pub selected_provider_provenance: Vec<provider_planning::SelectedProviderReviewProvenance>,
    pub resolved_semantic_bindings: Vec<crate::ResolvedAcceptedSemanticBinding>,
    pub component_progress: Option<effects::ComponentProgressManifest>,
    pub task_activations: task_plans::TaskActivationPlanSet,
}

pub struct SelectedExecutionRequest<'a> {
    pub exact_component_progress_root:
        Option<provider_planning::component_progress::ExactComponentProgressRoot<'a>>,
    pub provider_selection_target: target::NativeTarget,
    pub selected_target_profile: Option<target::TargetProfile>,
    pub selected_provider_provenance: Vec<provider_planning::SelectedProviderReviewProvenance>,
    pub opaque_representation_selections:
        &'a [representation_planning::OpaqueRepresentationSelection],
    pub accepted_console_binding: Option<&'a package_compilation::AcceptedSemanticBinding>,
    pub accepted_process_exit_binding: Option<&'a package_compilation::AcceptedSemanticBinding>,
    pub accepted_filesystem_binding: Option<&'a package_compilation::AcceptedSemanticBinding>,
    pub accepted_time_host_binding: Option<&'a package_compilation::AcceptedSemanticBinding>,
    pub expected_entry_role: Option<package_compilation::AcceptedSemanticBindingRole>,
    pub accepted_entry_binding: Option<&'a package_compilation::AcceptedSemanticBinding>,
}

/// Consume a complete checked surface and settle every selected execution
/// rewrite before publishing the final compiler-facing surface.
pub fn settle_selected_execution(
    mut checked: CheckedProviderProgram,
    mut settlement: SelectedExecutionRequest<'_>,
) -> Result<SelectedExecution, Vec<Diagnostic>> {
    let component_progress =
        provider_planning::component_progress::build_selected_component_progress_manifest(
            &checked.program,
            &checked.selected_provider_plan_facts,
            settlement.exact_component_progress_root,
            None,
        )?;
    checked.program = crate::settle_selected_execution_dispatch(
        checked.program,
        &checked.selected_provider_plan_facts,
    )?;
    let resolved_exit_bindings = crate::retain_selected_compiler_intrinsic_review_identities(
        &checked.program,
        &checked.selected_provider_plan_facts,
        &mut settlement.selected_provider_provenance,
        settlement
            .selected_target_profile
            .map(target::TargetProfile::target_name),
        settlement.accepted_console_binding,
        settlement.accepted_process_exit_binding,
    )?;
    let resolved_filesystem_binding = settlement
        .accepted_filesystem_binding
        .map(|binding| crate::resolve_accepted_service_binding(&checked.program, binding))
        .transpose()
        .map_err(|diagnostic| vec![diagnostic])?;
    let resolved_time_host_binding = settlement
        .accepted_time_host_binding
        .map(|binding| crate::resolve_accepted_service_binding(&checked.program, binding))
        .transpose()
        .map_err(|diagnostic| vec![diagnostic])?;
    let resolved_entry_binding = match (
        settlement.accepted_entry_binding,
        settlement.expected_entry_role,
    ) {
        (None, _) => None,
        (Some(binding), Some(role)) if binding.role() == role => Some(
            crate::resolve_accepted_service_binding(&checked.program, binding)
                .map_err(|diagnostic| vec![diagnostic])?,
        ),
        (Some(binding), _) => {
            return Err(vec![Diagnostic::error(format!(
                "accepted semantic binding {:?} was not consumed by the selected target's program-entry contract",
                binding.role(),
            ))]);
        }
    };
    let task_activations = provider_planning::task_plans::elaborate_task_activation_plans(
        &checked.program,
        &checked.selected_provider_plan_facts,
        settlement.provider_selection_target,
        settlement.opaque_representation_selections,
    )?;

    Ok(SelectedExecution {
        program: checked.program,
        selected_provider_plan_facts: checked.selected_provider_plan_facts,
        selected_provider_grants: checked.selected_provider_grants,
        callback_placements: checked.callback_placements,
        accepted_template_classifications: checked.accepted_template_classifications,
        contract_entailment_stand_downs: checked.contract_entailment_stand_downs,
        selected_provider_provenance: settlement.selected_provider_provenance,
        resolved_semantic_bindings: resolved_exit_bindings
            .into_iter()
            .chain(resolved_filesystem_binding)
            .chain(resolved_time_host_binding)
            .chain(resolved_entry_binding)
            .collect(),
        component_progress,
        task_activations,
    })
}
