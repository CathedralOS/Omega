//! builder slice of computations.

use super::{boundary_calls, integers, normal_return};
use crate::values::operator_is_builtin;
use crate::values::scalar::expression_facts::is_integer;
use crate::values::scalar::expression_plans::ScalarLocal;
use crate::values::scalar::scalar_lowering::lower_return_expression;
use crate::values::scalar::semantic_casts;
use crate::values::scalar_expression_type;
use checked_trees::CheckedBooleanExpression;
use checked_trees::CheckedOperatorFacts;
use checked_trees::CheckedScalarExpression;
use checked_trees::CheckedScalarExpressionPlans;
use checked_trees::CheckedScalarExpressionRole;
use checked_trees::{
    CheckedScalarComputation, CheckedScalarComputationHandle, CheckedScalarComputationKind,
    CheckedScalarComputationPlans, CheckedScalarComputationRoot, FlowFacts,
};
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::BinaryOperator;
use typed_trees::expression::ExpressionHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::expression::TableCastExpression;
use typed_trees::expression::UnaryOperator;
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;
use typed_trees::types::TypeReferenceNode;

pub(super) struct Builder<'program, 'plans> {
    pub(super) program: &'program TypedTrees,
    pub(super) operators: &'program CheckedOperatorFacts,
    pub(super) flow: &'program FlowFacts,
    pub(super) borrow: &'program checked_trees::BorrowFacts,
    pub(super) pure: &'program CheckedScalarExpressionPlans,
    pub(super) exact_integer_casts: &'program [validation::ExactIntegerCastFact],
    pub(super) machine: SymbolHandle,
    pub(super) state: SymbolHandle,
    pub(super) statement_index: usize,
    pub(super) parameters: &'program [StateParameter],
    pub(super) authored_parameters: &'program [StateParameter],
    pub(super) parameter_types: &'program [PrimitiveType],
    pub(super) locals: &'program [ScalarLocal],
    pub(super) plans: &'plans mut CheckedScalarComputationPlans,
    pub(super) structural_values: &'plans mut checked_trees::CheckedStructuralValuePlans,
    /// One statement's normalized source DAG may reference an operand more
    /// than once. Retain its computation identity, including the contextual
    /// carrier for anonymous integers; never merge different authored handles.
    pub(super) integer_occurrences: Vec<(
        ExpressionHandle,
        Option<PrimitiveType>,
        integers::IntegerOperand,
    )>,
}

impl Builder<'_, '_> {
    pub(super) fn record_root(
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
            || self.statement_owns_boundary_call(role, expression)
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

    /// An authored conversion that cannot travel the pure lane still needs an
    /// exact node at its operand coordinate: the qualification arms above
    /// admit only representation-identical transfers, while custody positions
    /// such as call arguments read their evidence from this graph. The
    /// operand lowers at its own primitive; the node carries the cast's
    /// destination carrier and authored domain for consumers and the
    /// qualification-facts rejoin.
    pub(super) fn authored_cast(
        &mut self,
        expression: ExpressionHandle,
        cast: &TableCastExpression,
        expected_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        if !semantic_casts::requires_custody(self.program, self.state, expression) {
            return None;
        }
        let operand = if let Some(source_primitive) =
            semantic_casts::result_type(self.program, self.state, cast.value)
                .and_then(|reference| self.program.primitive_type_reference(reference))
        {
            self.expression(cast.value, source_primitive)?
        } else {
            // The explicit cast supplies the carrier for an anonymous integer
            // expression. Preserve its exact landing and source occurrence;
            // the qualification remains an authored cast with its own proof.
            let target = self.program.primitive_type_reference(cast.target_type)?;
            let value = super::super::scalar_lowering::land_anonymous_scalar_expression(
                self.program,
                self.operators,
                cast.value,
                target,
            )?;
            let operand = self.insert(target, CheckedScalarComputationKind::Value(value));
            self.plans.nodes.get_mut(operand).value_source = cast.value;
            operand
        };
        let result_type = semantic_casts::result_type(self.program, self.state, expression)?;
        let source_primitive = self.plans.nodes.get(operand).primitive_type;
        let target_primitive = self.program.primitive_type_reference(result_type)?;
        // Payload completion and semantic qualification custody use separate
        // namespaces. Preserve the checker's exact cast occurrence and both
        // carriers here; the lowering must not search again after detagging
        // or mistake another cast of this operand for the checked conversion.
        let formation_decided = cast.domain == ArithmeticDomain::Exact
            && self.exact_integer_casts.iter().any(|fact| {
                fact.expression == expression
                    && fact.source_type == source_primitive
                    && fact.target_type == target_primitive
            });
        Some(self.insert(
            expected_type,
            CheckedScalarComputationKind::AuthoredCast {
                source_expression: expression,
                operand,
                result_type,
                domain: cast.domain,
                formation_decided,
            },
        ))
    }

    pub(super) fn insert(
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

    pub(super) fn boolean(&mut self, value: bool) -> CheckedScalarComputationHandle {
        self.insert(
            PrimitiveType::Bool,
            CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::Constant(value),
            ))),
        )
    }

    /// A cast to the same bare carrier leaves its payload unchanged, whether
    /// it selects an arithmetic policy or forgets predicate-only facts. Forward
    /// its already-checked operand through ordinary computation building; a
    /// call result must not depend on whether a policy changes. Earlier
    /// operations keep their selected policy. Semantic domains and carrier
    /// conversions retain their separate proof and qualification custody.
    pub(super) fn same_carrier_cast_value(
        &mut self,
        expression: ExpressionHandle,
        expected_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        let operand = self.same_carrier_cast_operand(expression)?;
        let ExpressionNode::Cast(cast) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if self.program.primitive_type_reference(cast.target_type) != Some(expected_type) {
            return None;
        }
        self.expression(operand, expected_type)
    }

    pub(super) fn same_carrier_cast_operand(
        &self,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let ExpressionNode::Cast(cast) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if cast.form.is_recast()
            || !cast.semantic_domain.is_empty()
            || !matches!(
                self.program
                    .type_reference_table
                    .type_reference(cast.target_type),
                TypeReferenceNode::Named { .. }
            )
        {
            return None;
        }
        let target = self.program.primitive_type_reference(cast.target_type)?;
        let operand_type = semantic_casts::result_type(self.program, self.state, cast.value)?;
        (!semantic_casts::has_declared_domains(self.program, operand_type)
            && self.program.primitive_type_reference(operand_type) == Some(target))
        .then_some(cast.value)
    }

    pub(super) fn expression(
        &mut self,
        expression: ExpressionHandle,
        expected_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        if let Some(field) = self.structural_scalar_field(expression) {
            return (field.primitive_type == expected_type)
                .then(|| self.structural_field(expression, field))
                .flatten();
        }
        if let Some(operand) = self.same_carrier_cast_value(expression, expected_type) {
            return Some(operand);
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
                return self.authored_cast(expression, &cast, expected_type);
            }
            let result_type = semantic_casts::result_type(self.program, self.state, expression)?;
            if semantic_casts::has_declared_domains(self.program, result_type) {
                return self.authored_cast(expression, &cast, expected_type);
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
                return self.authored_cast(expression, &cast, expected_type);
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
        if let ExpressionNode::Cast(cast) =
            self.program.expression_table.expression(expression).clone()
            && semantic_casts::requires_custody(self.program, self.state, expression)
        {
            return self.authored_cast(expression, &cast, expected_type);
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
            && let Some(primitive) = self.selected_comparison_primitive(operator_use)
        {
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
        if expected_type == PrimitiveType::Bool
            && let Some(comparison) = self.byte_literal_comparison(expression)
        {
            return Some(comparison);
        }
        if expected_type == PrimitiveType::Bool
            && let Some(predicate) = self.byte_predicate(expression)
        {
            return Some(predicate);
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
            && let Some(equality) = self.enum_equality(expression)
        {
            return Some(equality);
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
            let integer = self.integer_operand(expression, Some(expected_type))?;
            if scalar_expression_type(&integer.value)? != expected_type {
                return None;
            }
            return self.materialize_integer(integer);
        }
        match self.program.expression_table.expression(expression).clone() {
            ExpressionNode::Call(call) => {
                if let Some(selection) = self.integer_min_max(expression, &call, expected_type) {
                    return Some(selection);
                }
                if let Some(operator) = boundary_calls::boundary_operator(
                    self.program,
                    crate::flow::call_target_symbol(self.program, &call),
                ) {
                    return self.boundary_operator_call(expression, &call, operator, expected_type);
                }
                if self
                    .program
                    .dynamic_slot_by_symbol(call.target_symbol)
                    .is_some()
                {
                    return self.dynamic_call(expression, &call, expected_type);
                }
                if !call.carries_only_positional_arguments() || !call.selects_only_nominal_route() {
                    return None;
                }
                let (target_machine, target_state) =
                    crate::semantic::calls::find_machine_by_entry_state(
                        self.program,
                        call.target_symbol,
                    )?;
                // A bodyless boundary declaration has no checked body for this
                // graph to embed. A whole-value statement call is the
                // statement's own result operation (`record_root` leaves it
                // there); a nested application instead becomes a boundary
                // node that invokes the exact occurrence between its operands
                // and its consumer. A `boundary machine` with an authored
                // body stays an ordinary callee — the same predicate the call
                // operation applies. A bodyless external leaf is the same
                // boundary edge through its exact satisfied requirement.
                if let Some((requirement, qualifier)) =
                    validation::exact_external_leaf_boundary_requirement(
                        self.program,
                        call.target_symbol,
                    )
                {
                    return self.external_boundary_call(
                        expression,
                        &call,
                        requirement,
                        qualifier,
                        expected_type,
                    );
                }
                if target_machine.supply_mode.is_boundary_declaration()
                    && !target_machine.body_is_present
                {
                    return self.boundary_machine_call(
                        expression,
                        &call,
                        target_machine,
                        target_state,
                        expected_type,
                    );
                }
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
                    parameter.is_const
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
                let arguments = self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec();
                if has_runtime_receiver {
                    return None;
                }
                if arguments.len() != target_parameters.len() {
                    return None;
                }
                let (arguments, structural_arguments, temporary_cleanups) =
                    self.call_operands(source_call, &arguments, target_parameters)?;
                Some(self.insert(
                    expected_type,
                    CheckedScalarComputationKind::Call {
                        callee: checked_trees::CheckedScalarCallee::Body {
                            machine: target_machine.symbol,
                            state: target_state.symbol,
                        },
                        source_call,
                        call_ordinal,
                        arguments,
                        structural_arguments,
                        temporary_cleanups,
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

    pub(super) fn call_ordinal(
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
