//! Publish resolved executable types at the final immutable typed boundary.
//!
//! State roots retain the declaration telescope needed by place/result
//! resolution. Specialization deep-copies its occurrences before this point,
//! so handles identify occurrences, not spellings shared by generic bodies.
//! Lowering receives the decisions, including genuine absence; it cannot
//! rebuild a type from a local, parameter or source-shaped fallback.

use checked_trees::{ResolvedCarrierType, ResolvedExpressionType, ResolvedTypeFacts};
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;

pub(crate) fn resolve_program_types(program: &dyn validation::CheckingInput) -> ResolvedTypeFacts {
    let highest_expression = program
        .expression_table
        .iter_expressions()
        .map(|(handle, _)| handle.arena_index())
        .max()
        .unwrap_or(0);
    let mut expressions = vec![ResolvedExpressionType::default(); highest_expression as usize + 1];
    let mut pending = Vec::new();
    for machine in program.machines() {
        let states = program.machine_states(machine);
        if let Some(entry) = states.first() {
            pending.extend(
                program
                    .machine_owned_data(machine)
                    .iter()
                    .map(|field| field.initial_value),
            );
            resolve_occurrences(program, machine, entry, &mut pending, &mut expressions);
        }
        for state in states {
            for statement in program.statement_table.statements(state.statement_nodes) {
                pending.extend(
                    program
                        .tables
                        .expression_roots(statement)
                        .into_iter()
                        .map(|(_, expression)| expression),
                );
            }
            resolve_occurrences(program, machine, state, &mut pending, &mut expressions);
        }
    }
    let highest_reference = program
        .type_reference_table
        .type_reference_rows()
        .map(|(handle, _)| handle.arena_index())
        .max()
        .unwrap_or(0);
    let mut carriers = vec![ResolvedCarrierType::default(); highest_reference as usize + 1];
    for (reference, _) in program.type_reference_table.type_reference_rows() {
        carriers[reference.arena_index() as usize] = ResolvedCarrierType {
            reference,
            carrier: program
                .unwrapped_type_reference(reference)
                .unwrap_or_default(),
        };
    }
    ResolvedTypeFacts::with_rows(expressions, carriers)
}

fn resolve_occurrences(
    program: &dyn validation::CheckingInput,
    machine: &Machine,
    state: &State,
    pending: &mut Vec<ExpressionHandle>,
    expressions: &mut [ResolvedExpressionType],
) {
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            continue;
        }
        let row = &mut expressions[expression.arena_index() as usize];
        if row.expression == expression {
            continue;
        }
        *row = ResolvedExpressionType {
            expression,
            place: validation::declared_place_type_raw(program, machine, Some(state), expression)
                .unwrap_or_default(),
            result: validation::expression_result_type_reference(
                program, machine, state, expression,
            )
            .unwrap_or_default(),
        };
        super::push_every_expression_child(
            program,
            program.expression_table.expression(expression),
            pending,
        );
    }
}
