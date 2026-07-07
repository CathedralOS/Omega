use crate::pipeline::stages::{
    source_files_to_syntax_trees, symbol_resolved_trees_to_typed_trees,
    syntax_trees_to_symbol_resolved_trees, typed_trees_to_checked_trees,
};
use crate::pipeline::timing::CompileTimings;
use omega_checked_trees::CheckedTrees;
use omega_core::diagnostics::Diagnostic;
use std::path::Path;
use std::sync::Arc;

/// Runs ONLY the four frontend stages (lex/parse -> symbol resolution -> typing ->
/// checking) and returns the in-memory `CheckedTrees` program. No backend lowering,
/// no file output. This is the source-of-truth semantic representation that the
/// reference interpreter (`omega-interpreter`) evaluates as a differential oracle for
/// the native backend.
pub fn compile_to_checked(
    root_path: &Path,
    target_name: Option<&str>,
) -> Result<CheckedTrees, Vec<Diagnostic>> {
    let mut timings = CompileTimings::default();

    let (_source_file_count, mut syntax) =
        source_files_to_syntax_trees(root_path, target_name, &mut timings)?;
    // PLAN-LAID VALUE TYPES (layouts L4), desugar half -- exactly as the full
    // `compile` pipeline does.
    crate::pipeline::generic_instances::desugar_generic_data_instances(&mut syntax.syntax_trees)?;
    let plan_laid_records =
        crate::pipeline::plan_laid::desugar_plan_laid_value_types(&mut syntax.syntax_trees)?;
    let resolved = syntax_trees_to_symbol_resolved_trees(syntax, &mut timings)?;
    let mut typed = symbol_resolved_trees_to_typed_trees(resolved, &mut timings)?;
    // COMPTIME STAGE 1: substitute const-evaluated fixed-array lengths before
    // checking, exactly as the full `compile` pipeline does.
    crate::pipeline::const_lengths::evaluate_const_array_lengths(&mut typed)?;
    // PLAN-LAID VALUE TYPES, plan half: evaluate + validate + record.
    crate::pipeline::plan_laid::compute_plan_laid_layouts(&mut typed, &plan_laid_records)?;
    // WIRE PLANS (mint arc rung 2a): mirror the full pipeline so tests see
    // the same derived plans the codec selection consumes.
    crate::pipeline::wire_plans::compute_wire_plans(&mut typed)?;
    let checked = typed_trees_to_checked_trees(typed, &mut timings)?;

    // `typed_trees_to_checked_trees` wraps the program in an `Arc`; unwrap it for the
    // caller (this is the only owner at this point in the pipeline).
    Ok(Arc::try_unwrap(checked.program).unwrap_or_else(|shared| (*shared).clone()))
}
