//! Proof-side observations retain exact current places without executable reads.
//! Immutable shared descriptors may name their original referent; immutable
//! closed constructors may supply their stored scalar leaf. Neither operation
//! copies a referent's mutable initializer, crosses a recast, changes the
//! authored call actual, or creates an invocation from a captured call result.
//! Source type/loan/scope/precondition validation remains the admission owner.

use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::TypeReferenceNode;

pub(crate) fn resolve(
    program: &dyn ::validation::CheckingInput,
    state_symbol: symbols::SymbolHandle,
    before_statement: u32,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let state = program.state_by_symbol(state_symbol)?;
    let mut current = expression;
    let mut visited = Vec::new();
    loop {
        if visited.contains(&current)
            || visited.len() >= program.expression_table.expression_count()
        {
            return None;
        }
        visited.push(current);
        // Canonical place lookup intentionally peels recasts for alias geometry.
        // Proof observation cannot reuse that address identity as value identity.
        if !static_place_syntax(program, current) {
            return Some(current);
        }
        let Some(place) = validation::canonical_place_from_expression(program, current) else {
            return Some(current);
        };
        let Some(symbol) = place.root.as_symbol() else {
            return Some(current);
        };
        let Some((ordinal, local)) = program.state_local_declaration(state, symbol) else {
            return Some(current);
        };
        if ordinal >= before_statement as usize || !local.initial_value.is_valid() {
            return None;
        }
        if local.is_mutable {
            return Some(current);
        }
        if place.segments.is_empty() {
            if program.primitive_type_reference(local.type_reference).is_some() {
                // A closed immutable scalar needs no backing read. Captured calls
                // and other non-closed initializers retain their original SSA term.
                return Some(validation::closed_scalar_place_projection(
                    program, local.initial_value, local.type_reference, &[],
                ).map_or(current, |(leaf, _)| leaf));
            }
            let TypeReferenceNode::Reference { access, .. } = program
                .type_reference_table
                .type_reference(local.type_reference)
            else {
                return Some(current);
            };
            let ExpressionNode::Borrow(borrow) =
                program.expression_table.expression(local.initial_value)
            else {
                return Some(current);
            };
            if *access != language_core::ReferenceAccess::Shared || borrow.access != *access {
                return None;
            }
            current = borrow.target;
            continue;
        }
        let (leaf, _) = validation::closed_scalar_place_projection(
            program,
            local.initial_value,
            local.type_reference,
            &place.segments,
        )?;
        return Some(leaf);
    }
}

fn static_place_syntax(
    program: &dyn ::validation::CheckingInput,
    mut expression: ExpressionHandle,
) -> bool {
    for _ in 0..program.expression_table.expression_count() {
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(_) => return true,
            ExpressionNode::Member(member) => expression = member.receiver,
            // The canonical owner independently normalizes a fixed selection.
            // The closed-value projector rejects dynamic selectors below.
            ExpressionNode::Indexed(indexed) => expression = indexed.collection,
            _ => return false,
        }
    }
    false
}
