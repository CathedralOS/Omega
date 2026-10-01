//! Settle build-selected execution against the final typed program.
//! The resulting checked program and its sidecars remain one owned handoff.

use artifacts::compile_timings::CompileTimings;
use compilation_products::{BuiltTypedProgram, CheckedCompilationInputs};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use selected_dispatch::{
    ProviderCheckingInputs, ProviderCheckingRequest, SelectedConstantEvaluation,
    SelectedExecutionRequest, settle_selected_execution,
};

pub(crate) fn check_selected_execution(
    built: BuiltTypedProgram,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    optimization_rollback: &crate::checked::OptimizationRollback,
    timings: &mut CompileTimings,
    permit_unsettled_fused_service_fields: bool,
) -> Result<CheckedCompilationInputs, Vec<Diagnostic>> {
    let BuiltTypedProgram {
        mut typed,
        mut selected_target_machine_declarations,
        pending_pre_checks,
        computed_build_config,
        admission,
    } = built;
    let selected_build_machine_symbol = computed_build_config.selected_build_machine_symbol;
    let build_config = &computed_build_config.config;
    let selected_native_target = selected_target_profile
        .map(target::TargetProfile::native_target)
        .unwrap_or_else(target::NativeTarget::host);
    // The frontend typed every family's canonical body; this target's own
    // body replaces it before anything target-specific reads the program.
    selected_target_machine_declarations.select_product_target(
        &mut typed,
        selected_target_profile.unwrap_or_else(target::TargetProfile::host),
    )?;
    let boundary_calling_plan_realizations =
        provider_planning::calling_policy_plans::compute_boundary_calling_plans(
            &mut typed,
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
        &typed,
        &build_config.root_bindings,
        selected_target_profile,
        &boundary_calling_plan_realizations,
        package_inputs,
    )?;
    let settled_target_machines = selected_target_machine_declarations.settle_provider_defaults(
        provider_planning::ProviderPlanningContext::new(&typed, source::DependencyScope::Product),
    )?;
    let provider_planning::CheckedProviderSelection {
        settlement: mut providers,
        selected_provider_plan_facts,
        selected_provider_provenance,
    } = provider_planning::settle_checked_providers(
        &mut typed,
        settled_target_machines,
        selected_target_profile,
        package_inputs,
        &build_config.provider_selections,
        &build_config.accepted_component_assumption_digests(),
        boundary_calling_plan_realizations,
        &build_config.opaque_representation_selections,
    )?;
    let selected_native_target = selected_target_profile.map(target::TargetProfile::native_target);
    let provider_selection_target =
        selected_native_target.unwrap_or_else(target::NativeTarget::host);
    // `size_of<T>`/`align_of<T>` survive Psi lowering as retained typed
    // requests; only this realization's selected target can answer them. Fold
    // every query to an ordinary `u64` literal before checked-tree
    // construction so statement, scalar, and unit-plan building see the
    // settled constant rather than a call without a callee. Layout consumes
    // the typed program directly, under the same target custody as the rest
    // of settlement: the selected native target, or the host for a targetless
    // shared product.
    provider_planning::data_layout_queries::fold_data_layout_queries(
        &mut typed,
        provider_selection_target,
        opaque_representation_selections,
    )?;
    let const_evaluation = SelectedConstantEvaluation::evaluate_pending(
        &mut typed,
        &selected_provider_plan_facts,
        pending_pre_checks,
    )?;
    let root_grants = build_config.root_grant_selectors();
    let provider_request = ProviderCheckingRequest {
        native_target: selected_native_target,
        package_inputs,
        build_settings: &build_config.settings,
        selected_build_machine: selected_build_machine_symbol,
        boundary_calling_plan_realizations: &mut providers.boundary_calling_plan_realizations,
        opaque_representation_selections,
        provider_plans: &providers.provider_plans,
        selected_provider_plan_facts,
        root_grants: &root_grants,
        authored_root_grants: &build_config.grants,
    };
    let prepared = ProviderCheckingInputs::prepare(&typed, &provider_request)?;
    let checking = prepared
        .checking_request(&provider_request)
        .with_wire_compatibility_demands(&build_config.wire_compatibility_demands);
    let program = timings.record(
        artifacts::compile_timings::TYPED_TREES_TO_CHECKED_TREES,
        || {
            let started = std::time::Instant::now();
            let result = typed_trees_to_checked_trees::lower_typed_trees(typed, &checking);
            artifacts::compile_timings::counters::record_settled_check(started.elapsed());
            result
        },
    )?;
    let mut checked = prepared.bind(program, provider_request)?;
    // Every sibling was checked before realizing this target. Only the
    // selected bodies remain in its executable product.
    checked.program = std::sync::Arc::new(typed_trees_to_checked_trees::prune_target_siblings(
        std::sync::Arc::try_unwrap(checked.program).unwrap_or_else(|shared| (*shared).clone()),
    )?);
    const_evaluation.validate(
        &checked.program,
        &checked.selected_provider_plan_facts,
        package_inputs,
    )?;
    // Check all authored code before product pruning and sidecar derivation.
    let product_root_machines = build_config.checked_tree_product_roots(
        selected_target_profile,
        selected_program_entry.as_ref(),
        optimization_rollback,
    );
    let (program, product_selection) = typed_trees_to_checked_trees::select_checked_tree_product(
        checked.program,
        product_root_machines,
    )?;
    checked.program = program;
    let selected_execution_settlement = settle_selected_execution(
        checked,
        SelectedExecutionRequest {
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
        settled: selected_execution_settlement,
        build: computed_build_config,
        build_admission: admission.for_target(selected_target_profile),
        providers,
        product_selection,
    })
}
