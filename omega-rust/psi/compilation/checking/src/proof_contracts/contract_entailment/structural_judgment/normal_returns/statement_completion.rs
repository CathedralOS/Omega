//! A statement executes once; observers select outputs of that execution.
//! Observation cuts precede binding presentation and the final assignment write.
//! Refusal to represent a delivered value does not erase completed operands or
//! pre-operation entries, and an empty normal frontier never retries execution.

use super::*;
use typed_trees::statement::StatementNode;

pub(in crate::proof_contracts::contract_entailment) struct CompletedStatement<'program> {
    pub outcomes: Option<Vec<NormalReturnPath<'program>>>,
    pub observations: CompletedNormalObservations<'program>,
    pub observations_complete: bool,
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
        let requested = OperationObservations::completion(values, operations);
        let demand = if advance {
            NormalValueDemand::Value
        } else {
            NormalValueDemand::Observe(values)
        };
        let mut entries = NormalObservationEntries::default();
        let mut observations = Vec::new();
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
                &mut observations,
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
                    &mut observations,
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
        if matches!(
            statement,
            StatementNode::LocalData(_) | StatementNode::Expression(_)
        ) && (!values.is_empty() || !operations.is_empty())
            && let Some(paths) = &outcomes
        {
            observations.extend(paths.iter().map(|path| path.judge.clone()));
        }
        let observations_complete = outcomes.is_some() || arguments_completed;
        let outcomes = successors_available.then_some(outcomes).flatten();
        Some(CompletedStatement {
            outcomes,
            observations_complete,
            observations: CompletedNormalObservations {
                paths: observations,
                operations: entries.operations,
                values_unavailable: entries.values_unavailable,
            },
        })
    }
}
