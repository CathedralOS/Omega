//! Constant qualification is an ordinary checked result judgment.
//!
//! Encoding a value cannot establish its domain, and a syntax-only arithmetic
//! evaluator cannot interpret the full checked mathematical predicate language.
//! A completed initializer therefore returns through a private probe with its
//! authored destination. The existing checker owns construction, mathematical
//! substitution, total formation and issuer obligations. Nothing executes here.
//! Only successful judgments return to the exact declaration; private unqualified
//! copies never leave preparation or supply qualification evidence themselves.

use std::sync::Arc;

use diagnostics::Diagnostic;
use source::SourceMap;
use symbols::SourceScopedTopLevelBinding;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{ConstQualification, Item};
use syntax_trees::types::TypeReferenceNode;

pub(crate) fn prove_ready(
    mut syntax: SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
    bindings: &[SourceScopedTopLevelBinding],
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let candidates = syntax
        .root_item_handles()
        .iter()
        .filter_map(|item| {
            let Item::Const(definition) = syntax.root_item(*item) else {
                return None;
            };
            definition
                .qualification
                .is_none()
                .then_some((*item, definition.clone()))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(syntax);
    }

    let mut probe = syntax.clone();
    defer_unproved(&mut probe);
    let selection =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::prepare_const_initializer_selection(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: probe.clone(),
                sources: sources.clone(),
                top_level_bindings: bindings.to_vec(),
            },
        )?;
    let mut pending = Vec::new();
    for (item, mut definition) in candidates {
        // The initializer owner distinguishes completed case names from pending
        // const references and computations. Syntactic `Name` is not readiness.
        if selection.pending_leaves(&syntax, &definition)?.is_empty()
            && (definition.pending_qualification.is_some()
                || selection.initializer_requires_destination_proof(&syntax, &definition))
        {
            let expected = syntax_trees::item::ConstDestinationObligation {
                declaration: definition.name.source_span(),
                destination: definition.type_reference,
                initializer: definition.value,
            };
            if definition
                .pending_qualification
                .as_ref()
                .is_some_and(|actual| *actual != expected)
            {
                return Err(vec![
                    Diagnostic::error(
                        "constant destination obligation drifted from its declaration",
                    )
                    .with_source_span(definition.name.source_span()),
                ]);
            }
            definition.pending_qualification = Some(expected);
            syntax
                .items
                .replace_item(item, Item::Const(definition.clone()));
            let Item::Const(mut private) = probe.root_item(item).clone() else {
                return Err(vec![Diagnostic::error(
                    "constant destination probe lost its declaration",
                )]);
            };
            private.pending_qualification = definition.pending_qualification.clone();
            probe.items.replace_item(item, Item::Const(private));
            pending.push((item, definition));
        }
    }
    if pending.is_empty() {
        return Ok(syntax);
    }
    let declaration_frontier = probe.root_item_handles().len();
    let owners = pending
        .iter()
        .enumerate()
        .map(|(ordinal, (_, definition))| {
            super::const_generic_expressions::append_named_probe(
                &mut probe,
                syntax_trees::identifier::Identifier::new(
                    format!(
                        "@const-qualification-{}-{ordinal}",
                        definition.name.as_str(&syntax)
                    ),
                    syntax.expressions.source_span(definition.value),
                ),
                definition.value,
                definition.type_reference,
            )
        })
        .collect::<Vec<_>>();
    let mut probe_declarations = probe.root_item_handles()[declaration_frontier..].to_vec();
    for (_, definition) in &pending {
        let dependencies = selection.initializer_dependencies(&syntax, definition)?;
        crate::syntax_probes::extend_normalization_owners(
            &probe,
            &mut probe_declarations,
            &dependencies.normalization_owners,
        )?;
    }
    let probe = crate::syntax_probes::normalize_initializer_probe_data(
        probe,
        sources.clone(),
        bindings,
        None,
        &probe_declarations,
    )?;
    let frontier =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::resolve_deferred_initializer_probe(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: probe,
                sources,
                top_level_bindings: bindings.to_vec(),
            },
        )?;
    let owners = owners
        .iter()
        .map(|owner| owner.resolve(frontier.trees()))
        .collect::<Result<Vec<_>, _>>()?;
    let prepared = crate::syntax_probes::prepare_owned_probe_evaluation(frontier, &owners, &[])?;
    let program = prepared.domain.execution_program(&prepared.typed);
    let request = typed_trees_to_typed_trees::PreparationRequest::settled();
    let (program, preparation) =
        typed_trees_to_typed_trees::prepare_typed_trees(program, &request)?;
    typed_trees_to_checked_trees::lower_typed_trees(
        program,
        preparation,
        &typed_trees_to_checked_trees::CheckingRequest::new(),
    )?;

    for (item, mut definition) in pending {
        definition.pending_qualification = None;
        definition.qualification = Some(ConstQualification {
            destination: definition.type_reference,
            initializer: definition.value,
        });
        syntax.items.replace_item(item, Item::Const(definition));
    }
    Ok(syntax)
}

/// An unfinished declaration contributes its carrier to private typing, never
/// the unproved qualification. Completed declarations retain their checked
/// judgment so later initializer calls may consume their actual facts.
pub(crate) fn defer_unproved(syntax: &mut SyntaxTrees) {
    for item in syntax.root_item_handles().to_vec() {
        let Item::Const(mut definition) = syntax.root_item(item).clone() else {
            continue;
        };
        if definition
            .qualification
            .as_ref()
            .is_some_and(|qualification| {
                qualification.destination == definition.type_reference
                    && qualification.initializer == definition.value
            })
        {
            continue;
        }
        definition.qualification = None;
        while let TypeReferenceNode::Constrained { base_type, .. } = syntax
            .type_references
            .type_reference(definition.type_reference)
        {
            definition.type_reference = *base_type;
        }
        syntax.items.replace_item(item, Item::Const(definition));
    }
}
