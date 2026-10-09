//! Settle build-selected execution against the final typed program.
//! The resulting checked program and its sidecars remain one owned handoff.

use artifacts::compile_timings::CompileTimings;
use compilation_products::{CheckedCompilationInputs, PreparedProductSource};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use selected_dispatch::{
    ProviderCheckingInputs, ProviderCheckingRequest, SelectedConstantEvaluation,
    SelectedExecutionRequest, settle_selected_execution,
};

pub(crate) fn check_selected_execution(
    source: PreparedProductSource,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    optimization_rollback: &crate::checked::OptimizationRollback,
    timings: &mut CompileTimings,
    permit_unsettled_fused_service_fields: bool,
    settled_check_reuse: compilation_products::SettledCheckReuse,
) -> Result<
    (
        CheckedCompilationInputs,
        Option<typed_trees_to_checked_trees::RetainedCheck>,
    ),
    Vec<Diagnostic>,
> {
    let PreparedProductSource {
        mut typed,
        target_declarations: selected_target_machine_declarations,
        pending_pre_checks,
        computed_build_config,
        mut admission,
        ..
    } = source;
    admission.provider_defaults = selected_target_machine_declarations.retain_provider_defaults(
        provider_planning::ProviderPlanningContext::new(&typed, source::DependencyScope::Product),
    )?;
    let selected_build_machine_symbol = computed_build_config.selected_build_machine_symbol;
    let build_config = &computed_build_config.config;
    let selected_native_target = selected_target_profile
        .map(target::TargetProfile::native_target)
        .unwrap_or_else(target::NativeTarget::host);
    let boundary_calling_plan_realizations =
        provider_planning::calling_policy_plans::compute_boundary_calling_plans(
            &typed,
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
        &admission.provider_defaults,
        selected_target_profile.unwrap_or_else(target::TargetProfile::host),
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
    if !pending_pre_checks.is_empty() {
        let operator_selections =
            typed_trees_to_checked_trees::derive_pre_flow_operator_selections(&typed);
        SelectedConstantEvaluation::prepare(
            provider_planning::ProviderPlanningContext::new(
                &typed,
                source::DependencyScope::Product,
            ),
            &operator_selections,
            &selected_provider_plan_facts,
        )?
        .evaluate_pending(&mut typed, pending_pre_checks)?;
    }
    let root_grants = build_config.root_grant_selectors();
    let provider_request = ProviderCheckingRequest {
        native_target: selected_native_target,
        package_inputs,
        selected_build_machine: selected_build_machine_symbol,
        boundary_calling_plan_realizations: &mut providers.boundary_calling_plan_realizations,
        opaque_representation_selections,
        provider_plans: &providers.provider_plans,
        selected_provider_plan_facts,
        root_grants: &root_grants,
        authored_root_grants: &build_config.grants,
    };
    let prepared = ProviderCheckingInputs::prepare(&typed, &provider_request)?;
    // Source preparation/checking requests belong to the source coordinator,
    // not to provider settlement. Realization contributes admitted facts only.
    let preparation_request = typed_trees_to_typed_trees::PreparationRequest::settled()
        .with_opaque_property_receipts(prepared.opaque_property_receipts())
        .with_asm_authority(
            validation::AsmAuthorityAdmission::from_freestanding(
                build_config.settings.freestanding,
            )
            .with_grants(
                build_config.settings.privileged_services.port_io,
                build_config.settings.privileged_services.interrupt_table,
            ),
        )
        .with_wire_compatibility_demands(&build_config.wire_compatibility_demands);
    let checking = typed_trees_to_checked_trees::CheckingRequest::new().with_declaration_authority(
        package_inputs.map(|inputs| inputs as &dyn validation::DeclarationSelectionAuthority),
    );
    // Package review's bound pass hands back the discovery pass's judgment of
    // this occurrence. Bindings choose providers after the language judgment,
    // so settlement usually arrives at the same typed program; Psi takes the
    // judgment only when the whole input is equal. A taken judgment is not
    // counted as a settled check.
    let program = timings.record(
        artifacts::compile_timings::TYPED_TREES_TO_CHECKED_TREES,
        || {
            let started = std::time::Instant::now();
            let result =
                typed_trees_to_typed_trees::prepare_typed_trees(typed, &preparation_request)
                    .and_then(|(typed, preparation)| {
                        typed_trees_to_checked_trees::lower_typed_trees_reusing(
                            typed,
                            preparation,
                            &checking,
                            settled_check_reuse.prior.product.as_ref(),
                            settled_check_reuse.retain,
                        )
                    });
            if !result.as_ref().is_ok_and(|outcome| outcome.took_prior) {
                artifacts::compile_timings::counters::record_settled_check(started.elapsed());
            }
            result
        },
    )?;
    let typed_trees_to_checked_trees::CheckingOutcome {
        checked: program,
        retained: settled_check,
        ..
    } = program;
    // Check the common source before asking which declarations this product
    // realizes. In particular, a missing portable family contract is not a
    // missing target body, and must not be disguised as one by reach analysis.
    provider_planning::target_machines::reject_reached_inert_sibling_calls(
        &program.typed,
        selected_target_profile.unwrap_or_else(target::TargetProfile::host),
        selected_program_entry
            .as_ref()
            .map(|entry| entry.machine_symbol()),
        &selected_provider_provenance,
    )?;
    let mut checked = prepared.bind(program, provider_request)?;
    // Product target bodies remain checked input to Terminal production.
    // Only Omega realization may discard another target's implementation.
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

    Ok((
        CheckedCompilationInputs {
            settled: selected_execution_settlement,
            build: computed_build_config,
            build_admission: admission.for_target(selected_target_profile),
            providers,
            product_selection,
        },
        settled_check,
    ))
}
