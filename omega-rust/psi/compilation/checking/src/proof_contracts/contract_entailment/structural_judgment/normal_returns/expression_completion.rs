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
        let paths = self.clone().evaluate_normal_paths(
            expression,
            environment,
            scope,
            0,
            OperationObservations::completion(observations, operations),
            &mut entries,
            demand,
        );
        CompletedExpression { paths, entries }
    }
}
