//! Collection ordinals consume the same live, all-arrival integer observations
//! as arithmetic formation. Prepare exact statement/expression observations in
//! one state-prefix walk, rather than replaying the prefix for every index.
//! These are worker-owned input snapshots, not cached bounds verdicts. Each
//! access still admits its operands and proves every bound independently.

use super::{ArrivalWalk, ExpressionObservation, incoming_guard_environment};
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;

/// Immutable observations for one state's builtin collection bounds queries.
/// The bound program and declaration owners prevent lookup in another state.
/// A shared expression is joined within its statement, never across statements
/// whose intervening writes may change its storage observations.
pub struct CollectionBoundsObservations<'program> {
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    state: &'program State,
    statements: Vec<(usize, Vec<ExpressionObservation>)>,
}

impl<'program> CollectionBoundsObservations<'program> {
    pub fn new(
        program: &'program dyn crate::CheckingInput,
        machine: &'program Machine,
        state: &'program State,
    ) -> Self {
        let mut statements = Vec::new();
        for (statement_index, statement) in program
            .statement_table
            .indexed_statements(state.statement_nodes)
        {
            let mut expressions = Vec::new();
            crate::visit_statement_value_occurrences(
                program,
                machine.symbol,
                state.symbol,
                statement_index,
                statement,
                &mut |expression, _, _| {
                    if program.expression_table.indexed_node(expression).is_some() {
                        expressions.push(expression);
                    }
                },
            );
            expressions.sort_unstable_by_key(|expression| {
                (expression.arena_index(), expression.generation())
            });
            expressions.dedup();
            if !expressions.is_empty() {
                statements.push((
                    statement_index,
                    expressions
                        .into_iter()
                        .map(ExpressionObservation::environment)
                        .collect::<Vec<_>>(),
                ));
            }
        }
        let mut prepared = Self {
            program,
            machine,
            state,
            statements,
        };
        let Some((last_statement, _)) = prepared.statements.last() else {
            return prepared;
        };
        let last_statement = *last_statement;
        let frames = crate::CallFrameResolver::new(program);
        let mut environment = incoming_guard_environment(program, machine, state, frames.as_ref());
        let mut walk = ArrivalWalk {
            program,
            machine,
            frames: frames.as_ref(),
            arrivals: Vec::new(),
            observations: Vec::new(),
            infer_call_bodies: true,
        };
        let mut requested = prepared.statements.iter_mut().peekable();
        for (statement_index, statement) in program
            .statement_table
            .indexed_statements(state.statement_nodes)
            .take(last_statement + 1)
        {
            let mut observations =
                requested.next_if(|(requested_index, _)| *requested_index == statement_index);
            if let Some((_, observations)) = observations.as_mut() {
                // The existing walker still joins every branch evaluation of
                // each handle within this exact statement.
                walk.observations = std::mem::take(observations);
            }
            let continues =
                walk.statements(state, std::slice::from_ref(statement), &mut environment);
            if let Some((_, observations)) = observations {
                *observations = std::mem::take(&mut walk.observations);
            }
            // This consumer observes the current state, not its outgoing
            // arrival inventory. Incoming facts were already joined above.
            walk.arrivals.clear();
            if !continues {
                break;
            }
        }
        prepared
    }

    /// Prove one element/window under its exact post-child-effects snapshot.
    /// Operand admission retains the purity fence: effectful endpoints require
    /// captured child values, not reconstruction from later storage identities.
    pub fn proves(&self, statement_index: usize, expression: ExpressionHandle) -> bool {
        let Some(indexed) = self.program.expression_table.indexed_node(expression) else {
            return false;
        };
        let Ok(statement) = self
            .statements
            .binary_search_by_key(&statement_index, |(position, _)| *position)
        else {
            return false;
        };
        let observations = &self.statements[statement].1;
        let Ok(position) = observations.binary_search_by_key(
            &(expression.arena_index(), expression.generation()),
            |observation| {
                (
                    observation.expression.arena_index(),
                    observation.expression.generation(),
                )
            },
        ) else {
            return false;
        };
        let Some(environment) = observations[position].environment.as_ref() else {
            return false;
        };
        crate::proof_contracts::arithmetic_domains::live_integer_constraints::proves_collection_bounds(
            self.program, self.machine, self.state, environment, indexed.collection, indexed.index,
        )
    }
}
