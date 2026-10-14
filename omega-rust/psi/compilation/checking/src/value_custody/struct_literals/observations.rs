//! Construction checks share occurrence observations within one exact state.
//!
//! Collect a superset of the values construction checks may ask about, without
//! deciding whether a construction is valid. The ordinary validators still run
//! in their original order. The arrivals owner computes effects and snapshots;
//! this collection walk is not another fact-flow analysis. Laziness avoids a
//! state walk when its constructions never need an occurrence environment.

use crate::proof_contracts::arithmetic_domains::{
    OccurrenceValueEnvironments, ValueEnvironment, occurrence_value_environments,
};
use std::collections::HashSet;
use std::sync::OnceLock;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;

pub(super) struct ConstructionObservations<'program> {
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    state: &'program State,
    construction_root: ExpressionHandle,
    environments: OnceLock<OccurrenceValueEnvironments>,
}

impl<'program> ConstructionObservations<'program> {
    pub(super) fn new(
        program: &'program dyn crate::CheckingInput,
        machine: &'program Machine,
        state: &'program State,
        construction_root: ExpressionHandle,
    ) -> Self {
        Self {
            program,
            machine,
            state,
            construction_root,
            environments: OnceLock::new(),
        }
    }

    pub(super) fn environment(&self, expression: ExpressionHandle) -> ValueEnvironment {
        self.environments
            .get_or_init(|| {
                occurrence_value_environments(
                    self.program,
                    self.machine.symbol,
                    self.state.symbol,
                    self.requests(),
                )
            })
            .get(expression)
            .cloned()
            .unwrap_or_default()
    }

    fn requests(&self) -> Vec<ExpressionHandle> {
        let mut nodes = Vec::new();
        let mut seen = HashSet::<_, symbols::BuildSymbolHasher>::default();
        for statement in self
            .program
            .statement_table
            .statements(self.state.statement_nodes)
        {
            for (_, expression) in self.program.statement_table.expression_roots(statement) {
                crate::expression_traversal::collect_expression_nodes(
                    self.program,
                    expression,
                    &mut nodes,
                    &mut seen,
                );
            }
        }
        // Standalone validation may receive a compiler-supplied construction
        // outside the state roster. Its child handles can still name values
        // observed in that state. Collect its requests too, without treating
        // the supplied root as an executed statement or inventing facts for it.
        crate::expression_traversal::collect_expression_nodes(
            self.program,
            self.construction_root,
            &mut nodes,
            &mut seen,
        );
        let mut pending = Vec::new();
        for expression in nodes {
            match self.program.expression_table.expression(expression) {
                ExpressionNode::StructLiteral(literal) => pending.extend(
                    self.program
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .map(|field| field.value),
                ),
                ExpressionNode::ArrayLiteral(elements) => pending
                    .extend_from_slice(self.program.expression_table.expression_handles(*elements)),
                _ => {}
            }
        }
        // Scalar landing follows selected match arms; predicate valuation
        // follows explicit borrows. Include those possible requests without
        // selecting an arm or granting any construction obligation here.
        seen.clear();
        let mut requests = Vec::new();
        while let Some(expression) = pending.pop() {
            if !expression.is_valid() || !seen.insert(expression) {
                continue;
            }
            requests.push(expression);
            match self.program.expression_table.expression(expression) {
                ExpressionNode::Match(dispatch) => pending.extend(
                    self.program
                        .expression_table
                        .match_arms(dispatch.arms)
                        .iter()
                        .map(|arm| arm.value),
                ),
                ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
                _ => {}
            }
        }
        requests
    }
}
