//! A statement executes once; observers select outputs of that execution.
//! Observation cuts precede binding presentation and the final assignment write.
//! Refusal to represent a delivered value does not erase completed operands or
//! pre-operation entries, and an empty normal frontier never retries execution.

use super::*;
use typed_trees::statement::StatementNode;

pub(in crate::proof_contracts::contract_entailment) struct CompletedStatement<'program> {
    pub outcomes: Option<Vec<NormalReturnPath<'program>>>,
    pub observations: CompletedNormalObservations<'program>,
}

impl<'program> StructuralJudge<'program> {
    pub(in crate::proof_contracts::contract_entailment) fn complete_prefix_statement(
        &self,
        statement: &'program StatementNode,
        statement_index: usize,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        values: &[ExpressionHandle],
        operations: &[ExpressionHandle],
        advance: bool,
    ) -> Option<CompletedStatement<'program>> {
        let roots = self
            .program
            .tables
            .expression_roots(statement)
            .into_iter()
            .map(|(_, root)| root)
            .filter(|root| root.is_valid())
            .collect::<Vec<_>>();
        let observed_roots = if values.is_empty() && operations.is_empty() {
            &[][..]
        } else {
            roots.as_slice()
        };
        let requested =
            OperationObservations::completion(values, operations).with_roots(observed_roots);
        let demand = if advance {
            NormalValueDemand::Value
        } else {
            NormalValueDemand::ObserveOnly(values)
        };
        let mut entries = NormalObservationEntries::default();
        let mut successors_available = true;
        let mut arguments_completed = false;
        let outcomes = match statement {
            StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                let completed = self.complete_expression(
                    local.initial_value,
                    environment,
                    scope,
                    demand,
                    values,
                    operations,
                );
                entries = completed.entries;
                completed.paths
            }
            StatementNode::Expression(expression) => {
                let completed = self.complete_expression(
                    *expression,
                    environment,
                    scope,
                    demand,
                    values,
                    operations,
                );
                entries = completed.entries;
                completed.paths
            }
            StatementNode::Assignment(_) => self.evaluate_assignment_paths(
                statement,
                environment,
                scope,
                requested,
                &mut entries,
                advance,
                &mut successors_available,
            ),
            StatementNode::Call(call) if self.statement_has_runtime_completion(scope.0, call) => {
                self.evaluate_statement_completion(
                    call,
                    statement_index,
                    environment,
                    scope,
                    requested,
                    &mut entries,
                    advance,
                    &mut arguments_completed,
                )
                .map(|paths| {
                    paths
                        .into_iter()
                        .map(NormalReturnPath::unobserved)
                        .collect()
                })
            }
            _ => return None,
        };
        if outcomes.is_none() && !arguments_completed {
            for &root in observed_roots {
                if !entries.roots.iter().any(|entry| entry.expression == root)
                    && !entries.roots_unavailable.contains(&root)
                {
                    entries.roots_unavailable.push(root);
                }
            }
        }
        // Observation-only traversal has a query frontier, not normal exits.
        // In particular a stopped query is never reported as an all-crash body.
        let outcomes = (advance && successors_available)
            .then_some(outcomes)
            .flatten();
        Some(CompletedStatement {
            outcomes,
            observations: CompletedNormalObservations {
                roots: entries.roots,
                roots_unavailable: entries.roots_unavailable,
                operations: entries.operations,
                values_unavailable: entries.values_unavailable,
            },
        })
    }
}
