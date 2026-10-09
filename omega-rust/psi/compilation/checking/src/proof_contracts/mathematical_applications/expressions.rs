//! Mathematical declarations own their body and dependent-type applications;
//! their generic arguments are not executable machine selections.

use std::collections::HashSet;

use typed_trees::expression::ExpressionHandle;
use typed_trees::mathematical::{MathematicalBody, MathematicalType};

use crate::expression_traversal::collect_expression_nodes;

pub fn expression_nodes(program: &dyn crate::CheckingInput) -> Vec<ExpressionHandle> {
    let mut expressions = Vec::new();
    let mut expressions_seen = HashSet::new();
    for definition in program.mathematical_definitions() {
        for root in definition_roots(program, definition) {
            collect_expression_nodes(program, root, &mut expressions, &mut expressions_seen);
        }
    }
    if expressions.is_empty() {
        return expressions;
    }
    // A shared handle can also be demanded by executable storage or a state.
    // Mathematical ownership never exempts that second use from MP2b.
    let mut executable = Vec::new();
    let mut executable_seen = HashSet::new();
    for machine in program.machines() {
        for data in program.machine_owned_data(machine) {
            collect_expression_nodes(
                program,
                data.initial_value,
                &mut executable,
                &mut executable_seen,
            );
        }
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                for root in crate::machine_calls::calls::statement_value_expression_roots(
                    program, statement,
                ) {
                    collect_expression_nodes(program, root, &mut executable, &mut executable_seen);
                }
            }
        }
    }
    expressions.retain(|expression| !executable_seen.contains(expression));
    expressions
}

/// Ordered expression roots owned by one declaration: transparent body and
/// dependent type applications, including bodyless assumptions. Consumers
/// retain the declaration's exact telescope while walking each root.
pub fn definition_roots(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::mathematical::MathematicalDefinition,
) -> Vec<ExpressionHandle> {
    let mut roots = Vec::new();
    if let MathematicalBody::Definition(body) = definition.body {
        roots.push(body);
    }
    let mut types = vec![definition.result];
    types.extend(
        program
            .mathematical_parameters(definition.parameters)
            .iter()
            .map(|parameter| parameter.ty),
    );
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
                roots.extend_from_slice(program.expression_table.expression_handles(*arguments));
            }
        }
    }
    roots
}
