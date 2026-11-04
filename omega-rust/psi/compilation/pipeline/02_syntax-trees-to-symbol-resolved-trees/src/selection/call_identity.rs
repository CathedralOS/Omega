//! Declaration sources for resolved calls and their compiler copies.
//!
//! Constant evaluation retains which declarations its calls selected. A local
//! evidence child has no source token of its own, so that record names its
//! public method while the call retains the exact child and binder. A generated
//! machine entry similarly uses its owning machine's declaration source.

use source::SourceSpan;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind};

/// A constant must retain each call's declared method after evaluation. Record
/// the public method's source because the local evidence child has no source
/// declaration of its own. Keep that exact child as the separate call target;
/// invocation checking still verifies its binder and complete application.
pub(crate) fn call_custody_source(
    program: &SymbolResolvedTrees,
    target: SymbolHandle,
) -> Result<SourceSpan, String> {
    if let Some(requirement) =
        crate::symbols::requirement_namespaces::evidence_requirement(program, target)?
    {
        return program
            .symbols
            .symbol_source_span(requirement.symbol)
            .ok_or_else(|| {
                "constant evidence requirement lost its public declaration source".into()
            });
    }
    let symbol = program.symbols.get(target);
    // Other logical calls can be generated from an authored binder. Its
    // recorded origin identifies source custody; the exact call target remains
    // separate and still owes ordinary invocation checks.
    if matches!(
        symbol.kind,
        SymbolKind::Operator | SymbolKind::MachineParameter
    ) || (symbol.kind == SymbolKind::State
        && symbol.parent.is_valid()
        && program.symbols.get(symbol.parent).kind == SymbolKind::Trait)
    {
        return program
            .symbols
            .symbol_provenance_source_span(target)
            .ok_or_else(|| "constant logical call lost its exact authored provenance".into());
    }
    call_declaration_source(program, target)
}

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
