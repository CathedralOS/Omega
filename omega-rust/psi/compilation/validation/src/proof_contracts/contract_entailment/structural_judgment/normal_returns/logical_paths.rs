//! Short-circuit path selection shared by body and contract observations.
//! A right operand inherits the selecting left observation. Rejoins discard
//! branch-only facts; they never evaluate an unselected payload or effect.

use super::{
    NormalReturnPath, NormalValueDemand, NormalValueObservation, StructuralJudge,
    StructuralJudgment, boolean_term,
};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, TableBinaryExpression};

impl<'program> StructuralJudge<'program> {
    pub(super) fn evaluate_logical_paths<'query>(
        self,
        expression: ExpressionHandle,
        binary: &TableBinaryExpression,
        demand: NormalValueDemand<'query>,
        mut evaluate: impl FnMut(
            Self,
            ExpressionHandle,
            NormalValueDemand<'query>,
        ) -> Option<Vec<NormalReturnPath<'program>>>,
    ) -> Option<Vec<NormalReturnPath<'program>>> {
        let program = self.program;
        let pre_binary_judge = self.clone();
        let evaluate_right = binary.operator == BinaryOperator::And;
        let mut short_circuit_steps_pure = true;
        let mut outcomes = Vec::new();
        for left in evaluate(self, binary.left, NormalValueDemand::Value)? {
            let left_value = left.observed_value()?.clone();
            for truth in [false, true] {
                let mut selected = left.judge.clone();
                let guard_value = boolean_term(truth);
                if matches!(
                    selected.judge_equation(
                        selected.resolve(left.value.clone()),
                        guard_value.clone(),
                        0
                    ),
                    StructuralJudgment::Refuted
                ) {
                    continue;
                }
                selected.assume_normal_equation(left_value.clone(), guard_value);
                if truth == evaluate_right {
                    for right in evaluate(selected, binary.right, demand)? {
                        outcomes.push(right.at_expression(expression, demand));
                    }
                } else {
                    if !selected.has_no_new_effects_or_guarantees_since(&left.judge)
                        || !selected.has_no_new_non_boolean_facts_since(&left.judge)
                    {
                        short_circuit_steps_pure = false;
                    }
                    outcomes.push(
                        NormalReturnPath::observed(boolean_term(truth), selected)
                            .at_expression(expression, demand),
                    );
                }
            }
        }
        if outcomes.len() > 1
            && (matches!(demand, NormalValueDemand::Location)
                || matches!(
                    demand,
                    NormalValueDemand::Observe(obs) | NormalValueDemand::ObserveOnly(obs)
                        if !obs.iter().any(|&observed| {
                            crate::expression_owners::expression_contains(
                                program,
                                expression,
                                observed,
                            )
                        })
                ))
            && outcomes.iter().all(|path| {
                path.judge
                    .has_no_new_effects_or_guarantees_since(&pre_binary_judge)
            })
        {
            let judge_refs: Vec<&Self> = outcomes.iter().map(|path| &path.judge).collect();
            let rejoined = pre_binary_judge.rejoin_pure_outcomes(&judge_refs, Some(expression));
            return Some(vec![
                NormalReturnPath::unobserved(rejoined).at_expression(expression, demand),
            ]);
        }
        let short_circuit_value = boolean_term(!evaluate_right);
        if short_circuit_steps_pure && outcomes.len() > 1 {
            let sc_indices: Vec<usize> = outcomes
                .iter()
                .enumerate()
                .filter_map(|(idx, path)| {
                    (path.observation == NormalValueObservation::Observed
                        && path.value == short_circuit_value
                        && path
                            .judge
                            .has_no_new_effects_or_guarantees_since(&pre_binary_judge))
                    .then_some(idx)
                })
                .collect();
            let can_merge_sc = sc_indices.len() >= 2
                && sc_indices.iter().all(|&idx| {
                    outcomes[idx]
                        .judge
                        .has_no_new_non_boolean_facts_since(&pre_binary_judge)
                });
            if can_merge_sc {
                let sc_judges: Vec<&Self> =
                    sc_indices.iter().map(|&idx| &outcomes[idx].judge).collect();
                let rejoined_sc =
                    pre_binary_judge.rejoin_pure_outcomes(&sc_judges, Some(expression));
                let mut merged_sc = Some(
                    NormalReturnPath::observed(short_circuit_value, rejoined_sc)
                        .at_expression(expression, demand),
                );
                let first_sc = sc_indices[0];
                let mut merged_outcomes = Vec::with_capacity(outcomes.len() - sc_indices.len() + 1);
                for (idx, outcome) in outcomes.into_iter().enumerate() {
                    if idx == first_sc {
                        if let Some(merged) = merged_sc.take() {
                            merged_outcomes.push(merged);
                        }
                    } else if !sc_indices.contains(&idx) {
                        merged_outcomes.push(outcome);
                    }
                }
                outcomes = merged_outcomes;
            }
        }
        Some(outcomes)
    }
}
