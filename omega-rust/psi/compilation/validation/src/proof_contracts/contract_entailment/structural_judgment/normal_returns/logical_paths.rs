//! Short-circuit path selection shared by body and contract observations.
//! Pending connective frames keep finite expression depth separate from proof
//! unfolding. A right operand still inherits only its selecting left path;
//! rejoins never evaluate an unselected payload or import a sibling's effects.

use super::{
    NormalReturnPath, NormalValueDemand, NormalValueObservation, StructuralJudge,
    StructuralJudgment, boolean_term,
};
use std::collections::VecDeque;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, TableBinaryExpression};

pub(super) type LogicalOperand<'program, 'query> = (
    StructuralJudge<'program>,
    ExpressionHandle,
    NormalValueDemand<'query>,
);

enum LogicalBranch<'program> {
    Complete(NormalReturnPath<'program>),
    Right(StructuralJudge<'program>),
}

pub(super) struct LogicalFrame<'program, 'query> {
    pub expression: ExpressionHandle,
    pub demand: NormalValueDemand<'query>,
    right: ExpressionHandle,
    evaluate_right: bool,
    pre_binary_judge: StructuralJudge<'program>,
    awaiting_left: bool,
    short_circuit_steps_pure: bool,
    branches: VecDeque<LogicalBranch<'program>>,
    outcomes: Vec<NormalReturnPath<'program>>,
}

impl<'program, 'query> LogicalFrame<'program, 'query> {
    pub fn new(
        judge: StructuralJudge<'program>,
        expression: ExpressionHandle,
        binary: &TableBinaryExpression,
        demand: NormalValueDemand<'query>,
    ) -> (Self, LogicalOperand<'program, 'query>) {
        let frame = Self {
            expression,
            demand,
            right: binary.right,
            evaluate_right: binary.operator == BinaryOperator::And,
            pre_binary_judge: judge.clone(),
            awaiting_left: true,
            short_circuit_steps_pure: true,
            branches: VecDeque::new(),
            outcomes: Vec::new(),
        };
        (frame, (judge, binary.left, NormalValueDemand::Value))
    }

    /// Keep the original left-path / false-then-true ordering. A queued right
    /// branch owns the exact judge after its selecting observation.
    pub fn resume(
        &mut self,
        paths: Vec<NormalReturnPath<'program>>,
    ) -> Option<Option<LogicalOperand<'program, 'query>>> {
        if self.awaiting_left {
            self.awaiting_left = false;
            for left in paths {
                let left_value = left.observed_value()?.clone();
                for truth in [false, true] {
                    let mut selected = left.judge.clone();
                    let guard_value = boolean_term(truth);
                    if matches!(
                        selected.judge_equation(
                            selected.resolve(left.value.clone()),
                            guard_value.clone(),
                            0,
                        ),
                        StructuralJudgment::Refuted
                    ) {
                        continue;
                    }
                    selected.assume_normal_equation(left_value.clone(), guard_value);
                    if truth == self.evaluate_right {
                        self.branches.push_back(LogicalBranch::Right(selected));
                    } else {
                        if !selected.has_no_new_effects_or_guarantees_since(&left.judge)
                            || !selected.has_no_new_non_boolean_facts_since(&left.judge)
                        {
                            self.short_circuit_steps_pure = false;
                        }
                        self.branches.push_back(LogicalBranch::Complete(
                            NormalReturnPath::observed(boolean_term(truth), selected)
                                .at_expression(self.expression, self.demand),
                        ));
                    }
                }
            }
        } else {
            self.outcomes.extend(
                paths
                    .into_iter()
                    .map(|right| right.at_expression(self.expression, self.demand)),
            );
        }
        while let Some(branch) = self.branches.pop_front() {
            match branch {
                LogicalBranch::Complete(path) => self.outcomes.push(path),
                LogicalBranch::Right(judge) => return Some(Some((judge, self.right, self.demand))),
            }
        }
        Some(None)
    }

    pub fn finish(self) -> Vec<NormalReturnPath<'program>> {
        let Self {
            expression,
            demand,
            evaluate_right,
            pre_binary_judge,
            short_circuit_steps_pure,
            mut outcomes,
            ..
        } = self;
        let program = pre_binary_judge.program;
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
            let judge_refs: Vec<&StructuralJudge<'program>> =
                outcomes.iter().map(|path| &path.judge).collect();
            let rejoined = pre_binary_judge.rejoin_pure_outcomes(&judge_refs, Some(expression));
            return vec![NormalReturnPath::unobserved(rejoined).at_expression(expression, demand)];
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
                let sc_judges: Vec<&StructuralJudge<'program>> =
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
        outcomes
    }
}

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
        let (mut frame, mut operand) = LogicalFrame::new(self, expression, binary, demand);
        loop {
            let paths = evaluate(operand.0, operand.1, operand.2)?;
            match frame.resume(paths)? {
                Some(next) => operand = next,
                None => return Some(frame.finish()),
            }
        }
    }
}
