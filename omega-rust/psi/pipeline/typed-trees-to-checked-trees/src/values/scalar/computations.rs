//! Checked execution plans for call-bearing scalar arguments and returns.

use super::*;
use checked_trees::{
    CheckedScalarComputation, CheckedScalarComputationHandle, CheckedScalarComputationKind,
    CheckedScalarComputationPlans, CheckedScalarComputationRoot, FlowFacts, ProofFacts,
};
use symbols::SymbolHandle;

mod integers;
#[cfg(test)]
mod tests;

pub(crate) fn build_checked_scalar_computation_plans(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    flow: &FlowFacts,
    proof: &ProofFacts,
    pure: &CheckedScalarExpressionPlans,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> CheckedScalarComputationPlans {
    let mut plans = CheckedScalarComputationPlans::default();
    for machine in program.machines() {
        // Existing named-output emission joins statement binding positions.
        // It must not silently reinterpret computation-local call positions.
        if proof.proof_output_calls.iter().any(|(_, call)| {
            call.caller_machine_symbol == machine.symbol && call.runtime_call.is_some()
        }) {
            continue;
        }
        let states = program.machine_states(machine);
        for state in states {
            let parameters = program.state_parameters(state);
            if parameters.iter().any(|parameter| parameter.is_self) {
                continue;
            }
            let Some(parameter_types) = parameters
                .iter()
                .map(|parameter| program.primitive_type_reference(parameter.type_reference))
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let mut locals = Vec::new();
            let statements = program.statement_table.statements(state.statement_nodes);
            for (statement_index, statement) in statements.iter().enumerate() {
                if let StatementNode::LocalData(local) = statement {
                    if let Some(primitive_type) =
                        program.primitive_type_reference(local.type_reference)
                    {
                        locals.push(ScalarLocal {
                            is_mutable: local.is_mutable,
                            symbol: local.symbol,
                            name: local.name.as_str().to_owned(),
                            primitive_type,
                            arithmetic_domain: program
                                .arithmetic_domain_for_type_reference(local.type_reference),
                        });
                    }
                    continue;
                }
                let Ok(statement_ordinal) = u32::try_from(statement_index) else {
                    continue;
                };
                let mut builder = Builder {
                    program,
                    operators,
                    flow,
                    exact_integer_casts,
                    machine: machine.symbol,
                    state: state.symbol,
                    statement_index,
                    parameters,
                    parameter_types: &parameter_types,
                    locals: &locals,
                    plans: &mut plans,
                };
                if let StatementNode::Expression(expression) = statement
                    && statement_index + 1 == statements.len()
                    && let Some(result_type) = program.primitive_type_reference(state.return_type)
                {
                    builder.record_root(
                        pure,
                        statement_ordinal,
                        CheckedScalarExpressionRole::Return,
                        *expression,
                        result_type,
                    );
                }
                let StatementNode::Transition(transition) = statement else {
                    continue;
                };
                for (target, continuation) in
                    [(transition.target, false), (transition.continuation, true)]
                {
                    if !target.is_valid() {
                        continue;
                    }
                    if transition.exit == typed_trees::statement::TransitionExit::Ordinary
                        && let TransitionTargetNode::Value(expression) =
                            program.statement_table.transition_target(target)
                        && let Some(result_type) =
                            program.primitive_type_reference(state.return_type)
                    {
                        let role = if continuation {
                            CheckedScalarExpressionRole::ContinuationReturn
                        } else {
                            CheckedScalarExpressionRole::Return
                        };
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            role,
                            *expression,
                            result_type,
                        );
                    }
                    let TransitionTargetNode::Named {
                        path,
                        arguments,
                        evidence_arguments,
                        ..
                    } = program.statement_table.transition_target(target)
                    else {
                        continue;
                    };
                    if !evidence_arguments.is_empty() {
                        continue;
                    }
                    let Some(target_state) =
                        states.iter().find(|state| state.symbol == path.symbol)
                    else {
                        continue;
                    };
                    let target_parameters = program.state_parameters(target_state);
                    let arguments = program.statement_table.expression_handles(*arguments);
                    if arguments.len() != target_parameters.len() {
                        continue;
                    }
                    for (argument_index, (argument, parameter)) in
                        arguments.iter().zip(target_parameters).enumerate()
                    {
                        let Ok(argument_ordinal) = u32::try_from(argument_index) else {
                            continue;
                        };
                        let role = if continuation {
                            CheckedScalarExpressionRole::TransitionContinuationArgument {
                                argument_ordinal,
                            }
                        } else {
                            CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
                        };
                        let Some(expected_type) =
                            program.primitive_type_reference(parameter.type_reference)
                        else {
                            continue;
                        };
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            role,
                            *argument,
                            expected_type,
                        );
                    }
                }
            }
        }
    }
    plans
}

struct Builder<'program, 'plans> {
    program: &'program TypedTrees,
    operators: &'program CheckedOperatorFacts,
    flow: &'program FlowFacts,
    exact_integer_casts: &'program [validation::ExactIntegerCastFact],
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    parameters: &'program [StateParameter],
    parameter_types: &'program [PrimitiveType],
    locals: &'program [ScalarLocal],
    plans: &'plans mut CheckedScalarComputationPlans,
}

impl Builder<'_, '_> {
    fn record_root(
        &mut self,
        pure: &CheckedScalarExpressionPlans,
        statement_ordinal: u32,
        role: CheckedScalarExpressionRole,
        expression: ExpressionHandle,
        expected_type: PrimitiveType,
    ) {
        if pure
            .expression_at(self.state, statement_ordinal, role)
            .is_some()
        {
            return;
        }
        if let Some(root) = self.expression(expression, expected_type) {
            self.plans.nodes.get_mut(root).authored_root = expression;
            self.plans.roots.append(CheckedScalarComputationRoot {
                machine: self.machine,
                state: self.state,
                statement_ordinal,
                role,
                root,
            });
        }
    }

    fn insert(
        &mut self,
        primitive_type: PrimitiveType,
        kind: CheckedScalarComputationKind,
    ) -> CheckedScalarComputationHandle {
        self.plans.nodes.append(CheckedScalarComputation {
            authored_root: ExpressionHandle::invalid(),
            primitive_type,
            kind,
        })
    }

    fn boolean(&mut self, value: bool) -> CheckedScalarComputationHandle {
        self.insert(
            PrimitiveType::Bool,
            CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::Constant(value),
            ))),
        )
    }

    fn expression(
        &mut self,
        expression: ExpressionHandle,
        expected_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        if let Some(value) = lower_return_expression(
            self.program,
            self.operators,
            expression,
            self.parameters,
            self.parameter_types,
            self.locals,
            expected_type,
            self.exact_integer_casts,
        ) {
            return Some(self.insert(expected_type, CheckedScalarComputationKind::Value(value)));
        }
        if is_integer(expected_type)
            && !matches!(
                self.program.expression_table.expression(expression),
                ExpressionNode::Call(_)
            )
        {
            let integer = self.integer_operand(expression)?;
            if scalar_expression_type(&integer.value)? != expected_type {
                return None;
            }
            return self.materialize_integer(integer);
        }
        match self.program.expression_table.expression(expression).clone() {
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid()
                    || !call.machine_arguments.is_empty()
                    || !call.evidence_arguments.is_empty()
                    || call.static_requirement_dispatch.is_some()
                {
                    return None;
                }
                let target_machine = self.program.machines().iter().find(|machine| {
                    self.program
                        .machine_states(machine)
                        .first()
                        .is_some_and(|state| state.symbol == call.target_symbol)
                })?;
                let target_state = self.program.machine_states(target_machine).first()?;
                if self
                    .program
                    .primitive_type_reference(target_state.return_type)?
                    != expected_type
                {
                    return None;
                }
                let target_parameters = self.program.state_parameters(target_state);
                if target_parameters.iter().any(|parameter| {
                    parameter.is_self || parameter.is_const || parameter.is_mutable
                }) {
                    return None;
                }
                let (source_call, call_ordinal) =
                    self.call_ordinal(expression, call.target_symbol)?;
                let arguments = self
                    .program
                    .expression_table
                    .expression_handles(call.arguments);
                if arguments.len() != target_parameters.len() {
                    return None;
                }
                let mut computed_arguments = Vec::with_capacity(arguments.len());
                for (argument, parameter) in arguments.iter().zip(target_parameters) {
                    let primitive_type = self
                        .program
                        .primitive_type_reference(parameter.type_reference)?;
                    computed_arguments.push(self.expression(*argument, primitive_type)?);
                }
                let arguments = self.plans.operands.insert_many(computed_arguments);
                Some(self.insert(
                    expected_type,
                    CheckedScalarComputationKind::Call {
                        target_machine: target_machine.symbol,
                        target_state: target_state.symbol,
                        source_call,
                        call_ordinal,
                        arguments,
                    },
                ))
            }
            ExpressionNode::Binary(binary)
                if expected_type == PrimitiveType::Bool
                    && operator_is_builtin(self.operators, expression)
                    && matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
            {
                let condition = self.expression(binary.left, PrimitiveType::Bool)?;
                let evaluate_when = binary.operator == BinaryOperator::And;
                // A known skipped RHS has no FlowCallFact and must not need one.
                if let CheckedScalarComputationKind::Value(value) =
                    &self.plans.nodes.get(condition).kind
                    && let Some(facts::ScalarValue::Boolean(value)) =
                        crate::values::evaluate_checked_scalar(value, &mut |_| None)
                {
                    return if value == evaluate_when {
                        self.expression(binary.right, PrimitiveType::Bool)
                    } else {
                        Some(condition)
                    };
                }
                let right = self.expression(binary.right, PrimitiveType::Bool)?;
                let skipped = self.boolean(!evaluate_when);
                let (when_true, when_false) = if evaluate_when {
                    (right, skipped)
                } else {
                    (skipped, right)
                };
                Some(self.insert(
                    PrimitiveType::Bool,
                    CheckedScalarComputationKind::Select {
                        condition,
                        when_true,
                        when_false,
                    },
                ))
            }
            ExpressionNode::Unary(unary)
                if expected_type == PrimitiveType::Bool
                    && unary.operator == UnaryOperator::LogicalNot
                    && operator_is_builtin(self.operators, expression) =>
            {
                let operand = self.expression(unary.operand, PrimitiveType::Bool)?;
                let operands = self.plans.operands.insert_many([operand]);
                Some(self.insert(
                    PrimitiveType::Bool,
                    CheckedScalarComputationKind::Apply {
                        expression: CheckedScalarExpression::Boolean(Box::new(
                            CheckedBooleanExpression::Not(Box::new(
                                CheckedBooleanExpression::Parameter { position: 0 },
                            )),
                        )),
                        operands,
                    },
                ))
            }
            ExpressionNode::Binary(binary)
                if expected_type == PrimitiveType::Bool
                    && matches!(
                        binary.operator,
                        BinaryOperator::Equal
                            | BinaryOperator::NotEqual
                            | BinaryOperator::Less
                            | BinaryOperator::LessOrEqual
                            | BinaryOperator::Greater
                            | BinaryOperator::GreaterOrEqual
                    )
                    && operator_is_builtin(self.operators, expression) =>
            {
                if let Some(comparison) = self.integer_comparison(&binary) {
                    return Some(comparison);
                }
                if !matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) {
                    return None;
                }
                let left = self.expression(binary.left, PrimitiveType::Bool)?;
                let right = self.expression(binary.right, PrimitiveType::Bool)?;
                let operands = self.plans.operands.insert_many([left, right]);
                let mut template = CheckedBooleanExpression::Equal {
                    left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
                    right: Box::new(CheckedBooleanExpression::Parameter { position: 1 }),
                };
                if binary.operator == BinaryOperator::NotEqual {
                    template = CheckedBooleanExpression::Not(Box::new(template));
                }
                Some(self.insert(
                    PrimitiveType::Bool,
                    CheckedScalarComputationKind::Apply {
                        expression: CheckedScalarExpression::Boolean(Box::new(template)),
                        operands,
                    },
                ))
            }
            _ => None,
        }
    }

    fn call_ordinal(
        &self,
        expression: ExpressionHandle,
        target: SymbolHandle,
    ) -> Option<(arena::Handle<checked_trees::FlowCallFact>, u32)> {
        let state = self.flow.control.states.iter().find_map(|(_, state)| {
            (state.machine_symbol == self.machine && state.state_symbol == self.state)
                .then_some(state)
        })?;
        let mut matching = self.flow.control.calls.span_or_empty(state.calls).iter().filter(|call| {
            call.statement_index == self.statement_index && call.target_symbol == target
                && matches!(crate::semantic_calls::find_call_site(
                    self.program, self.machine, self.state, self.statement_index, call.call_ordinal,
                ), Some(crate::CallSite::Expression { expression: candidate, .. }) if candidate == expression)
        });
        let call = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        let handle = self
            .flow
            .control
            .calls
            .iter()
            .find_map(|(handle, candidate)| std::ptr::eq(candidate, call).then_some(handle))?;
        Some((handle, u32::try_from(call.call_ordinal).ok()?))
    }
}
