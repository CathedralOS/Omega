use super::PackageCompilationInputs;
use super::target_machines::SelectedTargetMachineDeclarations;
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;
use effects::provider_plan::ProviderPlan;
use provider_planning::ProviderSelection;
use provider_planning::calling_policy_plans::BoundaryCallingPlanRealization;
use provider_planning::evaluated_via_bindings::EvaluatedViaBindingTable;
use provider_planning::plans::SelectedProviderReviewProvenance;
use typed_trees::TypedTrees;

/// Final typed provider choices and their evidence, retained for checking and
/// selected execution. Candidate plans stay separate from selected plan facts.
pub(super) struct CheckedProviderSelection {
    pub(super) provider_plans: Vec<ProviderPlan>,
    pub(super) evaluated_via_bindings: EvaluatedViaBindingTable,
    pub(super) selected_provider_plan_facts: SelectedProviderPlanFacts,
    pub(super) selected_provider_provenance: Vec<SelectedProviderReviewProvenance>,
    pub(super) external_binding_rows: Vec<calling_conventions::ExternalBindingRow>,
}

/// Settle the final authored/generated target roster, validate and select its
/// provider candidates, then bind fused erasure and exact invocation evidence.
/// This must precede deferred const evaluation and typed-to-checked settlement.
pub(super) fn settle_checked_providers(
    typed: &mut TypedTrees,
    selected_target_machine_declarations: SelectedTargetMachineDeclarations,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    build_provider_selections: &[ProviderSelection],
    boundary_calling_plan_realizations: &[BoundaryCallingPlanRealization],
) -> Result<CheckedProviderSelection, Vec<Diagnostic>> {
    let target_name = selected_target_profile.map(target::TargetProfile::target_name);
    let provider_selection_target = selected_target_profile
        .map(target::TargetProfile::native_target)
        .unwrap_or_else(target::NativeTarget::host);
    let settled_target_machines =
        selected_target_machine_declarations.settle_provider_defaults(typed)?;
    let target_provider_defaults = settled_target_machines.provider_defaults;
    // PRV4 provider selection mirrors the native pipeline: candidates remain
    // separate by provider type and only the uniquely covering candidate may
    // rewrite adapter calls in the interpreter program.
    let evaluated_via_bindings = provider_planning::evaluated_via_bindings::evaluate_via_bindings(
        typed,
        selected_target_profile,
        package_inputs,
    )?;
    let derived_provider_plans =
        crate::pipeline::provider_plans::derive_satisfies_plans_with_evaluated_bindings_and_target_machine_origins(
            typed,
            target_name,
            &evaluated_via_bindings,
            &settled_target_machines.origins,
        )?;
    let provider_plans = derived_provider_plans
        .iter()
        .map(|derived| derived.plan.clone())
        .collect::<Vec<_>>();
    let diagnostics = crate::pipeline::provider_plans::validate_derived_provider_plan_candidates(
        typed,
        &evaluated_via_bindings,
        &derived_provider_plans,
    );
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let selected_provider_plans =
        crate::pipeline::provider_plans::select_provider_plans_with_provenance(
            &derived_provider_plans,
            provider_selection_target,
            &target_provider_defaults,
            build_provider_selections,
        )?;
    let mut fused_service_erasures = Vec::new();
    for selected in &selected_provider_plans {
        let composition_mode = selected
            .selected_by
            .composition_mode()
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        if composition_mode != provider_planning::CompositionMode::Fused {
            continue;
        }
        let requirement = selected.derived.provenance.schema.symbol();
        if typed
            .traits()
            .iter()
            .any(|definition| definition.is_boundary && definition.symbol == requirement)
        {
            fused_service_erasures.push(
                typed_trees::typed_trees::FusedServiceErasureAuthorization {
                    requirement,
                    provider_plan_digest: *selected.derived.plan.identity_digest().as_bytes(),
                },
            );
        }
    }
    typed
        .bind_fused_service_erasures(fused_service_erasures)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let selected_semantic_plans = selected_provider_plans
        .iter()
        .map(|selected| selected.derived.plan.clone())
        .collect::<Vec<_>>();
    crate::pipeline::provider_plans::validate_selected_synchronous_invocation_cycles(
        typed,
        &selected_semantic_plans,
    )?;
    let external_binding_rows = provider_planning::plans::extract_native_external_binding_rows(
        target_name,
        provider_selection_target,
        &selected_semantic_plans,
        boundary_calling_plan_realizations,
        typed,
    )?;
    let (selected_provider_plan_facts, selected_provider_provenance) =
        crate::pipeline::provider_plans::selected_provider_plan_facts_with_provenance(
            typed,
            &evaluated_via_bindings,
            selected_provider_plans,
        )?;
    Ok(CheckedProviderSelection {
        provider_plans,
        evaluated_via_bindings,
        selected_provider_plan_facts,
        selected_provider_provenance,
        external_binding_rows,
    })
}
