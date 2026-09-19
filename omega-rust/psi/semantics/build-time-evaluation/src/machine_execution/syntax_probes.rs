//! Resolve and normalize temporary syntax with the originating loader's custody.

use std::sync::Arc;

/// A provisional index supplies a carrier, not membership in the indexed domain.
/// Remove only affected constant qualifications in this private probe forest.
/// The authoritative declarations keep every constraint and are checked after
/// their real indices materialize; no probe qualification is exported as proof.
pub(crate) fn defer_pending_const_qualifications(
    probe: &mut syntax_trees::SyntaxTrees,
    arguments: &[syntax_trees::types::TypeReferenceHandle],
) {
    use syntax_trees::item::Item;
    use syntax_trees::types::{TypeConstraintNode, TypeReferenceNode};

    if arguments.is_empty() {
        return;
    }
    for item in probe.root_item_handles().to_vec() {
        let Item::Const(mut definition) = probe.root_item(item).clone() else {
            continue;
        };
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = probe
            .type_references
            .type_reference(definition.type_reference)
            .clone()
        else {
            continue;
        };
        let retained = probe
            .type_references
            .constraints(constraints)
            .iter()
            .filter(|constraint| {
                let TypeConstraintNode::Domain(domain) = constraint else {
                    return true;
                };
                !probe
                    .type_references
                    .type_reference_handles(domain.arguments)
                    .iter()
                    .any(|argument| arguments.contains(argument))
            })
            .cloned()
            .collect::<Vec<_>>();
        if retained.len() == constraints.len() {
            continue;
        }
        definition.type_reference = if retained.is_empty() {
            base_type
        } else {
            let retained = probe.type_references.insert_constraints(retained);
            probe
                .type_references
                .insert_constrained(base_type, retained)
        };
        probe.items.replace_item(item, Item::Const(definition));
    }
}

pub(crate) fn normalize_generic_data(
    syntax_trees: syntax_trees::SyntaxTrees,
    sources: Option<Arc<source::SourceMap>>,
    source_scoped_top_level_bindings: &[symbols::SourceScopedTopLevelBinding],
    retained_base: Option<&symbol_resolved_trees::SymbolResolvedTrees>,
) -> Result<syntax_trees::SyntaxTrees, Vec<diagnostics::Diagnostic>> {
    // Scoped bindings and a retained base travel with the source context;
    // a source-free probe has neither.
    let has_sources = sources.is_some();
    syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_generic_data(
        syntax_trees_to_symbol_resolved_trees::pre_resolution::GenericDataRequest {
            syntax: syntax_trees,
            sources,
            top_level_bindings: if has_sources {
                source_scoped_top_level_bindings.to_vec()
            } else {
                Vec::new()
            },
            retained_base: if has_sources { retained_base } else { None },
        },
    )
}

pub(crate) fn resolve(
    syntax_trees: &syntax_trees::SyntaxTrees,
    sources: Option<Arc<source::SourceMap>>,
    source_scoped_top_level_bindings: &[symbols::SourceScopedTopLevelBinding],
) -> Result<symbol_resolved_trees::SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    match sources {
        Some(sources) => syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: syntax_trees,
                sources: Some(sources),
                top_level_bindings: source_scoped_top_level_bindings.to_vec(),
            },
        ),
        None => syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax_trees),
        ),
    }
}

#[cfg(test)]
mod tests {
    use syntax_trees::item::Item;
    use syntax_trees::types::{TypeConstraintNode, TypeReferenceNode};

    #[test]
    fn pending_index_probe_retains_unrelated_constant_qualifications() {
        let tokens = source_files_to_tokens::Lexer::new(
            "domain<const Enabled: bool> u64::Gate<Enabled> requires Enabled;
             domain u64::Small requires self < 3;
             const VALUE: u64 in Gate<(!false)> & Small = 7;
             const OTHER: u64 in Small = 7;",
        )
        .tokenize()
        .expect("probe tokens");
        let original = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("probe syntax");
        let arguments = syntax_trees_to_symbol_resolved_trees::pre_resolution::closed_data_const_argument_expressions(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&original),
        ).expect("exact domain index discovery");
        assert_eq!(arguments.len(), 1);
        let declarations = original
            .root_item_handles()
            .iter()
            .filter_map(|item| match original.root_item(*item) {
                Item::Const(definition) => Some((*item, definition.type_reference)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut probe = original.clone();
        super::defer_pending_const_qualifications(&mut probe, &[arguments[0].0]);
        let Item::Const(value) = probe.root_item(declarations[0].0) else {
            panic!("VALUE")
        };
        let TypeReferenceNode::Constrained { constraints, .. } =
            probe.type_references.type_reference(value.type_reference)
        else {
            panic!("Small remains required")
        };
        let [TypeConstraintNode::Domain(domain)] = probe.type_references.constraints(*constraints)
        else {
            panic!("only pending Gate was deferred")
        };
        assert_eq!(domain.name.as_str(), "Small");
        let Item::Const(other) = probe.root_item(declarations[1].0) else {
            panic!("OTHER")
        };
        assert_eq!(
            other.type_reference, declarations[1].1,
            "unaffected owner stays unchanged"
        );
        let TypeReferenceNode::Constrained { constraints, .. } =
            original.type_references.type_reference(declarations[0].1)
        else {
            panic!("original qualifications")
        };
        assert_eq!(
            constraints.len(),
            2,
            "publication still owes both original qualifications"
        );
    }
}
