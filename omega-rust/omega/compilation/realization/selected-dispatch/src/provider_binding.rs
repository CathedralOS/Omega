//! Prepare complete selected execution from immutable checked Psi and Build inputs.
//!
//! Grants, callbacks, declaration catalogs, selected review and accepted host
//! bindings close here, before Terminal production borrows the result. None of
//! this work consumes an artifact. Native demand and receiving permission remain
//! later joins; a prepared direct-call supply is not permission to execute it.

use crate::SelectedExecution;
use checked_trees::CheckedTrees as CheckedProgram;
use diagnostics::Diagnostic;
use package_compilation::AcceptedSemanticBindingRole;
use std::sync::Arc;

/// Realization choices joined to a completed source judgment.
pub struct ProviderBindingRequest<'a> {
    pub exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
    pub package_inputs: Option<&'a package_compilation::PackageCompilationInputs>,
    pub boundary_calling_plan_realizations:
        &'a [provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    pub provider_candidates: &'a [provider_planning::ProviderPlanCandidate],
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
    pub external_declarations: &'a std::sync::OnceLock<
        Result<Arc<psi_terminal_production::PreparedDeclarationCatalog>, Vec<Diagnostic>>,
    >,
}

/// Close source-dependent realization once without rewriting the shared program.
/// Source/role failures precede Terminal lowering and checking-only task demand
/// validation. Native-only source refusals remain deferred until that handoff.
pub fn prepare_selected_execution(
    program: Arc<CheckedProgram>,
    accepted_template_classifications: trust_model::AcceptedTemplateClassifications,
    mut settlement: ProviderBindingRequest<'_>,
) -> Result<SelectedExecution, Vec<Diagnostic>> {
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
    if settlement.package_inputs.is_some() {
        trust_model::reject_package_non_provider_grants(
            &program.typed,
            settlement.root_grants,
            &provider_plans,
            &settlement.selected_provider_plan_facts,
        )?;
    }
    // Prepare immutable checked declaration evidence before source-free binding.
    let declaration_catalog = settlement
        .external_declarations
        .get_or_init(|| {
            psi_terminal_production::PreparedDeclarationCatalog::prepare(
                Arc::clone(&program),
                settlement.exact_toolchain_sources,
            )
            .map(Arc::new)
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "external provider declarations: {error:?}"
                ))]
            })
        })
        .clone()?;
    declaration_catalog
        .require_owner(&program, settlement.exact_toolchain_sources)
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "external provider declarations: {error:?}"
            ))]
        })?;
    let declarations = provider_planning::prepare_selected_plan_declarations(
        &program,
        &settlement.selected_provider_plan_facts,
        &settlement.selected_provider_provenance,
    );
    let binding = provider_selection::bind_selected_provider_plan_facts(
        provider_selection::BoundaryProviderAdmission {
            review: &declaration_catalog.boundary_provider_review(),
            identities: &declaration_catalog.identities,
            numeric_realizations: &declaration_catalog.conformances,
        },
        program.facts.semantic.admitted_qualification_issuers(),
        &provider_plans,
        settlement.selected_provider_plan_facts,
        declarations,
        settlement.root_grants,
        settlement.authored_root_grants,
    )?;
    let (selected_provider_plan_facts, selected_provider_grants) = binding.into_parts();
    // Close the existing exact source association once. Native ABI publication
    // is not needed to count calling declarations and must not add early errors.
    let calling_requirements = declaration_catalog.callbacks.calling_requirements(
        settlement
            .boundary_calling_plan_realizations
            .iter()
            .filter(|realization| realization.boundary_arguments.is_empty())
            .map(|realization| (realization.boundary_trait, realization.requirement_machine)),
    );
    let callback_roots = provider_planning::callback_roots::prepare(
        &program,
        &selected_provider_plan_facts,
        &declaration_catalog.callbacks.declarations,
        &calling_requirements,
    )?;
    let callback_roots = declaration_catalog.selected_callback_roots(callback_roots);
    let service_declarations = Arc::new(
        provider_planning::service_declarations::ServiceDeclarations::prepare(
            provider_planning::ProviderPlanningContext::new(
                &program.typed,
                source::DependencyScope::Product,
            ),
            settlement.boundary_calling_plan_realizations,
            settlement.exact_toolchain_sources,
        ),
    );
    let native_provider_sources = provider_planning::prepare_native_provider_sources(
        provider_planning::ProviderPlanningContext::new(
            &program.typed,
            source::DependencyScope::Product,
        ),
        settlement.exact_toolchain_sources,
        &settlement.selected_provider_provenance,
    )
    .map(Arc::new);

    let accepted_binding = |role| {
        settlement
            .package_inputs
            .and_then(|inputs| inputs.accepted_semantic_binding(role))
    };
    let resolved_exit_bindings = crate::retain_selected_compiler_intrinsic_review_identities(
        settlement.provider_candidates,
        &declaration_catalog,
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
        &declaration_catalog,
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
        declaration_catalog,
        native_provider_sources,
        program,
        selected_target_profile: settlement.selected_target_profile,
        selected_program_entry: settlement.selected_program_entry,
        x86_scalar_fma_provider: settlement.x86_scalar_fma_provider,
        selected_provider_plan_facts,
        selected_provider_grants,
        callback_roots,
        accepted_template_classifications,
        selected_provider_provenance: settlement.selected_provider_provenance,
        resolved_semantic_bindings,
    })
}
