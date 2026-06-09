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

    let (_source_file_count, syntax) =
        source_files_to_syntax_trees(root_path, target_name, &mut timings)?;
    let resolved = syntax_trees_to_symbol_resolved_trees(syntax, &mut timings)?;
    let typed = symbol_resolved_trees_to_typed_trees(resolved, &mut timings)?;
    let checked = typed_trees_to_checked_trees(typed, &mut timings)?;

    // `typed_trees_to_checked_trees` wraps the program in an `Arc`; unwrap it for the
    // caller (this is the only owner at this point in the pipeline).
    Ok(Arc::try_unwrap(checked.program).unwrap_or_else(|shared| (*shared).clone()))
}
