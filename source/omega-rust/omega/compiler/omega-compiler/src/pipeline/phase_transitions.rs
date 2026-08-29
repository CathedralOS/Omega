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
    pub(super) accepted_template_classifications:
        omega_trust_model::AcceptedTemplateClassifications,
    pub(super) contract_entailment_stand_downs: Vec<psi_validation::ContractEntailmentStandDown>,
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
) -> Result<CheckedProgramSurface, Vec<Diagnostic>> {
    timings.record(TYPED_TREES_TO_CHECKED_TREES, || {
        let accepted_template_classifications =
            omega_trust_model::AcceptedTemplateClassifications::capture(&typed);
        let contract_entailment_stand_downs =
            psi_validation::collect_contract_entailment_stand_downs(&typed);
        let program = psi_typed_trees_to_checked_trees::lower_typed_trees(typed)?;
        crate::pipeline::provider_approval::check_boundary_provider_approval(&program)?;
        Ok(CheckedProgramSurface {
            program: Arc::new(program),
            accepted_template_classifications,
            contract_entailment_stand_downs,
        })
    })
}
