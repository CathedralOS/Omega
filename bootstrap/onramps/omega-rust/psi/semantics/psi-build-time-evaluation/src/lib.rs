#![forbid(unsafe_code)]

//! Target-neutral admission and execution of compile-time Omega machines.

mod access_plans;
mod admission;
mod build_machines;
mod const_domain_facts;
mod const_generic_calls;
mod const_lengths;
mod layout_plans;
mod placed_views;
mod plan_laid;
mod wire_plans;

use std::sync::Arc;

pub use access_plans::{compute_access_plan, compute_placement_plan};
pub use admission::{
    BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeSelectionAuthority,
};
pub use build_machines::{
    BuildMachineEvaluationError, BuildMachineExecutionMode, BuildMachineFilesystemAccess,
    BuildMachineFilesystemGrantRoot, BuildMachineFilesystemGrantRootIdentity,
    BuildMachineFilesystemGrants, BuildMachineFilesystemMetadataLayout,
    BuildMachineFilesystemSponsor, PreparedBuildMachineProgram,
    evaluate_build_machine_arguments_measured,
};
pub use const_domain_facts::{
    evaluate_const_domain_facts, evaluate_const_domain_facts_with_authority,
};
pub use const_generic_calls::evaluate_const_generic_calls;
pub use const_lengths::{
    evaluate_const_array_lengths, evaluate_const_array_lengths_with_authority,
    evaluate_zero_argument_machine, evaluate_zero_argument_machine_for_invocation,
};
pub use layout_plans::{
    BuildTimeValue, compute_layout_plan, compute_layout_plan_with_authority,
    evaluate_and_materialize_typed_owned_layout_into, materialize_typed_owned_layout_into,
    normalized_schema_identity,
};
pub use placed_views::{
    PlacedViewRecord, desugar_placed_views, validate_placed_view_plans,
    validate_placed_view_plans_with_authority,
};
pub use plan_laid::{
    PlanLaidRecord, compute_plan_laid_layouts, compute_plan_laid_layouts_with_authority,
    desugar_plan_laid_value_types,
};
pub use wire_plans::{compute_wire_plans, compute_wire_plans_with_authority};

/// Target-neutral syntax elaboration that must finish before name resolution.
///
/// Target selection remains an Omega orchestration concern and may run on the
/// returned syntax after this service has finished owning language-level
/// elaboration.
pub struct PreResolutionEvaluation {
    pub syntax_trees: psi_syntax_trees::SyntaxTrees,
    pub placed_view_records: Vec<PlacedViewRecord>,
    pub plan_laid_records: Vec<PlanLaidRecord>,
}

pub fn evaluate_pre_resolution(
    syntax_trees: psi_syntax_trees::SyntaxTrees,
) -> Result<PreResolutionEvaluation, Vec<psi_diagnostics::Diagnostic>> {
    evaluate_pre_resolution_with_optional_sources(syntax_trees, None, None)
}

/// Package-aware pre-resolution evaluation.
///
/// Probe compilations must retain the same source/package custody as the
/// authoritative compilation. Otherwise a compile-time machine selected from
/// dependency source loses its owner before the execution-admission gate can
/// inspect it.
pub fn evaluate_pre_resolution_with_sources(
    syntax_trees: psi_syntax_trees::SyntaxTrees,
    sources: Arc<psi_source::SourceMap>,
) -> Result<PreResolutionEvaluation, Vec<psi_diagnostics::Diagnostic>> {
    evaluate_pre_resolution_with_optional_sources(syntax_trees, Some(sources), None)
}

pub fn evaluate_pre_resolution_with_sources_and_authority(
    syntax_trees: psi_syntax_trees::SyntaxTrees,
    sources: Arc<psi_source::SourceMap>,
    selection_authority: Arc<dyn BuildTimeSelectionAuthority>,
) -> Result<PreResolutionEvaluation, Vec<psi_diagnostics::Diagnostic>> {
    evaluate_pre_resolution_with_optional_sources(
        syntax_trees,
        Some(sources),
        Some(selection_authority),
    )
}

fn evaluate_pre_resolution_with_optional_sources(
    syntax_trees: psi_syntax_trees::SyntaxTrees,
    sources: Option<Arc<psi_source::SourceMap>>,
    selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
) -> Result<PreResolutionEvaluation, Vec<psi_diagnostics::Diagnostic>> {
    let mut syntax_trees = const_generic_calls::evaluate_const_generic_calls_with_optional_sources(
        syntax_trees,
        sources.clone(),
        selection_authority.clone(),
    )?;
    psi_syntax_trees_to_symbol_resolved_trees::synthesize_trait_defaults(&mut syntax_trees)?;
    let placed_view_records = placed_views::desugar_placed_views_with_optional_sources(
        &mut syntax_trees,
        sources,
        selection_authority,
    )?;
    let mut syntax_trees = psi_generic_instances::normalize_pre_resolution(syntax_trees)?;
    let plan_laid_records = desugar_plan_laid_value_types(&mut syntax_trees)?;
    Ok(PreResolutionEvaluation {
        syntax_trees,
        placed_view_records,
        plan_laid_records,
    })
}

fn lower_probe_with_optional_sources(
    syntax_trees: &psi_syntax_trees::SyntaxTrees,
    sources: Option<Arc<psi_source::SourceMap>>,
) -> Result<psi_symbol_resolved_trees::SymbolResolvedTrees, Vec<psi_diagnostics::Diagnostic>> {
    match sources {
        Some(sources) => {
            psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
                syntax_trees,
                sources,
            )
        }
        None => psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(syntax_trees),
    }
}

/// Apply the target-neutral build-time services that normalize typed trees
/// before checking. Target ABI/provider realization deliberately follows this
/// entry in Omega.
pub fn evaluate_pre_check(
    typed: &mut psi_typed_trees::TypedTrees,
    plan_laid_records: &[PlanLaidRecord],
    placed_view_records: &[PlacedViewRecord],
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    evaluate_pre_check_with_optional_authority(typed, plan_laid_records, placed_view_records, None)
}

pub fn evaluate_pre_check_with_authority(
    typed: &mut psi_typed_trees::TypedTrees,
    plan_laid_records: &[PlanLaidRecord],
    placed_view_records: &[PlacedViewRecord],
    selection_authority: Arc<dyn BuildTimeSelectionAuthority>,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    evaluate_pre_check_with_optional_authority(
        typed,
        plan_laid_records,
        placed_view_records,
        Some(selection_authority),
    )
}

fn evaluate_pre_check_with_optional_authority(
    typed: &mut psi_typed_trees::TypedTrees,
    plan_laid_records: &[PlanLaidRecord],
    placed_view_records: &[PlacedViewRecord],
    selection_authority: Option<Arc<dyn BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    evaluate_const_array_lengths_with_authority(typed, selection_authority.clone())?;
    evaluate_const_domain_facts_with_authority(typed, selection_authority.clone())?;
    compute_plan_laid_layouts_with_authority(
        typed,
        plan_laid_records,
        selection_authority.clone(),
    )?;
    validate_placed_view_plans_with_authority(
        typed,
        placed_view_records,
        selection_authority.clone(),
    )?;
    compute_wire_plans_with_authority(typed, selection_authority)
}

#[cfg(test)]
mod tests {
    use super::lower_probe_with_optional_sources;
    use psi_core::PackageKeyIdentity;
    use psi_source::{SourceMap, SourceOrigin};
    use psi_source_files_to_tokens::Lexer;
    use psi_tokens_to_syntax_trees::parse_syntax_trees_with_id;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn package_aware_probe_retains_authored_symbol_ownership() {
        let source = "machine selected() {}";
        let package =
            PackageKeyIdentity::from_digest([0x6a; 32]).expect("nonzero package identity");
        let mut sources = SourceMap::default();
        let source_id = sources
            .add_with_metadata(
                PathBuf::from("cache/selected/main.omg"),
                source.to_owned(),
                PathBuf::from("cache/selected"),
                Some(package),
                SourceOrigin::User,
            )
            .source_id;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees_with_id(source_id, &tokens).expect("parse");

        let resolved = lower_probe_with_optional_sources(&syntax, Some(Arc::new(sources)))
            .expect("package-aware probe resolution");
        let machine = resolved.machines.first().expect("selected machine");

        assert_eq!(
            resolved.symbols.symbol_package_identity(machine.symbol),
            Some(package)
        );
    }
}
