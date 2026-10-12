//! Save one subject in the operand prefix across ordered pattern evaluations.
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

pub(crate) struct DispatchEntry {
    pub(crate) pattern: CheckedScalarDispatchPattern,
    pub(crate) equality_use: checked_trees::CheckedOperatorUseHandle,
    pub(crate) target: usize,
}

impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn dispatch(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        subject: Computation,
        arms: arena::HandleSpan<CheckedScalarDispatchArm>,
        result_type: QualifiedScalarType,
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let plans = &self.checked.facts.values.scalar_computations;
        let arms = plans
            .dispatch_arms
            .span(arms)
            .ok_or(LoweringError::Unsupported(
                "scalar dispatch has a stale arm span",
            ))?
            .to_vec();
        for arm in &arms {
            if self.argument_type(
                identities,
                &Argument::Computation(arm.value),
                site,
                input_types,
            )? != result_type
            {
                return unsupported("scalar dispatch has an incompatible result");
            }
        }
        let mut entries = Vec::with_capacity(arms.len());
        for arm in arms {
            let selected = self.argument(
                identities,
                &Argument::Computation(arm.value),
                input_types,
                target,
                site,
                active,
            )?;
            entries.push(DispatchEntry {
                pattern: arm.pattern,
                equality_use: arm.equality_use,
                target: selected,
            });
        }
        self.ordered_dispatch_entries(identities, subject, entries, input_types, site, active)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn ordered_dispatch(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        subject: Computation,
        arms: Vec<DispatchEntry>,
        input_types: &[QualifiedScalarType],
        state: symbols::SymbolHandle,
        statement: u32,
        bindings: &crate::expression_preparation::bindings::ScalarBindings,
    ) -> Result<usize, LoweringError> {
        self.ordered_dispatch_entries(
            identities,
            subject,
            arms,
            input_types,
            &Site {
                state,
                statement,
                bindings,
            },
            &mut Vec::new(),
        )
    }

    fn ordered_dispatch_entries(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        subject: Computation,
        arms: Vec<DispatchEntry>,
        input_types: &[QualifiedScalarType],
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let subject_type = self.argument_type(
            identities,
            &Argument::Computation(subject),
            site,
            input_types,
        )?;
        let plans = &self.checked.facts.values.scalar_computations;
        let mut boolean_coverage = [false; 2];
        let mut covered = false;
        for arm in &arms {
            if covered {
                return unsupported("ordered dispatch has an unreachable arm");
            }
            match arm.pattern {
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
                            "ordered dispatch subject and pattern carriers disagree",
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
            return unsupported("ordered dispatch has no independently covered terminal arm");
        }
        let mut saved_types = input_types.to_vec();
        saved_types.push(subject_type);
        let mut tested_types = saved_types.clone();
        tested_types.push(subject_type);
        let mut continuation = None;
        for arm in arms.iter().rev() {
            // Both result schedules discard the saved test operands on selection.
            let selected = arm.target;
            let entry = match arm.pattern {
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
