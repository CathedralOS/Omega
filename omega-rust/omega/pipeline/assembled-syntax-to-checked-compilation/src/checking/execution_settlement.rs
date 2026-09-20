//! Settle build-selected execution against the final typed program.
//! The resulting checked program and its sidecars remain one owned handoff.

use super::build_continuation::BuiltCheckedProgram;
use super::const_evaluation;
use crate::checking::phase_transitions::{
    SelectedExecutionSettlementInput, SelectedExecutionSettlementSurface,
    TypedToCheckedSettlementInput, settle_selected_execution, typed_trees_to_checked_trees,
};
use artifacts::compile_timings::CompileTimings;
use build_time_evaluation::SelectedBuildTimeOperators;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CheckedExecution {
    pub(super) settled: SelectedExecutionSettlementSurface,
    pub(super) const_evaluation: const_evaluation::SelectedConstEvaluation,
    pub(super) subsystem: u16,
    pub(super) application_intent: Option<build_evaluation::HostedApplicationIntent>,
    pub(super) application_identifier: Option<build_evaluation::ApplicationIdentifier>,
    /// The validated authored `builder.application` declaration; its name
    /// supplies the `.app` basename and inner executable leaf at publication
    /// and its artifact-only intent narrows the admitted route.
    pub(super) application: Option<build_declarations::ApplicationDeclaration>,
    pub(super) pcc_requests: build_evaluation::PccRequests,
    pub(super) selected_target_profile: Option<target::TargetProfile>,
    pub(super) selected_native_target: Option<target::NativeTarget>,
    pub(super) x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    pub(super) x86_scalar_fma_plan_associations:
        Vec<provider_planning::x86_fma_plan_association::CheckedX86ScalarFmaPlanAssociation>,
    pub(super) selected_program_entry: Option<build_evaluation::SelectedCompilerProgramEntry>,
    pub(super) selected_build_machine_symbol: Option<symbols::SymbolHandle>,
    pub(super) selected_build_machine_identity: Option<String>,
    pub(super) opaque_representation_selections:
        Vec<representation_planning::OpaqueRepresentationSelection>,
    pub(super) boundary_calling_plan_realizations:
        Vec<provider_planning::calling_policy_plans::BoundaryCallingPlanRealization>,
    pub(super) optimization: crate::optimization::checked_handoff::CheckedOptimizationHandoff,
    /// Identity-bearing checked-tree product selection evidence, present only
    /// when the effective build selection named `CheckedTreeProductPruning`.
    pub(super) product_selection: Option<typed_trees_to_checked_trees::CheckedTreeProductSelection>,
    pub(super) provider_plans: Vec<effects::provider_plan::ProviderPlan>,
    pub(super) evaluated_via_bindings:
        provider_planning::evaluated_via_bindings::EvaluatedViaBindingTable,
    pub(super) external_binding_rows: Vec<calling_conventions::ExternalBindingRow>,
    pub(super) root_grants: Vec<String>,
    /// Behavior exclusions authored by the authoritative build machine
    /// (wiki/spec/build/behavior_exclusions.md): product-admission
    /// requirements over the exact selected executable composition, each
    /// retaining its exact toolchain case identity and authored span.
    pub(super) behavior_exclusions: Vec<build_evaluation::AuthoredBehaviorExclusion>,
    pub(super) build_evaluation_usage: Option<build_evaluation::BuildEvaluationUsage>,
    pub(super) build_observation_summary: Option<build_evaluation::BuildObservationSummary>,
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

pub(super) fn check_selected_execution(
    built: BuiltCheckedProgram,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    optimization_rollback: &crate::OptimizationRollback,
    timings: &mut CompileTimings,
) -> Result<CheckedExecution, Vec<Diagnostic>> {
    let BuiltCheckedProgram {
        mut typed,
        selected_target_machine_declarations,
        pending_pre_checks,
        computed_build_config,
        restricted_build_requests,
        application,
        selected_build_machine_symbol,
        selected_build_machine_identity,
    } = built;
    let build_evaluation_usage = computed_build_config.evaluation_usage;
    let build_observation_summary = computed_build_config.observation_summary;
    let optimization_report = computed_build_config.optimization_report_request;
    let build_config = computed_build_config.config;
    let selected_native_target = selected_target_profile
        .map(target::TargetProfile::native_target)
        .unwrap_or_else(target::NativeTarget::host);
    let mut boundary_calling_plan_realizations =
        provider_planning::calling_policy_plans::compute_boundary_calling_plans(
            &mut typed,
            selected_native_target,
            &build_config.opaque_representation_selections,
            package_inputs,
        )?;
    let opaque_representation_selections = build_config.opaque_representation_selections.clone();
    let x86_scalar_fma_provider = build_config.x86_scalar_fma_provider;
    let subsystem = build_config.subsystem;
    let application_intent = build_config.application_intent;
    let application_identifier = build_config.application_identifier.clone();
    let optimization = crate::optimization::checked_handoff::CheckedOptimizationHandoff::retain(
        build_config.optimizations.clone(),
        optimization_report,
    );
    // Compatibility demands are semantic checks, not report-mode behavior.
    // Validate them on the canonical checked route even when no auxiliary
    // artifact writer is requested by the outer compiler coordinator.
    build_evaluation::validate_wire_protocol(&typed, &build_config.wire_compatibility_demands)?;
    // A semantic-only checked compilation has no selected target and therefore
    // no storage root. Authored bindings remain available in the evaluated
    // build configuration, but only an exact target selection may activate one
    // for interpreter or production execution.
    let program_entry_binding_role = selected_target_profile
        .map(|profile| profile.program_entry_slot())
        .and_then(|slot| slot.physical_contract_package)
        .map(build_evaluation::program_entry_semantic_binding_role);
    let mut selected_program_entry = build_evaluation::select_compiler_program_entry(
        &typed,
        &build_config,
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
    let build_evaluation::CheckedProviderSelection {
        provider_plans,
        evaluated_via_bindings,
        selected_provider_plan_facts,
        selected_provider_provenance,
        external_binding_rows,
    } = build_evaluation::settle_checked_providers(
        &mut typed,
        selected_target_machine_declarations,
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
    let mut const_evaluation = const_evaluation::SelectedConstEvaluation::default();
    if !pending_pre_checks.is_empty() {
        validation::land_float_literal_destinations(&mut typed);
        const_evaluation.operators =
            const_evaluation::selected_operators(&typed, &selected_provider_plan_facts)?;
        const_evaluation.provider_bodies =
            const_evaluation::selected_provider_bodies(&typed, &selected_provider_plan_facts)?;
        for pre_check in pending_pre_checks {
            const_evaluation
                .folds
                .extend(pre_check.evaluate_selected_operators(
                    &mut typed,
                    SelectedBuildTimeOperators {
                        operators: &const_evaluation.operators,
                        provider_bodies: &const_evaluation.provider_bodies,
                    },
                )?);
        }
    }
    const_evaluation::require_evaluated_array_lengths(&typed)?;
    const_evaluation::require_evaluated_range_endpoints(&typed)?;
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
    let checked = typed_trees_to_checked_trees(
        typed,
        timings,
        TypedToCheckedSettlementInput {
            native_target: selected_native_target,
            package_inputs,
            selected_build_machine: selected_build_machine_symbol,
            freestanding: build_config.freestanding,
            boundary_calling_plan_realizations: &mut boundary_calling_plan_realizations,
            opaque_representation_selections: &opaque_representation_selections,
            provider_plans: &provider_plans,
            selected_provider_plan_facts,
            root_grants: &root_grants,
            authored_root_grants: &build_config.grants,
        },
    )?;
    const_evaluation.validate(
        &checked.program,
        &checked.selected_provider_plan_facts,
        package_inputs,
    )?;
    if let Some(package_inputs) = package_inputs {
        crate::package::declaration_admission::validate_authored_declaration_selections(
            &checked.program,
            package_inputs,
        )?;
    }
    // The CheckedTrees optimization phase runs only after all authored code
    // has been checked and before selected-execution sidecars derive from the
    // product. Its exact roots come from this compilation's own product
    // selection: the validated program-entry binding for a selected target,
    // or every authored root binding for a targetless shared product. The
    // release rollback settles before the phase so the effective selection is
    // what executes.
    let effective_optimizations = optimization_rollback.settle(&build_config.optimizations);
    let product_root_machines: std::collections::HashSet<symbols::SymbolHandle> =
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
    let (checked, product_selection) = crate::optimization::checked_trees::execute(
        checked,
        effective_optimizations.effective(),
        product_root_machines.into_iter().collect(),
    )?;
    let exact_component_progress_root = selected_program_entry.as_ref().map(|entry| {
        let source = entry.source_signature();
        provider_planning::component_progress::ExactComponentProgressRoot::new(
            source.machine_symbol(),
            source.normalized_callable_identity(),
        )
    });
    let selected_execution_settlement = settle_selected_execution(
        checked,
        SelectedExecutionSettlementInput {
            exact_component_progress_root,
            provider_selection_target,
            selected_target_profile,
            selected_provider_provenance,
            opaque_representation_selections: &opaque_representation_selections,
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
        )?;
        entry
            .bind_fused_service_establishments(establishments)
            .map_err(|message| vec![Diagnostic::error(message)])?;
    }

    Ok(CheckedExecution {
        settled: selected_execution_settlement,
        const_evaluation,
        subsystem,
        application_intent,
        application_identifier,
        application,
        pcc_requests: build_config.pcc,
        selected_target_profile,
        selected_native_target,
        x86_scalar_fma_provider,
        x86_scalar_fma_plan_associations,
        selected_program_entry,
        selected_build_machine_symbol,
        selected_build_machine_identity,
        opaque_representation_selections,
        boundary_calling_plan_realizations,
        optimization,
        product_selection,
        provider_plans,
        evaluated_via_bindings,
        external_binding_rows,
        root_grants,
        behavior_exclusions: build_config.behavior_exclusions,
        build_evaluation_usage,
        build_observation_summary,
        restricted_build_requests,
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
