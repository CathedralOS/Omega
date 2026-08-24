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

pub use access_plans::{compute_access_plan, compute_placement_plan};
pub use admission::BuildTimeAdmissionPlan;
pub use build_machines::{
    BuildMachineEvaluationError, BuildMachineExecutionMode, BuildMachineFilesystemAccess,
    BuildMachineFilesystemGrants, BuildMachineFilesystemSponsor, PreparedBuildMachineProgram,
    evaluate_build_machine_arguments_measured,
};
pub use const_domain_facts::evaluate_const_domain_facts;
pub use const_generic_calls::evaluate_const_generic_calls;
pub use const_lengths::{evaluate_const_array_lengths, evaluate_zero_argument_machine};
pub use layout_plans::{
    BuildTimeValue, compute_layout_plan, evaluate_and_materialize_typed_owned_layout_into,
    materialize_typed_owned_layout_into, normalized_schema_identity,
};
pub use placed_views::{PlacedViewRecord, desugar_placed_views, validate_placed_view_plans};
pub use plan_laid::{PlanLaidRecord, compute_plan_laid_layouts, desugar_plan_laid_value_types};
pub use wire_plans::compute_wire_plans;

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
    let mut syntax_trees = evaluate_const_generic_calls(syntax_trees)?;
    psi_syntax_trees_to_symbol_resolved_trees::synthesize_trait_defaults(&mut syntax_trees)?;
    let placed_view_records = desugar_placed_views(&mut syntax_trees)?;
    let mut syntax_trees = psi_generic_instances::normalize_pre_resolution(syntax_trees)?;
    let plan_laid_records = desugar_plan_laid_value_types(&mut syntax_trees)?;
    Ok(PreResolutionEvaluation {
        syntax_trees,
        placed_view_records,
        plan_laid_records,
    })
}

/// Apply the target-neutral build-time services that normalize typed trees
/// before checking. Target ABI/provider realization deliberately follows this
/// entry in Omega.
pub fn evaluate_pre_check(
    typed: &mut psi_typed_trees::TypedTrees,
    plan_laid_records: &[PlanLaidRecord],
    placed_view_records: &[PlacedViewRecord],
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    evaluate_const_array_lengths(typed)?;
    evaluate_const_domain_facts(typed)?;
    compute_plan_laid_layouts(typed, plan_laid_records)?;
    validate_placed_view_plans(typed, placed_view_records)?;
    compute_wire_plans(typed)
}
