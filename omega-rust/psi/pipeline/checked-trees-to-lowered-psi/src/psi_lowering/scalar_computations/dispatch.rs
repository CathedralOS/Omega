//! Save one subject in the operand prefix across ordered pattern evaluations.

use super::*;
use checked_trees::{CheckedScalarDispatchArm, CheckedScalarDispatchPattern};

impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn dispatch(
        &mut self,
        subject: Computation,
        arms: arena::HandleSpan<CheckedScalarDispatchArm>,
        result_type: QualifiedScalarType,
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let subject_type =
            self.argument_type(&Argument::Computation(subject), site, input_types)?;
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
                || self.argument_type(&Argument::Computation(arm.value), site, input_types)?
                    != result_type
            {
                return unsupported(
                    "scalar dispatch has an unreachable arm or incompatible result",
                );
            }
            match arm.pattern {
                CheckedScalarDispatchPattern::Wildcard => covered = true,
                CheckedScalarDispatchPattern::Value(pattern) => {
                    if self.argument_type(&Argument::Computation(pattern), site, input_types)?
                        != subject_type
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
        for arm in arms.iter().rev() {
            // Arm bodies receive the original prefix. The saved subject and
            // last pattern value are no longer live once an arm is selected.
            let selected = self.argument(
                &Argument::Computation(arm.value),
                input_types,
                target,
                site,
                active,
            )?;
            let entry = match arm.pattern {
                CheckedScalarDispatchPattern::Wildcard => self.push(LoweredScalarBranchState {
                    structural_parameters: Vec::new(),
                    structural_effects: Vec::new(),
                    parameter_types: saved_types.clone(),
                    bindings: Vec::new(),
                    terminator: LoweredScalarBranchTerminator::Jump {
                        trivial_affine_discards: Vec::new(),
                        target: selected,
                        arguments: parameters(input_types),
                        structural_arguments: Vec::new(),
                    },
                }),
                CheckedScalarDispatchPattern::Value(pattern) => {
                    let mut comparison_bindings = Vec::new();
                    let terminator = if let Some(when_false_target) = continuation {
                        let condition = if arm.equality_use.is_valid() {
                            comparison_bindings.push(self.comparison_binding(
                                arm.equality_use,
                                parameter(input_types.len(), subject_type),
                                parameter(saved_types.len(), subject_type),
                                site,
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
                            when_false_target,
                            when_false_arguments: parameters(&saved_types),
                        }
                    } else {
                        // Source replay and the coverage pass above establish
                        // this final literal Boolean alternative, not a default.
                        LoweredScalarBranchTerminator::Jump {
                            trivial_affine_discards: Vec::new(),
                            target: selected,
                            arguments: parameters(input_types),
                            structural_arguments: Vec::new(),
                        }
                    };
                    let tested = self.push(LoweredScalarBranchState {
                        structural_parameters: Vec::new(),
                        structural_effects: Vec::new(),
                        parameter_types: tested_types.clone(),
                        bindings: comparison_bindings,
                        terminator,
                    });
                    self.argument(
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
            &Argument::Computation(subject),
            input_types,
            continuation,
            site,
            active,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_expansion_saves_subject_once_and_drops_it_only_on_selection() {
        let mut checked = CheckedTrees::default();
        let plans = &mut checked.facts.values.scalar_computations;
        let subject = plans.nodes.append(CheckedScalarComputation {
            kind: CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::Parameter { position: 0 },
            ))),
            ..Default::default()
        });
        let mut boolean = |value| {
            plans.nodes.append(CheckedScalarComputation {
                kind: CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(
                    Box::new(CheckedBooleanExpression::Constant(value)),
                )),
                ..Default::default()
            })
        };
        let first_pattern = boolean(true);
        let first_result = boolean(false);
        let second_result = boolean(true);
        let arms = plans.dispatch_arms.insert_many([
            CheckedScalarDispatchArm {
                pattern: CheckedScalarDispatchPattern::Value(first_pattern),
                value: first_result,
                ..Default::default()
            },
            CheckedScalarDispatchArm {
                value: second_result,
                ..Default::default()
            },
        ]);
        let root = plans.nodes.append(CheckedScalarComputation {
            kind: CheckedScalarComputationKind::Dispatch {
                source_expression: Handle::invalid(),
                subject,
                arms,
            },
            ..Default::default()
        });
        let bindings = storage::ScalarBindings::new(1);
        let qualifications = PreparedScalarQualifications::prepare(&checked, &[])
            .expect("empty fixture qualifications");
        let mut expansion = Expansion::new(
            &checked,
            &qualifications,
            symbols::SymbolHandle::invalid(),
            0,
        );
        let target = usize::MAX;
        expansion
            .argument(
                &Argument::Computation(root),
                &[ScalarType::Boolean.into()],
                target,
                &Site {
                    state: symbols::SymbolHandle::invalid(),
                    statement: 0,
                    bindings: &bindings,
                },
                &mut Vec::new(),
            )
            .unwrap();
        let states = expansion.finish();
        let subject_reads = states.iter().flat_map(|state| &state.bindings).filter(|binding| {
            matches!(binding, LoweredScalarBinding::Expression(LoweredDirectExpression::Boolean { expression: value })
                if matches!(value.as_ref(), LoweredBooleanReturnExpression::Parameter { position: 0 }))
        }).count();
        assert_eq!(subject_reads, 1);
        let dispatch = states
            .iter()
            .find(|state| {
                matches!(
                    state.terminator,
                    LoweredScalarBranchTerminator::Conditional { .. }
                )
            })
            .unwrap();
        assert_eq!(
            dispatch.parameter_types.len(),
            3,
            "caller, saved subject, completed pattern"
        );
        let LoweredScalarBranchTerminator::Conditional {
            when_true_arguments,
            when_false_arguments,
            ..
        } = &dispatch.terminator
        else {
            panic!("ordered test");
        };
        assert_eq!(
            when_true_arguments.len(),
            1,
            "selected body drops the subject"
        );
        assert_eq!(
            when_false_arguments.len(),
            2,
            "failed test preserves the subject"
        );
        assert_eq!(
            states
                .iter()
                .filter(|state| matches!(state.terminator,
            LoweredScalarBranchTerminator::Jump { target: selected, .. } if selected == target))
                .count(),
            2,
            "both results join the actual continuation"
        );
    }
}
