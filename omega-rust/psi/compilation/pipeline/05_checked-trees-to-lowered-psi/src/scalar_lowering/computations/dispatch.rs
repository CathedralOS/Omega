//! Save one subject in the operand prefix across ordered pattern evaluations.
//! Structural subjects retain their ordinary value producer and lexical home
//! alias outside the scalar prefix; each selected path observes that value.
use super::super::{CheckedBooleanExpression, CheckedScalarExpression};
use super::{
    Argument, CheckedScalarComputationKind, Computation, Expansion, LoweringError,
    QualifiedScalarType, ScalarType, Site, parameter, parameters, unsupported,
};
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::integer::LoweredIntegerComparisonKind;
use crate::scalar_lowering::branch_states::prepared::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator,
};
use checked_trees::{CheckedScalarDispatchArm, CheckedScalarDispatchPattern};
impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    fn structural_dispatch(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: typed_trees::expression::ExpressionHandle,
        subject: checked_trees::CheckedStructuralValueHandle,
        alias: typed_trees::expression::ExpressionHandle,
        arms: arena::HandleSpan<CheckedScalarDispatchArm>,
        result_type: QualifiedScalarType,
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let request =
            crate::scalar_lowering::branch_states::structural_values::ValueRequest::prepare(
                identities,
                self.checked,
                subject,
                self.structural_types,
            )?;
        let arms = self
            .checked
            .facts
            .values
            .scalar_computations
            .dispatch_arms
            .span(arms)
            .filter(|arms| !arms.is_empty())
            .ok_or(LoweringError::Unsupported(
                "structural dispatch has no live arms",
            ))?
            .to_vec();
        let prefix = self.established_values.len();
        // This is a dependency available on every selected path. Its producer
        // is scheduled below, before the first predicate, not replayed by an
        // observation. The common emitter binds the real completed home.
        self.established_values
            .push(super::EstablishedValue::Produced { root: subject });
        let lowered = (|| {
            let mut continuation = None;
            for (position, arm) in arms.iter().enumerate().rev() {
                if self.argument_type(
                    identities,
                    &Argument::Computation(arm.value),
                    site,
                    input_types,
                )? != result_type
                {
                    return unsupported(
                        "structural dispatch result carrier disagrees with its continuation",
                    );
                }
                let mut route = (0..position)
                    .map(
                        |decision| crate::emission::claim_bindings::selections::SelectedDecision {
                            expression,
                            decision,
                            selected: false,
                        },
                    )
                    .collect::<Vec<_>>();
                if position + 1 < arms.len() {
                    route.push(
                        crate::emission::claim_bindings::selections::SelectedDecision {
                            expression,
                            decision: position,
                            selected: true,
                        },
                    );
                }
                let mut completed_types = input_types.to_vec();
                completed_types.push(result_type);
                let completion =
                    self.claim_continuation(expression, route, &completed_types, target, site)?;
                let selected = self.argument(
                    identities,
                    &Argument::Computation(arm.value),
                    input_types,
                    completion,
                    site,
                    active,
                )?;
                let entry = match arm.pattern {
                    CheckedScalarDispatchPattern::Wildcard => selected,
                    CheckedScalarDispatchPattern::Value(_) => {
                        return unsupported(
                            "structural dispatch retained an unclassified scalar value pattern",
                        );
                    }
                    CheckedScalarDispatchPattern::Predicate(predicate) => {
                        if self.argument_type(
                            identities,
                            &Argument::Computation(predicate),
                            site,
                            input_types,
                        )? != ScalarType::Boolean.into()
                        {
                            return unsupported("structural dispatch predicate is not Boolean");
                        }
                        let mut tested_types = input_types.to_vec();
                        tested_types.push(ScalarType::Boolean.into());
                        // Source formation owns finite case coverage. On its
                        // final total alternative false is impossible, but the
                        // predicate still completes before entering the body.
                        let terminator = if let Some(fallback) = continuation {
                            LoweredScalarBranchTerminator::Conditional {
                                condition: LoweredBooleanReturnExpression::Parameter {
                                    position: input_types.len(),
                                },
                                when_true_target: selected,
                                when_true_arguments: parameters(input_types),
                                when_true_erased_arguments: Vec::new(),
                                when_true_erased_proof_arguments: Vec::new(),
                                when_false_target: fallback,
                                when_false_arguments: parameters(input_types),
                                when_false_erased_arguments: Vec::new(),
                                when_false_erased_proof_arguments: Vec::new(),
                            }
                        } else {
                            LoweredScalarBranchTerminator::jump(selected, parameters(input_types))
                        };
                        let tested = self.push(LoweredScalarBranchState {
                            structural_parameters: Vec::new(),
                            structural_effects: Vec::new(),
                            parameter_types: tested_types,
                            erased_formal_types: Vec::new(),
                            erased_proof_formals: Vec::new(),
                            bindings: Vec::new(),
                            terminator,
                        });
                        self.argument(
                            identities,
                            &Argument::Computation(predicate),
                            input_types,
                            tested,
                            site,
                            active,
                        )?
                    }
                };
                continuation = Some(entry);
            }
            continuation.ok_or(LoweringError::Unsupported(
                "structural dispatch has no entry",
            ))
        })();
        self.established_values.truncate(prefix);
        Ok(request.finish_with_alias(alias, input_types, lowered?, self))
    }
}

impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn dispatch(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: typed_trees::expression::ExpressionHandle,
        subject: checked_trees::CheckedDispatchSubject,
        arms: arena::HandleSpan<CheckedScalarDispatchArm>,
        result_type: QualifiedScalarType,
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let subject = match subject {
            checked_trees::CheckedDispatchSubject::Scalar(subject) => subject,
            checked_trees::CheckedDispatchSubject::Structural { value, alias, .. } => {
                return self.structural_dispatch(
                    identities,
                    expression,
                    value,
                    alias,
                    arms,
                    result_type,
                    input_types,
                    target,
                    site,
                    active,
                );
            }
        };
        let subject_type = self.argument_type(
            identities,
            &Argument::Computation(subject),
            site,
            input_types,
        )?;
        let plans = &self.checked.facts.values.scalar_computations;
        let arms = plans
            .dispatch_arms
            .span(arms)
            .ok_or(LoweringError::Unsupported(
                "scalar dispatch has a stale arm span",
            ))?
            .to_vec();
        let mut boolean_coverage = [false; 2];
        let mut covered = false;
        for arm in &arms {
            if covered
                || self.argument_type(
                    identities,
                    &Argument::Computation(arm.value),
                    site,
                    input_types,
                )? != result_type
            {
                return unsupported(
                    "scalar dispatch has an unreachable arm or incompatible result",
                );
            }
            match arm.pattern {
                CheckedScalarDispatchPattern::Predicate(_) => {
                    return unsupported("scalar subject cannot carry a structural predicate arm");
                }
                CheckedScalarDispatchPattern::Wildcard => covered = true,
                CheckedScalarDispatchPattern::Value(pattern) => {
                    if self.argument_type(
                        identities,
                        &Argument::Computation(pattern),
                        site,
                        input_types,
                    )? != subject_type
                    {
                        return unsupported(
                            "scalar dispatch subject and pattern carriers disagree",
                        );
                    }
                    if subject_type == ScalarType::Boolean.into()
                        && let CheckedScalarComputationKind::Value(
                            CheckedScalarExpression::Boolean(value),
                        ) = &plans.nodes.get(pattern).kind
                        && let CheckedBooleanExpression::Constant(value) = value.as_ref()
                    {
                        boolean_coverage[usize::from(*value)] = true;
                        covered = boolean_coverage.iter().all(|value| *value);
                    }
                }
            }
        }
        if !covered {
            return unsupported("scalar dispatch has no independently covered terminal arm");
        }
        let mut saved_types = input_types.to_vec();
        saved_types.push(subject_type);
        let mut tested_types = saved_types.clone();
        tested_types.push(subject_type);
        let mut continuation = None;
        for (position, arm) in arms.iter().enumerate().rev() {
            let mut route = (0..position)
                .map(
                    |decision| crate::emission::claim_bindings::selections::SelectedDecision {
                        expression,
                        decision,
                        selected: false,
                    },
                )
                .collect::<Vec<_>>();
            if position + 1 < arms.len() {
                route.push(
                    crate::emission::claim_bindings::selections::SelectedDecision {
                        expression,
                        decision: position,
                        selected: true,
                    },
                );
            }
            let mut completed_types = input_types.to_vec();
            completed_types.push(result_type);
            let completion =
                self.claim_continuation(expression, route, &completed_types, target, site)?;
            // Arm bodies receive the original prefix. The saved subject and
            // last pattern value are no longer live once an arm is selected.
            let selected = self.argument(
                identities,
                &Argument::Computation(arm.value),
                input_types,
                completion,
                site,
                active,
            )?;
            let entry = match arm.pattern {
                CheckedScalarDispatchPattern::Predicate(_) => {
                    return unsupported("scalar subject cannot carry a structural predicate arm");
                }
                CheckedScalarDispatchPattern::Wildcard => self.push(LoweredScalarBranchState {
                    structural_parameters: Vec::new(),
                    structural_effects: Vec::new(),
                    parameter_types: saved_types.clone(),
                    erased_formal_types: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    bindings: Vec::new(),
                    terminator: LoweredScalarBranchTerminator::jump(
                        selected,
                        parameters(input_types),
                    ),
                }),
                CheckedScalarDispatchPattern::Value(pattern) => {
                    let mut comparison_bindings = Vec::new();
                    let terminator = if let Some(when_false_target) = continuation {
                        let condition = if arm.equality_use.is_valid() {
                            comparison_bindings.push(self.comparison_binding(
                                arm.equality_use,
                                parameter(input_types.len(), subject_type),
                                parameter(saved_types.len(), subject_type),
                            )?);
                            LoweredBooleanReturnExpression::Parameter {
                                position: tested_types.len(),
                            }
                        } else if subject_type == ScalarType::Boolean.into() {
                            LoweredBooleanReturnExpression::Equal {
                                left: Box::new(LoweredBooleanReturnExpression::Parameter {
                                    position: input_types.len(),
                                }),
                                right: Box::new(LoweredBooleanReturnExpression::Parameter {
                                    position: saved_types.len(),
                                }),
                            }
                        } else {
                            LoweredBooleanReturnExpression::IntegerComparison {
                                kind: LoweredIntegerComparisonKind::Equal,
                                left: Box::new(parameter(input_types.len(), subject_type)),
                                right: Box::new(parameter(saved_types.len(), subject_type)),
                            }
                        };
                        LoweredScalarBranchTerminator::Conditional {
                            condition,
                            when_true_target: selected,
                            when_true_arguments: parameters(input_types),
                            when_true_erased_arguments: Vec::new(),
                            when_true_erased_proof_arguments: Vec::new(),
                            when_false_target,
                            when_false_arguments: parameters(&saved_types),
                            when_false_erased_arguments: Vec::new(),
                            when_false_erased_proof_arguments: Vec::new(),
                        }
                    } else {
                        // Source replay and the coverage pass above establish
                        // this final literal Boolean alternative, not a default.
                        LoweredScalarBranchTerminator::jump(selected, parameters(input_types))
                    };
                    let tested = self.push(LoweredScalarBranchState {
                        structural_parameters: Vec::new(),
                        structural_effects: Vec::new(),
                        parameter_types: tested_types.clone(),
                        erased_formal_types: Vec::new(),
                        erased_proof_formals: Vec::new(),
                        bindings: comparison_bindings,
                        terminator,
                    });
                    self.argument(
                        identities,
                        &Argument::Computation(pattern),
                        &saved_types,
                        tested,
                        site,
                        active,
                    )?
                }
            };
            continuation = Some(entry);
        }
        let continuation =
            continuation.ok_or(LoweringError::Unsupported("scalar dispatch has no arms"))?;
        self.argument(
            identities,
            &Argument::Computation(subject),
            input_types,
            continuation,
            site,
            active,
        )
    }
}
