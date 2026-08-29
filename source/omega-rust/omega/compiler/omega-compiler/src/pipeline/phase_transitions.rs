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
    pub(super) callback_placements: Vec<omega_backend_plan::BoundNominalCallbackPlacement>,
    pub(super) accepted_template_classifications:
        omega_trust_model::AcceptedTemplateClassifications,
    pub(super) contract_entailment_stand_downs: Vec<psi_validation::ContractEntailmentStandDown>,
}

/// Final typed settlements that must finish inside the phase transition that
/// produces the checked program surface.
pub(super) struct TypedToCheckedSettlementInput<'a> {
    pub(super) native_target: Option<omega_target::NativeTarget>,
    pub(super) package_inputs: Option<&'a crate::pipeline::PackageCompilationInputs>,
    pub(super) boundary_calling_plan_realizations:
        &'a mut [crate::pipeline::calling_policy_plans::BoundaryCallingPlanRealization],
    pub(super) provider_plans: &'a [omega_effects::provider_plan::ProviderPlan],
    pub(super) selected_provider_plan_facts: omega_effects::SelectedProviderPlanFacts,
    pub(super) root_grants: &'a [String],
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

pub(super) fn symbol_resolved_trees_to_typed_trees(
    resolved: SymbolResolvedTrees,
    timings: &mut CompileTimings,
) -> Result<TypedTrees, Vec<Diagnostic>> {
    timings.record(SYMBOL_RESOLVED_TREES_TO_TYPED_TREES, || {
        psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_owned(resolved)
            .map_err(|diagnostic| vec![diagnostic])
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
        let mut program = psi_typed_trees_to_checked_trees::lower_typed_trees(typed)?;
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
            )?;
        let (program, selected_provider_plan_facts) = selected_provider_binding.into_parts();
        Ok(CheckedProgramSurface {
            program,
            selected_provider_plan_facts,
            callback_placements,
            accepted_template_classifications,
            contract_entailment_stand_downs,
        })
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
        let program = psi_typed_trees_to_checked_trees::lower_typed_trees(typed)?;
        crate::pipeline::provider_approval::check_boundary_provider_approval(&program)?;
        Ok(Arc::new(program))
    })
}
