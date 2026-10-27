//! Constant qualification is an ordinary checked result judgment.
//!
//! Encoding a value cannot establish its domain, and a syntax-only arithmetic
//! evaluator cannot interpret the full checked mathematical predicate language.
//! A completed initializer therefore returns through a private probe with its
//! authored destination. The existing checker owns construction, mathematical
//! substitution, total formation and issuer obligations. Nothing executes here.
//! Only successful judgments return to the exact declaration; private unqualified
//! copies never leave preparation or supply qualification evidence themselves.

use diagnostics::Diagnostic;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{ConstQualification, Item};
use syntax_trees::types::TypeReferenceNode;

pub(crate) fn prove_ready(
    mut syntax: SyntaxTrees,
    context: &crate::BuildTimeSources<'_>,
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
            context.resolution_request(probe.clone()),
        )?;
    let mut pending = Vec::new();
    for (item, mut definition) in candidates {
        // The initializer owner distinguishes completed case names from pending
        // const references and computations. Syntactic `Name` is not readiness.
        if (definition.pending_qualification.is_some()
            || selection.initializer_requires_destination_proof(&syntax, &definition))
            && selection.pending_leaves(&syntax, &definition)?.is_empty()
            && destination_inputs_ready(&selection, &syntax, &definition)?
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
        context.sources.clone(),
        context.source_scoped_top_level_bindings,
        context.retained_base(),
        &probe_declarations,
    )?;
    let frontier =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::resolve_deferred_initializer_probe(
            context.resolution_request(probe),
        )?;
    let owners = owners
        .iter()
        .map(|owner| owner.resolve(frontier.trees()))
        .collect::<Result<Vec<_>, _>>()?;
    let prepared = crate::syntax_probes::prepare_owned_probe_evaluation(
        frontier,
        &owners,
        &[],
        context.predecessor.clone(),
    )?;
    let program = prepared.domain.execution_program(&prepared.typed);
    let request = typed_trees_to_typed_trees::PreparationRequest::settled();
    let (program, preparation) =
        typed_trees_to_typed_trees::prepare_typed_trees(program, &request)?;
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        program,
        preparation,
        &typed_trees_to_checked_trees::CheckingRequest::new(),
    )?;

    // A conditional checker receipt is not a completed destination judgment.
    // Reuse the shared demanded-closure fence, including returned type owners;
    // unrelated pending declarations remain outside each exact probe root.
    for (owner, (_, definition)) in owners.iter().zip(&pending) {
        require_complete_destination_judgment(&checked, *owner, definition.name.source_span())?;
    }

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

/// Literal elements do not settle the destination's geometry. The existing
/// declaration dependency owner includes exact named extents, generic actuals
/// and predicate dependencies; none may be replaced by a provisional value.
/// A retained completed payload is readiness only, never destination proof.
fn destination_inputs_ready(
    selection: &syntax_trees_to_symbol_resolved_trees::pre_resolution::ConstInitializerSelection,
    syntax: &SyntaxTrees,
    definition: &syntax_trees::item::ConstDefinition,
) -> Result<bool, Vec<Diagnostic>> {
    for (_, source) in selection
        .initializer_dependencies(syntax, definition)?
        .constants
    {
        let dependency = selection
            .trees()
            .const_declaration_at_source_span(source)
            .ok_or_else(|| {
                vec![
                    Diagnostic::error("constant destination dependency lost its exact declaration")
                        .with_source_span(source),
                ]
            })?;
        if dependency.evaluated_value.is_none() {
            return Ok(false);
        }
    }
    Ok(true)
}

/// This is the publication boundary for a completed destination probe. Keeping
/// it separate from receipt mutation makes conditional checked products directly
/// testable without inventing a second proof or completeness implementation.
fn require_complete_destination_judgment(
    checked: &checked_trees::CheckedTrees,
    owner: symbols::SymbolHandle,
    declaration: source::SourceSpan,
) -> Result<(), Vec<Diagnostic>> {
    if checked.typed.machine_by_symbol(owner).is_none() {
        return Err(vec![
            Diagnostic::error("constant qualification lost its exact checked probe root")
                .with_source_span(declaration),
        ]);
    }
    crate::machine_execution::admission::require_complete_checked_conditions(checked, owner)
        .map_err(|reason| {
            vec![
                Diagnostic::error(format!(
                    "constant destination qualification remains conditional: {reason}",
                ))
                .with_source_span(declaration),
            ]
        })
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

#[cfg(test)]
mod tests {
    use super::require_complete_destination_judgment;
    use checked_trees::{CheckFacts, CheckedTrees, IntegerRangeObligation, IntegerRangeRecipe};
    use numerics::bignum::BigInt;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::machine::Machine;

    #[test]
    fn destination_receipt_requires_its_exact_complete_checked_probe() {
        let probe = SymbolHandle::from_parts(1, 1);
        let unrelated = SymbolHandle::from_parts(2, 1);
        let missing = SymbolHandle::from_parts(3, 1);
        let mut program = TypedTrees::default();
        for symbol in [probe, unrelated] {
            program.push_machine(Machine {
                symbol,
                ..Default::default()
            });
        }
        let mut checked = CheckedTrees::with_roots(program, CheckFacts::default());
        let declaration = source::SourceSpan::default();
        assert!(require_complete_destination_judgment(&checked, probe, declaration).is_ok());
        assert!(require_complete_destination_judgment(&checked, missing, declaration).is_err());

        // Both owners are real retained declarations. An unrelated conditional
        // judgment supplies no premise and does not block this exact probe.
        checked
            .facts
            .integer_range_obligations
            .push(IntegerRangeObligation::retain(
                ExpressionHandle::invalid(),
                unrelated,
                IntegerRangeRecipe::bounds(BigInt::from_i64(0), BigInt::from_i64(1)),
                BigInt::from_i64(0),
                BigInt::from_i64(1),
                "unrelated retained target condition".to_owned(),
            ));
        assert!(require_complete_destination_judgment(&checked, probe, declaration).is_ok());
        // The same checked condition owned by the destination probe makes the
        // product conditional. It cannot reach the caller's drain/stamp block.
        checked
            .facts
            .integer_range_obligations
            .push(IntegerRangeObligation::retain(
                ExpressionHandle::invalid(),
                probe,
                IntegerRangeRecipe::bounds(BigInt::from_i64(0), BigInt::from_i64(1)),
                BigInt::from_i64(0),
                BigInt::from_i64(1),
                "destination retained target condition".to_owned(),
            ));
        assert!(require_complete_destination_judgment(&checked, probe, declaration).is_err());
    }
}
