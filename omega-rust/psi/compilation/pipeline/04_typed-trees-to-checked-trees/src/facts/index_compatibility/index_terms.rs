//! Index compatibility borrows the canonical identity owner. Call bindings are
//! simultaneous caller-side values; neither token shape nor diagnostic spelling
//! can identify an operation or license reordering. Checking creates no AST.

use super::{BoundIndexArgument, BoundIndexSubstitutions};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::type_identity::{IndexIdentityArgument, normalized_index_identity};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn index_identity_bindings<'bindings>(
    substitutions: &'bindings BoundIndexSubstitutions<'_>,
) -> Vec<(SymbolHandle, IndexIdentityArgument<'bindings>)> {
    substitutions
        .iter()
        .map(|(symbol, value)| {
            (
                *symbol,
                match value {
                    BoundIndexArgument::Subject(subject) => {
                        IndexIdentityArgument::Subject(*subject)
                    }
                    BoundIndexArgument::Reference(reference) => {
                        IndexIdentityArgument::TypeReference(*reference)
                    }
                    BoundIndexArgument::Algebra(argument) => {
                        IndexIdentityArgument::StaticArgument(argument)
                    }
                    BoundIndexArgument::Expression(expression) => {
                        IndexIdentityArgument::Expression(*expression)
                    }
                },
            )
        })
        .collect()
}

pub(super) fn index_arguments_equal(
    program: &dyn ::validation::CheckingInput,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, IndexIdentityArgument<'_>)],
) -> bool {
    normalized_index_identity(
        program,
        IndexIdentityArgument::TypeReference(actual),
        substitutions,
    )
    .zip(normalized_index_identity(
        program,
        IndexIdentityArgument::TypeReference(expected),
        substitutions,
    ))
    .is_some_and(|(actual, expected)| actual == expected)
}

pub(super) fn bound_index_arguments_equal(
    program: &dyn ::validation::CheckingInput,
    actual: &[TypeReferenceHandle],
    expected: &[TypeReferenceHandle],
    substitutions: &BoundIndexSubstitutions,
) -> bool {
    let bindings = index_identity_bindings(substitutions);
    !actual.is_empty()
        && actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| index_arguments_equal(program, *actual, *expected, &bindings))
}

pub(super) fn is_closed_index_argument(
    program: &dyn ::validation::CheckingInput,
    argument: TypeReferenceHandle,
    substitutions: &BoundIndexSubstitutions,
) -> bool {
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(argument)
    else {
        return false;
    };
    if let Some((_, bound)) = substitutions.iter().find(|(binder, _)| *binder == *symbol) {
        return match bound {
            BoundIndexArgument::Reference(reference) => {
                is_closed_index_argument(program, *reference, &Vec::new())
            }
            _ => false,
        };
    }
    language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str()).is_some()
        || name.as_str().parse::<i128>().is_ok()
}

/// The replacement search needs a caller-owned expression, not another copy of
/// the callee's spelling. Traverse structural edges without selecting an algebra.
pub(crate) fn expression_uses_binder(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    substitutions: &BoundIndexSubstitutions,
) -> bool {
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() || visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        if let ExpressionNode::Name(path) = node {
            let symbol = if path.symbol.is_valid() {
                path.symbol
            } else {
                path.head_symbol
            };
            if symbol.is_valid() && substitutions.iter().any(|(binder, _)| *binder == symbol) {
                return true;
            }
        }
        validation::expression_traversal::children(program, node, |child| pending.push(child));
    }
    false
}

pub(crate) fn bound_index_expressions_equal(
    program: &dyn ::validation::CheckingInput,
    actual: ExpressionHandle,
    expected: ExpressionHandle,
    substitutions: &BoundIndexSubstitutions,
) -> bool {
    let bindings = index_identity_bindings(substitutions);
    normalized_index_identity(
        program,
        IndexIdentityArgument::Expression(actual),
        &bindings,
    )
    .zip(normalized_index_identity(
        program,
        IndexIdentityArgument::Expression(expected),
        &bindings,
    ))
    .is_some_and(|(actual, expected)| actual == expected)
}
