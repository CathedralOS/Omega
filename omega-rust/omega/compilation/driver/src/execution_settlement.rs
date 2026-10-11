//! Join each target's provider selection against the invocation's immutable
//! checked source, then realize the canonical artifact and the selected
//! execution beside it. Selection is the Omega-00 join's producing half — it
//! consumes the Psi facade's sealed checked product and Build rows; provider
//! selection owns target choices, never another Psi checking pass. The
//! installation half runs after Terminal production: the target-neutral
//! artifact exists before the selected-execution rewrites that consume it.

use crate::checked::CheckedCompilation;
use crate::checked::checking::{attribute_diagnostics, timed};
use artifacts::compile_timings::{CompileTimings, EXECUTION_SETTLEMENT};
use compilation_products::{
    BuildAdmission, CheckedCompilationInputs, CheckedCompileOutcome, CheckedProductSource,
    CompileFailure, RetainedCompilationChecks,
};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use selected_dispatch::{
    CheckedProviderProgram, ProviderBindingRequest, SelectedExecutionRequest,
    bind_checked_providers, settle_selected_execution,
};
use std::sync::Arc;

/// One target's complete provider selection beside the shared source custody.
/// Terminal production reads this record to mint the target-neutral artifact;
/// installation consumes it with the artifact to settle the selected
/// execution. None of its fields re-enter checking or rewrite the program.
pub(crate) struct SelectedProviderCustody {
    pub(crate) checked: CheckedProviderProgram,
    pub(crate) settlement: provider_planning::ProviderSettlement,
    pub(crate) selected_provider_provenance:
        Vec<provider_planning::SelectedProviderReviewProvenance>,
    pub(crate) selected_program_entry:
        Option<provider_planning::program_entry::SelectedCompilerProgramEntry>,
    pub(crate) selected_target_profile: Option<target::TargetProfile>,
    pub(crate) x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    pub(crate) build: Arc<compilation_products::ComputedBuildConfig>,
    pub(crate) build_admission: BuildAdmission,
    pub(crate) provider_defaults:
        Arc<[build_time_evaluation::provider_declarations::TargetProviderDefaults]>,
    pub(crate) product_selection: Option<psi_compiler::CheckedTreeProductSelection>,
    pub(crate) sources: Arc<package_compilation::CheckedSourceCustody>,
    pub(crate) timings: CompileTimings,
}

fn select_checked_providers(
    source: &CheckedProductSource,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<
    (
        CheckedProviderProgram,
        provider_planning::ProviderSettlement,
        Vec<provider_planning::SelectedProviderReviewProvenance>,
        Option<provider_planning::program_entry::SelectedCompilerProgramEntry>,
        Option<target::AdmittedX86ScalarFmaProvider>,
    ),
    Vec<Diagnostic>,
> {
    let typed = &source.program.typed;
    let build_config = &source.computed_build_config.config;
    let selected_native_target = selected_target_profile
        .map(target::TargetProfile::native_target)
        .unwrap_or_else(target::NativeTarget::host);
    let mut boundary_calling_plan_realizations =
        provider_planning::calling_policy_plans::compute_boundary_calling_plans(
            typed,
            selected_native_target,
            &build_config.opaque_representation_selections,
            package_inputs,
        )?;
    // Provider schemas and provenance commit to the complete calling
    // application. Close target callback geometry before either captures it;
    // otherwise nominal placements retain no private materialization and a
    // later closure would invalidate the already-captured plan commitment.
    // This consumes the shared source judgment, never another Psi check.
    if selected_target_profile.is_some() {
        provider_planning::calling_policy_plans::close_outbound_callback_materializations(
            typed,
            &mut boundary_calling_plan_realizations,
            selected_native_target,
            &build_config.opaque_representation_selections,
            package_inputs,
        )?;
    }
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
        &source.provider_defaults,
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
        &source.program.facts.synchronous_invocations,
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
    Ok((
        checked,
        providers,
        selected_provider_provenance,
        selected_program_entry,
        x86_scalar_fma_provider,
    ))
}

/// Select this target's provider custody without starting installation.
/// The returned record carries everything Terminal production and the
/// selected-execution settle read; it has no production or admission side.
pub(crate) fn select_product_providers(
    source: &CheckedProductSource,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<SelectedProviderCustody, Vec<Diagnostic>> {
    let mut timings = source.timings.clone();
    let selection = timed(&mut timings, EXECUTION_SETTLEMENT, |_| {
        select_checked_providers(source, selected_target_profile, package_inputs)
    });
    let selection = selection.map_err(|mut diagnostics| {
        if let Some(sources) = source.program.typed.symbols.source_map().as_deref() {
            attribute_diagnostics(&mut diagnostics, sources, package_inputs);
        }
        diagnostics
    })?;
    let (checked, settlement, provenance, entry, x86) = selection;
    Ok(SelectedProviderCustody {
        checked,
        settlement,
        selected_provider_provenance: provenance,
        selected_program_entry: entry,
        selected_target_profile,
        x86_scalar_fma_provider: x86,
        build: Arc::clone(&source.computed_build_config),
        build_admission: source.admission.clone().for_target(selected_target_profile),
        provider_defaults: Arc::clone(&source.provider_defaults),
        product_selection: source.product_selection.clone(),
        sources: Arc::clone(&source.sources),
        timings,
    })
}

/// Install one already-selected execution against its produced artifact
/// custody. Installation never reruns selection or checking; the artifact
/// beside this record is what later joins read.
pub(crate) fn install_selected_target(
    selection: SelectedProviderCustody,
    retained_checks: RetainedCompilationChecks,
    package_inputs: Option<&PackageCompilationInputs>,
    permit_unsettled_fused_service_fields: bool,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    let source_map = selection.checked.program.typed.symbols.source_map();
    let SelectedProviderCustody {
        checked,
        settlement,
        selected_provider_provenance,
        selected_program_entry,
        selected_target_profile,
        x86_scalar_fma_provider,
        build,
        build_admission,
        provider_defaults,
        product_selection,
        sources,
        mut timings,
    } = selection;
    let settled = timed(&mut timings, EXECUTION_SETTLEMENT, |_| {
        settle_selected_execution(
            checked,
            SelectedExecutionRequest {
                calling_plans: &settlement.boundary_calling_plan_realizations,
                selected_program_entry,
                selected_target_profile,
                package_inputs,
                x86_scalar_fma_provider,
                permit_unsettled_fused_service_fields,
                selected_provider_provenance,
                opaque_representation_selections: &build.config.opaque_representation_selections,
            },
        )
    });
    settled
        .map(|settled| {
            CheckedCompilation::from_settled_execution(
                CheckedCompilationInputs {
                    settled,
                    build,
                    build_admission,
                    provider_defaults,
                    providers: settlement,
                    product_selection,
                },
                sources,
                timings,
            )
            .with_settled_checks(retained_checks)
        })
        .map_err(|mut diagnostics| {
            if let Some(sources) = source_map.as_deref() {
                attribute_diagnostics(&mut diagnostics, sources, package_inputs);
            }
            diagnostics
        })
}

/// Install one selected target and wrap the outcome for check-only callers.
fn install_product_target(
    source: &CheckedProductSource,
    selection: SelectedProviderCustody,
    package_inputs: Option<&PackageCompilationInputs>,
    permit_unsettled_fused_service_fields: bool,
) -> CheckedCompileOutcome {
    let independent_components = source.computed_build_config.independent_components();
    let result = install_selected_target(
        selection,
        source.retained_checks.clone(),
        package_inputs,
        permit_unsettled_fused_service_fields,
    );
    CheckedCompileOutcome {
        result: result.map_err(CompileFailure::from),
        independent_components,
    }
}

/// Bind one target to shared checked source, never a resumable frontend.
/// Build-time and check-only callers settle selection and installation in one
/// pass; the product route produces the canonical artifact between them.
pub(crate) fn settle_product_target(
    source: &CheckedProductSource,
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    permit_unsettled_fused_service_fields: bool,
) -> CheckedCompileOutcome {
    match select_product_providers(source, selected_target_profile, package_inputs) {
        Ok(selection) => install_product_target(
            source,
            selection,
            package_inputs,
            permit_unsettled_fused_service_fields,
        ),
        Err(diagnostics) => CheckedCompileOutcome {
            result: Err(CompileFailure::from(diagnostics)),
            independent_components: source.computed_build_config.independent_components(),
        },
    }
}
