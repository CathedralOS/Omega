/*
    Declaration sources for resolved calls and their compiler copies.

    Constant evaluation retains which declarations its calls selected. A local
    evidence child has no source token of its own, so that record names its
    public method while the call retains the exact child and binder. A generated
    machine entry similarly uses its owning machine's declaration source.
*/

use crate::source_resolved::SymbolResolvedTrees;
use source_text::SourceSpan;
use symbols::{SymbolHandle, SymbolKind};

/*
    Copies of a declaration selection must still name the same declaration.
    Evidence methods use their exact bound requirement and generated entries
    use their exact owning machine. Ordinary declarations keep their own
    source; selected binders and complete applications remain separate.
*/
pub(crate) fn selected_declaration_source(
    program: &SymbolResolvedTrees,
    target: SymbolHandle,
) -> Result<SourceSpan, String> {
    if let Some(requirement) =
        crate::resolution::names::requirement_namespaces::evidence_requirement(program, target)?
    {
        return program
            .symbols
            .symbol_source_span(requirement.symbol)
            .ok_or_else(|| {
                "constant evidence requirement lost its public declaration source".into()
            });
    }
    let symbol = program.symbols.get(target);
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
    // Copied selections can still name an inactive incarnation of this
    // machine. Compare against that exact owner's retained children.
    let entry = crate::resolution::names::select_machine_entry(
        program.symbols.name(symbol.parent),
        |name| {
            program.symbols.child_handles(symbol.parent)?.find(|child| {
                program.symbols.get(*child).kind == SymbolKind::State
                    && program.symbols.name(*child) == name
            })
        },
    );
    if entry != Some(target) {
        return Err("source-less call target is not its exact owning entry".to_owned());
    }
    program
        .symbols
        .symbol_source_span(symbol.parent)
        .ok_or_else(|| "implicit call entry lost its authored machine declaration".to_owned())
}
