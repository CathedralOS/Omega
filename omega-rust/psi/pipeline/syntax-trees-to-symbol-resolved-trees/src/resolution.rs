//! Start here: the resolution route.
//!
//! Every public entry below prepares one syntax forest and hands it to
//! [`resolve`], which runs the phases in order: `module_normalization`
//! validates, `trait_defaults` synthesizes default machines, `lowering`
//! translates each root item into the carrier while the `Lowerer` collects
//! pending selections, and [`finish`] assigns symbols and settles those
//! selections through `symbols`, `constant`, and `selection`. The extension
//! entries run the same route against a retained base and return a carrier
//! the typed continuation rebases; see `continuations`. `lowerer` is the
//! working state this route drives.

mod continuations;
pub(crate) mod lowerer;

use crate::lowering::item::lower_item;
pub use continuations::{
    ConstInitializerSelection, RebasedSeededSymbolResolvedTrees, SeededSymbolResolvedTrees,
};
use diagnostics::Diagnostic;
use lowerer::{ConstResolutionMode, Lowerer, RootWatermarks};
use source::SourceMap;
use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;
use syntax_trees::SyntaxTrees;

pub fn lower_syntax_trees(
    syntax_trees: &SyntaxTrees,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    resolve(
        syntax_trees,
        None,
        Vec::new(),
        ConstResolutionMode::Complete,
    )
    .map(|prepared| prepared.trees)
}

pub fn lower_syntax_trees_with_sources(
    syntax_trees: &SyntaxTrees,
    sources: Arc<SourceMap>,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    resolve(
        syntax_trees,
        Some(sources),
        Vec::new(),
        ConstResolutionMode::Complete,
    )
    .map(|prepared| prepared.trees)
}

pub fn lower_syntax_trees_with_sources_and_top_level_bindings(
    syntax_trees: &SyntaxTrees,
    sources: Arc<SourceMap>,
    bindings: Vec<symbols::SourceScopedTopLevelBinding>,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    resolve(
        syntax_trees,
        Some(sources),
        bindings,
        ConstResolutionMode::Complete,
    )
    .map(|prepared| prepared.trees)
}

/// Resolve raw index expressions in their authored owners through normal
/// lexical selection. This performs no generic instance synthesis or evaluation;
/// callers must still admit every selected leaf and checked operator meaning.
pub fn lower_syntax_trees_for_const_argument_selection(
    syntax: &SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
    bindings: Vec<symbols::SourceScopedTopLevelBinding>,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    resolve(
        syntax,
        sources,
        bindings,
        ConstResolutionMode::ArgumentSelection,
    )
    .map(|prepared| prepared.trees)
}

/// Resolve declaration dependencies without inventing provisional values.
/// Computed fixed-integer/Boolean initializers remain authored expression roots
/// with no canonical encoding. Every operator occurrence remains an obligation
/// for ordinary typed selection, not an assertion of builtin execution meaning.
pub fn lower_syntax_trees_for_const_initializer_selection(
    syntax: &SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
    bindings: Vec<symbols::SourceScopedTopLevelBinding>,
) -> Result<ConstInitializerSelection, Vec<Diagnostic>> {
    let preparation = resolve(
        syntax,
        sources,
        bindings,
        ConstResolutionMode::InitializerSelection,
    )?;
    for definition in syntax.root_items().filter_map(|item| match item {
        syntax_trees::item::Item::Const(definition)
            if crate::constant::requires_const_initializer_evaluation(syntax, definition) =>
        {
            Some(definition)
        }
        _ => None,
    }) {
        preparation
            .trees
            .const_declarations
            .iter()
            .find(|declaration| {
                preparation
                    .trees
                    .symbols
                    .symbol_source_span(declaration.symbol)
                    == Some(definition.name.source_span())
            })
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "initializer preparation lost its exact declaration",
                )]
            })?;
        preparation.pending_leaves(syntax, definition)?;
    }
    Ok(preparation)
}

/// Append one already-parsed later-stratum syntax forest to an exact retained
/// symbol-resolved base. Existing arenas and symbol tables are consumed and
/// extended in place; no source bytes are read and neither forest is parsed
/// again.
pub fn lower_syntax_extension_against_resolved_base(
    base: SymbolResolvedTrees,
    extension_syntax: &SyntaxTrees,
    sources: Arc<SourceMap>,
    additional_source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    lower_syntax_extension_with_authored_selection_frontier(
        base,
        extension_syntax,
        sources,
        additional_source_scoped_top_level_bindings,
    )
    .map(SeededSymbolResolvedTrees::into_unrebased_trees)
}

/// Resolve one syntax extension while retaining the exact append frontier of
/// every authored-selection occurrence store.
///
/// The returned carrier is readable, but its trees can enter a later seeded
/// phase only by transactionally rebasing the extension suffix against that
/// phase's exact retained authored-selection ledger.
pub fn lower_syntax_extension_with_authored_selection_frontier(
    base: SymbolResolvedTrees,
    extension_syntax: &SyntaxTrees,
    sources: Arc<SourceMap>,
    additional_source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
) -> Result<SeededSymbolResolvedTrees, Vec<Diagnostic>> {
    let constant_selection =
        crate::preparation::generic_data::constant_selection::ConstantSelection::new(
            extension_syntax,
            Some(sources.clone()),
            additional_source_scoped_top_level_bindings.clone(),
        )?;
    crate::preparation::module_normalization::validate_with_selection(
        extension_syntax,
        &constant_selection,
    )?;
    let retained_sources = base.symbols.source_files().collect::<Vec<_>>();
    if retained_sources.len() > sources.len()
        || !retained_sources
            .iter()
            .copied()
            .eq(sources.files().take(retained_sources.len()))
    {
        return Err(vec![Diagnostic::error(
            "seeded symbol resolution source map does not retain the exact base frontier",
        )]);
    }
    let authored_selection_frontier = base.authored_selection_extension_frontier();
    let retained_base = base.clone();
    let roots = RootWatermarks::capture(&base);
    let retained_service_reaches = base.service_reaches.clone();
    let retained_service_reach_rows = base.service_reach_rows.clone();
    let mut syntax_trees = extension_syntax.clone();
    crate::preparation::trait_defaults::synthesize_trait_defaults_after_module_validation(
        &mut syntax_trees,
        &constant_selection,
    )?;
    let mut lowerer = Lowerer::new(Some(sources), additional_source_scoped_top_level_bindings);
    lowerer.constant_selection = Some(constant_selection);
    lowerer.seed_resolved_base(base);

    for item in syntax_trees.root_items() {
        lower_item(&mut lowerer, &syntax_trees, item).map_err(|diagnostic| vec![diagnostic])?;
    }
    for selection in &mut lowerer.pending_const_selections {
        selection.declaration_ordinal = selection
            .declaration_ordinal
            .checked_add(roots.const_declarations)
            .expect("seeded const declaration ordinal overflow");
    }

    finish(
        lowerer,
        FinishMode::Seeded {
            roots,
            retained_service_reaches,
            retained_service_reach_rows,
        },
    )
    .map(|trees| SeededSymbolResolvedTrees {
        trees,
        authored_selection_frontier,
        retained_base: Box::new(retained_base),
    })
}

/// Resolve one forest: rewrite syntax before any symbol exists, translate
/// every root item into the carrier, then settle everything left pending.
fn resolve(
    syntax_trees: &SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
    source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    const_resolution_mode: ConstResolutionMode,
) -> Result<ConstInitializerSelection, Vec<Diagnostic>> {
    let constant_selection =
        crate::preparation::generic_data::constant_selection::ConstantSelection::new(
            syntax_trees,
            sources.clone(),
            source_scoped_top_level_bindings.clone(),
        )?;
    crate::preparation::module_normalization::validate_with_const_resolution_mode(
        syntax_trees,
        &constant_selection,
        const_resolution_mode,
    )?;
    let mut syntax_trees = syntax_trees.clone();
    crate::preparation::trait_defaults::synthesize_trait_defaults_after_module_validation(
        &mut syntax_trees,
        &constant_selection,
    )?;
    let mut lowerer = Lowerer::new(sources, source_scoped_top_level_bindings);
    lowerer.constant_selection = Some(constant_selection);
    lowerer.const_resolution_mode = const_resolution_mode;

    for item in syntax_trees.root_items() {
        lower_item(&mut lowerer, &syntax_trees, item).map_err(|diagnostic| vec![diagnostic])?;
    }

    let selection = lowerer.constant_selection.take().ok_or_else(|| {
        vec![Diagnostic::error(
            "constant preparation lost its source-aware selector",
        )]
    })?;
    let trees = finish(lowerer, FinishMode::Complete)?;
    Ok(ConstInitializerSelection { trees, selection })
}

enum FinishMode {
    Complete,
    Seeded {
        roots: RootWatermarks,
        retained_service_reaches: language_semantics::ServiceReachTable,
        retained_service_reach_rows: language_semantics::ServiceReachRowTable,
    },
}

/// Assign symbols and settle every pending selection, in the one order the
/// passes' preconditions allow.
fn finish(
    mut lowerer: Lowerer,
    finish_mode: FinishMode,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    crate::selection::domain_operator_homes::normalize_domain_operator_homes(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.namespace_declarations,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    match &finish_mode {
        FinishMode::Complete => crate::symbols::assign_symbols(
            &mut lowerer.symbol_resolved_trees,
            lowerer.sources.take(),
            std::mem::take(&mut lowerer.source_scoped_top_level_bindings),
            &lowerer.pending_const_declarations,
            &lowerer.namespace_declarations,
        )?,
        FinishMode::Seeded { roots, .. } => {
            let sources = lowerer.sources.take().ok_or_else(|| {
                vec![Diagnostic::error(
                    "seeded symbol resolution requires retained source custody",
                )]
            })?;
            crate::symbols::assign_symbols_against_resolved_base(
                &mut lowerer.symbol_resolved_trees,
                sources,
                std::mem::take(&mut lowerer.source_scoped_top_level_bindings),
                *roots,
                &lowerer.pending_const_declarations,
                &lowerer.namespace_declarations,
            )?;
        }
    }
    crate::symbols::normalize_static_module_calls(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_static_module_calls,
        &lowerer.pending_static_module_statement_calls,
    );
    crate::lowering::state::finalize_outcome_specific_contract_symbols(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_outcome_specific_contracts,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    crate::constant::finalize_const_declarations(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_const_declarations,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    crate::constant::finalize_const_argument_selections(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_const_argument_selections,
        &lowerer.pending_const_argument_slots,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    crate::symbols::assign_constant_expression_symbols(
        &mut lowerer.symbol_resolved_trees,
        lowerer
            .pending_const_values
            .iter()
            .copied()
            .chain(lowerer.pending_const_argument_expressions.iter().copied()),
    );
    crate::selection::authored_selections::finalize_constant_expression_selections(
        &mut lowerer.symbol_resolved_trees,
        lowerer
            .pending_const_values
            .iter()
            .copied()
            .chain(lowerer.pending_const_argument_expressions.iter().copied()),
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    crate::constant::initializer_normalization::finalize(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_const_initializers,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    crate::constant::substitute_resolved_constants(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_authored_expressions,
        &mut lowerer.pending_const_selections,
        lowerer.const_resolution_mode != ConstResolutionMode::Complete,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    crate::constant::finalize_const_selections(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_const_selections,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    crate::selection::authored_selections::finalize_authored_expression_selections(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_authored_expressions,
        &lowerer.pending_authored_proof_memberships,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    crate::constant::initializer_normalization::finalize_operator_obligations(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_const_initializers,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let compatibility =
        crate::selection::signature_free_requirements::validate_signature_free_requirement_compatibility(
            &lowerer.symbol_resolved_trees,
        );
    if !compatibility.is_empty() {
        return Err(compatibility);
    }
    crate::selection::machine_parameter_requirements::normalize_nominal_machine_parameter_requirements(
        &mut lowerer.symbol_resolved_trees,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    crate::selection::machine_parameter_requirements::normalize_trait_machine_requirement_arguments(
        &mut lowerer.symbol_resolved_trees,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    crate::selection::evidence_forwardings::bind_evidence_forwarding_owners(
        &mut lowerer.symbol_resolved_trees,
    );
    let (pending_machine_service_reaches, pending_signature_service_reaches) =
        lowerer.pending_service_reaches();
    crate::selection::conformance_blocks::normalize_closed_conformance_blocks(
        &mut lowerer.symbol_resolved_trees,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    crate::selection::authored_selections::finalize_conformance_reference_selections(
        &mut lowerer.symbol_resolved_trees,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    crate::selection::domain_establishment::normalize_domain_establishment_routes(
        &mut lowerer.symbol_resolved_trees,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    match finish_mode {
        FinishMode::Complete => crate::selection::service_reaches::normalize_service_reaches(
            &mut lowerer.symbol_resolved_trees,
            &pending_machine_service_reaches,
            &pending_signature_service_reaches,
        ),
        FinishMode::Seeded {
            retained_service_reaches,
            retained_service_reach_rows,
            ..
        } => crate::selection::service_reaches::normalize_service_reaches_with_retained_tables(
            &mut lowerer.symbol_resolved_trees,
            &pending_machine_service_reaches,
            &pending_signature_service_reaches,
            retained_service_reaches,
            retained_service_reach_rows,
        ),
    }
    .map_err(|diagnostic| vec![diagnostic])?;
    lowerer.symbol_resolved_trees.rebuild_tables();
    crate::selection::conformance_blocks::route_inline_member_calls(
        &mut lowerer.symbol_resolved_trees,
    );
    Ok(finished_trees(lowerer.symbol_resolved_trees))
}

/// Rebuild the tables from the lowered roots. The interned semantic rows and
/// domains built during lowering survive the rebuild.
fn finished_trees(lowered: SymbolResolvedTrees) -> SymbolResolvedTrees {
    let SymbolResolvedTrees {
        roots,
        tables,
        symbols,
        service_reaches,
        service_reach_rows,
        authored_service_reach_rows,
        semantic_domains,
        external_bindings,
        evidence_forwardings,
    } = lowered;

    let mut trees = SymbolResolvedTrees::with_roots(roots, tables, symbols);
    trees.service_reaches = service_reaches;
    trees.service_reach_rows = service_reach_rows;
    trees.authored_service_reach_rows = authored_service_reach_rows;
    trees.semantic_domains = semantic_domains;
    trees.external_bindings = external_bindings;
    trees.evidence_forwardings = evidence_forwardings;
    trees
}

#[cfg(test)]
mod tests;
