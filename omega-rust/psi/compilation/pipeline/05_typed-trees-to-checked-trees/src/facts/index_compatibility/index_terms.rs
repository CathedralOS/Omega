//! Index compatibility borrows the canonical identity owner. Call bindings are
//! simultaneous caller-side values; neither token shape nor diagnostic spelling
//! can identify an operation or license reordering. Checking creates no AST.

use super::{BoundIndexArgument, BoundIndexSubstitutions};
use symbols::SymbolHandle;
use typed_trees::type_identity::{IndexIdentityArgument, normalized_index_identity};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn index_identity_bindings<'bindings>(
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
    actual_substitutions: &[(SymbolHandle, IndexIdentityArgument<'_>)],
    expected_substitutions: &[(SymbolHandle, IndexIdentityArgument<'_>)],
) -> bool {
    normalized_index_identity(
        program,
        IndexIdentityArgument::TypeReference(actual),
        actual_substitutions,
    )
    .zip(normalized_index_identity(
        program,
        IndexIdentityArgument::TypeReference(expected),
        expected_substitutions,
    ))
    .is_some_and(|(actual, expected)| actual == expected)
}

pub(super) fn bound_index_arguments_equal(
    program: &dyn ::validation::CheckingInput,
    actual: &[TypeReferenceHandle],
    expected: &[TypeReferenceHandle],
    actual_substitutions: &BoundIndexSubstitutions,
    expected_substitutions: &BoundIndexSubstitutions,
) -> bool {
    let actual_bindings = index_identity_bindings(actual_substitutions);
    let expected_bindings = index_identity_bindings(expected_substitutions);
    !actual.is_empty()
        && actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(actual, expected)| {
            index_arguments_equal(
                program,
                *actual,
                *expected,
                &actual_bindings,
                &expected_bindings,
            )
        })
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
