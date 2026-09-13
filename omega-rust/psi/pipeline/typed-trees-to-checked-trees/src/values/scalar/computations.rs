//! Checked execution plans for call-bearing scalar writes, guards, arguments, and returns.
//!
//! A known normal-return result is not an effect-free computation. Short-circuit
//! selection may omit an unreachable RHS, but must retain the evaluated left
//! graph, including calls whose result becomes known only at a later selection.
//! Keep exact source occurrences on retained applications and selections so
//! folding an enclosing guard does not change their operand custody.

use super::*;
use checked_trees::{
    CheckedScalarComputation, CheckedScalarComputationHandle, CheckedScalarComputationKind,
    CheckedScalarComputationPlans, CheckedScalarComputationRoot, FlowFacts, ProofFacts,
};
use symbols::SymbolHandle;

mod call_arguments;
mod cases;
mod dispatch;
mod integers;
mod normal_return;
mod structural_fields;
mod structural_values;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) fn build_checked_scalar_computation_plans(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    flow: &FlowFacts,
    borrow: &checked_trees::BorrowFacts,
    proof: &ProofFacts,
    pure: &CheckedScalarExpressionPlans,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> CheckedScalarComputationPlans {
    build_checked_value_computation_plans(
        program,
        operators,
        flow,
        borrow,
        proof,
        pure,
        exact_integer_casts,
    )
    .0
}

pub(crate) fn build_checked_value_computation_plans(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    flow: &FlowFacts,
    borrow: &checked_trees::BorrowFacts,
    proof: &ProofFacts,
    pure: &CheckedScalarExpressionPlans,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> (
    CheckedScalarComputationPlans,
    checked_trees::CheckedStructuralValuePlans,
) {
    let mut plans = CheckedScalarComputationPlans::default();
    let mut structural_values = checked_trees::CheckedStructuralValuePlans::default();
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
                    borrow,
                    exact_integer_casts,
                    machine: machine.symbol,
                    state: state.symbol,
                    statement_index,
                    parameters,
                    authored_parameters: program.state_parameters(state),
                    parameter_types: &parameter_types,
                    locals: &locals,
                    plans: &mut plans,
                };
                let construction_destination = match statement {
                    StatementNode::LocalData(local) => {
                        Some((local.initial_value, local.type_reference))
                    }
                    StatementNode::Expression(expression) => Some((*expression, state.return_type)),
                    _ => None,
                };
                // A whole owned place is a structural value just like a
                // constructor operand. Let the same builder check its selected
                // type and storage; local copies must not lose their operation
                // merely because they are the root of an initializer.
                // A whole returned reference instead belongs to reference
                // completion's affine establishment and exact ingress map.
                // Reference nodes remain valid inside record constructors.
                if let Some((expression, expected)) = construction_destination
                    && (validation::is_scalar_case_value(program, expression, expected)
                        || (validation::reference_result_custody::parts(program, expected)
                            .is_none()
                            && matches!(
                                program.expression_table.expression(expression),
                                ExpressionNode::Name(_)
                            ))
                        || matches!(program.expression_table.expression(expression), ExpressionNode::StructLiteral(literal) if literal.case_symbol.is_none()))
                    && let Some(root) =
                        builder.structural_value(expression, expected, &mut structural_values, pure)
                {
                    structural_values
                        .roots
                        .append(checked_trees::CheckedStructuralValueRoot {
                            machine: machine.symbol,
                            state: state.symbol,
                            statement_ordinal,
                            expression,
                            type_reference: expected,
                            root,
                        });
                }
                if let Some((expression, expected)) = construction_destination
                    && let Some(elements) = validation::scalar_array_elements(
                        program,
                        machine.symbol,
                        expression,
                        expected,
                    )
                {
                    for (element_index, (element, primitive_type)) in
                        elements.elements.into_iter().enumerate()
                    {
                        let Ok(element_ordinal) = u32::try_from(element_index) else {
                            break;
                        };
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            CheckedScalarExpressionRole::ArrayElement {
                                source: checked_trees::CheckedArrayConstructionSource::Statement,
                                element_ordinal,
                            },
                            element,
                            primitive_type,
                        );
                    }
                }
                for construction in
                    call_array_constructions(program, flow, machine, state, statement_index)
                {
                    let Some(array) = validation::scalar_array_elements(
                        program,
                        machine.symbol,
                        construction.expression,
                        construction.type_reference,
                    ) else {
                        continue;
                    };
                    for (element_index, (element, primitive_type)) in
                        array.elements.into_iter().enumerate()
                    {
                        let Ok(element_ordinal) = u32::try_from(element_index) else {
                            break;
                        };
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            CheckedScalarExpressionRole::ArrayElement {
                                source: construction.source,
                                element_ordinal,
                            },
                            element,
                            primitive_type,
                        );
                    }
                }
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
                            // A result operation owns its outer call; otherwise the
                            // ordinary computation must retain it even when all
                            // arguments are pure. Purity of operands cannot route
                            // a call away from state-local operation sequencing.
                            if !argument_roots {
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
                    if validation::unit_statement_call_is_supported(
                        program,
                        machine,
                        state,
                        *expression,
                    ) {
                        continue;
                    }
                }
                if let StatementNode::Assignment(assignment) = statement {
                    if matches!(
                        program.expression_table.expression(assignment.target),
                        ExpressionNode::Member(_)
                    ) && let Some(primitive_type) = validation::declared_place_type_raw(
                        program,
                        machine,
                        Some(state),
                        assignment.target,
                    )
                    .and_then(|reference| program.primitive_type_reference(reference))
                    {
                        builder.record_root(
                            pure,
                            statement_ordinal,
                            CheckedScalarExpressionRole::AssignmentValue,
                            assignment.value,
                            primitive_type,
                        );
                    }
                    if let ExpressionNode::Name(name) =
                        program.expression_table.expression(assignment.target)
                        && name.symbol.is_valid()
                        && name.head_symbol == name.symbol
                        && let Some(primitive_type) = locals
                            .iter()
                            .find(|local| local.symbol == name.symbol && local.is_mutable)
                            .map(|local| local.primitive_type)
                            .or_else(|| {
                                // Storage destinations use the authored parameter roster;
                                // dense scalar inputs deliberately exclude reference parameters.
                                program
                                    .state_parameters(state)
                                    .iter()
                                    .find(|parameter| parameter.symbol == name.symbol)
                                    .filter(|parameter| {
                                        parameter.is_mutable
                                            && !parameter.is_self
                                            && !parameter.is_const
                                    })
                                    .and_then(|parameter| {
                                        super::assignment_target_primitive_type(
                                            program,
                                            parameter.type_reference,
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
                        crate::checks::termination::named_transition_target_state_index(
                            program,
                            machine,
                            path.symbol,
                        )
                        .and_then(|target_index| states.get(target_index))
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
    (plans, structural_values)
}

struct Builder<'program, 'plans> {
    program: &'program TypedTrees,
    operators: &'program CheckedOperatorFacts,
    flow: &'program FlowFacts,
    borrow: &'program checked_trees::BorrowFacts,
    exact_integer_casts: &'program [validation::ExactIntegerCastFact],
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    parameters: &'program [StateParameter],
    authored_parameters: &'program [StateParameter],
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
        // Structural value traversal and the ordinary call-operand roster can
        // reach the same authored operand. Retain its computation once; a
        // different source or type at this coordinate must remain a conflicting
        // root for independent custody checking to reject.
        if let Some(existing) = self.plans.root_at(self.state, statement_ordinal, role)
            && existing.machine == self.machine
            && self.plans.nodes.is_valid(existing.root)
            && self.plans.nodes.get(existing.root).authored_root == expression
            && self.plans.nodes.get(existing.root).primitive_type == expected_type
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
            value_source: ExpressionHandle::invalid(),
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
        if let Some(field) = self.local_scalar_record_field(expression) {
            return (field.primitive_type == expected_type)
                .then(|| self.structural_field(expression, field));
        }
        if let ExpressionNode::Cast(cast) =
            self.program.expression_table.expression(expression).clone()
            && cast.semantic_domain.is_empty()
            && let Some(source_type) =
                semantic_casts::result_type(self.program, self.state, cast.value)
            && semantic_casts::has_declared_domains(self.program, source_type)
        {
            if cast.form.is_recast()
                || cast.domain != ArithmeticDomain::Exact
                || !matches!(
                    self.program
                        .type_reference_table
                        .type_reference(cast.target_type),
                    TypeReferenceNode::Named { .. }
                )
                || !semantic_casts::has_only_vacuous_tags(self.program, source_type)
                || self.program.primitive_type_reference(source_type) != Some(expected_type)
                || self.program.primitive_type_reference(cast.target_type) != Some(expected_type)
            {
                return None;
            }
            let result_type = semantic_casts::result_type(self.program, self.state, expression)?;
            if semantic_casts::has_declared_domains(self.program, result_type) {
                return None;
            }
            let operand = self.expression(cast.value, expected_type)?;
            return Some(self.insert(
                expected_type,
                CheckedScalarComputationKind::Qualification {
                    source_expression: expression,
                    operand,
                    result_type,
                },
            ));
        }
        if let ExpressionNode::Cast(cast) =
            self.program.expression_table.expression(expression).clone()
            && !cast.semantic_domain.is_empty()
        {
            // Qualification changes semantic custody, not payload. It cannot
            // become a pure expression whose source marker erases the cast.
            // Final checked qualification facts are built after this graph;
            // share their declaration rule and let the consumer rejoin the
            // completed exact use fact before any publication.
            if cast.form.is_recast()
                || !matches!(
                    self.program
                        .type_reference_table
                        .type_reference(cast.target_type),
                    typed_trees::types::TypeReferenceNode::Named { .. }
                )
                || cast.domain != ArithmeticDomain::Exact
                || !cast.semantic_domain_id.is_valid()
                || !crate::facts::domain_is_vacuous(
                    self.program,
                    cast.semantic_domain_symbol,
                    &mut Vec::new(),
                )
                || self.program.primitive_type_reference(cast.target_type) != Some(expected_type)
                || self.program.primitive_type_reference(cast.result_type) != Some(expected_type)
            {
                return None;
            }
            let operand = self.expression(cast.value, expected_type)?;
            return Some(self.insert(
                expected_type,
                CheckedScalarComputationKind::Qualification {
                    source_expression: expression,
                    operand,
                    result_type: cast.result_type,
                },
            ));
        }
        if let ExpressionNode::Match(dispatch) =
            self.program.expression_table.expression(expression).clone()
        {
            return self.dispatch(expression, &dispatch, expected_type);
        }
        if expected_type == PrimitiveType::Bool
            && let Some(operator_use) = self.comparison_use(
                expression,
                checked_trees::CheckedOperatorOccurrence::Expression,
            )
        {
            let (_, primitive) = self
                .operators
                .selected_float_comparison(self.program, operator_use)?;
            let operands = self
                .operators
                .uses
                .get(operator_use)
                .operands(self.program)?;
            let left = self.expression(operands[0], primitive)?;
            let right = self.expression(operands[1], primitive)?;
            return Some(self.insert(
                PrimitiveType::Bool,
                CheckedScalarComputationKind::SelectedComparison {
                    operator_use,
                    left,
                    right,
                },
            ));
        }
        if !semantic_casts::requires_custody(self.program, self.state, expression)
            && let Some(value) = lower_return_expression(
                self.program,
                self.operators,
                expression,
                self.parameters,
                self.authored_parameters,
                self.parameter_types,
                self.locals,
                expected_type,
                self.exact_integer_casts,
            )
        {
            let computation =
                self.insert(expected_type, CheckedScalarComputationKind::Value(value));
            self.plans.nodes.get_mut(computation).value_source = expression;
            return Some(computation);
        }
        if expected_type == PrimitiveType::Bool
            && let Some(membership) = self.case_membership(expression)
        {
            return Some(membership);
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
                let has_runtime_receiver =
                    !self
                        .program
                        .call_has_no_runtime_receiver(&call, target_machine, target_state);
                if self
                    .program
                    .primitive_type_reference(target_state.return_type)?
                    != expected_type
                {
                    return None;
                }
                let target_parameters = self.program.state_parameters(target_state);
                if target_parameters.iter().any(|parameter| {
                    (parameter.is_self && !has_runtime_receiver)
                        || parameter.is_const
                        || (parameter.is_mutable
                            && self
                                .program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some()
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
                let mut arguments = self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec();
                if has_runtime_receiver {
                    // The receiver is an ordinary structural operand, ordered
                    // before explicit actuals, not ambient attachment storage.
                    if !call.receiver.is_valid()
                        || !target_parameters
                            .first()
                            .is_some_and(|parameter| parameter.is_self)
                        || target_parameters
                            .iter()
                            .filter(|parameter| parameter.is_self)
                            .count()
                            != 1
                    {
                        return None;
                    }
                    arguments.insert(0, call.receiver);
                }
                if arguments.len() != target_parameters.len() {
                    return None;
                }
                let mut computed_arguments = Vec::with_capacity(arguments.len());
                let mut structural_arguments = Vec::new();
                for (argument, parameter) in arguments.iter().zip(target_parameters) {
                    if let Some(primitive_type) = self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                    {
                        let root = self.expression(*argument, primitive_type)?;
                        self.plans.nodes.get_mut(root).authored_root = *argument;
                        computed_arguments.push(root);
                    } else {
                        if let Some(array) = validation::scalar_array_elements(
                            self.program,
                            self.machine,
                            *argument,
                            parameter.type_reference,
                        ) {
                            if parameter.is_mutable
                                || target_machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                                || array.projections.iter().any(|projection| {
                                    self.operators.expression_use(*projection).is_some_and(|selected| {
                                        selected.spelling != language_core::OperatorSpelling::Index
                                            || selected.selected_operator_symbol.is_valid()
                                            || selected.candidate_count != 0
                                            || !matches!(selected.status,
                                                CheckedOperatorResolutionStatus::Missing
                                                    | CheckedOperatorResolutionStatus::BuiltinFallback)
                                    })
                                })
                            {
                                return None;
                            }
                            let mut elements = Vec::with_capacity(array.elements.len());
                            for (expression, primitive_type) in array.elements {
                                let element = self.expression(expression, primitive_type)?;
                                self.plans.nodes.get_mut(element).authored_root = expression;
                                elements.push(element);
                            }
                            let elements = self.plans.operands.insert_many(elements);
                            structural_arguments.push(
                                checked_trees::CheckedScalarComputationStructuralArgument::Array {
                                    expression: *argument,
                                    type_reference: parameter.type_reference,
                                    elements,
                                },
                            );
                            continue;
                        }
                        let state =
                            crate::find_state_in_machine(self.program, self.machine, self.state)?;
                        structural_arguments.push(
                            checked_trees::CheckedScalarComputationStructuralArgument::Place(
                                crate::flow::structural_computation_argument(
                                    self.program,
                                    self.borrow,
                                    self.machine,
                                    state,
                                    self.flow.control.calls.get(source_call),
                                    *argument,
                                    parameter,
                                )?,
                            ),
                        );
                    }
                }
                let arguments = self.plans.operands.insert_many(computed_arguments);
                let structural_arguments = self
                    .plans
                    .structural_arguments
                    .insert_many(structural_arguments);
                Some(self.insert(
                    expected_type,
                    CheckedScalarComputationKind::Call {
                        target_machine: target_machine.symbol,
                        target_state: target_state.symbol,
                        source_call,
                        call_ordinal,
                        arguments,
                        structural_arguments,
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
                if normal_return::boolean_result(self.plans, condition) == Some(!evaluate_when) {
                    // Unlike replacing this with a constant, returning the
                    // original graph preserves every effect leading to its result.
                    return Some(condition);
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
                        source_expression: expression,
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
                        source_expression: expression,
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
                if let Some(comparison) = self.integer_comparison(expression, &binary) {
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
                        source_expression: expression,
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
