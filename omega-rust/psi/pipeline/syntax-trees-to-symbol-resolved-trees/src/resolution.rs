//! Start here: the resolution route.
//!
//! [`resolve`] takes one [`ResolutionRequest`]; [`begin`] prepares the forest
//! and a lowerer, and [`drive`] is the route: translate every root item, then
//! let each owner settle its phase in the one order their preconditions
//! allow. The other operations run the same route for a different product.
//! [`resolve_const_argument_selection`] selects const arguments without
//! synthesis or evaluation. [`prepare_const_initializer_selection`] stops at
//! preparation evidence that grants no typing authority. [`resolve_extension`]
//! resolves a later stratum against a retained base and returns the carrier
//! the typed continuation rebases; see `continuations`. `lowerer` is the
//! working state this route drives.

mod continuations;
pub(crate) mod lowerer;

use crate::preparation::generic_data::constant_selection::ConstantSelection;
use crate::{constant, lowering, preparation, selection};
pub use continuations::{
    ConstInitializerSelection, RebasedSeededSymbolResolvedTrees, SeededSymbolResolvedTrees,
};
use diagnostics::Diagnostic;
use lowerer::{ConstResolutionMode, Lowerer};
use source::SourceMap;
use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;
use syntax_trees::SyntaxTrees;

/// One syntax forest and the custody it resolves under.
pub struct ResolutionRequest<'a> {
    pub syntax: &'a SyntaxTrees,
    /// Source custody for visibility and stratum checks. A source-free forest
    /// has none, and those checks stay permissive for it.
    pub sources: Option<Arc<SourceMap>>,
    pub top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
}

impl<'a> ResolutionRequest<'a> {
    /// A source-free forest with no scoped top-level bindings.
    pub fn new(syntax: &'a SyntaxTrees) -> Self {
        Self {
            syntax,
            sources: None,
            top_level_bindings: Vec::new(),
        }
    }
}

/// A later-stratum forest appended to an exact retained base.
pub struct ExtensionRequest<'a> {
    pub base: SymbolResolvedTrees,
    pub syntax: &'a SyntaxTrees,
    /// Must retain the base's exact source frontier.
    pub sources: Arc<SourceMap>,
    pub top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
}

/// Resolve every name in the forest to its exact declaration.
pub fn resolve(request: ResolutionRequest<'_>) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    let (syntax, lowerer) = begin(request, ConstResolutionMode::Complete)?;
    drive(lowerer, &syntax).map(|(trees, _)| trees)
}

/// Resolve raw index expressions in their authored owners through normal
/// lexical selection. This performs no generic instance synthesis or evaluation;
/// callers must still admit every selected leaf and checked operator meaning.
pub fn resolve_const_argument_selection(
    request: ResolutionRequest<'_>,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    let (syntax, lowerer) = begin(request, ConstResolutionMode::ArgumentSelection)?;
    drive(lowerer, &syntax).map(|(trees, _)| trees)
}

/// Resolve declaration dependencies without inventing provisional values.
/// Computed fixed-integer/Boolean initializers remain authored expression roots
/// with no canonical encoding. Every operator occurrence remains an obligation
/// for ordinary typed selection, not an assertion of builtin execution meaning.
pub fn prepare_const_initializer_selection(
    request: ResolutionRequest<'_>,
) -> Result<ConstInitializerSelection, Vec<Diagnostic>> {
    let forest = request.syntax;
    let (syntax, lowerer) = begin(request, ConstResolutionMode::InitializerSelection)?;
    let (trees, selection) = drive(lowerer, &syntax)?;
    let preparation = ConstInitializerSelection { trees, selection };
    preparation.validate_initializer_leaves(forest)?;
    Ok(preparation)
}

/// Resolve one syntax extension against its retained base while keeping the
/// exact append frontier of every authored-selection occurrence store.
///
/// Existing arenas and symbol tables are consumed and extended in place; no
/// source bytes are read and neither forest is parsed again. The returned
/// carrier is readable, but its trees can enter a later seeded phase only by
/// transactionally rebasing the extension suffix against that phase's exact
/// retained authored-selection ledger.
pub fn resolve_extension(
    request: ExtensionRequest<'_>,
) -> Result<SeededSymbolResolvedTrees, Vec<Diagnostic>> {
    let ExtensionRequest {
        base,
        syntax,
        sources,
        top_level_bindings,
    } = request;
    let authored_selection_frontier = base.authored_selection_extension_frontier();
    let retained_base = base.clone();
    let request = ResolutionRequest {
        syntax,
        sources: Some(sources),
        top_level_bindings,
    };
    let (syntax, mut lowerer) = begin(request, ConstResolutionMode::Complete)?;
    lowerer.seed_resolved_base(base)?;
    let (trees, _) = drive(lowerer, &syntax)?;
    Ok(SeededSymbolResolvedTrees {
        trees,
        authored_selection_frontier,
        retained_base: Box::new(retained_base),
    })
}

/// Prepare the forest under its constant selector and a lowerer that owns the
/// request's custody.
fn begin(
    request: ResolutionRequest<'_>,
    constants: ConstResolutionMode,
) -> Result<(SyntaxTrees, Lowerer), Vec<Diagnostic>> {
    let prepared = preparation::prepare(
        request.syntax,
        request.sources.clone(),
        request.top_level_bindings.clone(),
        constants,
    )?;
    let mut lowerer = Lowerer::new(request.sources, request.top_level_bindings);
    lowerer.constant_selection = Some(prepared.constant_selection);
    lowerer.const_resolution_mode = constants;
    Ok((prepared.syntax, lowerer))
}

/// The route. Each phase is one call into its owner, in the one order their
/// preconditions allow: operator homes need no symbols; constants need the
/// table; the authored-selection ledger needs substituted constants; operator
/// obligations need the ledger; every remaining selection needs all of it;
/// domain homes of token-bearing machines need assigned attached symbols;
/// duplicate machine token bindings compare settled operand identities.
fn drive(
    mut lowerer: Lowerer,
    syntax: &SyntaxTrees,
) -> Result<(SymbolResolvedTrees, ConstantSelection<'static>), Vec<Diagnostic>> {
    // Top-level `let`/`boundary let` declarations parse into their own root
    // collection (`SyntaxTreeRoots::mathematical_definitions`); their lowering
    // is a separate PROOF-CONTRACT-MIGRATION leg. Refuse here — before item
    // lowering — rather than silently dropping a declaration the author wrote.
    if let Some(span) = syntax
        .root_mathematical_definition_handles()
        .first()
        .map(|handle| {
            syntax
                .items
                .mathematical_definition(*handle)
                .name
                .source_span()
        })
    {
        return Err(vec![
            Diagnostic::error(
                "top-level `let`/`boundary let` mathematical declarations are parsed but \
                 symbol resolution for them is not implemented yet (PROOF-CONTRACT-MIGRATION)",
            )
            .with_source_span(span),
        ]);
    }
    lowering::lower_items(&mut lowerer, syntax)?;
    let constant_selection = lowerer.take_constant_selection()?;
    selection::select_operator_homes(&mut lowerer)?;
    crate::symbols::assign(&mut lowerer)?;
    // A token-bearing machine attached to a domain gives that domain its
    // denotation role before the selections below classify predicate-only
    // domains; it needs the attached symbols the assignment just settled.
    lowering::machine::mark_token_bound_domain_homes(&mut lowerer.symbol_resolved_trees);
    constant::finalize(&mut lowerer)?;
    selection::finalize_authored_selections(&mut lowerer)?;
    constant::finalize_operator_obligations(&mut lowerer)?;
    selection::finalize(&mut lowerer)?;
    lowering::machine::reject_duplicate_direct_token_bindings(&lowerer.symbol_resolved_trees)?;
    Ok((lowerer.into_trees(), constant_selection))
}

#[cfg(test)]
mod tests;
