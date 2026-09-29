//! Close execution dispatch, accepted host bindings, task activation and entry
//! establishment for one selected realization. Target/entry custody and x86
//! FMA associations leave with the program they were settled against. The
//! compiler neither projects service roles nor completes this result afterward.

use crate::provider_checking::CheckedProviderProgram;
use checked_trees::CheckedTrees as CheckedProgram;
use diagnostics::Diagnostic;
use package_compilation::AcceptedSemanticBindingRole;
use std::sync::Arc;

/// Checked semantics after selected execution has been settled in the exact
/// realization-owned dispatch order. This surface owns the now-closed review
/// provenance alongside every checked-phase sidecar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExecution {
    pub program: Arc<CheckedProgram>,
    pub selected_target_profile: Option<target::TargetProfile>,
    pub selected_program_entry:
        Option<provider_planning::program_entry::SelectedCompilerProgramEntry>,
    /// Exact admitted deployment provider; absence retains the SSE2 baseline.
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    /// Exact source demands joined to that provider, never fabricated by admission.
    pub x86_scalar_fma_plan_associations:
        Vec<provider_planning::x86_fma_plan_association::CheckedX86ScalarFmaPlanAssociation>,
    pub selected_provider_plan_facts: effects::SelectedProviderPlanFacts,
    /// Authored Build grants resolved to the exact selected plans.
    pub selected_provider_grants: Vec<trust_model::ResolvedAuthoredSelectedProviderGrant>,
    /// Target-owned recipes joined to checked nominal uses. Engines consume
    /// these plans rather than reconstructing placement from source semantics.
    pub callback_placements: Vec<backend_plan::BoundNominalCallbackPlacement>,
    pub accepted_template_classifications: trust_model::AcceptedTemplateClassifications,
    /// Claims deliberately left unjudged by ordinary validation. Package review
    /// rejects them until independent later-discharge evidence exists.
    pub contract_entailment_stand_downs: Vec<validation::ContractEntailmentStandDown>,
    pub selected_provider_provenance: Vec<provider_planning::SelectedProviderReviewProvenance>,
    pub resolved_semantic_bindings: Vec<crate::ResolvedAcceptedSemanticBinding>,
    pub component_progress: Option<effects::ComponentProgressManifest>,
    pub task_activations: task_plans::TaskActivationPlanSet,
}

/// Whole owner inputs, not independently projected service bindings or a
/// second asserted component root. This remains a checked-tree realization
/// boundary; it does not claim the intended Terminal-only provider handoff.
pub struct SelectedExecutionRequest<'a> {
    pub selected_program_entry:
        Option<provider_planning::program_entry::SelectedCompilerProgramEntry>,
    pub selected_target_profile: Option<target::TargetProfile>,
    pub package_inputs: Option<&'a package_compilation::PackageCompilationInputs>,
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    pub permit_unsettled_fused_service_fields: bool,
    pub selected_provider_provenance: Vec<provider_planning::SelectedProviderReviewProvenance>,
    pub opaque_representation_selections:
        &'a [representation_planning::OpaqueRepresentationSelection],
}

/// Consume a complete checked surface and settle every selected execution
/// rewrite before publishing the final compiler-facing surface.
pub fn settle_selected_execution(
    mut checked: CheckedProviderProgram,
    mut settlement: SelectedExecutionRequest<'_>,
) -> Result<SelectedExecution, Vec<Diagnostic>> {
    // The same selected entry supplies component progress and receives fused
    // field establishments below. Callers cannot nominate an unrelated root
    // beside it or pair a native ABI with a different deployment profile.
    let exact_component_progress_root = settlement.selected_program_entry.as_ref().map(|entry| {
        let source = entry.source_signature();
        provider_planning::component_progress::ExactComponentProgressRoot::new(
            source.machine_symbol(),
            source.normalized_callable_identity(),
        )
    });
    let component_progress =
        provider_planning::component_progress::build_selected_component_progress_manifest(
            &checked.program,
            &checked.selected_provider_plan_facts,
            exact_component_progress_root,
            None,
        )?;
    checked.program = crate::settle_selected_execution_dispatch(
        checked.program,
        &checked.selected_provider_plan_facts,
        &settlement.selected_provider_provenance,
    )?;
    let accepted_binding = |role| {
        settlement
            .package_inputs
            .and_then(|inputs| inputs.accepted_semantic_binding(role))
    };
    let resolved_exit_bindings = crate::retain_selected_compiler_intrinsic_review_identities(
        &checked.program,
        &checked.selected_provider_plan_facts,
        &mut settlement.selected_provider_provenance,
        settlement
            .selected_target_profile
            .map(target::TargetProfile::target_name),
        accepted_binding(AcceptedSemanticBindingRole::ConsoleExitProcessI32),
        accepted_binding(AcceptedSemanticBindingRole::ProcessExitExitProcessI32),
    )?;
    let resolved_filesystem_binding =
        accepted_binding(AcceptedSemanticBindingRole::FilesystemHostService)
            .map(|binding| crate::resolve_accepted_service_binding(&checked.program, binding))
            .transpose()
            .map_err(|diagnostic| vec![diagnostic])?;
    let resolved_time_host_binding = accepted_binding(AcceptedSemanticBindingRole::TimeHostService)
        .map(|binding| crate::resolve_accepted_service_binding(&checked.program, binding))
        .transpose()
        .map_err(|diagnostic| vec![diagnostic])?;
    // Package lookup is keyed by the exact role of this target's entry slot;
    // the coordinator no longer supplies a separately asserted expected role.
    let resolved_entry_binding = settlement
        .selected_target_profile
        .map(|profile| profile.program_entry_slot())
        .and_then(|slot| slot.physical_contract_package)
        .map(package_compilation::program_entry_semantic_binding_role)
        .and_then(accepted_binding)
        .map(|binding| crate::resolve_accepted_service_binding(&checked.program, binding))
        .transpose()
        .map_err(|diagnostic| vec![diagnostic])?;
    let task_activations = provider_planning::task_plans::elaborate_task_activation_plans(
        &checked.program,
        &checked.selected_provider_plan_facts,
        settlement
            .selected_target_profile
            .map(target::TargetProfile::native_target)
            .unwrap_or_else(target::NativeTarget::host),
        settlement.opaque_representation_selections,
    )?;

    let x86_scalar_fma_plan_associations =
        provider_planning::x86_fma_plan_association::bind_checked_x86_scalar_fma_plan_associations(
            &checked.program,
            &checked.selected_provider_plan_facts,
            &settlement.selected_provider_provenance,
            settlement.x86_scalar_fma_provider,
            settlement.selected_target_profile,
        )?;
    if let Some(entry) = settlement.selected_program_entry.as_mut() {
        let establishments = crate::derive_fused_program_entry_establishments(
            &checked.program,
            entry.source_signature(),
            &settlement.selected_provider_provenance,
            settlement.permit_unsettled_fused_service_fields,
        )?;
        entry
            .bind_fused_service_establishments(establishments)
            .map_err(|message| vec![Diagnostic::error(message)])?;
    }

    Ok(SelectedExecution {
        program: checked.program,
        selected_target_profile: settlement.selected_target_profile,
        selected_program_entry: settlement.selected_program_entry,
        x86_scalar_fma_provider: settlement.x86_scalar_fma_provider,
        x86_scalar_fma_plan_associations,
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
