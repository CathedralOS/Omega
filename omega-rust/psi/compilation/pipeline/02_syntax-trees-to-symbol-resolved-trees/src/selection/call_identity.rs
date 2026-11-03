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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolution::{ResolutionRequest, resolve};

    #[test]
    fn evidence_custody_distinguishes_methods_without_merging_local_children() {
        let text = r#"
trait Reading {
    machine first() -> u64;
    machine last() -> u64;
}
trait Applying {
    machine apply<Operations satisfies Reading>() -> u64;
}
machine observed<First satisfies Reading, Second satisfies Reading>() -> u64 {
    First::first()
}
"#;
        let mut sources = source::SourceMap::default();
        let source = sources
            .add("custody/main.omg".into(), text.into())
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap();
        let program = resolve(ResolutionRequest {
            syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .unwrap();
        let machine = &program.machines[0];
        let first = program
            .symbols
            .child_handles(machine.conformance_bounds[0].binder)
            .unwrap()
            .collect::<Vec<_>>();
        let second = program
            .symbols
            .child_handles(machine.conformance_bounds[1].binder)
            .unwrap()
            .collect::<Vec<_>>();
        assert_eq!(first.len(), 2);
        assert_eq!(second.len(), 2);
        assert_ne!(first[0], second[0]);
        let first_source = call_custody_source(&program, first[0]).unwrap();
        let last_source = call_custody_source(&program, first[1]).unwrap();
        assert_ne!(first_source, last_source);
        assert_eq!(
            first_source,
            call_custody_source(&program, second[0]).unwrap()
        );
        assert_eq!(
            last_source,
            call_custody_source(&program, second[1]).unwrap()
        );
        assert!(call_declaration_source(&program, first[0]).is_err());
        assert!(call_declaration_source(&program, second[0]).is_err());
        let method = &program.trait_machine_signatures(program.traits[1].machines)[0];
        let method_children = program
            .symbols
            .child_handles(method.conformance_bounds[0].binder)
            .unwrap()
            .collect::<Vec<_>>();
        assert_eq!(method_children.len(), 2);
        assert_eq!(
            first_source,
            call_custody_source(&program, method_children[0]).unwrap()
        );
        assert_eq!(
            last_source,
            call_custody_source(&program, method_children[1]).unwrap()
        );
    }
}
