//! External completion of a selected checked body, in its checked execution
//! order. Satisfaction nominates a promise; it cannot prove its own body.
//! Calls retain their original leaf identity and state arguments transport the
//! entry value. Returning paths, cycles, and unresolved service redispatch do
//! not establish an external completion.

use checked_trees::{
    CheckedCallScalarArgument, CheckedComposedUnitControlTerminatorPlan as Terminator,
    CheckedConditionalReturnArm, CheckedScalarComputationKind, CheckedScalarExpression,
    CheckedStructuralControlSuccessorPlan, CheckedTrees, CheckedUnitCallRoute,
    CheckedUnitEffectOperationPlan as Operation, CheckedUnitLocalBindingPlan,
};
use symbols::SymbolHandle;

type Origin = Option<usize>;

pub(super) fn retain_completions(
    checked: &CheckedTrees,
    selected: &mut [crate::SelectedProviderReviewProvenance],
    target: Option<&str>,
    candidates: &[crate::DerivedProviderPlan],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    for provider in selected {
        let mut rows = Vec::with_capacity(provider.plan.rows.len());
        for (&requirement, &realization) in provider
            .provider
            .row_requirements
            .iter()
            .zip(&provider.provider.row_realizations)
        {
            let completion =
                checked_trees::typed_trees::boundary_completions::boundary_requirement_completion(
                    &checked.typed,
                    requirement,
                );
            rows.push(
                if matches!(
                    completion,
                    semantic_vocabulary::BoundaryCompletion::External(_)
                ) {
                    checked_completion(checked, realization, requirement, target, candidates)
                        .map_err(|reason| {
                            vec![diagnostics::Diagnostic::error(format!(
                                "selected completion provider `{}`: {reason}",
                                checked.symbols.display_path(realization, "::")
                            ))]
                        })?
                } else {
                    Vec::new()
                },
            );
        }
        provider.row_external_completion_leaves = rows;
    }
    Ok(())
}

#[derive(Clone)]
struct Values {
    state: SymbolHandle,
    parameters: Vec<Origin>,
    locals: Vec<(usize, Origin)>,
}

impl Values {
    fn pure(&self, expression: &CheckedScalarExpression) -> Origin {
        match expression {
            CheckedScalarExpression::Parameter { position, .. } => {
                self.parameters.get(*position).copied().flatten()
            }
            CheckedScalarExpression::Local { position, .. } => self
                .locals
                .iter()
                .find(|(ordinal, _)| self.parameters.len() + ordinal == *position)
                .and_then(|(_, origin)| *origin),
            _ => None,
        }
    }
}

#[derive(Default)]
struct Outcome {
    returns: bool,
    result: Origin,
    leaves: Vec<SymbolHandle>,
}

impl Outcome {
    fn returning() -> Self {
        Self {
            returns: true,
            result: None,
            leaves: Vec::new(),
        }
    }
    fn merge(&mut self, other: Self) {
        if other.returns {
            self.result = if self.returns && self.result != other.result {
                None
            } else {
                other.result
            };
        }
        self.returns |= other.returns;
        for leaf in other.leaves {
            if !self.leaves.contains(&leaf) {
                self.leaves.push(leaf);
            }
        }
    }
}

/// Verify the externally completing row through the checked operation graph.
/// `leaves` is the exact target's original satisfaction inventory, never the
/// selected wrapper substituted as an intrinsic. The result retains the leaves
/// that actually discharge the completion promise.
pub(super) fn checked_completion(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    requirement: SymbolHandle,
    target: Option<&str>,
    candidates: &[crate::DerivedProviderPlan],
) -> Result<Vec<SymbolHandle>, String> {
    let mut review = CompletionReview {
        checked,
        requirement,
        target,
        candidates,
        active: Vec::new(),
    };
    let result = review.machine(machine, SymbolHandle::invalid(), vec![Some(0)])?;
    if result.returns || result.leaves.is_empty() {
        return Err("selected external-completion provider has a normal return path".into());
    }
    Ok(result.leaves)
}

struct CompletionReview<'a> {
    checked: &'a CheckedTrees,
    requirement: SymbolHandle,
    target: Option<&'a str>,
    candidates: &'a [crate::DerivedProviderPlan],
    active: Vec<(SymbolHandle, SymbolHandle)>,
}

impl CompletionReview<'_> {
    fn machine(
        &mut self,
        symbol: SymbolHandle,
        state: SymbolHandle,
        arguments: Vec<Origin>,
    ) -> Result<Outcome, String> {
        let context =
            crate::ProviderPlanningContext::new(self.checked, source::DependencyScope::Product);
        let machine = crate::target_machines::realized_row_machine(context, symbol, self.target)
            .ok_or_else(|| "completion call has no exact target realization".to_owned())?;
        if !machine.body_is_present {
            let requirement = self.requirement;
            let leaves: Vec<_> = self.candidates.iter().flat_map(|candidate| {
                candidate.plan.rows.iter().enumerate().filter_map(move |(position, row)| {
                    (candidate.provenance.row_realizations.get(position) == Some(&machine.symbol)
                        && candidate.provenance.row_requirements.get(position) == Some(&requirement)
                        && matches!(row.binding, effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                            | effects::provider_plan::ProviderBinding::Import { .. }))
                        .then_some(machine.symbol)
                })
            }).collect();
            if leaves.is_empty() {
                return Err("completion call has no original external satisfaction leaf (service redispatch is not a leaf)".into());
            }
            if arguments != [Some(0)] {
                return Err(
                    "external-completion provider does not preserve its exact status argument"
                        .into(),
                );
            }
            return Ok(Outcome {
                returns: false,
                result: None,
                leaves: vec![machine.symbol],
            });
        }
        let flow = &self.checked.facts.flow;
        let ordinary = flow
            .terminal_unit_effects
            .machines
            .iter()
            .chain(
                flow.terminal_partial_affine_unit_cleanups
                    .machines
                    .iter()
                    .map(|plan| &plan.machine),
            )
            .find(|plan| plan.machine == machine.symbol);
        let composed = flow
            .terminal_unit_effects
            .composed_for_machine(machine.symbol);
        let state = if state.is_valid() {
            state
        } else {
            ordinary
                .map(|plan| plan.state)
                .or_else(|| composed.and_then(|plan| plan.states.first().map(|state| state.state)))
                .ok_or_else(|| "completion body has no checked execution plan".to_owned())?
        };
        let key = (machine.symbol, state);
        if self.active.contains(&key) {
            return Err("external-completion provider has a cyclic checked invocation path".into());
        }
        self.active.push(key);
        let mut values = Values {
            state,
            parameters: arguments,
            locals: Vec::new(),
        };
        let result = if let Some(plan) = ordinary {
            if plan.state != state {
                return Err("completion call selects a different checked entry".into());
            }
            let mut outcome = self.operations(&plan.operations, &mut values)?;
            if outcome.returns {
                if let Some(control) = &plan.scalar_control {
                    outcome.returns = false;
                    outcome.merge(self.scalar_terminator(
                        machine.symbol,
                        &control.terminator,
                        &values,
                    )?);
                } else if let Some(result) = &plan.scalar_result {
                    outcome.result = values
                        .locals
                        .iter()
                        .find(|(ordinal, _)| *ordinal == result.binding_ordinal as usize)
                        .and_then(|(_, origin)| *origin);
                }
            }
            Ok(outcome)
        } else if let Some(plan) = composed {
            let state_plan = plan
                .state(state)
                .ok_or_else(|| "completion edge has no checked state".to_owned())?;
            let mut outcome = self.operations(&state_plan.operations, &mut values)?;
            if outcome.returns {
                outcome.returns = false;
                outcome.merge(self.terminator(machine.symbol, &state_plan.terminator, &values)?);
            }
            Ok(outcome)
        } else {
            Err("completion body has no unique checked execution plan".into())
        };
        self.active.pop();
        result
    }

    fn operations(
        &mut self,
        operations: &[Operation],
        values: &mut Values,
    ) -> Result<Outcome, String> {
        let mut outcome = Outcome::returning();
        for operation in operations {
            if !outcome.returns {
                break;
            }
            outcome.returns = false;
            outcome.merge(self.operation(operation, values)?);
        }
        Ok(outcome)
    }

    fn operation(&mut self, operation: &Operation, values: &mut Values) -> Result<Outcome, String> {
        match operation {
            Operation::Call(call) => {
                if !call.structural_arguments.is_empty()
                    || matches!(call.route(), CheckedUnitCallRoute::Dynamic { .. })
                {
                    return Err(
                        "completion review cannot close this call's structural or dynamic operands"
                            .into(),
                    );
                }
                let arguments = call
                    .scalar_arguments
                    .iter()
                    .map(|argument| self.scalar(argument, values))
                    .collect::<Result<Vec<_>, _>>()?;
                let outcome = self.machine(call.target.machine, call.target.state, arguments)?;
                if outcome.returns
                    && let Some(result) = call.scalar_result()
                {
                    values
                        .locals
                        .push((result.binding_ordinal as usize, outcome.result));
                }
                Ok(outcome)
            }
            Operation::EstablishLocal(CheckedUnitLocalBindingPlan::Scalar { result, value }) => {
                let origin = self.scalar(value, values)?;
                values
                    .locals
                    .push((result.binding_ordinal as usize, origin));
                Ok(Outcome::returning())
            }
            Operation::Complete { .. } => Ok(Outcome::returning()),
            _ => Err("completion review has no evaluation rule for this checked operation".into()),
        }
    }

    fn scalar(
        &mut self,
        argument: &CheckedCallScalarArgument,
        values: &Values,
    ) -> Result<Origin, String> {
        match argument {
            CheckedCallScalarArgument::Pure(expression) => Ok(values.pure(expression)),
            CheckedCallScalarArgument::Computation(handle) => {
                let nodes = &self.checked.facts.values.scalar_computations.nodes;
                if !nodes.is_valid(*handle) {
                    return Err("completion operand has no checked computation".into());
                }
                let node = nodes.get(*handle);
                match &node.kind {
                    CheckedScalarComputationKind::Value(expression) => Ok(values.pure(expression)),
                    CheckedScalarComputationKind::Qualification { operand, .. } => {
                        self.scalar(&CheckedCallScalarArgument::Computation(*operand), values)
                    }
                    CheckedScalarComputationKind::Call { .. } => {
                        let result = self.scalar_invocation(&node.kind, values)?;
                        if !result.returns || !result.leaves.is_empty() {
                            return Err("completion operand has an external completion before its consuming invocation".into());
                        }
                        Ok(result.result)
                    }
                    CheckedScalarComputationKind::Apply {
                        expression,
                        operands,
                        ..
                    } => {
                        let computations = &self.checked.facts.values.scalar_computations;
                        let parameters = computations
                            .operands
                            .span(*operands)
                            .ok_or("completion expression has missing checked operand storage")?
                            .iter()
                            .map(|operand| {
                                self.scalar(
                                    &CheckedCallScalarArgument::Computation(*operand),
                                    values,
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok(Values {
                            state: values.state,
                            parameters,
                            locals: Vec::new(),
                        }
                        .pure(expression))
                    }
                    CheckedScalarComputationKind::Select {
                        condition,
                        when_true,
                        when_false,
                        ..
                    } => {
                        self.scalar(&CheckedCallScalarArgument::Computation(*condition), values)?;
                        let when_true = self
                            .scalar(&CheckedCallScalarArgument::Computation(*when_true), values)?;
                        let when_false = self
                            .scalar(&CheckedCallScalarArgument::Computation(*when_false), values)?;
                        Ok(if when_true == when_false {
                            when_true
                        } else {
                            None
                        })
                    }
                    _ => Err(
                        "completion review requires the ordered computed operand's completion"
                            .into(),
                    ),
                }
            }
        }
    }

    fn located_scalar_argument(
        &self,
        statement_ordinal: u32,
        role: checked_trees::CheckedScalarExpressionRole,
        values: &Values,
    ) -> Result<CheckedCallScalarArgument, String> {
        let computations = &self.checked.facts.values.scalar_computations;
        let mut roots =
            computations.roots_for_statement_role(values.state, statement_ordinal, role);
        let pure = self
            .checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(values.state, statement_ordinal, role);
        let value = match (roots.next(), roots.next(), pure) {
            (Some(root), None, None) => CheckedCallScalarArgument::Computation(root.root),
            (None, None, Some((_, expression))) => {
                CheckedCallScalarArgument::Pure(expression.clone())
            }
            _ => return Err("completion has missing or ambiguous checked scalar custody".into()),
        };
        Ok(value)
    }

    fn located_scalar(
        &mut self,
        statement_ordinal: u32,
        role: checked_trees::CheckedScalarExpressionRole,
        values: &Values,
    ) -> Result<Origin, String> {
        let value = self.located_scalar_argument(statement_ordinal, role, values)?;
        self.scalar(&value, values)
    }

    /// A no-normal marker is not an external-completion receipt. Follow its
    /// retained checked call and original actuals through the same selected
    /// provider review; a crash, unresolved call or cycle still refuses.
    fn no_normal_scalar(
        &mut self,
        argument: &CheckedCallScalarArgument,
        values: &Values,
    ) -> Result<Outcome, String> {
        let CheckedCallScalarArgument::Computation(handle) = argument else {
            return Err("nonreturning completion has no checked invocation".into());
        };
        let computations = &self.checked.facts.values.scalar_computations;
        if !computations.nodes.is_valid(*handle) {
            return Err("nonreturning completion has missing checked computation".into());
        }
        match &computations.nodes.get(*handle).kind {
            CheckedScalarComputationKind::Qualification { operand, .. } => {
                self.no_normal_scalar(&CheckedCallScalarArgument::Computation(*operand), values)
            }
            CheckedScalarComputationKind::Call { .. } => {
                let outcome =
                    self.scalar_invocation(&computations.nodes.get(*handle).kind, values)?;
                if outcome.returns {
                    return Err(
                        "nonreturning completion invocation has a normal return path".into(),
                    );
                }
                Ok(outcome)
            }
            _ => Err(
                "completion review cannot authenticate this nonreturning scalar invocation".into(),
            ),
        }
    }

    /// Both receiving and nonreceiving scalar positions use the same checked
    /// call admission and exact argument-origin transport.
    fn scalar_invocation(
        &mut self,
        computation: &CheckedScalarComputationKind,
        values: &Values,
    ) -> Result<Outcome, String> {
        let CheckedScalarComputationKind::Call {
            callee: checked_trees::CheckedScalarCallee::Body { machine, state },
            arguments,
            structural_arguments,
            temporary_cleanups,
            claim_transfers,
            ..
        } = computation
        else {
            return Err("completion review requires an exact checked body invocation".into());
        };
        if !structural_arguments.is_empty()
            || !temporary_cleanups.is_empty()
            || !claim_transfers.is_empty()
        {
            return Err(
                "completion review cannot close this scalar call's structural custody".into(),
            );
        }
        let computations = &self.checked.facts.values.scalar_computations;
        let arguments = computations
            .operands
            .span(*arguments)
            .ok_or("completion call has missing checked operand storage")?
            .iter()
            .map(|argument| self.scalar(&CheckedCallScalarArgument::Computation(*argument), values))
            .collect::<Result<Vec<_>, _>>()?;
        self.machine(*machine, *state, arguments)
    }

    fn scalar_destination(
        &mut self,
        machine: SymbolHandle,
        destination: &checked_trees::CheckedScalarBranchDestination,
        values: &Values,
    ) -> Result<Outcome, String> {
        match destination {
            checked_trees::CheckedScalarBranchDestination::Jump(successor) => {
                self.successor(machine, successor, values)
            }
            checked_trees::CheckedScalarBranchDestination::Crash { .. } => {
                Err("aborting provider does not establish external completion".into())
            }
            checked_trees::CheckedScalarBranchDestination::InvokeNoNormal {
                statement_ordinal,
                is_continuation,
            } => {
                let role = if *is_continuation {
                    checked_trees::CheckedScalarExpressionRole::ContinuationReturn
                } else {
                    checked_trees::CheckedScalarExpressionRole::Return
                };
                let value = self.located_scalar_argument(*statement_ordinal, role, values)?;
                self.no_normal_scalar(&value, values)
            }
            checked_trees::CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation,
            } => {
                let role = if *is_continuation {
                    checked_trees::CheckedScalarExpressionRole::ContinuationReturn
                } else {
                    checked_trees::CheckedScalarExpressionRole::Return
                };
                Ok(Outcome {
                    returns: true,
                    result: self.located_scalar(*statement_ordinal, role, values)?,
                    leaves: Vec::new(),
                })
            }
        }
    }

    fn scalar_terminator(
        &mut self,
        machine: SymbolHandle,
        terminator: &checked_trees::CheckedScalarStateTerminator,
        values: &Values,
    ) -> Result<Outcome, String> {
        use checked_trees::CheckedScalarStateTerminator;
        match terminator {
            CheckedScalarStateTerminator::Return { statement_ordinal } => self.scalar_destination(
                machine,
                &checked_trees::CheckedScalarBranchDestination::Return {
                    statement_ordinal: *statement_ordinal,
                    is_continuation: false,
                },
                values,
            ),
            CheckedScalarStateTerminator::Crash { .. } => {
                Err("aborting provider does not establish external completion".into())
            }
            CheckedScalarStateTerminator::Jump(successor) => {
                self.successor(machine, successor, values)
            }
            CheckedScalarStateTerminator::Conditional {
                guard_statement_ordinal,
                when_true,
                when_false,
            } => {
                self.located_scalar(
                    *guard_statement_ordinal,
                    checked_trees::CheckedScalarExpressionRole::Guard,
                    values,
                )?;
                let mut outcome = self.scalar_destination(machine, when_true, values)?;
                outcome.merge(self.scalar_destination(machine, when_false, values)?);
                Ok(outcome)
            }
            CheckedScalarStateTerminator::Guarded { arms, fallback } => {
                let arms = self
                    .checked
                    .facts
                    .flow
                    .guarded_exits
                    .exits
                    .span(*arms)
                    .ok_or("completion has missing checked guarded exits")?;
                let mut outcome = Outcome::default();
                for arm in arms {
                    self.located_scalar(
                        arm.guard_statement_ordinal,
                        checked_trees::CheckedScalarExpressionRole::Guard,
                        values,
                    )?;
                    outcome.merge(self.scalar_destination(machine, &arm.destination, values)?);
                }
                if let Some(fallback) = fallback {
                    outcome.merge(self.scalar_destination(machine, fallback, values)?);
                }
                Ok(outcome)
            }
        }
    }

    fn successor(
        &mut self,
        machine: SymbolHandle,
        successor: &CheckedStructuralControlSuccessorPlan,
        values: &Values,
    ) -> Result<Outcome, String> {
        let mut values = values.clone();
        let operations: Vec<_> = successor.operand_operations().cloned().collect();
        let mut outcome = self.operations(&operations, &mut values)?;
        if outcome.returns {
            let mut arguments = vec![None; successor.scalar_arguments.len()];
            for argument in &successor.scalar_arguments {
                let origin = match argument.source {
                    checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter {
                        index,
                    } => values.parameters.get(index as usize).copied().flatten(),
                    checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression => {
                        let role = checked_trees::CheckedScalarExpressionRole::TransitionArgument {
                            argument_ordinal: argument.argument_ordinal,
                        };
                        self.located_scalar(successor.statement_ordinal, role, &values)?
                    }
                };
                let slot = arguments
                    .get_mut(argument.target_scalar_parameter_index as usize)
                    .ok_or_else(|| {
                        "completion successor has an invalid scalar parameter position".to_owned()
                    })?;
                *slot = origin;
            }
            outcome.returns = false;
            outcome.merge(self.machine(machine, successor.target_state, arguments)?);
        }
        Ok(outcome)
    }

    fn arm(
        &mut self,
        arm: &CheckedConditionalReturnArm,
        values: &Values,
    ) -> Result<Outcome, String> {
        match arm {
            CheckedConditionalReturnArm::Call {
                operands,
                operation,
                completions,
            } => {
                let mut values = values.clone();
                let operations = operands.iter().cloned().collect::<Vec<_>>();
                let mut outcome = self.operations(&operations, &mut values)?;
                if outcome.returns {
                    outcome.returns = false;
                    outcome.merge(self.operation(operation, &mut values)?);
                }
                if outcome.returns {
                    let completions = completions.iter().cloned().collect::<Vec<_>>();
                    let result = outcome.result;
                    outcome.returns = false;
                    outcome.merge(self.operations(&completions, &mut values)?);
                    if outcome.returns {
                        outcome.result = result;
                    }
                }
                Ok(outcome)
            }
            CheckedConditionalReturnArm::Crash { .. } => {
                Err("aborting provider does not establish external completion".into())
            }
            CheckedConditionalReturnArm::Scalar {
                statement_ordinal, ..
            } => {
                let result = self.located_scalar(
                    *statement_ordinal,
                    checked_trees::CheckedScalarExpressionRole::Return,
                    values,
                )?;
                Ok(Outcome {
                    returns: true,
                    result,
                    leaves: Vec::new(),
                })
            }
            _ => Ok(Outcome::returning()),
        }
    }

    fn terminator(
        &mut self,
        machine: SymbolHandle,
        terminator: &Terminator,
        values: &Values,
    ) -> Result<Outcome, String> {
        match terminator {
            Terminator::Jump { successor } => self.successor(machine, successor, values),
            Terminator::Conditional {
                guard,
                when_true,
                when_false,
            } => {
                self.scalar(guard, values)?;
                let mut outcome = self.successor(machine, when_true, values)?;
                outcome.merge(self.successor(machine, when_false, values)?);
                Ok(outcome)
            }
            Terminator::ConditionalReturn {
                guard,
                jump,
                return_arm,
                ..
            } => {
                self.scalar(guard, values)?;
                let mut outcome = self.successor(machine, jump, values)?;
                outcome.merge(self.arm(return_arm, values)?);
                Ok(outcome)
            }
            Terminator::ConditionalCompletion {
                guard,
                when_true,
                when_false,
            } => {
                self.scalar(guard, values)?;
                let mut outcome = self.arm(when_true, values)?;
                outcome.merge(self.arm(when_false, values)?);
                Ok(outcome)
            }
            Terminator::ReturnScalar { completion } => match completion {
                checked_trees::CheckedScalarReturnPlan::Binding(result) => Ok(Outcome {
                    returns: true,
                    result: values
                        .locals
                        .iter()
                        .find(|(ordinal, _)| *ordinal == result.binding_ordinal as usize)
                        .and_then(|(_, origin)| *origin),
                    leaves: Vec::new(),
                }),
                checked_trees::CheckedScalarReturnPlan::Exits(control) => {
                    self.scalar_terminator(machine, &control.terminator, values)
                }
            },
            Terminator::ReturnUnit | Terminator::ReturnStructural { .. } => {
                Ok(Outcome::returning())
            }
            Terminator::Crash { .. } => {
                Err("aborting provider does not establish external completion".into())
            }
            _ => Err("completion review has no evaluation rule for this checked terminator".into()),
        }
    }
}
