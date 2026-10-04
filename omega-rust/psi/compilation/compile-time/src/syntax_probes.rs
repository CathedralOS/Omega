//! Resolve and normalize temporary syntax with the originating loader's custody.

use std::sync::Arc;

mod evaluation;
pub(crate) use evaluation::prepare_evaluation;
pub(crate) use evaluation::{PreparedEvaluationInputs, prepare_evaluation_inputs};

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
