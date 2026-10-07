//! Nested boundary applications as ordered computation operands.
//!
//! A boundary call that is a statement's whole value (`let x = a * b`,
//! `self.x = a * b`) is that statement's result operation: the unit plan binds
//! its scalar result directly. An application nested under another operand
//! (`a * b + c`, `a * b > c`, `sqrt(a * a)`) has no statement that can own it,
//! so it becomes a computation node here. Its operands complete left to right
//! through the ordinary builder, then its exact source call runs once and its
//! scalar result feeds the enclosing operand. That keeps the authored
//! evaluation schedule of `wiki/spec/language/expressions.md`: ordered,
//! single invocation, and no invocation on a skipped short-circuit path.
//!
//! Mutable primitive storage consumes a computed value, not a separate call
//! result binding. Its initializer and whole-place assignments therefore keep
//! the boundary application as a computation even when it is the entire RHS.
//! Reads of the old storage complete before the same ordinary store replaces it.
//!
//! Receiver-free applications share the ordinary computation operand evaluator:
//! structural places, selector snapshots, and scalar computations retain their
//! formal order. Their published boundary signature still owns access, claims,
//! completion and contract requirements independently of evaluation.
use super::{
    Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind,
    CheckedScalarExpressionRole, ExpressionHandle, ExpressionNode, PrimitiveType,
};

impl Builder<'_, '_> {
    /// Destinations whose statement already owns a whole-value boundary call
    /// as its result operation. Building a node there would sequence the same
    /// occurrence twice: once by the statement and once by the computation.
    /// A cast around the call belongs to the operand computation, including
    /// a same-carrier policy change: the statement's direct-call route does
    /// not own that enclosing expression. See [`Self::same_carrier_cast_value`].
    pub(super) fn statement_owns_boundary_call(
        &self,
        role: CheckedScalarExpressionRole,
        expression: ExpressionHandle,
    ) -> bool {
        // A transition arm owns its return through the exit computation,
        // including a whole boundary application. Only a final expression
        // statement has a separate statement-level result operation. Treating
        // the Return role alone as ownership would omit the arm's only call.
        if matches!(
            role,
            CheckedScalarExpressionRole::Return | CheckedScalarExpressionRole::ContinuationReturn
        ) {
            return crate::semantic::calls::find_state_in_machine(
                self.program,
                self.machine,
                self.state,
            )
            .and_then(|state| {
                self.program
                    .statement_table
                    .statements(state.statement_nodes)
                    .get(self.statement_index)
            })
            .is_some_and(|statement| {
                matches!(
                    statement,
                    typed_trees::statement::StatementNode::Expression(_)
                )
            }) && self.is_bodyless_boundary_call(expression);
        }
        // Whole primitive stores use the same computation owner as calls nested
        // inside their RHS. Field stores retain their statement result operation;
        // confusing these owners either drops the call or evaluates it twice.
        if role == CheckedScalarExpressionRole::StorageInitializer {
            return false;
        }
        if role == CheckedScalarExpressionRole::AssignmentValue
            && crate::semantic::calls::find_state_in_machine(
                self.program, self.machine, self.state,
            )
            .and_then(|state| {
                self.program.statement_table.statements(state.statement_nodes)
                    .get(self.statement_index)
            })
            .is_some_and(|statement| {
                matches!(statement, typed_trees::statement::StatementNode::Assignment(assignment)
                    if matches!(self.program.expression_table.expression(assignment.target),
                        ExpressionNode::Name(_)))
            })
        {
            return false;
        }
        matches!(
            role,
            CheckedScalarExpressionRole::LocalInitializer { .. }
                | CheckedScalarExpressionRole::AssignmentValue
        ) && self.is_bodyless_boundary_call(expression)
    }

    fn is_bodyless_boundary_call(&self, mut expression: ExpressionHandle) -> bool {
        let call = loop {
            if !self
                .program
                .expression_table
                .expression_is_valid(expression)
            {
                return false;
            }
            match self.program.expression_table.expression(expression) {
                ExpressionNode::Atomic(atomic) => expression = atomic.value,
                // A statement owns only a whole call. Every enclosing cast
                // belongs to the computation graph, which sequences the call
                // once and then retains its conversion or qualification edge.
                // Peeling a semantic cast here drops that outer operation.
                ExpressionNode::Cast(_) => return false,
                ExpressionNode::Call(call) => break call,
                _ => return false,
            }
        };
        boundary_operator(
            self.program,
            crate::flow::call_target_symbol(self.program, call),
        )
        .is_some()
            || validation::exact_external_leaf_boundary_requirement(
                self.program,
                call.target_symbol,
            )
            .is_some()
            || crate::semantic::calls::find_machine_by_entry_state(self.program, call.target_symbol)
                .is_some_and(|(machine, _)| {
                    machine.supply_mode.is_boundary_declaration() && !machine.body_is_present
                })
    }

    /// A nested application of a bodyless `boundary machine` declaration.
    pub(super) fn boundary_machine_call(
        &mut self,
        expression: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
        target_machine: &typed_trees::machine::Machine,
        target_state: &typed_trees::state::State,
        expected_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        if call.receiver.is_valid()
            || !self
                .program
                .call_has_no_runtime_receiver(call, target_machine, target_state)
        {
            return None;
        }
        let operands = self
            .program
            .expression_table
            .expression_handles(call.arguments)
            .to_vec();
        self.boundary_application(
            expression,
            call.target_symbol,
            (target_machine.symbol, target_state.symbol),
            self.program.state_parameters(target_state),
            target_state.return_type,
            &operands,
            expected_type,
        )
    }

    /// A realized external leaf invokes its exact public boundary requirement.
    /// Keep the authored leaf occurrence as source identity while the existing
    /// boundary node names the requirement's executable contract.
    pub(super) fn external_boundary_call(
        &mut self,
        expression: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
        requirement: symbols::SymbolHandle,
        qualifier: symbols::SymbolHandle,
        expected_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        let (source_call, _) = self.call_ordinal(expression, call.target_symbol)?;
        let source = self.flow.control.calls.get(source_call);
        if source.has_receiver && source.receiver_symbol != qualifier {
            return None;
        }
        let mut signatures = self
            .program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
            .flat_map(|definition| self.program.trait_machine_signatures(definition))
            .filter(|signature| signature.symbol == requirement);
        let signature = signatures.next()?;
        if signatures.next().is_some() {
            return None;
        }
        let operands = self
            .program
            .expression_table
            .expression_handles(call.arguments)
            .to_vec();
        self.boundary_application(
            expression,
            call.target_symbol,
            (requirement, requirement),
            // The external realization already proves signature correspondence.
            // Borrow occurrences still name the authored leaf's formal symbols.
            crate::semantic::calls::call_target_parameters(self.program, call.target_symbol)?,
            signature.return_type,
            &operands,
            expected_type,
        )
    }

    /// A nested application of a nongeneric legacy named boundary operator.
    /// A qualifier receiver (`F64::`) is not an operand: the operand roster is
    /// the one the operator's crash and requirement facts already use.
    pub(super) fn boundary_operator_call(
        &mut self,
        expression: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
        operator: &typed_trees::operator::OperatorDefinition,
        expected_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        if !self.program.operator_type_parameters(operator).is_empty()
            || !call.carries_only_positional_arguments()
        {
            return None;
        }
        let parameters = self.program.operator_parameters(operator);
        let operands =
            crate::facts::operator_crashes::named_call_operands(self.program, call, parameters)?;
        if operands.len()
            != self
                .program
                .expression_table
                .expression_handles(call.arguments)
                .len()
        {
            return None;
        }
        self.boundary_application(
            expression,
            operator.symbol,
            (operator.symbol, operator.symbol),
            parameters,
            operator.return_type,
            &operands,
            expected_type,
        )
    }

    /// Build the node once the declaration is known: every formal must be an
    /// supported call operand and the result must be the expected scalar.
    #[allow(clippy::too_many_arguments)]
    fn boundary_application(
        &mut self,
        expression: ExpressionHandle,
        call_target: symbols::SymbolHandle,
        (target_machine, target_state): (symbols::SymbolHandle, symbols::SymbolHandle),
        parameters: &[typed_trees::signature::StateParameter],
        return_type: typed_trees::types::TypeReferenceHandle,
        operands: &[ExpressionHandle],
        expected_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        // The scalar carrier is the executed payload. A policy-only cast may
        // change later arithmetic, but this call retains its own source use.
        if policy_carrier(self.program, return_type)? != expected_type
            || operands.len() != parameters.len()
        {
            return None;
        }
        let (source_call, call_ordinal) = self.call_ordinal(expression, call_target)?;
        let (arguments, structural_arguments, temporary_cleanups) =
            self.call_operands(source_call, operands, parameters)?;
        Some(self.insert(
            expected_type,
            CheckedScalarComputationKind::Call {
                source_call,
                callee: checked_trees::CheckedScalarCallee::Boundary {
                    machine: target_machine,
                    state: target_state,
                },
                call_ordinal,
                arguments,
                structural_arguments,
                temporary_cleanups,
            },
        ))
    }
}

/// A legacy named boundary operator (`F64::from_i32`) names one requirement
/// symbol for both its machine and state coordinates.
pub(super) fn boundary_operator(
    program: &typed_trees::TypedTrees,
    target: symbols::SymbolHandle,
) -> Option<&typed_trees::operator::OperatorDefinition> {
    program
        .operators()
        .iter()
        .find(|operator| operator.symbol == target && operator.is_boundary)
}

/// The primitive carrier of a type that is bare or qualified only by an
/// arithmetic policy. Declared domains and ranges keep their own custody.
fn policy_carrier(
    program: &typed_trees::TypedTrees,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Option<PrimitiveType> {
    if let Some(primitive) = program.primitive_type_reference(reference) {
        return Some(primitive);
    }
    let typed_trees::types::TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    program
        .type_reference_table
        .constraints(*constraints)
        .iter()
        .all(|constraint| {
            matches!(
                constraint,
                typed_trees::types::TypeConstraintNode::ArithmeticDomain(_)
            )
        })
        .then(|| program.primitive_type_reference(*base_type))
        .flatten()
}
