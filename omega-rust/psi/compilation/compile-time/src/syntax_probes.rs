//! Resolve and normalize temporary syntax with the originating loader's custody.

use std::sync::Arc;

mod evaluation;
pub(crate) use evaluation::PreparedEvaluationInputs;
pub(crate) use evaluation::{prepare_evaluation, prepare_owned_probe_evaluation};

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
    syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_generic_data(
        normalization_request(
            syntax_trees,
            sources,
            source_scoped_top_level_bindings,
            retained_base,
        ),
    )
}

/// Exact probes and their selected dependency owners own normalization work. Unready
/// surrounding applications remain selected lookup/signature context, never
/// provisional aggregate const-index identities. The ordinary complete route
/// above still normalizes and validates every authored application.
pub(crate) fn normalize_initializer_probe_data(
    syntax_trees: syntax_trees::SyntaxTrees,
    sources: Option<Arc<source::SourceMap>>,
    source_scoped_top_level_bindings: &[symbols::SourceScopedTopLevelBinding],
    retained_base: Option<&symbol_resolved_trees::SymbolResolvedTrees>,
    declarations: &[syntax_trees::item::ItemHandle],
) -> Result<syntax_trees::SyntaxTrees, Vec<diagnostics::Diagnostic>> {
    syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_initializer_probe_data(
        normalization_request(
            syntax_trees,
            sources,
            source_scoped_top_level_bindings,
            retained_base,
        ),
        declarations,
    )
}

/// Rejoin exact selected declaration coordinates to this private syntax clone.
/// Missing or ambiguous ownership cannot expand the normalization domain.
pub(crate) fn extend_normalization_owners(
    syntax: &syntax_trees::SyntaxTrees,
    declarations: &mut Vec<syntax_trees::item::ItemHandle>,
    owners: &[source::SourceSpan],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    for owner in owners {
        let matches = syntax
            .root_item_handles()
            .iter()
            .copied()
            .filter(|item| {
                let mut machines = Vec::new();
                let name = match syntax.root_item(*item) {
                    syntax_trees::item::Item::Const(definition) => Some(&definition.name),
                    syntax_trees::item::Item::Data(definition) => Some(&definition.name),
                    syntax_trees::item::Item::Machine(definition) => {
                        machines.push(definition);
                        None
                    }
                    syntax_trees::item::Item::Conformance(definition) => {
                        for member in syntax.items.conformance_members(definition.members) {
                            match member {
                                syntax_trees::item::ConformanceMember::Machine(machine)
                                | syntax_trees::item::ConformanceMember::Synthesized(machine)
                                | syntax_trees::item::ConformanceMember::TraitDefault {
                                    machine,
                                    ..
                                } => machines.push(machine),
                                _ => {}
                            }
                        }
                        definition.alias.as_ref()
                    }
                    _ => None,
                };
                if name.is_some_and(|name| name.source_span() == *owner) {
                    return true;
                }
                // Normalize the exact containing root, including its static
                // context; duplicated coordinates still fail uniqueness below.
                while let Some(machine) = machines.pop() {
                    if machine.name.source_span() == *owner {
                        return true;
                    }
                    machines.extend(
                        syntax
                            .items
                            .nested_machine_items(machine.nested_machines)
                            .iter()
                            .filter_map(syntax_trees::item::Item::as_machine),
                    );
                }
                false
            })
            .collect::<Vec<_>>();
        let [item] = matches.as_slice() else {
            return Err(vec![
                diagnostics::Diagnostic::error(
                    "initializer normalization lost exact declaration ownership",
                )
                .with_source_span(*owner),
            ]);
        };
        if !declarations.contains(item) {
            declarations.push(*item);
        }
    }
    Ok(())
}

fn normalization_request<'base>(
    syntax: syntax_trees::SyntaxTrees,
    sources: Option<Arc<source::SourceMap>>,
    bindings: &[symbols::SourceScopedTopLevelBinding],
    retained_base: Option<&'base symbol_resolved_trees::SymbolResolvedTrees>,
) -> syntax_trees_to_symbol_resolved_trees::pre_resolution::GenericDataRequest<'base> {
    let has_sources = sources.is_some();
    syntax_trees_to_symbol_resolved_trees::pre_resolution::GenericDataRequest {
        syntax,
        sources,
        top_level_bindings: if has_sources {
            bindings.to_vec()
        } else {
            Vec::new()
        },
        retained_base: if has_sources { retained_base } else { None },
    }
}
