//! One expression completion supplies its delivered value and observation cuts.
//! The statement owner chooses execution and receiving scope. Output requests
//! cannot make an unavailable value erase independently completed operands.

use super::*;

pub(super) struct CompletedExpression<'program> {
    pub paths: Option<Vec<NormalReturnPath<'program>>>,
    pub entries: NormalObservationEntries<'program>,
}

impl<'program> StructuralJudge<'program> {
    pub(super) fn complete_expression(
        &self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        demand: NormalValueDemand<'_>,
        observations: &[ExpressionHandle],
        operations: &[ExpressionHandle],
    ) -> CompletedExpression<'program> {
        // Ordinary values retain their selected expression type. Only literals
        // without a standalone type use the receiving contextual presentation;
        // no destination domain is installed in this entry judge.
        let reference =
            crate::expression_result_type_reference(self.program, scope.0, scope.1, expression)
                .unwrap_or(scope.2);
        let scope = (scope.0, scope.1, reference);
        let mut entries = NormalObservationEntries::default();
        let roots = [expression];
        let observed_roots = if observations.is_empty() && operations.is_empty() {
            &[][..]
        } else {
            &roots[..]
        };
        let paths = self
            .clone()
            .evaluate_normal_operands(
                &[NormalOperand {
                    expression,
                    reference,
                    demand,
                }],
                environment,
                scope,
                0,
                OperationObservations::completion(observations, operations)
                    .with_roots(observed_roots),
                &mut entries,
                None,
            )
            .and_then(|paths| {
                paths
                    .into_iter()
                    .map(|(judge, values)| {
                        let [(_, result)] = values.as_slice() else {
                            return None;
                        };
                        Some(NormalReturnPath {
                            value: result.value.clone(),
                            observation: result.observation,
                            value_requested: matches!(demand, NormalValueDemand::Value),
                            judge,
                        })
                    })
                    .collect::<Option<Vec<_>>>()
            });
        if paths.is_none()
            && !observed_roots.is_empty()
            && !entries.roots_unavailable.contains(&expression)
        {
            entries.roots_unavailable.push(expression);
        }
        CompletedExpression { paths, entries }
    }
}
