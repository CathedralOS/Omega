//! Settle build-selected execution against the final typed program.
//! The resulting checked program and its sidecars remain one owned handoff.

use super::build_continuation::BuiltCheckedProgram;
use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use selected_dispatch::{
    ProviderCheckingInputs, ProviderCheckingRequest, SelectedConstantEvaluation, SelectedExecution,
    SelectedExecutionRequest, settle_selected_execution,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedExecution {
    pub(super) settled: SelectedExecution,
    pub(super) const_evaluation: SelectedConstantEvaluation,
    /// One immutable evaluated Build shared by target products and review clones.
    /// Authored choices stay distinct from the realized settlements below.
    pub(super) build: std::sync::Arc<build_evaluation::ComputedBuildConfig>,
    /// The validated authored `builder.application` declaration; its name
    /// supplies the `.app` basename and inner executable leaf at publication
    /// and its artifact-only intent narrows the admitted route.
    pub(super) application: Option<build_declarations::ApplicationDeclaration>,
    pub(super) selected_target_profile: Option<target::TargetProfile>,
    pub(super) selected_native_target: Option<target::NativeTarget>,
    pub(super) x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    pub(super) x86_scalar_fma_plan_associations:
        Vec<provider_planning::x86_fma_plan_association::CheckedX86ScalarFmaPlanAssociation>,
    pub(super) selected_program_entry:
        Option<provider_planning::program_entry::SelectedCompilerProgramEntry>,
    pub(super) selected_build_machine_identity: Option<String>,
    pub(super) boundary_calling_plan_realizations:
        Vec<provider_planning::calling_policy_plans::BoundaryCallingPlanRealization>,
    pub(super) optimization_selection_identity: optimization_core::OptimizationSelectionIdentity,
    /// Identity-bearing checked-tree product selection evidence, present only
    /// when the effective build selection named `CheckedTreeProductPruning`.
    pub(super) product_selection: Option<typed_trees_to_checked_trees::CheckedTreeProductSelection>,
    pub(super) provider_plans: Vec<effects::provider_plan::ProviderPlan>,
    pub(super) evaluated_via_bindings:
        provider_planning::evaluated_via_bindings::EvaluatedViaBindingTable,
    pub(super) external_binding_rows: Vec<calling_conventions::ExternalBindingRow>,
    pub(super) root_grants: Vec<String>,
    /// The normalized restricted build-host requests the admitted build
    /// activation asked of the host before it executed.
    pub(super) restricted_build_requests: Vec<build_evaluation::RestrictedBuildRequest>,
    /// The component descriptions this package compilation attached and
    /// provider settlement verified, in package-identity order. Retained as
    /// custody so consumers replaying the selected-provider join can re-verify
    /// the same admission instead of rediscovering unattached inputs.
    pub(super) independent_component_descriptions:
        Vec<package_compilation::IndependentComponentDescription>,
    /// The component-assumption digests the authoritative build machine
    /// accepted through `builder.accept_component_assumption`. Retained
    /// beside the admitted descriptions so a replay re-verifies them under
    /// the same acceptance set the original settlement applied.
    pub(super) accepted_component_assumptions: std::collections::BTreeSet<[u8; 32]>,
}

pub(crate) fn check_selected_execution(
    built: BuiltCheckedProgram,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    optimization_rollback: &crate::checked::OptimizationRollback,
    timings: &mut CompileTimings,
    permit_unsettled_fused_service_fields: bool,
) -> Result<CheckedExecution, Vec<Diagnostic>> {
    let BuiltCheckedProgram {
        mut typed,
        mut selected_target_machine_declarations,
        pending_pre_checks,
        computed_build_config,
        restricted_build_requests,
        application,
        selected_build_machine_identity,
    } = built;
    let selected_build_machine_symbol = computed_build_config.selected_build_machine_symbol;
    let build_config = &computed_build_config.config;
    let selected_native_target = selected_target_profile
        .map(target::TargetProfile::native_target)
        .unwrap_or_else(target::NativeTarget::host);
    // The frontend typed every family's canonical body; this target's own
    // body replaces it before anything target-specific reads the program.
    selected_target_machine_declarations
        .select_product_target(&mut typed, selected_native_target)?;
    let mut boundary_calling_plan_realizations =
        provider_planning::calling_policy_plans::compute_boundary_calling_plans(
            &mut typed,
            selected_native_target,
            &build_config.opaque_representation_selections,
            package_inputs,
        )?;
    let opaque_representation_selections = &build_config.opaque_representation_selections;
    let x86_scalar_fma_provider = build_config
        .x86_deployment_claim
        .admit_for(selected_target_profile)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let optimization_selection_identity = build_config.optimizations.identity();
    // Compatibility demands are semantic checks, not report-mode behavior.
    // Validate them on the canonical checked route even when no auxiliary
    // artifact writer is requested by the outer compiler coordinator.
    build_time_evaluation::wire_compatibility::validate_wire_protocol(
        &typed,
        &build_config.wire_compatibility_demands,
    )?;
    // A semantic-only checked compilation has no selected target and therefore
    // no storage root. Authored bindings remain available in the evaluated
    // build configuration, but only an exact target selection may activate one
    // for interpreter or production execution.
    let program_entry_binding_role = selected_target_profile
        .map(|profile| profile.program_entry_slot())
        .and_then(|slot| slot.physical_contract_package)
        .map(build_evaluation::program_entry_semantic_binding_role);
    let mut selected_program_entry =
        provider_planning::program_entry::select_compiler_program_entry(
            &typed,
            &build_config.root_bindings,
            selected_target_profile,
            &boundary_calling_plan_realizations,
            package_inputs.and_then(|inputs| {
                program_entry_binding_role.and_then(|role| inputs.accepted_semantic_binding(role))
            }),
        )?;
    let accepted_component_assumptions: std::collections::BTreeSet<[u8; 32]> = build_config
        .accepted_component_assumptions
        .iter()
        .map(|acceptance| acceptance.digest)
        .collect();
    let settled_target_machines =
        selected_target_machine_declarations.settle_provider_defaults(&mut typed)?;
    let provider_planning::CheckedProviderSelection {
        provider_plans,
        evaluated_via_bindings,
        selected_provider_plan_facts,
        selected_provider_provenance,
        external_binding_rows,
    } = provider_planning::settle_checked_providers(
        &mut typed,
        settled_target_machines,
        selected_target_profile,
        package_inputs,
        &build_config.provider_selections,
        &accepted_component_assumptions,
        &boundary_calling_plan_realizations,
        &build_config.opaque_representation_selections,
    )?;
    let selected_native_target = selected_target_profile.map(target::TargetProfile::native_target);
    let provider_selection_target =
        selected_native_target.unwrap_or_else(target::NativeTarget::host);
    let const_evaluation = SelectedConstantEvaluation::evaluate_pending(
        &mut typed,
        &selected_provider_plan_facts,
        pending_pre_checks,
    )?;
    let root_grants = build_config
        .grants
        .iter()
        .map(|grant| grant.selector.clone())
        .collect::<Vec<_>>();
    if package_inputs.is_some() {
        trust_model::reject_package_non_provider_grants(
            &typed,
            &root_grants,
            &provider_plans,
            &selected_provider_plan_facts,
        )?;
    }
    let provider_request = ProviderCheckingRequest {
        native_target: selected_native_target,
        package_inputs,
        selected_build_machine: selected_build_machine_symbol,
        boundary_calling_plan_realizations: &mut boundary_calling_plan_realizations,
        opaque_representation_selections,
        provider_plans: &provider_plans,
        selected_provider_plan_facts,
        root_grants: &root_grants,
        authored_root_grants: &build_config.grants,
    };
    let prepared = ProviderCheckingInputs::prepare(&typed, &provider_request)?;
    let checking = prepared
        .checking_request()
        .with_asm_authority(
            typed_trees_to_checked_trees::AsmAuthorityAdmission::from_freestanding(
                build_config.freestanding,
            )
            .with_grants(
                build_config.privileged_services.port_io,
                build_config.privileged_services.interrupt_table,
            ),
        )
        .with_declaration_authority(
            package_inputs.map(|inputs| inputs as &dyn validation::DeclarationSelectionAuthority),
        );
    let program = timings.record(
        artifacts::compile_timings::TYPED_TREES_TO_CHECKED_TREES,
        || {
            let started = std::time::Instant::now();
            let result = typed_trees_to_checked_trees::lower_typed_trees(typed, &checking);
            super::record_settled_check(started.elapsed());
            result
        },
    )?;
    let mut checked = prepared.bind(program, provider_request)?;
    // `size_of<T>`/`align_of<T>` survive Psi as retained typed requests; only
    // this realization's selected target can answer them. Fold every query to
    // an ordinary `u64` literal before the const replay validator, checked
    // optimization, and codegen see the program. The same target custody the
    // rest of this settlement uses applies: the native target the build
    // selected, or the host for a targetless shared product.
    provider_planning::data_layout_queries::fold_data_layout_queries(
        std::sync::Arc::make_mut(&mut checked.program),
        provider_selection_target,
        opaque_representation_selections,
    )?;
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
    // The CheckedTrees optimization phase runs only after all authored code
    // has been checked and before selected-execution sidecars derive from the
    // product. Its exact roots come from this compilation's own product
    // selection: the validated program-entry binding for a selected target,
    // or every authored root binding for a targetless shared product. The
    // release rollback settles before the phase so the effective selection is
    // what executes.
    let effective_optimizations = optimization_rollback.settle(&build_config.optimizations);
    let product_root_machines = effective_optimizations
        .effective()
        .contains(optimization_core::Optimization::CheckedTreeProductPruning)
        .then(|| {
            let roots: std::collections::HashSet<symbols::SymbolHandle> =
                match selected_target_profile {
                    Some(_) => selected_program_entry
                        .iter()
                        .map(|entry| entry.source_signature().machine_symbol())
                        .collect(),
                    None => build_config
                        .root_bindings
                        .iter()
                        .map(|binding| binding.implementation_symbol)
                        .collect(),
                };
            roots.into_iter().collect()
        });
    let (program, product_selection) = typed_trees_to_checked_trees::select_checked_tree_product(
        checked.program,
        product_root_machines,
    )?;
    checked.program = program;
    let exact_component_progress_root = selected_program_entry.as_ref().map(|entry| {
        let source = entry.source_signature();
        provider_planning::component_progress::ExactComponentProgressRoot::new(
            source.machine_symbol(),
            source.normalized_callable_identity(),
        )
    });
    let selected_execution_settlement = settle_selected_execution(
        checked,
        SelectedExecutionRequest {
            exact_component_progress_root,
            expected_entry_role: program_entry_binding_role,
            provider_selection_target,
            selected_target_profile,
            selected_provider_provenance,
            opaque_representation_selections,
            accepted_console_binding: package_inputs.and_then(|inputs| {
                inputs.accepted_semantic_binding(
                    package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32,
                )
            }),
            accepted_process_exit_binding: package_inputs.and_then(|inputs| {
                inputs.accepted_semantic_binding(
                    package_compilation::AcceptedSemanticBindingRole::ProcessExitExitProcessI32,
                )
            }),
            accepted_filesystem_binding: package_inputs.and_then(|inputs| {
                inputs.accepted_semantic_binding(
                    package_compilation::AcceptedSemanticBindingRole::FilesystemHostService,
                )
            }),
            accepted_time_host_binding: package_inputs.and_then(|inputs| {
                inputs.accepted_semantic_binding(
                    package_compilation::AcceptedSemanticBindingRole::TimeHostService,
                )
            }),
            accepted_entry_binding: package_inputs.and_then(|inputs| {
                program_entry_binding_role.and_then(|role| inputs.accepted_semantic_binding(role))
            }),
        },
    )?;
    let x86_scalar_fma_plan_associations =
        provider_planning::x86_fma_plan_association::bind_checked_x86_scalar_fma_plan_associations(
            &selected_execution_settlement.program,
            &selected_execution_settlement.selected_provider_plan_facts,
            &selected_execution_settlement.selected_provider_provenance,
            x86_scalar_fma_provider,
            selected_target_profile,
        )?;

    if let Some(entry) = selected_program_entry.as_mut() {
        let establishments = selected_dispatch::derive_fused_program_entry_establishments(
            &selected_execution_settlement.program,
            entry.source_signature(),
            &selected_execution_settlement.selected_provider_provenance,
            permit_unsettled_fused_service_fields,
        )?;
        entry
            .bind_fused_service_establishments(establishments)
            .map_err(|message| vec![Diagnostic::error(message)])?;
    }

    Ok(CheckedExecution {
        settled: selected_execution_settlement,
        build: computed_build_config,
        optimization_selection_identity,
        const_evaluation,
        application,
        selected_target_profile,
        selected_native_target,
        x86_scalar_fma_provider,
        x86_scalar_fma_plan_associations,
        selected_program_entry,
        selected_build_machine_identity,
        boundary_calling_plan_realizations,
        product_selection,
        provider_plans,
        evaluated_via_bindings,
        external_binding_rows,
        root_grants,
        // The Build evaluated once for every target; this target's
        // compilation records the requests as its own.
        restricted_build_requests: restricted_build_requests
            .into_iter()
            .map(|request| request.for_target(selected_target_profile))
            .collect(),
        independent_component_descriptions: package_inputs
            .map(|inputs| {
                inputs
                    .independent_component_descriptions()
                    .cloned()
                    .collect()
            })
            .unwrap_or_default(),
        accepted_component_assumptions,
    })
}
