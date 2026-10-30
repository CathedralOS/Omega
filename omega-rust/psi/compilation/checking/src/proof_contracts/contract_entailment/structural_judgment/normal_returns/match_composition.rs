//! Ordered Match selection shared by executed values and specification terms.
//! The caller supplies operand observation; this owner preserves capture custody,
//! preceding failures, selected assumptions, and retirement of the saved subject.

use super::super::StaticObservations;
use super::{
    NormalReturnPath, NormalValueDemand, StructuralJudge, StructuralJudgment, StructuralTerm,
    ValueScope, boolean_term,
};
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, MatchPattern, TableMatchExpression,
};

impl<'program> StructuralJudge<'program> {
    pub(super) fn evaluate_match_paths<'query>(
        self,
        expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        demand: NormalValueDemand<'query>,
        mut evaluate: impl FnMut(
            Self,
            ExpressionHandle,
            NormalValueDemand<'query>,
        ) -> Option<Vec<NormalReturnPath<'program>>>,
    ) -> Option<Vec<NormalReturnPath<'program>>> {
        let program = self.program;
        let mut outcomes = Vec::new();
        let mut remaining = evaluate(self, dispatch.subject, NormalValueDemand::Value)?;
        for subject in &mut remaining {
            subject.value = subject.judge.complete_match_subject(
                expression,
                dispatch,
                subject.value.clone(),
                scope,
            )?;
        }
        for arm in program.expression_table.match_arms(dispatch.arms) {
            // A structural value-pattern head classifies this exact carrier;
            // it does not construct a value or invoke scalar equality. Keep
            // that decision with the same owner used by formation/coverage.
            let case = if matches!(arm.pattern, MatchPattern::Value(_)) {
                crate::structural_match_pattern_case(
                    program,
                    scope.0,
                    scope.1,
                    dispatch,
                    arm.pattern,
                )
                .filter(|case| case.is_valid())
            } else {
                None
            };
            let mut next_remaining = Vec::new();
            for subject in remaining {
                let pattern = match arm.pattern {
                    MatchPattern::Value(pattern) => Some((pattern, false)),
                    MatchPattern::Destructure(pattern) => Some((pattern.condition, true)),
                    MatchPattern::Wildcard => None,
                };
                if let Some((pattern, predicate)) = pattern {
                    // Patterns execute only after every preceding pattern
                    // failed. Keep their guarantees in that remaining path.
                    let tested_paths = if let Some(case) = case {
                        let reference = crate::structural_match_subject_type(
                            program, scope.0, scope.1, dispatch,
                        )?;
                        let definition = crate::value_custody::places::data_definition_for_type(
                            program, reference,
                        )?;
                        let observed = subject.value.clone();
                        let observation = subject.observation;
                        subject.judge.evaluate_case_paths(
                            pattern,
                            dispatch.subject,
                            definition.symbol,
                            case,
                            environment,
                            scope,
                            NormalValueDemand::Value,
                            |judge, _| {
                                Some(vec![NormalReturnPath {
                                    value: observed.clone(),
                                    judge,
                                    observation,
                                }])
                            },
                        )?
                    } else {
                        evaluate(subject.judge, pattern, NormalValueDemand::Value)?
                    };
                    let predicate = predicate || case.is_some();
                    for tested in tested_paths {
                        let primitive = if predicate {
                            Some(typed_trees::types::PrimitiveType::Bool)
                        } else {
                            crate::match_subject_primitive_type(program, dispatch)
                        };
                        let equation_meaning = primitive.is_some_and(|primitive| {
                            primitive == typed_trees::types::PrimitiveType::Bool
                                || primitive.is_integer()
                        });
                        let comparison = if equation_meaning {
                            tested.judge.judge_equation(
                                tested.judge.resolve(if predicate {
                                    tested.value.clone()
                                } else {
                                    subject.value.clone()
                                }),
                                if predicate {
                                    boolean_term(true)
                                } else {
                                    tested.judge.resolve(tested.value.clone())
                                },
                                0,
                            )
                        } else {
                            StructuralJudgment::Unknown
                        };
                        if !matches!(comparison, StructuralJudgment::Proven) {
                            let mut skipped = tested.judge.clone();
                            if predicate {
                                skipped.assume_normal_equation(
                                    tested.value.clone(),
                                    boolean_term(false),
                                );
                            } else if primitive == Some(typed_trees::types::PrimitiveType::Bool) {
                                let pattern = skipped.resolve(tested.value.clone());
                                for truth in [false, true] {
                                    if matches!(
                                        skipped.judge_equation(
                                            pattern.clone(),
                                            boolean_term(truth),
                                            0
                                        ),
                                        StructuralJudgment::Proven
                                    ) {
                                        skipped.assume_normal_equation(
                                            subject.value.clone(),
                                            boolean_term(!truth),
                                        );
                                        break;
                                    }
                                }
                            }
                            next_remaining.push(NormalReturnPath {
                                value: subject.value.clone(),
                                judge: skipped,
                                observation: subject.observation,
                            });
                        }
                        if matches!(comparison, StructuralJudgment::Refuted) {
                            continue;
                        }
                        let mut selected = tested.judge;
                        if predicate {
                            selected.assume_normal_equation(tested.value, boolean_term(true));
                        } else if equation_meaning {
                            selected.assume_normal_equation(subject.value.clone(), tested.value);
                        }
                        for value in evaluate(selected, arm.value, demand)? {
                            outcomes.push(value.at_expression(expression, demand));
                        }
                    }
                } else {
                    for value in evaluate(subject.judge, arm.value, demand)? {
                        outcomes.push(value.at_expression(expression, demand));
                    }
                }
            }
            remaining = next_remaining;
            if matches!(arm.pattern, MatchPattern::Wildcard) {
                break;
            }
        }
        for outcome in &mut outcomes {
            outcome.judge.retire_match_subject(dispatch);
        }
        Some(outcomes)
    }
}

impl<'program> StructuralJudge<'program> {
    pub(super) fn evaluate_case_paths(
        self,
        expression: ExpressionHandle,
        subject_expression: ExpressionHandle,
        source_definition: symbols::SymbolHandle,
        source_variant: symbols::SymbolHandle,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        demand: NormalValueDemand<'_>,
        mut evaluate: impl FnMut(Self, ExpressionHandle) -> Option<Vec<NormalReturnPath<'program>>>,
    ) -> Option<Vec<NormalReturnPath<'program>>> {
        let program = self.program;
        let subject_type = self.normal_value_type_reference(subject_expression, scope)?;
        let definition = program.data_constructor_definition(subject_type, source_definition)?;
        let variant =
            program.data_constructor_case(subject_type, source_definition, source_variant)?;
        let mut outcomes = Vec::new();
        for subject in evaluate(self, subject_expression)? {
            let observed_subject = subject
                .judge
                .completed_match_case_observation(&subject.value);
            let judgment = subject.judge.judge_case(
                observed_subject.clone(),
                definition.symbol,
                variant.symbol,
            );
            for truth in [false, true] {
                if (truth && matches!(judgment, StructuralJudgment::Refuted))
                    || (!truth && matches!(judgment, StructuralJudgment::Proven))
                {
                    continue;
                }
                let mut selected = subject.judge.clone();
                if truth {
                    // Keep the original admitted value's case at this path cut,
                    // as well as the scoped snapshot used for payload access.
                    if observed_subject.observation_identity_key()
                        != subject.value.observation_identity_key()
                    {
                        selected.intake_case_guarantee(&super::super::CaseGuarantee {
                            subject: observed_subject.clone(),
                            data: definition.symbol,
                            case: variant.symbol,
                        });
                    }
                    selected.intake_case_guarantee(&super::super::CaseGuarantee {
                        subject: subject.value.clone(),
                        data: definition.symbol,
                        case: variant.symbol,
                    });
                    selected.intake_completed_match_case(
                        expression,
                        subject_expression,
                        &subject.value,
                        subject_type,
                        scope.0,
                        scope.1,
                        environment,
                    );
                }
                outcomes.push(
                    NormalReturnPath::observed(boolean_term(truth), selected)
                        .at_expression(expression, demand),
                );
            }
        }
        Some(outcomes)
    }
}

impl<'program> StructuralJudge<'program> {
    /// Read a specification Match with the same ordered captures as body values.
    /// Operand terms use the existing specification admission and storage fences;
    /// they never execute a call to obtain its normal-return guarantees.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn specification_match_term(
        &self,
        expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
        environment: &[(String, StructuralTerm)],
        static_observations: StaticObservations<'_>,
        scope: ValueScope<'_>,
        depth: usize,
    ) -> Option<StructuralTerm> {
        let outcomes = self.clone().evaluate_match_paths(
            expression,
            dispatch,
            environment,
            scope,
            NormalValueDemand::Value,
            |judge, operand, _| {
                judge.specification_value_paths(
                    operand,
                    environment,
                    static_observations,
                    scope,
                    depth + 1,
                )
            },
        )?;
        let mut common = None;
        for outcome in outcomes {
            let value = outcome.judge.resolve(outcome.value);
            match &common {
                None => common = Some(value),
                Some(previous) if previous == &value => {}
                Some(_) => {
                    if super::super::super::struct_trace() {
                        eprintln!(
                            "STRUCT match-term-refused expression={} reason=distinct-feasible-values",
                            self.program.expression_table.display_name(expression)
                        );
                    }
                    return None;
                }
            }
        }
        // No arm means no completed value. Branch assumptions and captures
        // stay inside the query; only a common value crosses the join.
        common
    }

    fn specification_value_paths(
        self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        static_observations: StaticObservations<'_>,
        scope: ValueScope<'_>,
        depth: usize,
    ) -> Option<Vec<NormalReturnPath<'program>>> {
        if depth >= 32 {
            return None;
        }
        if let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
            && let Some((definition, variant)) =
                super::super::super::structural_terms::case_guard_classifier(
                    self.program,
                    scope.0,
                    Some(scope.1),
                    expression,
                )
        {
            return self.evaluate_case_paths(
                expression,
                binary.left,
                definition.symbol,
                variant.symbol,
                environment,
                scope,
                NormalValueDemand::Value,
                |judge, operand| {
                    judge.specification_value_paths(
                        operand,
                        environment,
                        static_observations,
                        scope,
                        depth + 1,
                    )
                },
            );
        }
        let value = self.callee_term(
            expression,
            environment,
            static_observations,
            Some(scope),
            depth + 1,
        )?;
        Some(vec![NormalReturnPath::observed(value, self)])
    }
}
