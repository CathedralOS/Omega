//! Authored declaration identity for resolved calls and their compiler copies.
//!
//! A machine's implicit entry is an implementation coordinate, not a second
//! authored declaration. Selection reconciliation and constant custody share
//! this exact-owner check; an arbitrary source-less child cannot inherit it.

use source::SourceSpan;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind};

pub(crate) fn call_declaration_source(
    program: &SymbolResolvedTrees,
    target: SymbolHandle,
) -> Result<SourceSpan, String> {
    if let Some(source) = program.symbols.symbol_source_span(target) {
        return Ok(source);
    }
    let symbol = program.symbols.get(target);
    if symbol.kind != SymbolKind::State
        || program.symbols.get(symbol.parent).kind != SymbolKind::Machine
    {
        return Err("source-less call target is not its exact owning entry".to_owned());
    }
    // Symbol assignment publishes machine states in declaration order. The
    // first state is its implicit entry. This hierarchy outlives a prepared
    // machine roster: a retained selection may still name the predecessor's
    // entry after its declaration is replaced by a compiler copy.
    let entry = program
        .symbols
        .child_handles(symbol.parent)
        .and_then(|mut children| {
            children.find(|child| program.symbols.get(*child).kind == SymbolKind::State)
        });
    if entry != Some(target) {
        return Err("source-less call target is not its exact owning entry".to_owned());
    }
    program
        .symbols
        .symbol_source_span(symbol.parent)
        .ok_or_else(|| "implicit call entry lost its authored machine declaration".to_owned())
}
