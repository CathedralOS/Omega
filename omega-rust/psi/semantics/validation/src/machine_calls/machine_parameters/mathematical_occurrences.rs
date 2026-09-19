//! Mathematical declarations own their body and dependent-type applications;
//! their generic arguments are not executable machine selections.

use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::mathematical::{MathematicalBody, MathematicalType};

use crate::value_custody::expression_types::collect_expression_nodes;

pub(super) fn expression_nodes(program: &TypedTrees) -> Vec<ExpressionHandle> {
    let mut expressions = Vec::new();
    let mut types = Vec::new();
    for definition in program.mathematical_definitions() {
        if let MathematicalBody::Definition(body) = definition.body {
            collect_expression_nodes(program, body, &mut expressions);
        }
        types.push(definition.result);
        for parameter in program.mathematical_parameters(definition.parameters) {
            types.push(parameter.ty);
        }
    }
    let mut visited = Vec::new();
    while let Some(handle) = types.pop() {
        if !handle.is_valid() || visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        match program.mathematical_type(handle) {
            MathematicalType::Ordinary(_) => {}
            MathematicalType::Arrow {
                domain, codomain, ..
            } => {
                types.push(*domain);
                types.push(*codomain);
            }
            MathematicalType::Application { callee, arguments } => {
                types.push(*callee);
                for argument in program.expression_table.expression_handles(*arguments) {
                    collect_expression_nodes(program, *argument, &mut expressions);
                }
            }
        }
    }
    if expressions.is_empty() {
        return expressions;
    }
    // A shared handle can also be demanded by executable storage or a state.
    // Mathematical ownership never exempts that second use from MP2b.
    let mut executable = Vec::new();
    for machine in program.machines() {
        for data in program.machine_owned_data(machine) {
            collect_expression_nodes(program, data.initial_value, &mut executable);
        }
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                for root in crate::machine_calls::calls::statement_value_expression_roots(
                    program, statement,
                ) {
                    collect_expression_nodes(program, root, &mut executable);
                }
            }
        }
    }
    expressions.retain(|expression| !executable.contains(expression));
    expressions
}
