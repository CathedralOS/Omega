use artifacts::compile_timings::CompileTimings;
use artifacts::compile_timings::{
    SYMBOL_RESOLVED_TREES_TO_TYPED_TREES, SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES,
    TYPED_TREES_TO_CHECKED_TREES,
};
use checked_trees::CheckedTrees as CheckedProgram;
use diagnostics::Diagnostic;
use source_files_to_assembled_syntax::AssembledSyntax;
use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;
use typed_trees::TypedTrees;

/// Checked Psi plus the exact predecessor facts that must be captured before
/// typed ownership moves into checking. This is the output of one phase
/// transition, not source-loading state.
pub(crate) struct CheckedProgramSurface {
    pub(crate) program: Arc<CheckedProgram>,
    pub(crate) selected_provider_plan_facts: effects::SelectedProviderPlanFacts,
    pub(crate) selected_provider_grants: Vec<trust_model::ResolvedAuthoredSelectedProviderGrant>,
    pub(crate) callback_placements: Vec<backend_plan::BoundNominalCallbackPlacement>,
    pub(crate) accepted_template_classifications: trust_model::AcceptedTemplateClassifications,
    pub(crate) contract_entailment_stand_downs: Vec<validation::ContractEntailmentStandDown>,
}

/// Checked semantics after selected execution has been settled in the exact
/// compiler-owned dispatch order. This surface owns the now-closed review
/// provenance alongside every checked-phase sidecar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelectedExecutionSettlementSurface {
    pub(crate) program: Arc<CheckedProgram>,
    pub(crate) dispatch_source_edits: selected_dispatch::SelectedDispatchSourceEdits,
    pub(crate) selected_provider_plan_facts: effects::SelectedProviderPlanFacts,
    pub(crate) selected_provider_grants: Vec<trust_model::ResolvedAuthoredSelectedProviderGrant>,
    pub(crate) callback_placements: Vec<backend_plan::BoundNominalCallbackPlacement>,
    pub(crate) accepted_template_classifications: trust_model::AcceptedTemplateClassifications,
    pub(crate) contract_entailment_stand_downs: Vec<validation::ContractEntailmentStandDown>,
    pub(crate) selected_provider_provenance:
        Vec<provider_planning::SelectedProviderReviewProvenance>,
    pub(crate) resolved_semantic_bindings: Vec<selected_dispatch::ResolvedAcceptedSemanticBinding>,
    pub(crate) component_progress: Option<effects::ComponentProgressManifest>,
    pub(crate) task_activations: task_plans::TaskActivationPlanSet,
}

pub(crate) struct SelectedExecutionSettlementInput<'a> {
    pub(crate) exact_component_progress_root:
        Option<provider_planning::component_progress::ExactComponentProgressRoot<'a>>,
    pub(crate) provider_selection_target: target::NativeTarget,
    pub(crate) selected_target_profile: Option<target::TargetProfile>,
    pub(crate) selected_provider_provenance:
        Vec<provider_planning::SelectedProviderReviewProvenance>,
    pub(crate) opaque_representation_selections:
        &'a [representation_planning::OpaqueRepresentationSelection],
    pub(crate) accepted_console_binding: Option<&'a package_compilation::AcceptedSemanticBinding>,
    pub(crate) accepted_process_exit_binding:
        Option<&'a package_compilation::AcceptedSemanticBinding>,
    pub(crate) accepted_filesystem_binding:
        Option<&'a package_compilation::AcceptedSemanticBinding>,
    pub(crate) accepted_entry_binding: Option<&'a package_compilation::AcceptedSemanticBinding>,
}

/// Final typed settlements that must finish inside the phase transition that
/// produces the checked program surface.
pub(crate) struct TypedToCheckedSettlementInput<'a> {
    pub(crate) native_target: Option<target::NativeTarget>,
    pub(crate) package_inputs: Option<&'a package_compilation::PackageCompilationInputs>,
    pub(crate) selected_build_machine: Option<symbols::SymbolHandle>,
    /// The evaluated `Build.freestanding` selection. The asm authority
    /// discharge is a typed-program validation whose only input outside the
    /// trees is this build.omg fact, which `lower_*` deliberately never sees;
    /// the settlement input carries it so the gate joins this transition's
    /// program-validation pass on the exact graph about to be checked.
    pub(crate) freestanding: bool,
    pub(crate) boundary_calling_plan_realizations:
        &'a mut [provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    pub(crate) opaque_representation_selections:
        &'a [representation_planning::OpaqueRepresentationSelection],
    pub(crate) provider_plans: &'a [effects::provider_plan::ProviderPlan],
    pub(crate) selected_provider_plan_facts: effects::SelectedProviderPlanFacts,
    pub(crate) root_grants: &'a [String],
    pub(crate) authored_root_grants: &'a [trust_model::AuthoredRootGrant],
}

pub(crate) fn syntax_trees_to_symbol_resolved_trees(
    syntax: AssembledSyntax,
    timings: &mut CompileTimings,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    timings.record(SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES, || {
        syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: &syntax.syntax_trees,
                sources: Some(syntax.sources),
                top_level_bindings: syntax.source_scoped_top_level_bindings,
            },
        )
    })
}

pub(crate) fn symbol_resolved_trees_to_seeded_base(
    resolved: SymbolResolvedTrees,
    timings: &mut CompileTimings,
) -> Result<symbol_resolved_trees_to_typed_trees::SeededTypingBase, Vec<Diagnostic>> {
    timings.record(SYMBOL_RESOLVED_TREES_TO_TYPED_TREES, || {
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_to_seeded_base(resolved)
            .map_err(|diagnostic| vec![diagnostic])
    })
}

pub(crate) fn resolve_seeded_syntax_extension(
    base: SymbolResolvedTrees,
    extension: &syntax_trees::SyntaxTrees,
    sources: Arc<source::SourceMap>,
    timings: &mut CompileTimings,
) -> Result<syntax_trees_to_symbol_resolved_trees::SeededSymbolResolvedTrees, Vec<Diagnostic>> {
    timings.record(SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES, || {
        syntax_trees_to_symbol_resolved_trees::resolve_extension(
            syntax_trees_to_symbol_resolved_trees::ExtensionRequest {
                base,
                syntax: extension,
                sources,
                top_level_bindings: Vec::new(),
            },
        )
    })
}

pub(crate) fn type_seeded_extension(
    source: syntax_trees_to_symbol_resolved_trees::RebasedSeededSymbolResolvedTrees,
    base: symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    timings: &mut CompileTimings,
) -> Result<
    TypedTrees,
    (
        symbol_resolved_trees_to_typed_trees::SeededTypingBase,
        symbol_resolved_trees_to_typed_trees::SeededContinuationError,
    ),
> {
    timings.record_result(SYMBOL_RESOLVED_TREES_TO_TYPED_TREES, || {
        symbol_resolved_trees_to_typed_trees::lower_seeded_extension(source, base)
    })
}

pub(crate) fn typed_trees_to_checked_trees(
    typed: TypedTrees,
    timings: &mut CompileTimings,
    settlement: TypedToCheckedSettlementInput<'_>,
) -> Result<CheckedProgramSurface, Vec<Diagnostic>> {
    timings.record(TYPED_TREES_TO_CHECKED_TREES, || {
        let accepted_template_classifications =
            trust_model::AcceptedTemplateClassifications::capture(&typed);
        let contract_entailment_stand_downs =
            validation::collect_contract_entailment_stand_downs(&typed);
        let selected_generic_operator_providers =
            selected_generic_operator_provider_specializations(
                &typed,
                &settlement.selected_provider_plan_facts,
            )?;
        // A selected boundary family row commits its provider to the complete
        // roster now, before checking: the generated tuple bodies are what
        // settle-time row resolution looks up.
        let selected_boundary_families =
            selected_dispatch::selected_boundary_family_specializations(
                &typed,
                &settlement.selected_provider_plan_facts,
            );
        let rederived_opaque_representation_selections =
            representation_planning::rederive_opaque_representation_selections(
                &typed,
                settlement.selected_build_machine,
                settlement.opaque_representation_selections,
            )?;
        let opaque_property_receipts = rederived_opaque_representation_selections
            .iter()
            .filter(|&selection| selection.copy_disposition()
                    == representation_planning::OpaqueRepresentationCopyDisposition::CheckedSemanticCopy ).map(|selection| validation::OpaqueDataPropertyReceipt::copy(selection.opaque()))
            .collect::<Vec<_>>();
        typed_trees_to_checked_trees::validate_asm_discharge(&typed, settlement.freestanding)?;
        let mut program = if settlement.package_inputs.is_some() {
            typed_trees_to_checked_trees::lower_package_typed_trees_with_selected_generic_operator_providers(
                typed,
                &selected_generic_operator_providers,
                &selected_boundary_families,
                &opaque_property_receipts,
            )?
        } else {
            typed_trees_to_checked_trees::lower_typed_trees_with_selected_generic_operator_providers(
                typed,
                &selected_generic_operator_providers,
                &selected_boundary_families,
                &opaque_property_receipts,
            )?
        };
        provider_planning::approval::check_boundary_provider_approval(&program)?;
        if let Some(package_inputs) = settlement.package_inputs {
            crate::package::declaration_admission::validate_authored_declaration_selections(
                &program,
                package_inputs,
            )?;
        }
        if let Some(native_target) = settlement.native_target {
            provider_planning::calling_policy_plans::close_outbound_callback_materializations(
                &mut program,
                settlement.boundary_calling_plan_realizations,
                native_target,
                settlement.opaque_representation_selections,
                settlement.package_inputs,
            )?;
        }
        let callback_placements =
            provider_planning::calling_policy_plans::validate_nominal_callback_placement_bindings(
                &program,
                settlement.boundary_calling_plan_realizations,
            )?;
        let program = Arc::new(program);
        let selected_provider_binding =
            provider_planning::bind_selected_provider_plan_facts(
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
    typed: &typed_trees::TypedTrees,
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<
    Vec<typed_trees_to_checked_trees::SelectedGenericOperatorProviderSpecialization>,
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
                    && typed_trees::operator::boundary_operator_requirement_identity(
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
            effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
        ) {
            continue;
        }
        let provider = match provider_planning::exact_checked_adapter(typed, plan, row) {
            Ok(provider) => provider,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        if typed.machine_type_parameters(provider).is_empty() {
            continue;
        }
        let request = typed_trees_to_checked_trees::SelectedGenericOperatorProviderSpecialization {
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
pub(crate) fn settle_selected_execution(
    mut checked: CheckedProgramSurface,
    mut settlement: SelectedExecutionSettlementInput<'_>,
) -> Result<SelectedExecutionSettlementSurface, Vec<Diagnostic>> {
    let component_progress =
        provider_planning::component_progress::build_selected_component_progress_manifest(
            &checked.program,
            &checked.selected_provider_plan_facts,
            settlement.exact_component_progress_root,
            None,
        )?;
    let dispatch_source_edits =
        selected_dispatch::settle_selected_execution_dispatch_with_source_edits(
            &mut checked.program,
            &checked.selected_provider_plan_facts,
        )?;
    let resolved_exit_bindings =
        selected_dispatch::retain_selected_compiler_intrinsic_review_identities(
            &checked.program,
            &checked.selected_provider_plan_facts,
            &mut settlement.selected_provider_provenance,
            settlement
                .selected_target_profile
                .map(target::TargetProfile::target_name),
            settlement.accepted_console_binding,
            settlement.accepted_process_exit_binding,
        )?;
    let resolved_filesystem_binding = settlement
        .accepted_filesystem_binding
        .map(|binding| {
            selected_dispatch::resolve_accepted_service_binding(&checked.program, binding)
        })
        .transpose()
        .map_err(|diagnostic| vec![diagnostic])?;
    let expected_entry_role = settlement
        .selected_target_profile
        .map(|profile| profile.program_entry_slot())
        .and_then(|slot| slot.physical_contract_package)
        .map(build_evaluation::program_entry_semantic_binding_role);
    let resolved_entry_binding = match (settlement.accepted_entry_binding, expected_entry_role) {
        (None, _) => None,
        (Some(binding), Some(role)) if binding.role() == role => Some(
            selected_dispatch::resolve_accepted_service_binding(&checked.program, binding)
                .map_err(|diagnostic| vec![diagnostic])?,
        ),
        (Some(binding), _) => {
            return Err(vec![Diagnostic::error(format!(
                "accepted semantic binding {:?} was not consumed by the selected target's program-entry contract",
                binding.role(),
            ))]);
        }
    };
    selected_dispatch::settle_selected_boundary_adapter_dispatch(
        &mut checked.program,
        &checked.selected_provider_plan_facts,
    )?;
    let task_activations = provider_planning::task_plans::elaborate_task_activation_plans(
        &checked.program,
        &checked.selected_provider_plan_facts,
        settlement.provider_selection_target,
        settlement.opaque_representation_selections,
    )?;

    Ok(SelectedExecutionSettlementSurface {
        program: checked.program,
        dispatch_source_edits,
        selected_provider_plan_facts: checked.selected_provider_plan_facts,
        selected_provider_grants: checked.selected_provider_grants,
        callback_placements: checked.callback_placements,
        accepted_template_classifications: checked.accepted_template_classifications,
        contract_entailment_stand_downs: checked.contract_entailment_stand_downs,
        selected_provider_provenance: settlement.selected_provider_provenance,
        resolved_semantic_bindings: resolved_exit_bindings
            .into_iter()
            .chain(resolved_filesystem_binding)
            .chain(resolved_entry_binding)
            .collect(),
        component_progress,
        task_activations,
    })
}

/// Preliminary package-selection validation needs ordinary checked Psi but no
/// target/provider settlement. Keep that intentionally incomplete observation
/// separate from [`CheckedProgramSurface`], which is final-path complete.
pub(crate) fn typed_trees_to_preliminary_checked_trees(
    typed: TypedTrees,
    timings: &mut CompileTimings,
) -> Result<Arc<CheckedProgram>, Vec<Diagnostic>> {
    timings.record(TYPED_TREES_TO_CHECKED_TREES, || {
        let program = typed_trees_to_checked_trees::lower_preliminary_typed_trees(typed)?;
        provider_planning::approval::check_boundary_provider_approval(&program)?;
        Ok(Arc::new(program))
    })
}
