use artifacts::compile_timings::CompileTimings;
use artifacts::compile_timings::{
    SYMBOL_RESOLVED_TREES_TO_TYPED_TREES, SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES,
    TYPED_TREES_TO_PRELIMINARY_CHECKED_TREES,
};
use build_evaluation::sources::AssembledSyntax;
use checked_trees::CheckedTrees as CheckedProgram;
use diagnostics::Diagnostic;
use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;
use typed_trees::TypedTrees;

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

/// Preliminary package-selection validation needs ordinary checked Psi but no
/// target/provider settlement. Keep that intentionally incomplete observation
/// separate from final provider realization.
///
/// Its result cannot serve the settled check's input either. Between this
/// checkpoint and settled checking the admitted build may
/// append generated source, target selection replaces canonical bodies with
/// the selected target's own, provider settlement records its erasure
/// authorizations, and pre-check const folds land the endpoints and opaque
/// receipts only this preliminary mode tolerates pending. The settled
/// request also carries the specialization rosters this checkpoint leaves
/// empty and checks the target siblings this pass prunes first — different
/// program, different request — so each compile pays for both checkpoints
/// and only the exact-input memo serves the repeats a later review pass
/// issues for an identical frozen graph.
pub(crate) fn typed_trees_to_preliminary_checked_trees(
    typed: TypedTrees,
    timings: &mut CompileTimings,
    authority: &dyn validation::DeclarationSelectionAuthority,
) -> Result<Arc<CheckedProgram>, Vec<Diagnostic>> {
    timings.record(TYPED_TREES_TO_PRELIMINARY_CHECKED_TREES, || {
        let check_started = std::time::Instant::now();
        let check_result = typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::preliminary()
                .with_declaration_authority(Some(authority)),
        );
        super::record_preliminary_check(check_started.elapsed());
        let program = check_result?;
        let program = typed_trees_to_checked_trees::prune_target_siblings(program)?;
        Ok(Arc::new(program))
    })
}
