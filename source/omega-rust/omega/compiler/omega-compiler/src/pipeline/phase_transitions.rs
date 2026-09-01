use crate::pipeline::source_assembly::AssembledSyntax;
use crate::pipeline::stage::{
    SYMBOL_RESOLVED_TREES_TO_TYPED_TREES, SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES,
    TYPED_TREES_TO_CHECKED_TREES,
};
use crate::pipeline::timing::CompileTimings;
use psi_checked_trees::CheckedTrees as CheckedProgram;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_typed_trees::TypedTrees;
use std::sync::Arc;

/// Checked Psi plus the exact predecessor facts that must be captured before
/// typed ownership moves into checking. This is the output of one phase
/// transition, not source-loading state.
pub(super) struct CheckedProgramSurface {
    pub(super) program: Arc<CheckedProgram>,
    pub(super) selected_provider_plan_facts: omega_effects::SelectedProviderPlanFacts,
    pub(super) selected_provider_grants:
        Vec<omega_trust_model::ResolvedAuthoredSelectedProviderGrant>,
    pub(super) callback_placements: Vec<omega_backend_plan::BoundNominalCallbackPlacement>,
    pub(super) accepted_template_classifications:
        omega_trust_model::AcceptedTemplateClassifications,
    pub(super) contract_entailment_stand_downs: Vec<psi_validation::ContractEntailmentStandDown>,
}

/// Checked semantics after selected execution has been settled in the exact
/// compiler-owned dispatch order. This surface owns the now-closed review
/// provenance alongside every checked-phase sidecar.
pub(super) struct SelectedExecutionSettlementSurface {
    pub(super) program: Arc<CheckedProgram>,
    pub(super) selected_provider_plan_facts: omega_effects::SelectedProviderPlanFacts,
    pub(super) selected_provider_grants:
        Vec<omega_trust_model::ResolvedAuthoredSelectedProviderGrant>,
    pub(super) callback_placements: Vec<omega_backend_plan::BoundNominalCallbackPlacement>,
    pub(super) accepted_template_classifications:
        omega_trust_model::AcceptedTemplateClassifications,
    pub(super) contract_entailment_stand_downs: Vec<psi_validation::ContractEntailmentStandDown>,
    pub(super) selected_provider_provenance:
        Vec<crate::pipeline::provider_plans::SelectedProviderReviewProvenance>,
    pub(super) resolved_semantic_bindings:
        Vec<omega_selected_dispatch::ResolvedAcceptedSemanticBinding>,
    pub(super) component_progress: Option<omega_effects::ComponentProgressManifest>,
    pub(super) task_activations: omega_task_plans::TaskActivationPlanSet,
}

pub(super) struct SelectedExecutionSettlementInput<'a> {
    pub(super) exact_component_progress_root:
        Option<crate::pipeline::component_progress::ExactComponentProgressRoot<'a>>,
    pub(super) provider_selection_target: omega_target::NativeTarget,
    pub(super) selected_target_profile: Option<omega_target::TargetProfile>,
    pub(super) selected_provider_provenance:
        Vec<crate::pipeline::provider_plans::SelectedProviderReviewProvenance>,
    pub(super) opaque_representation_selections:
        &'a [omega_representation_planning::OpaqueRepresentationSelection],
    pub(super) accepted_console_binding:
        Option<&'a omega_package_compilation::AcceptedSemanticBinding>,
    pub(super) accepted_filesystem_binding:
        Option<&'a omega_package_compilation::AcceptedSemanticBinding>,
    pub(super) accepted_uefi_binding:
        Option<&'a omega_package_compilation::AcceptedSemanticBinding>,
}

/// Final typed settlements that must finish inside the phase transition that
/// produces the checked program surface.
pub(super) struct TypedToCheckedSettlementInput<'a> {
    pub(super) native_target: Option<omega_target::NativeTarget>,
    pub(super) package_inputs: Option<&'a crate::pipeline::PackageCompilationInputs>,
    pub(super) selected_build_machine: Option<psi_symbols::SymbolHandle>,
    pub(super) boundary_calling_plan_realizations:
        &'a mut [crate::pipeline::calling_policy_plans::BoundaryCallingPlanRealization],
    pub(super) opaque_representation_selections:
        &'a [omega_representation_planning::OpaqueRepresentationSelection],
    pub(super) provider_plans: &'a [omega_effects::provider_plan::ProviderPlan],
    pub(super) selected_provider_plan_facts: omega_effects::SelectedProviderPlanFacts,
    pub(super) root_grants: &'a [String],
    pub(super) authored_root_grants: &'a [omega_trust_model::AuthoredRootGrant],
}

pub(super) fn syntax_trees_to_symbol_resolved_trees(
    syntax: AssembledSyntax,
    timings: &mut CompileTimings,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    timings.record(SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES, || {
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources_and_top_level_bindings(
            &syntax.syntax_trees,
            syntax.sources,
            syntax.source_scoped_top_level_bindings,
        )
    })
}

pub(super) fn symbol_resolved_trees_to_seeded_plain_data_base(
    resolved: SymbolResolvedTrees,
    timings: &mut CompileTimings,
) -> Result<psi_symbol_resolved_trees_to_typed_trees::SeededPlainDataTypingBase, Vec<Diagnostic>> {
    timings.record(SYMBOL_RESOLVED_TREES_TO_TYPED_TREES, || {
        psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_to_seeded_plain_data_base(
            resolved,
        )
        .map_err(|diagnostic| vec![diagnostic])
    })
}

pub(super) fn resolve_seeded_syntax_extension(
    base: SymbolResolvedTrees,
    extension: &psi_syntax_trees::SyntaxTrees,
    sources: Arc<psi_source::SourceMap>,
    timings: &mut CompileTimings,
) -> Result<psi_syntax_trees_to_symbol_resolved_trees::SeededSymbolResolvedTrees, Vec<Diagnostic>> {
    timings.record(SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES, || {
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_extension_with_authored_selection_frontier(
            base,
            extension,
            sources,
            Vec::new(),
        )
    })
}

pub(super) fn type_seeded_plain_data_extension(
    source: psi_syntax_trees_to_symbol_resolved_trees::RebasedSeededSymbolResolvedTrees,
    base: psi_symbol_resolved_trees_to_typed_trees::SeededPlainDataTypingBase,
    timings: &mut CompileTimings,
) -> Result<
    TypedTrees,
    (
        psi_symbol_resolved_trees_to_typed_trees::SeededPlainDataTypingBase,
        psi_symbol_resolved_trees_to_typed_trees::SeededPlainDataContinuationError,
    ),
> {
    timings.record_result(SYMBOL_RESOLVED_TREES_TO_TYPED_TREES, || {
        psi_symbol_resolved_trees_to_typed_trees::lower_seeded_plain_data_extension(source, base)
    })
}

pub(super) fn typed_trees_to_checked_trees(
    typed: TypedTrees,
    timings: &mut CompileTimings,
    settlement: TypedToCheckedSettlementInput<'_>,
) -> Result<CheckedProgramSurface, Vec<Diagnostic>> {
    timings.record(TYPED_TREES_TO_CHECKED_TREES, || {
        let accepted_template_classifications =
            omega_trust_model::AcceptedTemplateClassifications::capture(&typed);
        let contract_entailment_stand_downs =
            psi_validation::collect_contract_entailment_stand_downs(&typed);
        let selected_generic_operator_providers =
            selected_generic_operator_provider_specializations(
                &typed,
                &settlement.selected_provider_plan_facts,
            )?;
        let rederived_opaque_representation_selections =
            omega_representation_planning::rederive_opaque_representation_selections(
                &typed,
                settlement.selected_build_machine,
                settlement.opaque_representation_selections,
            )?;
        let opaque_property_receipts = rederived_opaque_representation_selections
            .iter()
            .filter_map(|selection| {
                (selection.copy_disposition()
                    == omega_representation_planning::OpaqueRepresentationCopyDisposition::CheckedSemanticCopy)
                    .then(|| psi_validation::OpaqueDataPropertyReceipt::copy(selection.opaque()))
            })
            .collect::<Vec<_>>();
        let mut program = if settlement.package_inputs.is_some() {
            psi_typed_trees_to_checked_trees::lower_package_typed_trees_with_selected_generic_operator_providers(
                typed,
                &selected_generic_operator_providers,
                &opaque_property_receipts,
            )?
        } else {
            psi_typed_trees_to_checked_trees::lower_typed_trees_with_selected_generic_operator_providers(
                typed,
                &selected_generic_operator_providers,
                &opaque_property_receipts,
            )?
        };
        crate::pipeline::provider_approval::check_boundary_provider_approval(&program)?;
        if let Some(package_inputs) = settlement.package_inputs {
            crate::pipeline::package_declaration_admission::validate_authored_declaration_selections(
                &program,
                package_inputs,
            )?;
        }
        if let Some(native_target) = settlement.native_target {
            crate::pipeline::calling_policy_plans::close_outbound_callback_materializations(
                &mut program,
                settlement.boundary_calling_plan_realizations,
                native_target,
                settlement.opaque_representation_selections,
                settlement.package_inputs,
            )?;
        }
        let callback_placements =
            crate::pipeline::calling_policy_plans::validate_nominal_callback_placement_bindings(
                &program,
                settlement.boundary_calling_plan_realizations,
            )?;
        let program = Arc::new(program);
        let selected_provider_binding =
            crate::pipeline::provider_plans::bind_selected_provider_plan_facts(
                &program,
                settlement.provider_plans,
                settlement.selected_provider_plan_facts,
                settlement.root_grants,
                settlement.authored_root_grants,
            )?;
        let (program, selected_provider_plan_facts, selected_provider_grants) =
            selected_provider_binding.into_parts();
        Ok(CheckedProgramSurface {
            program,
            selected_provider_plan_facts,
            selected_provider_grants,
            callback_placements,
            accepted_template_classifications,
            contract_entailment_stand_downs,
        })
    })
}

fn selected_generic_operator_provider_specializations(
    typed: &psi_typed_trees::TypedTrees,
    selected: &omega_effects::SelectedProviderPlanFacts,
) -> Result<
    Vec<psi_typed_trees_to_checked_trees::SelectedGenericOperatorProviderSpecialization>,
    Vec<Diagnostic>,
> {
    let mut requests = Vec::new();
    let mut diagnostics = Vec::new();
    for plan in selected.plans() {
        let operators = typed
            .operators()
            .iter()
            .filter(|operator| {
                operator.is_boundary
                    && psi_typed_trees::operator::boundary_operator_requirement_identity(
                        typed, operator,
                    ) == plan.schema.trait_name
            })
            .collect::<Vec<_>>();
        let [operator] = operators.as_slice() else {
            continue;
        };
        if typed.operator_type_parameters(operator).is_empty() {
            continue;
        }
        let [row] = plan.rows.as_slice() else {
            diagnostics.push(Diagnostic::error(format!(
                "selected generic boundary-operator ProviderPlan `{}` must retain exactly one row",
                plan.name,
            )));
            continue;
        };
        if !matches!(
            row.binding,
            omega_effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
        ) {
            continue;
        }
        let provider = match omega_provider_planning::plans::exact_checked_adapter(typed, plan, row)
        {
            Ok(provider) => provider,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        if typed.machine_type_parameters(provider).is_empty() {
            continue;
        }
        let request =
            psi_typed_trees_to_checked_trees::SelectedGenericOperatorProviderSpecialization {
                requirement_operator: operator.symbol,
                realization_machine: provider.symbol,
            };
        if !requests.contains(&request) {
            requests.push(request);
        }
    }
    if diagnostics.is_empty() {
        Ok(requests)
    } else {
        Err(diagnostics)
    }
}

/// Consume a complete checked surface and settle every selected execution
/// rewrite before publishing the final compiler-facing surface.
pub(super) fn settle_selected_execution(
    mut checked: CheckedProgramSurface,
    mut settlement: SelectedExecutionSettlementInput<'_>,
) -> Result<SelectedExecutionSettlementSurface, Vec<Diagnostic>> {
    let component_progress =
        crate::pipeline::component_progress::build_selected_component_progress_manifest(
            &checked.program,
            &checked.selected_provider_plan_facts,
            settlement.exact_component_progress_root,
            None,
        )?;
    omega_selected_dispatch::settle_selected_execution_dispatch(
        &mut checked.program,
        &checked.selected_provider_plan_facts,
    )?;
    let resolved_console_binding =
        omega_selected_dispatch::retain_selected_compiler_intrinsic_review_identities(
            &checked.program,
            &checked.selected_provider_plan_facts,
            &mut settlement.selected_provider_provenance,
            settlement
                .selected_target_profile
                .map(omega_target::TargetProfile::target_name),
            settlement.accepted_console_binding,
        )?;
    let resolved_filesystem_binding = settlement
        .accepted_filesystem_binding
        .map(|binding| {
            omega_selected_dispatch::resolve_accepted_service_binding(&checked.program, binding)
        })
        .transpose()
        .map_err(|diagnostic| vec![diagnostic])?;
    let resolved_uefi_binding = match (
        settlement.accepted_uefi_binding,
        settlement.selected_target_profile,
    ) {
        (None, _) => None,
        (Some(binding), Some(omega_target::TargetProfile::UefiX64)) => Some(
            omega_selected_dispatch::resolve_accepted_service_binding(&checked.program, binding)
                .map_err(|diagnostic| vec![diagnostic])?,
        ),
        (Some(binding), _) => {
            return Err(vec![Diagnostic::error(format!(
                "accepted semantic binding {:?} was not consumed by the UEFI x86-64 target",
                binding.role(),
            ))]);
        }
    };
    omega_selected_dispatch::settle_selected_boundary_adapter_dispatch(
        &mut checked.program,
        &checked.selected_provider_plan_facts,
    )?;
    let task_activations = crate::pipeline::task_plans::elaborate_task_activation_plans(
        &checked.program,
        &checked.selected_provider_plan_facts,
        settlement.provider_selection_target,
        settlement.opaque_representation_selections,
    )?;

    Ok(SelectedExecutionSettlementSurface {
        program: checked.program,
        selected_provider_plan_facts: checked.selected_provider_plan_facts,
        selected_provider_grants: checked.selected_provider_grants,
        callback_placements: checked.callback_placements,
        accepted_template_classifications: checked.accepted_template_classifications,
        contract_entailment_stand_downs: checked.contract_entailment_stand_downs,
        selected_provider_provenance: settlement.selected_provider_provenance,
        resolved_semantic_bindings: resolved_console_binding
            .into_iter()
            .chain(resolved_filesystem_binding)
            .chain(resolved_uefi_binding)
            .collect(),
        component_progress,
        task_activations,
    })
}

/// Preliminary package-selection validation needs ordinary checked Psi but no
/// target/provider settlement. Keep that intentionally incomplete observation
/// separate from [`CheckedProgramSurface`], which is final-path complete.
pub(super) fn typed_trees_to_preliminary_checked_trees(
    typed: TypedTrees,
    timings: &mut CompileTimings,
) -> Result<Arc<CheckedProgram>, Vec<Diagnostic>> {
    timings.record(TYPED_TREES_TO_CHECKED_TREES, || {
        let program = psi_typed_trees_to_checked_trees::lower_preliminary_typed_trees(typed)?;
        crate::pipeline::provider_approval::check_boundary_provider_approval(&program)?;
        Ok(Arc::new(program))
    })
}
