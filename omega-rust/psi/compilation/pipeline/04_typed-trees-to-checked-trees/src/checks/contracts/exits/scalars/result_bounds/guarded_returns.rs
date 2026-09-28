//! Bounds on an exact selected return, including its taken-arm guard.
//!
//! Selected reads share a value only across a proven unchanged storage interval.
//! Captured predicates retain their earlier selected producers; complete write
//! frames preserve the collection and selector through the taken-arm return.

use super::{
    BigInt, ExitScalars, ExpressionHandle, ExpressionNode, exact_integer_carrier,
    exit_return_expression, interval_comparison, is_result_reference,
};
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarExpression,
    CheckedScalarExpressionRole,
};
use facts::IntegerRange;
use symbols::SymbolHandle;
use typed_trees::statement::{StatementNode, TransitionGuardNode};

#[cfg(test)]
mod tests;

impl ExitScalars<'_, '_> {
    pub(super) fn guarded_return_bounds_comparison(
        &self,
        expression: ExpressionHandle,
    ) -> Option<bool> {
        let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        let (argument, result_on_left) =
            if is_result_reference(self.program, self.machine, binary.left) {
                (binary.right, true)
            } else if is_result_reference(self.program, self.machine, binary.right) {
                (binary.left, false)
            } else {
                return None;
            };
        let entry = self.program.machine_states(self.machine).first()?;
        let primitive = exact_integer_carrier(self.program, entry.return_type)?;
        let argument_type = validation::expression_result_type_reference(
            self.program,
            self.machine,
            entry,
            argument,
        );
        if argument_type.is_some_and(|reference| {
            exact_integer_carrier(self.program, reference) != Some(primitive)
        }) {
            return None;
        }
        let endpoint = validation::closed_integer_range_bound(self.program, argument)?;
        typed_trees::closed_numeric::land_integer(&endpoint, primitive)?;
        let returned = exit_return_expression(self.program, self.exit);
        if !self.return_expression_is_stable(returned) {
            return None;
        }
        let (selected, symbols) = self.selected_return_expression(returned)?;
        if selected.primitive_type() != Some(primitive) {
            return None;
        }
        let mut bounds = crate::values::bounds::primitive_range(primitive)?;
        if let CheckedScalarExpression::IntegerLiteral { literal } = selected {
            let value = literal.value_bignum()?;
            typed_trees::closed_numeric::land_integer(&value, primitive)?;
            bounds = IntegerRange {
                minimum: value.clone(),
                maximum: value,
            };
        }
        let state = crate::semantic::calls::find_state_in_machine(
            self.program,
            self.exit.machine_symbol,
            self.exit.state_symbol,
        )?;
        if let Some(StatementNode::Transition(transition)) = self
            .program
            .statement_table
            .statements(state.statement_nodes)
            .get(self.exit.statement_index)
            && self.exit.transition_target.is_valid()
            && self.exit.transition_target == transition.target
            && let TransitionGuardNode::When(guard) = transition.guard
            && self.return_expression_is_stable(guard)
            && let Some((CheckedScalarExpression::Boolean(predicate), guard_symbols)) = self
                .selected_scalar_expression(
                    u32::try_from(self.exit.statement_index).ok()?,
                    CheckedScalarExpressionRole::Guard,
                    guard,
                )
            && guard_symbols == symbols
        {
            // Guard and return are independently source-bound to this exact
            // statement and declaration roster. No writes can occur between
            // their reads, including writes from a later guard conjunct.
            self.refine_guarded_return(
                predicate,
                guard_symbols,
                u32::try_from(self.exit.statement_index).ok()?,
                selected,
                &mut bounds,
                &mut 4096,
            );
        }
        let argument = IntegerRange {
            minimum: endpoint.clone(),
            maximum: endpoint,
        };
        if result_on_left {
            interval_comparison(binary.operator, &bounds, &argument)
        } else {
            interval_comparison(binary.operator, &argument, &bounds)
        }
    }
}

impl ExitScalars<'_, '_> {
    /// A captured guard denotes its selected initializer, not a reread of the
    /// source syntax. Earlier rosters are exact prefixes of the exit roster;
    /// complete write frames keep every captured storage read current to exit.
    fn captured_scalar_local(
        &self,
        position: usize,
        symbols: &[SymbolHandle],
        before_statement: u32,
        remaining: &mut usize,
    ) -> Option<(&CheckedScalarExpression, &[SymbolHandle], u32)> {
        *remaining = remaining.checked_sub(1)?;
        let symbol = *symbols.get(position)?;
        let state = crate::semantic::calls::find_state_in_machine(
            self.program,
            self.exit.machine_symbol,
            self.exit.state_symbol,
        )?;
        let statements = self
            .program
            .statement_table
            .statements(state.statement_nodes);
        let mut locals = statements
            .iter()
            .enumerate()
            .filter_map(|(statement, node)| {
                let StatementNode::LocalData(local) = node else {
                    return None;
                };
                (!local.is_mutable
                    && local.initial_value.is_valid()
                    && self
                        .program
                        .primitive_type_reference(local.type_reference)
                        .is_some())
                .then_some((statement, local))
            })
            .enumerate()
            .filter(|(_, (_, local))| local.symbol == symbol);
        let (binding_ordinal, (statement, local)) = locals.next()?;
        if locals.next().is_some() || statement >= before_statement as usize {
            return None;
        }
        let statement_ordinal = u32::try_from(statement).ok()?;
        let role = CheckedScalarExpressionRole::LocalInitializer {
            binding_ordinal: u32::try_from(binding_ordinal).ok()?,
        };
        let mut bindings = self
            .facts
            .values
            .scalar_expressions
            .source_bindings
            .iter()
            .filter(|(_, binding)| {
                binding.state == self.exit.state_symbol && binding.destination == symbol
            });
        let (_, binding) = bindings.next()?;
        if bindings.next().is_some()
            || binding.statement_ordinal != statement_ordinal
            || binding.role != role
            || binding.expression != local.initial_value
        {
            return None;
        }
        let (selected, operands) =
            self.selected_scalar_expression(statement_ordinal, role, local.initial_value)?;
        if !symbols.starts_with(operands)
            || selected.primitive_type()
                != self.program.primitive_type_reference(local.type_reference)
        {
            return None;
        }
        let frames = self.call_frames?;
        for node in statements.get(statement..self.exit.statement_index)? {
            *remaining = remaining.checked_sub(1)?;
            if !frames
                .statement_value_write_frame(self.machine, node)
                .into_complete_paths()
                .is_some_and(|paths| paths.is_empty())
            {
                return None;
            }
            let stable = match node {
                StatementNode::LocalData(local) => {
                    self.return_expression_is_stable(local.initial_value)
                }
                StatementNode::Expression(expression) => {
                    self.return_expression_is_stable(*expression)
                }
                StatementNode::Assignment(assignment) => {
                    self.return_expression_is_stable(assignment.target)
                        && self.return_expression_is_stable(assignment.value)
                        && frames
                            .assignment_write_frame(self.machine, node)
                            .into_complete_paths()
                            .is_some_and(|paths| paths.is_empty())
                }
                StatementNode::Call(call) => {
                    self.program
                        .statement_table
                        .expression_handles(call.arguments)
                        .iter()
                        .all(|argument| self.return_expression_is_stable(*argument))
                        && frames
                            .may_write_frame(self.machine, call)
                            .into_complete_paths()
                            .is_some_and(|paths| paths.is_empty())
                }
                // Crossing control or declaration authority needs its own
                // predecessor evidence, not ordinal adjacency.
                StatementNode::Transition(_)
                | StatementNode::RootBinding(_)
                | StatementNode::AssemblyFact(_) => false,
            };
            if !stable {
                return None;
            }
        }
        Some((selected, operands, statement_ordinal))
    }

    fn uncaptured_scalar<'selected>(
        &'selected self,
        mut expression: &'selected CheckedScalarExpression,
        mut symbols: &'selected [SymbolHandle],
        mut before_statement: u32,
        remaining: &mut usize,
    ) -> Option<&'selected CheckedScalarExpression> {
        while let CheckedScalarExpression::Local {
            position,
            primitive_type,
        } = expression
        {
            let (selected, operands, statement) =
                self.captured_scalar_local(*position, symbols, before_statement, remaining)?;
            if selected.primitive_type() != Some(*primitive_type) {
                return None;
            }
            expression = selected;
            symbols = operands;
            before_statement = statement;
        }
        Some(expression)
    }

    fn refine_guarded_return(
        &self,
        predicate: &CheckedBooleanExpression,
        symbols: &[SymbolHandle],
        before_statement: u32,
        returned: &CheckedScalarExpression,
        bounds: &mut IntegerRange,
        remaining: &mut usize,
    ) {
        let Some(next) = remaining.checked_sub(1) else {
            return;
        };
        *remaining = next;
        match predicate {
            CheckedBooleanExpression::Local { position } => {
                let Some((CheckedScalarExpression::Boolean(predicate), operands, statement)) =
                    self.captured_scalar_local(*position, symbols, before_statement, remaining)
                else {
                    return;
                };
                self.refine_guarded_return(
                    predicate, operands, statement, returned, bounds, remaining,
                );
            }
            CheckedBooleanExpression::And { left, right } => {
                self.refine_guarded_return(
                    left,
                    symbols,
                    before_statement,
                    returned,
                    bounds,
                    remaining,
                );
                self.refine_guarded_return(
                    right,
                    symbols,
                    before_statement,
                    returned,
                    bounds,
                    remaining,
                );
            }
            CheckedBooleanExpression::Equal { left, right } => {
                if matches!(left.as_ref(), CheckedBooleanExpression::Constant(true)) {
                    self.refine_guarded_return(
                        right,
                        symbols,
                        before_statement,
                        returned,
                        bounds,
                        remaining,
                    );
                } else if matches!(right.as_ref(), CheckedBooleanExpression::Constant(true)) {
                    self.refine_guarded_return(
                        left,
                        symbols,
                        before_statement,
                        returned,
                        bounds,
                        remaining,
                    );
                }
            }
            CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
                let Some(left) = self.uncaptured_scalar(left, symbols, before_statement, remaining)
                else {
                    return;
                };
                let Some(right) =
                    self.uncaptured_scalar(right, symbols, before_statement, remaining)
                else {
                    return;
                };
                let (endpoint, returned_on_left) = if left == returned {
                    (right, true)
                } else if right == returned {
                    (left, false)
                } else {
                    return;
                };
                let CheckedScalarExpression::IntegerLiteral { literal } = endpoint else {
                    return;
                };
                let Some(value) = literal.value_bignum() else {
                    return;
                };
                let one = BigInt::from_i64(1);
                match (kind, returned_on_left) {
                    (CheckedIntegerComparisonKind::Equal, _) => {
                        bounds.minimum = bounds.minimum.clone().max(value.clone());
                        bounds.maximum = bounds.maximum.clone().min(value);
                    }
                    (CheckedIntegerComparisonKind::LessThan, true) => {
                        bounds.maximum = bounds.maximum.clone().min(value.sub(&one));
                    }
                    (CheckedIntegerComparisonKind::LessOrEqual, true) => {
                        bounds.maximum = bounds.maximum.clone().min(value);
                    }
                    (CheckedIntegerComparisonKind::LessThan, false) => {
                        bounds.minimum = bounds.minimum.clone().max(value.add(&one));
                    }
                    (CheckedIntegerComparisonKind::LessOrEqual, false) => {
                        bounds.minimum = bounds.minimum.clone().max(value);
                    }
                }
            }
            _ => {}
        }
    }
}
