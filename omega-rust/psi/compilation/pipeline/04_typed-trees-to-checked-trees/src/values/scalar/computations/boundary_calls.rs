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
use super::call_arguments::ComputedCallOperands;
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
            return validation::find_state_in_machine(self.program, self.machine, self.state)
                .and_then(|state| {
                    self.program
                        .statement_table
                        .statement_at(state, self.statement_index)
                })
                .is_some_and(|statement| {
                    matches!(
                        statement,
                        typed_trees::statement::StatementNode::Expression(_)
                    )
                })
                && self.is_bodyless_boundary_call(expression);
        }
        // Whole primitive stores use the same computation owner as calls nested
        // inside their RHS. Field stores retain their statement result operation;
        // confusing these owners either drops the call or evaluates it twice.
        if role == CheckedScalarExpressionRole::StorageInitializer {
            return false;
        }
        if role == CheckedScalarExpressionRole::AssignmentValue
            && validation::find_state_in_machine(
                self.program, self.machine, self.state,
            )
            .and_then(|state| {
                self.program.statement_table.statement_at(state, self.statement_index)
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
        bodyless_boundary_target(self.program, call).is_some()
    }

    /// The resolved declaration owns the signature; having an authored body
    /// state is not a prerequisite for entering the shared operand evaluator.
    pub(super) fn boundary_call(
        &mut self,
        expression: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
        target: (symbols::SymbolHandle, symbols::SymbolHandle),
        expected_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        let program = self.program;
        let authored_target = validation::call_target_symbol(program, call);
        // Namespace qualifiers have no runtime operand. A real receiver keeps
        // its evaluation/custody requirement and cannot be silently discarded.
        if !program.call_has_no_runtime_receiver_for_target(call) {
            return None;
        }
        let operands = program
            .expression_table
            .expression_handles(call.arguments)
            .to_vec();
        self.boundary_application(
            expression,
            authored_target,
            target,
            // External leaves retain their authored formal identities while
            // the public requirement owns the execution/result contract.
            validation::call_target_parameters(program, authored_target)?,
            validation::call_target_return_type(program, target.1)?,
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
        let ComputedCallOperands {
            arguments,
            structural_arguments,
            temporary_cleanups,
            claim_transfers,
        } = self.call_operands(source_call, operands, parameters)?;
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
                claim_transfers,
            },
        ))
    }
}

/// Classify by callable ownership rather than syntax or a representative
/// provider. Authored boundary bodies remain ordinary checked callees.
pub(super) fn bodyless_boundary_target(
    program: &dyn ::validation::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<(symbols::SymbolHandle, symbols::SymbolHandle)> {
    let target = validation::call_target_symbol(program, call);
    if let Some((requirement, _)) =
        validation::exact_external_leaf_boundary_requirement(program, target)
    {
        return Some((requirement, requirement));
    }
    if let Some((machine, state)) = validation::find_machine_by_entry_state(program, target) {
        return (machine.supply_mode.is_boundary_declaration() && !machine.body_is_present)
            .then_some((machine.symbol, state.symbol));
    }
    crate::values::scalar::call_lowering::call_is_boundary(program, target)
        .then_some((target, target))
}

/// The primitive carrier of a type that is bare or qualified only by an
/// arithmetic policy. Declared domains and ranges keep their own custody.
fn policy_carrier(
    program: &dyn ::validation::CheckingInput,
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
