//! Join each target's per-target realization against the invocation's
//! immutable checked source: provider installation, program-entry selection
//! and selected dispatch. This is the Omega-00 join — it consumes the Psi
//! facade's sealed checked product and Build rows; provider selection owns
//! target choices, never another Psi checking pass.

use crate::checked::CheckedCompilation;
use crate::checked::checking::{attribute_diagnostics, timed};
use artifacts::compile_timings::EXECUTION_SETTLEMENT;
use compilation_products::{
    CheckedCompilationInputs, CheckedCompileOutcome, CheckedProductSource, CompileFailure,
};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use selected_dispatch::{
    ProviderBindingRequest, SelectedExecutionRequest, bind_checked_providers,
    settle_selected_execution,
};
use std::sync::Arc;

pub(crate) fn settle_checked_execution(
    source: &CheckedProductSource,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    permit_unsettled_fused_service_fields: bool,
) -> Result<CheckedCompilationInputs, Vec<Diagnostic>> {
    let typed = &source.program.typed;
    let build_config = &source.computed_build_config.config;
    let selected_native_target = selected_target_profile
        .map(target::TargetProfile::native_target)
        .unwrap_or_else(target::NativeTarget::host);
    let boundary_calling_plan_realizations =
        provider_planning::calling_policy_plans::compute_boundary_calling_plans(
            typed,
            selected_native_target,
            &build_config.opaque_representation_selections,
            package_inputs,
        )?;
    let opaque_representation_selections = &build_config.opaque_representation_selections;
    let x86_scalar_fma_provider = build_config
        .settings
        .x86_deployment_claim
        .admit_for(selected_target_profile)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let selected_program_entry = provider_planning::program_entry::select_compiler_program_entry(
        typed,
        &build_config.root_bindings,
        selected_target_profile,
        &boundary_calling_plan_realizations,
        package_inputs,
        |program, definition| validation::data_requires_establishment(program, definition),
    )?;
    let settled_target_machines = provider_planning::target_machines::settle_provider_defaults(
        provider_planning::ProviderPlanningContext::new(typed, source::DependencyScope::Product),
        &source.admission.provider_defaults,
        selected_target_profile.unwrap_or_else(target::TargetProfile::host),
    );
    let provider_planning::CheckedProviderSelection {
        settlement: providers,
        selected_provider_plan_facts,
        selected_provider_provenance,
    } = provider_planning::settle_checked_providers(
        typed,
        settled_target_machines,
        selected_target_profile,
        package_inputs,
        &build_config.provider_selections,
        &build_config.accepted_component_assumption_digests(),
        boundary_calling_plan_realizations,
        &build_config.opaque_representation_selections,
        &source.inferred_synchronous_invocations,
    )?;
    provider_planning::target_machines::reject_reached_inert_sibling_calls(
        typed,
        selected_target_profile.unwrap_or_else(target::TargetProfile::host),
        selected_program_entry
            .as_ref()
            .map(|entry| entry.machine_symbol()),
        &selected_provider_provenance,
    )?;
    let root_grants = build_config.root_grant_selectors();
    let checked = bind_checked_providers(
        Arc::clone(&source.program),
        source.accepted_template_classifications.clone(),
        ProviderBindingRequest {
            package_inputs,
            boundary_calling_plan_realizations: &providers.boundary_calling_plan_realizations,
            provider_plans: &providers.provider_plans,
            selected_provider_plan_facts,
            root_grants: &root_grants,
            authored_root_grants: &build_config.grants,
        },
    )?;
    let settled = settle_selected_execution(
        checked,
        SelectedExecutionRequest {
            calling_plans: &providers.boundary_calling_plan_realizations,
            selected_program_entry,
            selected_target_profile,
            package_inputs,
            x86_scalar_fma_provider,
            permit_unsettled_fused_service_fields,
            selected_provider_provenance,
            opaque_representation_selections,
        },
    )?;
    Ok(CheckedCompilationInputs {
        settled,
        build: Arc::clone(&source.computed_build_config),
        build_admission: source.admission.clone().for_target(selected_target_profile),
        providers,
        product_selection: source.product_selection.clone(),
    })
}

/// Bind one target to shared checked source, never a resumable frontend.
pub(crate) fn settle_product_target(
    source: &CheckedProductSource,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    permit_unsettled_fused_service_fields: bool,
) -> CheckedCompileOutcome {
    let independent_components = source.computed_build_config.independent_components();
    let mut timings = source.timings.clone();
    let result = timed(&mut timings, EXECUTION_SETTLEMENT, |_| {
        settle_checked_execution(
            source,
            selected_target_profile,
            package_inputs,
            permit_unsettled_fused_service_fields,
        )
    });
    let result = result.map(|execution| {
        CheckedCompilation::from_settled_execution(
            execution,
            std::sync::Arc::clone(&source.sources),
            timings,
        )
        .with_settled_checks(source.retained_checks.clone())
    });
    let result = result.map_err(|mut diagnostics| {
        if let Some(sources) = source.program.typed.symbols.source_map().as_deref() {
            attribute_diagnostics(&mut diagnostics, sources, package_inputs);
        }
        diagnostics
    });
    CheckedCompileOutcome {
        prepared_source: None,
        result: result.map_err(CompileFailure::from),
        independent_components,
    }
}

