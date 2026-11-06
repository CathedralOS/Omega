//! Bind a target's selected implementations using declarations saved before
//! Terminal publication. Grants, calls and callbacks are checked in order;
//! this step cannot read source or choose a different implementation.

use crate::SelectedExecution;
use diagnostics::Diagnostic;
use package_compilation::AcceptedSemanticBindingRole;
use provider_planning::service_declarations::ServiceDeclarations;
use std::sync::Arc;

mod preparation;
pub use preparation::PreparedBindingDeclarations;

/// Realization choices joined to a completed source judgment.
pub struct ProviderBindingRequest<'a> {
    pub declarations: &'a PreparedBindingDeclarations,
    pub selected_declarations:
        Vec<Result<language_semantics::provider_declarations::ProviderDeclarations, Diagnostic>>,
    pub native_provider_sources: Result<Arc<backend_plan::NativeProviderSources>, Vec<Diagnostic>>,
    pub entry_service_requirements: Vec<symbols::SymbolHandle>,
    pub package_inputs: Option<&'a package_compilation::PackageCompilationInputs>,
    pub boundary_calling_plan_realizations:
        &'a [provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    pub provider_candidates: &'a [provider_planning::ProviderPlanCandidate],
    pub boundary_provider_demands: &'a [effects::provider_plan::ProviderPlan],
    pub provider_declarations: &'a Arc<[backend_plan::TerminalTargetProviderPlan]>,
    pub calling_program: &'a provider_planning::calling_policy_plans::CallingProgram,
    pub exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
    pub selected_provider_plan_facts: effects::SelectedProviderPlanFacts,
    pub root_grants: &'a [String],
    pub selected_provider_provenance: Vec<provider_planning::SelectedProviderReviewProvenance>,
    pub authored_root_grants: &'a [trust_model::AuthoredRootGrant],
    /// Derivation target is not a deployment choice for checking-only products.
    pub provider_planning_target_profile: Option<target::TargetProfile>,
    pub selected_target_profile: Option<target::TargetProfile>,
    pub selected_program_entry:
        Option<provider_planning::program_entry::SelectedCompilerProgramEntry>,
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
}

/// Finish target binding without reopening source. Saved errors are reported
/// at their original step; native-only errors remain deferred until emission.
pub fn prepare_selected_execution(
    mut settlement: ProviderBindingRequest<'_>,
) -> Result<SelectedExecution, Vec<Diagnostic>> {
    settlement.declarations.owner.clone()?;
    if !std::sync::Weak::ptr_eq(
        &settlement.declarations.source_owner,
        &settlement.calling_program.source_owner,
    ) {
        return Err(vec![Diagnostic::error(
            "selected provider inventory belongs to another checked program",
        )]);
    }
    if settlement.selected_target_profile.is_some()
        && settlement.selected_target_profile != settlement.provider_planning_target_profile
    {
        return Err(vec![Diagnostic::error(
            "selected deployment target differs from retained provider planning target",
        )]);
    }
    let provider_plans = settlement
        .provider_candidates
        .iter()
        .map(provider_planning::ProviderPlanCandidate::plan)
        .collect::<Vec<_>>();
    if settlement.package_inputs.is_some() && !settlement.root_grants.is_empty() {
        trust_model::reject_package_non_provider_grants(
            &settlement.declarations.trust,
            settlement.root_grants,
            &provider_plans,
            &settlement.selected_provider_plan_facts,
        )?;
    }
    let external_providers = settlement.declarations.external.clone()?;
    external_providers
        .require_prepared_owner(
            &settlement.calling_program.source_owner,
            settlement.exact_toolchain_sources,
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "external provider declarations: {error:?}"
            ))]
        })?;
    let declarations = provider_planning::prepare_selected_plan_declarations(
        settlement.selected_declarations,
        &settlement.selected_provider_plan_facts,
        &settlement.selected_provider_provenance,
    );
    let binding = provider_selection::bind_selected_provider_plan_facts(
        provider_selection::BoundaryProviderAdmission {
            demands: settlement.boundary_provider_demands,
            review: &external_providers.boundary_provider_review(),
            identities: &external_providers.identities,
            external_providers: &external_providers.conformances,
        },
        settlement
            .declarations
            .issuers
            .as_ref()
            .map(|issuers| issuers.iter().collect())
            .map_err(|reason| *reason),
        &provider_plans,
        settlement.selected_provider_plan_facts,
        declarations,
        settlement.root_grants,
        settlement.authored_root_grants,
    )?;
    let (selected_provider_plan_facts, selected_provider_grants) = binding.into_parts();
    let selected_provider_plan_facts = Arc::new(selected_provider_plan_facts);
    // Close the existing exact source association once. Native ABI publication
    // is not needed to count calling declarations and must not add early errors.
    let calling_requirements = external_providers.callbacks.calling_requirements(
        settlement
            .boundary_calling_plan_realizations
            .iter()
            .map(|realization| {
                (
                    realization.calling_owner_application(),
                    realization.requirement_machine,
                )
            }),
    );
    let callback_roots = {
        let providers = provider_selection::CallbackProviderDeclarations::prepare(
            &selected_provider_plan_facts,
            &external_providers.callbacks.declarations,
            settlement.provider_declarations,
            settlement.calling_program.artifact.semantic_module(),
        )?;
        let mut selected = provider_selection::select_callback_declarations(
            &external_providers.callbacks.declarations,
            &calling_requirements,
            &providers,
        )?;
        for installed in external_providers
            .callbacks
            .installed_roots(&calling_requirements)?
        {
            if !selected.contains(&installed) {
                selected.push(installed);
            }
        }
        external_providers.selected_callback_roots(selected)
    };
    let service_declarations = Arc::new(ServiceDeclarations::prepare(
        settlement.declarations.services.declared_traits(),
        settlement.boundary_calling_plan_realizations,
    ));

    let accepted_binding = |role| {
        settlement
            .package_inputs
            .and_then(|inputs| inputs.accepted_semantic_binding(role))
    };
    let resolved_exit_bindings = crate::retain_selected_compiler_intrinsic_review_identities(
        settlement.provider_candidates,
        &external_providers,
        &service_declarations,
        &selected_provider_plan_facts,
        &mut settlement.selected_provider_provenance,
        settlement
            .provider_planning_target_profile
            .map(target::TargetProfile::target_name),
        accepted_binding(AcceptedSemanticBindingRole::ConsoleExitProcessI32),
        accepted_binding(AcceptedSemanticBindingRole::ProcessExitExitProcessI32),
    )?;
    let direct_boundary_leaves = crate::intrinsic_review::produce_direct_boundary_leaves(
        &external_providers,
        &settlement.selected_provider_provenance,
        settlement.provider_candidates,
        &service_declarations,
        settlement
            .provider_planning_target_profile
            .map(target::TargetProfile::target_name),
        &resolved_exit_bindings,
    )?;
    let mut resolved_semantic_bindings = resolved_exit_bindings;
    let entry_role = settlement
        .selected_target_profile
        .map(|profile| profile.program_entry_slot())
        .and_then(|slot| slot.physical_contract_package)
        .map(package_compilation::program_entry_semantic_binding_role);
    for role in [
        Some(AcceptedSemanticBindingRole::FilesystemHostService),
        Some(AcceptedSemanticBindingRole::TimeHostService),
        entry_role,
    ] {
        if let Some(binding) = role.and_then(accepted_binding) {
            let resolved = crate::resolve_accepted_service_binding(&service_declarations, binding)
                .map_err(|diagnostic| vec![diagnostic])?;
            resolved_semantic_bindings.push(resolved);
        }
    }
    Ok(SelectedExecution {
        direct_boundary_leaves,
        service_declarations,
        external_providers,
        native_provider_sources: settlement.native_provider_sources,
        selected_provider_plan_facts,
        entry_service_requirements: settlement.entry_service_requirements,
        trust_declarations: Arc::clone(&settlement.declarations.trust),
        has_proof_obligations: settlement.declarations.has_proof_obligations,
        selected_target_profile: settlement.selected_target_profile,
        selected_program_entry: settlement.selected_program_entry,
        x86_scalar_fma_provider: settlement.x86_scalar_fma_provider,
        selected_provider_grants,
        callback_roots,
        selected_provider_provenance: settlement.selected_provider_provenance,
        resolved_semantic_bindings,
    })
}
