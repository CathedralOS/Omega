//! Checked execution plans for call-bearing scalar writes, guards, arguments, and returns.

use super::*;
use checked_trees::{
    CheckedScalarComputation, CheckedScalarComputationHandle, CheckedScalarComputationKind,
    CheckedScalarComputationPlans, CheckedScalarComputationRoot, FlowFacts, ProofFacts,
};
use symbols::SymbolHandle;

mod call_arguments;
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
            let scalar_parameters = program
                .state_parameters(state)
                .iter()
                .filter(|parameter| {
                    program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
                })
                .cloned()
                .collect::<Vec<_>>();
            let parameters = scalar_parameters.as_slice();
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
                for (call_ordinal, site) in super::call_arguments::nested_structural_call_sites(
                    program,
                    flow,
                    machine,
                    state,
                    statement_index,
                ) {
                    let crate::CallSite::Expression { call, .. } = site else {
                        continue;
                    };
                    let Ok(call_ordinal) = u32::try_from(call_ordinal) else {
                        continue;
                    };
                    builder.record_call_arguments(
                        pure,
                        statement_ordinal,
                        call_ordinal,
                        call.target_symbol,
                        program.expression_table.expression_handles(call.arguments),
                    );
                }
                if let StatementNode::LocalData(local) = statement {
                    let argument_roots = !local.is_mutable
                        && validation::result_initializer_call_is_supported(
                            program,
                            machine,
                            local.initial_value,
                        );
                    if argument_roots
                        && let ExpressionNode::Call(call) =
                            program.expression_table.expression(local.initial_value)
                    {
                        // The result operation owns the outer call. Only its operands
                        // become computations, before the destination enters scope.
                        builder.record_call_arguments(
                            pure,
                            statement_ordinal,
                            0,
                            call.target_symbol,
                            program.expression_table.expression_handles(call.arguments),
                        );
                    }
                    if local.initial_value.is_valid()
                        && let Some(primitive_type) =
                            program.primitive_type_reference(local.type_reference)
                    {
                        let binding_ordinal = u32::try_from(
                            locals
                                .iter()
                                .filter(|local: &&ScalarLocal| !local.is_mutable)
                                .count(),
                        );
                        if let Ok(binding_ordinal) = binding_ordinal {
                            let role = if local.is_mutable {
                                CheckedScalarExpressionRole::StorageInitializer
                            } else {
                                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal }
                            };
                            // Keep the established direct-call binding coordinates when
                            // every argument already has a pure checked plan.
                            if !argument_roots
                                && (local.is_mutable
                                    || !has_pure_call_arguments(
                                        program,
                                        pure,
                                        state.symbol,
                                        statement_ordinal,
                                        binding_ordinal,
                                        local.initial_value,
                                    ))
                            {
                                builder.record_root(
                                    pure,
                                    statement_ordinal,
                                    role,
                                    local.initial_value,
                                    primitive_type,
                                );
                            }
                        }
                        // The initializer may read earlier locals, never its own binding.
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
                if let StatementNode::Call(call) = statement {
                    builder.record_call_arguments(
                        pure,
                        statement_ordinal,
                        0,
                        call.target_symbol,
                        program.statement_table.expression_handles(call.arguments),
                    );
                    continue;
                }
                if let StatementNode::Expression(expression) = statement
                    && validation::unit_return_call_is_supported(
                        program,
                        machine,
                        state,
                        *expression,
                    )
                    && let ExpressionNode::Call(call) =
                        program.expression_table.expression(*expression)
                {
                    builder.record_call_arguments(
                        pure,
                        statement_ordinal,
                        0,
                        call.target_symbol,
                        program.expression_table.expression_handles(call.arguments),
                    );
                    continue;
                }
                if let StatementNode::Assignment(assignment) = statement {
                    if let ExpressionNode::Name(name) =
                        program.expression_table.expression(assignment.target)
                        && name.symbol.is_valid()
                        && name.head_symbol == name.symbol
                        && let Some(primitive_type) = locals
                            .iter()
                            .find(|local| local.symbol == name.symbol && local.is_mutable)
                            .map(|local| local.primitive_type)
                            .or_else(|| {
                                parameters
                                    .iter()
                                    .find(|parameter| parameter.symbol == name.symbol)
                                    .and_then(|parameter| {
                                        crate::values::mutable_scalar_parameter_type(
                                            program, parameter,
                                        )
                                    })
                            })
                    {
                        // The completed RHS replaces storage only after evaluation.
                        // Reads here retain the existing destination symbol; assignments
                        // neither append an immutable local nor change its namespace.
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            CheckedScalarExpressionRole::AssignmentValue,
                            assignment.value,
                            primitive_type,
                        );
                    }
                    continue;
                }
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
                if let typed_trees::statement::TransitionGuardNode::When(expression) =
                    transition.guard
                {
                    builder.record_root(
                        pure,
                        statement_ordinal,
                        CheckedScalarExpressionRole::Guard,
                        expression,
                        PrimitiveType::Bool,
                    );
                }
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

fn has_pure_call_arguments(
    program: &TypedTrees,
    pure: &CheckedScalarExpressionPlans,
    state: SymbolHandle,
    statement_ordinal: u32,
    binding_ordinal: u32,
    expression: ExpressionHandle,
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
        return false;
    }
    let Some(parameters) = crate::call_target_parameters(program, call.target_symbol) else {
        return false;
    };
    let arguments = program.expression_table.expression_handles(call.arguments);
    arguments.len() == parameters.len()
        && !parameters.iter().any(|parameter| {
            parameter.is_self
                || parameter.is_const
                || (parameter.is_mutable
                    && crate::values::mutable_scalar_parameter_type(program, parameter).is_none())
        })
        && arguments.iter().enumerate().all(|(index, _)| {
            u32::try_from(index).ok().is_some_and(|argument_ordinal| {
                pure.expression_at(
                    state,
                    statement_ordinal,
                    CheckedScalarExpressionRole::CallArgument {
                        binding_ordinal,
                        argument_ordinal,
                    },
                )
                .is_some()
            })
        })
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
                if !call.machine_arguments.is_empty()
                    || !call.evidence_arguments.is_empty()
                    || call.static_requirement_dispatch.is_some()
                    || call.quotient_operation.is_some()
                    || call.private_layout_operation.is_some()
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
                if !self
                    .program
                    .call_has_no_runtime_receiver(&call, target_machine, target_state)
                {
                    return None;
                }
                if self
                    .program
                    .primitive_type_reference(target_state.return_type)?
                    != expected_type
                {
                    return None;
                }
                let target_parameters = self.program.state_parameters(target_state);
                if target_parameters.iter().any(|parameter| {
                    parameter.is_self
                        || parameter.is_const
                        || (parameter.is_mutable
                            && crate::values::mutable_scalar_parameter_type(
                                self.program,
                                parameter,
                            )
                            .is_none())
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
        let mut matching = self
            .flow
            .control
            .calls
            .span_or_empty(state.calls)
            .iter()
            .filter(|call| {
                call.statement_index == self.statement_index
                    && call.target_symbol == target
                    && call.authored_expression == expression
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
