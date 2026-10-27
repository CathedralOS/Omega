//! Exact immutable operand bounds project through the current builtin guard.

use super::{
    ExpressionHandle, Machine, State, ValueEnvironment, comparison_interval, negate_comparison,
    place_path,
};
pub(super) fn narrow(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &mut ValueEnvironment,
    condition: ExpressionHandle,
    positive: bool,
) {
    let Some(state) = state else {
        return;
    };
    let Some(comparison) = program.expression_table.binary_node(condition) else {
        return;
    };
    let Some((left, right)) = super::super::invariant_bounds::builtin_comparison_intervals(
        program, machine, state, condition,
    ) else {
        return;
    };
    let operator = if positive {
        comparison.operator
    } else {
        let Some(operator) = negate_comparison(comparison.operator) else {
            return;
        };
        operator
    };
    for (subject, subject_bounds, operand_bounds, subject_on_left) in [
        (comparison.left, left, right, true),
        (comparison.right, right, left, false),
    ] {
        // The bound owner already checked exact state-parameter custody and
        // immutability. Computed operands can supply a bound, not a new place.
        let Some(_) = program.expression_table.name_path_node(subject) else {
            continue;
        };
        let Some(name) = place_path(program, subject) else {
            continue;
        };
        let interval = comparison_interval(operator, operand_bounds, subject_on_left)
            .intersect(subject_bounds);
        environment.narrow(name, interval);
    }
}

/// Keep a checked comparison's selected endpoint on its exact current place.
/// The enclosing condition owner supplies builtin meaning and branch polarity;
/// ordinary stores and joins own the resulting recipe's lifetime.
pub(super) fn narrow_recipe(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &mut ValueEnvironment,
    comparison: &typed_trees::expression::TableBinaryExpression,
    positive: bool,
) {
    let operator = if positive {
        comparison.operator
    } else {
        let Some(operator) = negate_comparison(comparison.operator) else {
            return;
        };
        operator
    };
    for (subject, bound, operator) in [
        (comparison.left, comparison.right, operator),
        (comparison.right, comparison.left, operator.mirrored()),
    ] {
        let Some(place) =
            crate::value_custody::places::canonical_place_from_expression(program, subject)
        else {
            continue;
        };
        let Some(path) = place_path(program, subject) else {
            continue;
        };
        let Some(primitive) =
            super::super::declared_place_type_raw(program, machine, state, subject)
                .and_then(|reference| program.primitive_type_reference(reference))
        else {
            continue;
        };
        let Some(recipe) = super::super::deferred_bounds::comparison_recipe(
            program,
            machine.symbol,
            primitive,
            bound,
            operator,
        ) else {
            continue;
        };
        if let Some(stored) = environment
            .integer_ranges
            .iter_mut()
            .find(|stored| stored.place == place)
        {
            stored.recipe = stored.recipe.intersect(&recipe);
        } else {
            environment
                .integer_ranges
                .push(super::super::value_environment::StoredIntegerRange {
                    place,
                    path,
                    recipe,
                });
        }
    }
}
