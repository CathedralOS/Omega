//! The present endpoints of one authored collection range.
//!
//! A call argument, a transition argument and a `let` initializer narrow a
//! view with the same builtin range, so every site lowers its endpoints here
//! under the shared `SubsliceStart`/`SubsliceEnd` roles keyed by its
//! `CheckedSubsliceSite`. An omitted endpoint produces no row: zero and the
//! source length are supplied by the range's consumer, not by a scalar fact.
//! An inclusive end retains its authored value here too; its exact successor is
//! descriptor normalization after evaluation, not a second scalar source site.
use validation::CallSite;

use checked_trees::{
    CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedScalarExpressionRole,
    CheckedSubsliceSite,
};
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::StatementNode;

/// Lower the present endpoints of `expression` when it is a range
/// whose operator use (if any) resolved to the builtin meaning. `lower`
/// evaluates one endpoint in its authored integer carrier in the site's scalar
/// namespace. Descriptor emission performs the exact count conversion later.
pub(crate) fn subslice_endpoints<Value>(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    site: CheckedSubsliceSite,
    mut lower: impl FnMut(ExpressionHandle) -> Option<Value>,
) -> Vec<(ExpressionHandle, CheckedScalarExpressionRole, Value)> {
    let Some(expression) = validation::subslice_expression(program, expression) else {
        return Vec::new();
    };
    let Some(indexed) = program.expression_table.indexed_node(expression) else {
        return Vec::new();
    };
    let Some(range) = program.expression_table.range_node(indexed.index) else {
        return Vec::new();
    };
    if operators
        .expression_use(expression)
        .is_some_and(|selected| {
            selected.spelling != language_core::OperatorSpelling::Range
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    CheckedOperatorResolutionStatus::Missing
                        | CheckedOperatorResolutionStatus::BuiltinFallback
                )
        })
    {
        return Vec::new();
    }
    [
        (
            range.start,
            CheckedScalarExpressionRole::SubsliceStart { site },
        ),
        (range.end, CheckedScalarExpressionRole::SubsliceEnd { site }),
    ]
    .into_iter()
    .filter(|(endpoint, _)| endpoint.is_valid())
    .filter_map(|(endpoint, role)| Some((endpoint, role, lower(endpoint)?)))
    .collect()
}

/// The source occurrence, not the result's destination or the endpoint's syntax,
/// identifies a range operand. Both scalar plan producers consume this inventory.
pub(crate) fn sites(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
) -> Vec<(ExpressionHandle, CheckedSubsliceSite)> {
    let mut result = Vec::new();
    // Formation captures these operands once. Permission to replace the
    // receiving binding later does not change the initializer's coordinates.
    if let Some(local) = statement.as_local_data() {
        result.push((local.initial_value, CheckedSubsliceSite::LocalBinding));
    }
    for (call_ordinal, site) in
        validation::collect_call_sites_in_statement(program, machine, state, statement_index)
            .into_iter()
            .enumerate()
    {
        let Ok(call_ordinal) = u32::try_from(call_ordinal) else {
            break;
        };
        // Traversal reserves an ordinal for an internal state edge, but it
        // invokes no machine. Its operands use TransitionArgument below;
        // nested endpoint calls keep their original subsequent ordinals.
        if let validation::CallSite::TransitionNamed { path, .. } = &site
            && validation::terminal_callable_target(program, machine, path.symbol).is_none()
        {
            continue;
        }
        for (argument, argument_ordinal) in structural_call_arguments(program, &site) {
            result.push((
                argument,
                CheckedSubsliceSite::CallArgument {
                    call_ordinal,
                    argument_ordinal,
                },
            ));
        }
    }
    if let Some(transition) = statement.as_transition()
        && transition.target.is_valid()
        && let typed_trees::statement::TransitionTargetNode::Named {
            path, arguments, ..
        } = program.statement_table.transition_target(transition.target)
        && let Some(parameters) = validation::call_target_parameters(program, path.symbol)
    {
        for (argument, (position, parameter)) in program
            .statement_table
            .expression_handles(*arguments)
            .iter()
            .zip(parameters.iter().enumerate())
        {
            if !parameter.relevance.is_erased()
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
                && let Ok(argument_ordinal) = u32::try_from(position)
            {
                result.push((
                    *argument,
                    CheckedSubsliceSite::TransitionArgument { argument_ordinal },
                ));
            }
        }
    }
    let table = &program.expression_table;
    let mut pending: Vec<_> = program
        .tables
        .expression_roots(statement)
        .into_iter()
        .map(|(_, root)| root)
        .filter(|root| root.is_valid())
        .collect();
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !table.expression_is_valid(expression) || visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        result.push((expression, CheckedSubsliceSite::Expression { expression }));
        validation::expression_traversal::children(
            program,
            table.expression(expression),
            |child| {
                if child.is_valid() {
                    pending.push(child);
                }
            },
        );
    }
    result
}

/// Structural formals have their own dense ordinal. Erased and scalar formals
/// consume their authored actual but no structural position.
fn structural_call_arguments(
    program: &dyn ::validation::CheckingInput,
    site: &validation::CallSite<'_>,
) -> Vec<(typed_trees::expression::ExpressionHandle, u32)> {
    let target = match site {
        CallSite::Statement(call) => call.target_symbol,
        CallSite::Expression { call, .. } => validation::call_target_symbol(program, call),
        CallSite::TransitionNamed { path, .. } => path.symbol,
    };
    let Some(parameters) = validation::call_target_parameters(program, target) else {
        return Vec::new();
    };
    let arguments = validation::call_site_argument_expressions(program, site);
    let explicit_parameters = parameters.iter();
    if parameters.len() != arguments.len() {
        return Vec::new();
    }
    let mut structural = Vec::new();
    for (argument, parameter) in arguments.iter().zip(explicit_parameters) {
        let Some(erased) = crate::execution::terminal_unit::strips_erased_parameter(parameter)
        else {
            return Vec::new();
        };
        if erased
            || program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        {
            continue;
        }
        let Ok(ordinal) = u32::try_from(structural.len()) else {
            return Vec::new();
        };
        structural.push((*argument, ordinal));
    }
    structural
}
