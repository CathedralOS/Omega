//! Callable result types come from their exact retained declaration. Contextual
//! primitive/static typing remains with the ordinary source metadata query.

use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

fn declared_or_contextual(
    program: &dyn validation::CheckingInput,
    expression: ExpressionHandle,
    contextual: impl FnOnce() -> Option<TypeReferenceHandle>,
) -> Option<TypeReferenceHandle> {
    match program
        .typed_program()
        .declared_callable_expression_type_reference(expression)
    {
        Ok(Some(reference)) => Some(reference),
        Ok(None) => contextual(),
        // The legacy contextual query is not a recovery path for a stale row.
        // Its Option interface records a failed type query, never a type.
        Err(_) => None,
    }
}

pub(crate) fn expression_result_type_reference(
    program: &dyn validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    declared_or_contextual(program, expression, || {
        validation::expression_result_type_reference(program, machine, state, expression)
    })
}

pub(crate) fn parameter_expression_result_type_reference(
    program: &dyn validation::CheckingInput,
    owner: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    declared_or_contextual(program, expression, || {
        validation::parameter_expression_result_type_reference(
            program, owner, parameters, expression,
        )
    })
}

pub(crate) fn parameter_expression_result_type_reference_in_scope(
    program: &dyn validation::CheckingInput,
    owner: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    scope: Option<typed_trees::trait_definition::RetainedTraitScope>,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    declared_or_contextual(program, expression, || {
        validation::parameter_expression_result_type_reference_in_scope(
            program, owner, parameters, scope, expression,
        )
    })
}
