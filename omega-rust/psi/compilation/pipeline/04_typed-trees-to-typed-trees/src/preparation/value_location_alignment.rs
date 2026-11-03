//! Settle the subject of a value-form `align_of(place)` query.
//!
//! Typing keeps the authored operand because a place's declared type is known
//! only once locals and parameters resolve in their state. The query observes
//! that place's storage location: a reference observes its referent, never
//! the reference descriptor. Every Psi and Omega consumer then answers the
//! same portable selected-layout alignment request the type form uses, so the
//! location's guaranteed alignment is the validated minimum of the storage
//! type under the default storage rule.
//!
//! The operand is observed once without being read or moved: it leaves the
//! executable call here, so no load, loan or initialization is generated.
//! A value leaf (a call result, literal or arithmetic) owns no location and
//! rejects. A slice owes a proved nonempty view before its first element can
//! be observed; that obligation is not yet checked, so slices reject rather
//! than fabricate a location.

use crate::tables::{expressions_of, expressions_of_mut};
use arena::HandleSpan;
use diagnostics::Diagnostic;
use language_core::data_layout_queries::DataLayoutQueryKind;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use validation::expression_traversal::collect_statement_expression_trees;

pub(super) fn settle(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut settled = Vec::new();
    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let mut handles = Vec::new();
                collect_statement_expression_trees(program, statement, &mut handles);
                for query in handles {
                    let Some(place) = value_query_operand(program, query) else {
                        continue;
                    };
                    if settled.iter().any(|(handle, _)| *handle == query) {
                        continue;
                    }
                    match observed_storage_type(program, state.symbol, statement_index, place) {
                        Ok(subject) => settled.push((query, subject)),
                        Err(message) => diagnostics.push(
                            Diagnostic::error(message)
                                .with_source_span(expressions_of(program).source_span(query)),
                        ),
                    }
                }
            }
        }
    }
    for (query, subject) in settled {
        if let ExpressionNode::Call(call) = expressions_of_mut(program).expression_mut(query)
            && let Some(request) = call.layout_query.as_mut()
        {
            request.operand.type_reference = subject;
            call.arguments = HandleSpan::empty();
        }
    }
    // A query outside an executable statement (a `requires` contract, for
    // example) has no state in which its place was resolved. Reject it with
    // its source rather than let an unsettled request reach lowering.
    let roots: Vec<_> = expressions_of(program)
        .iter_expressions()
        .map(|(root, _)| root)
        .collect();
    let mut unsettled = Vec::new();
    for root in roots {
        let mut handles = Vec::new();
        validation::expression_traversal::collect_expression_tree(program, root, &mut handles);
        unsettled.extend(
            handles
                .into_iter()
                .filter(|query| value_query_operand(program, *query).is_some()),
        );
    }
    unsettled.sort_by_key(|query| (query.arena_index(), query.generation()));
    unsettled.dedup();
    diagnostics.extend(unsettled.into_iter().map(|query| {
        Diagnostic::error(
            "`align_of(value)` is answered only in executable statements; a contract or other non-statement position is not yet supported",
        )
        .with_source_span(expressions_of(program).source_span(query))
    }));
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// A value-form query keeps exactly one authored operand until settled here.
fn value_query_operand(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Call(call) = expressions_of(program).expression(expression) else {
        return None;
    };
    let request = call.layout_query.as_ref()?;
    if request.kind != DataLayoutQueryKind::Alignment || request.operand.type_reference.is_valid() {
        return None;
    }
    match expressions_of(program).expression_handles(call.arguments) {
        [operand] => Some(*operand),
        _ => None,
    }
}

/// The storage type whose selected layout bounds the observed location.
fn observed_storage_type(
    program: &TypedTrees,
    state: symbols::SymbolHandle,
    statement_index: usize,
    place: ExpressionHandle,
) -> Result<TypeReferenceHandle, &'static str> {
    // Only a local or parameter names storage; a constant is a value and a
    // value leaf (call result, literal, arithmetic) owns no location.
    let canonical =
        validation::canonical_place_from_expression_in_state(program, state, statement_index, place)
            .filter(|canonical| {
                canonical.root.as_symbol().is_some_and(|root| {
                    matches!(
                        program.symbols.get(root).kind,
                        symbols::SymbolKind::Local | symbols::SymbolKind::Parameter
                    )
                })
            })
            .ok_or(
                "`align_of(value)` observes an addressable storage place; this operand is a value with no storage location",
            )?;
    // A field inside an explicit representation (`LayoutStorage<T, Policy>`)
    // sits wherever that policy placed it, so the field type's default
    // minimum is no guarantee for it. Reject rather than overstate it.
    for prefix in 0..canonical.segments.len() {
        let enclosing = validation::CanonicalPlace {
            root: canonical.root.clone(),
            segments: canonical.segments[..prefix].to_vec(),
        };
        let through_representation =
            validation::canonical_place_type_reference(program, state, statement_index, &enclosing)
                .map(|enclosing| referent(program, enclosing))
                .is_some_and(|enclosing| program.layout_storage_arguments(enclosing).is_some());
        if through_representation {
            return Err(
                "`align_of(value)` on a field of an explicit layout representation needs that policy's placement, which is not yet observed",
            );
        }
    }
    let declared =
        validation::expression_type_reference_in_state(program, state, statement_index, place)
            .ok_or("`align_of(value)` requires an operand whose storage type is known")?;
    // A reference observes its referent's location, not its descriptor.
    let subject = referent(program, declared);
    if matches!(
        program.type_reference_table.type_reference(subject),
        TypeReferenceNode::Slice { .. }
    ) {
        return Err(
            "`align_of(value)` on a slice observes its first element and requires a proved nonempty view, which is not yet checked",
        );
    }
    Ok(subject)
}

fn referent(program: &TypedTrees, reference: TypeReferenceHandle) -> TypeReferenceHandle {
    let reference = unconstrained(program, reference);
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { referee, .. } => unconstrained(program, *referee),
        _ => reference,
    }
}

fn unconstrained(program: &TypedTrees, mut reference: TypeReferenceHandle) -> TypeReferenceHandle {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    reference
}
