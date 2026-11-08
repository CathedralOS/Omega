//! Save one subject in the operand prefix across ordered pattern evaluations.
//! Structural subjects retain their ordinary value producer and lexical home
//! alias outside the scalar prefix; each selected path observes that value.
use super::super::{CheckedBooleanExpression, CheckedScalarExpression};
use super::{
    Argument, CheckedScalarComputationKind, Computation, Expansion, LoweringError,
    QualifiedScalarType, ScalarType, Site, parameter, parameters, unsupported,
};
use crate::emission::claim_bindings::selections::SelectedDecision;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::integer::LoweredIntegerComparisonKind;
use crate::scalar_lowering::branch_states::prepared::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator,
};
use crate::scalar_lowering::branch_states::structural_values::ValueRequest;
use crate::shared::checked_facts::value_facts;
use checked_trees::{CheckedScalarDispatchArm, CheckedScalarDispatchPattern};

fn dispatch_route(
    expression: typed_trees::expression::ExpressionHandle,
    position: usize,
    arm_count: usize,
) -> Vec<SelectedDecision> {
    let mut route = (0..position)
        .map(|decision| SelectedDecision {
            expression,
            decision,
            selected: false,
        })
        .collect::<Vec<_>>();
    if position + 1 < arm_count {
        route.push(SelectedDecision {
            expression,
            decision: position,
            selected: true,
        });
    }
    route
}

impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    fn structural_dispatch(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: typed_trees::expression::ExpressionHandle,
        subject: checked_trees::CheckedStructuralValueHandle,
        subject_expression: checked_trees::expression::ExpressionHandle,
        place: checked_trees::CheckedUnitStructuralArgumentPlan,
        alias: typed_trees::expression::ExpressionHandle,
        arms: arena::HandleSpan<CheckedScalarDispatchArm>,
        result_type: QualifiedScalarType,
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
        normal_result: Option<&super::returns::NormalDispatchScope>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        #[derive(Clone, Copy)]
        enum SubjectRequest {
            Value(ValueRequest),
            Place {
                reserved: semantic_vocabulary::PlaceId,
                structural_type: semantic_vocabulary::StructuralTypeId,
            },
        }
        let request = if subject.is_valid() {
            SubjectRequest::Value(ValueRequest::prepare(
                identities,
                self.checked,
                subject,
                self.structural_types,
            )?)
        } else {
            SubjectRequest::Place {
                reserved: crate::shared::terminal_identities::place_id(
                    crate::shared::terminal_identities::allocate_dense(self.next_place)?,
                ),
                structural_type:
                    crate::shared::terminal_identities::structural_declaration_by_identity(
                        self.structural_types,
                        identities,
                        self.checked,
                        place.type_identity,
                        "dispatch subject has no checked carrier",
                    )?
                    .id,
            }
        };
        let arms = if let Some(normal) = normal_result {
            normal
                .arms
                .iter()
                .map(|(arm, result)| (arm.clone(), Some(result.clone())))
                .collect::<Vec<_>>()
        } else {
            let arms = value_facts(self.checked)
                .scalar_computations
                .dispatch_arms
                .span(arms)
                .filter(|arms| !arms.is_empty())
                .ok_or(LoweringError::Unsupported(
                    "structural dispatch has no live arms",
                ))?
                .to_vec();
            arms.into_iter().map(|arm| (arm, None)).collect::<Vec<_>>()
        };
        // Predicates and the selected arm share this subject. Its private
        // snapshot ends only after the arm has captured its scalar result,
        // before the outer continuation can retire any borrowed backing.
        let mut normal_result = normal_result.cloned();
        let target = if let SubjectRequest::Value(request) = request
            && !self.has_established_value(subject)
        {
            let mut completed_types = input_types.to_vec();
            completed_types.push(result_type);
            if let Some(normal) = &mut normal_result {
                if let Some(effect) = request.snapshot_completion(self.checked)? {
                    normal.before_completion(effect);
                }
                target
            } else {
                request.complete_snapshot(self.checked, &completed_types, target, self)?
            }
        } else {
            target
        };
        let prefix = self.established_values.len();
        // This is a dependency available on every selected path. Its producer
        // is scheduled below, before the first predicate, not replayed by an
        // observation. The common emitter binds the real completed home.
        if subject.is_valid() {
            self.established_values
                .push(super::EstablishedValue::Produced { root: subject });
        }
        let bindings = match request {
            SubjectRequest::Place {
                reserved,
                structural_type,
            } => Some(site.bindings.clone().with_dispatch_subject(
                alias,
                reserved,
                structural_type,
            )?),
            SubjectRequest::Value(_) => None,
        };
        let site = &Site {
            state: site.state,
            statement: site.statement,
            bindings: bindings.as_ref().unwrap_or(site.bindings),
        };

        let lowered = (|| {
            let mut continuation = None;
            let destinations: std::sync::Arc<[_]> =
                crate::emission::claim_bindings::selections::destinations(
                    self.checked,
                    self.machine,
                    site.state,
                    site.statement,
                    expression,
                    0,
                )?
                .into();
            for (position, (arm, result)) in arms.iter().enumerate().rev() {
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
                let route = dispatch_route(expression, position, arms.len());
                let mut completed_types = input_types.to_vec();
                completed_types.push(result_type);
                let arm_result =
                    normal_result
                        .as_ref()
                        .zip(result.as_ref())
                        .map(|(normal, result)| {
                            normal.arm(result.clone(), destinations.clone(), route.clone())
                        });
                let completion = if arm_result.is_some() {
                    target
                } else {
                    self.claim_continuation(expression, route, &completed_types, target, site)?
                };
                let selected = self.argument_with_normal_result(
                    identities,
                    &Argument::Computation(arm.value),
                    input_types,
                    completion,
                    site,
                    arm_result.as_ref(),
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
                        let tested = self.push(LoweredScalarBranchState::continuing(
                            tested_types,
                            terminator,
                        ));
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
        let lowered = lowered?;
        Ok(match request {
            SubjectRequest::Value(request) => request.finish_with_alias(alias, input_types, lowered, self),
            SubjectRequest::Place { reserved, .. } => self.push(LoweredScalarBranchState::jumping(
                lowered, Vec::new(), input_types.to_vec(), input_types,
                vec![crate::scalar_lowering::branch_states::prepared::LoweredScalarEffect::CaptureDispatchSubject {
                    expression: subject_expression, source: place, alias, place: reserved, statement: site.statement,
                }],
            )),
        })
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
        normal_result: Option<&super::returns::NormalDispatchScope>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let subject = match subject {
            checked_trees::CheckedDispatchSubject::Scalar(subject) => subject,
            checked_trees::CheckedDispatchSubject::Structural {
                expression: subject_expression,
                value,
                place,
                alias,
            } => {
                return self.structural_dispatch(
                    identities,
                    expression,
                    value,
                    subject_expression,
                    place,
                    alias,
                    arms,
                    result_type,
                    input_types,
                    target,
                    site,
                    normal_result,
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
        let arms = if let Some(normal) = normal_result {
            normal
                .arms
                .iter()
                .map(|(arm, result)| (arm.clone(), Some(result.clone())))
                .collect::<Vec<_>>()
        } else {
            let arms = plans
                .dispatch_arms
                .span(arms)
                .ok_or(LoweringError::Unsupported(
                    "scalar dispatch has a stale arm span",
                ))?
                .to_vec();
            arms.into_iter().map(|arm| (arm, None)).collect::<Vec<_>>()
        };
        let mut boolean_coverage = [false; 2];
        let mut covered = false;
        for (arm, _) in &arms {
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
        let destinations: std::sync::Arc<[_]> =
            crate::emission::claim_bindings::selections::destinations(
                self.checked,
                self.machine,
                site.state,
                site.statement,
                expression,
                0,
            )?
            .into();
        for (position, (arm, result)) in arms.iter().enumerate().rev() {
            let route = dispatch_route(expression, position, arms.len());
            let mut completed_types = input_types.to_vec();
            completed_types.push(result_type);
            let arm_result = normal_result.zip(result.as_ref()).map(|(normal, result)| {
                normal.arm(result.clone(), destinations.clone(), route.clone())
            });
            let completion = if arm_result.is_some() {
                target
            } else {
                self.claim_continuation(expression, route, &completed_types, target, site)?
            };
            // Subjects and patterns remain ordinary operands; only selected
            // result arms carry their source-owned normal completion.
            let selected = self.argument_with_normal_result(
                identities,
                &Argument::Computation(arm.value),
                input_types,
                completion,
                site,
                arm_result.as_ref(),
                active,
            )?;
            let entry = match arm.pattern {
                CheckedScalarDispatchPattern::Predicate(_) => {
                    return unsupported("scalar subject cannot carry a structural predicate arm");
                }
                CheckedScalarDispatchPattern::Wildcard => {
                    self.push(LoweredScalarBranchState::continuing(
                        saved_types.clone(),
                        LoweredScalarBranchTerminator::jump(selected, parameters(input_types)),
                    ))
                }
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
