//! Boolean denotation of the selected execution graph. State values, application
//! operands, and callee formals are separate positional namespaces. Rejoin their
//! exact producers before substitution; never execute a callee body to discover
//! its result. This reader proves only normal-return equations. It neither edits
//! the execution graph nor imports branch facts into the surrounding context.
//! Terminal lowering still independently replays source custody and emits every
//! operation and its proof obligations, including effects simplified out of this
//! result equation. Graph and captured-local expansion share one bounded budget.

use super::{
    CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar,
    CheckedScalarExpressionRole, ExitScalars, ExpressionHandle, PrimitiveType, SymbolHandle,
    bind_boolean,
};
use crate::checks::contracts::prover::has_builtin_operators;
use checked_trees::{CheckedScalarComputationHandle, CheckedScalarComputationKind as Computation};
use typed_trees::expression::{BinaryOperator, ExpressionNode, UnaryOperator};

impl ExitScalars<'_, '_> {
    pub(super) fn bind_boolean_expression_at(
        &self,
        statement: u32,
        role: CheckedScalarExpressionRole,
        expression: ExpressionHandle,
        entry_position: &dyn Fn(SymbolHandle) -> Option<usize>,
        remaining: &mut usize,
        depth: usize,
    ) -> Option<Boolean> {
        if let Some((Scalar::Boolean(value), symbols)) =
            self.selected_scalar_expression(statement, role, expression)
        {
            return self.bind_selected_boolean(
                value,
                symbols,
                statement,
                entry_position,
                remaining,
                depth,
            );
        }
        // A failed or ambiguous pure plan is not permission to switch producers.
        if self
            .facts
            .values
            .scalar_expressions
            .source_bindings
            .iter()
            .any(|(_, binding)| {
                binding.state == self.exit.state_symbol
                    && binding.statement_ordinal == statement
                    && binding.role == role
            })
        {
            return None;
        }
        let plans = &self.facts.values.scalar_computations;
        let root = plans.root_at(self.exit.state_symbol, statement, role)?;
        if root.machine != self.machine.symbol
            || !plans.nodes.is_valid(root.root)
            || plans.nodes.get(root.root).authored_root != expression
        {
            return None;
        }
        let state = crate::find_state_in_machine(
            self.program,
            self.machine.symbol,
            self.exit.state_symbol,
        )?;
        // Computation Value nodes use the same dense immutable roster as pure
        // plans. Applications below instead index only their own operand stream.
        let mut symbols = self
            .program
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                self.program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
            })
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        symbols.extend(
            self.program
                .statement_table
                .statements(state.statement_nodes)
                .get(..statement as usize)?
                .iter()
                .filter_map(|statement| {
                    let typed_trees::statement::StatementNode::LocalData(local) = statement else {
                        return None;
                    };
                    (!local.is_mutable
                        && local.initial_value.is_valid()
                        && self
                            .program
                            .primitive_type_reference(local.type_reference)
                            .is_some())
                    .then_some(local.symbol)
                }),
        );
        self.bind_boolean_computation(
            root.root,
            expression,
            statement,
            &symbols,
            entry_position,
            remaining,
            depth,
            &mut Vec::new(),
        )
    }

    fn bind_boolean_computation(
        &self,
        handle: CheckedScalarComputationHandle,
        source: ExpressionHandle,
        statement: u32,
        symbols: &[SymbolHandle],
        entry_position: &dyn Fn(SymbolHandle) -> Option<usize>,
        remaining: &mut usize,
        depth: usize,
        active: &mut Vec<CheckedScalarComputationHandle>,
    ) -> Option<Boolean> {
        let plans = &self.facts.values.scalar_computations;
        if depth >= 64
            || *remaining == 0
            || !plans.nodes.is_valid(handle)
            || active.contains(&handle)
        {
            return None;
        }
        *remaining -= 1;
        let node = plans.nodes.get(handle);
        if node.primitive_type != PrimitiveType::Bool {
            return None;
        }
        active.push(handle);
        let result = (|| match &node.kind {
            Computation::Value(Scalar::Boolean(value)) => {
                if node.value_source != source {
                    return None;
                }
                let state = crate::find_state_in_machine(
                    self.program,
                    self.machine.symbol,
                    self.exit.state_symbol,
                )?;
                // Reconstruct the pure operation/slot relationship, not its
                // runtime value. StorageRead is still rejected by the binder.
                let selected = crate::values::lower_unit_scalar_argument(
                    self.program,
                    &self.facts.operators,
                    state,
                    statement as usize,
                    source,
                    PrimitiveType::Bool,
                )?;
                if selected != Scalar::Boolean(value.clone()) {
                    return None;
                }
                self.bind_selected_boolean(
                    value,
                    symbols,
                    statement,
                    entry_position,
                    remaining,
                    depth + 1,
                )
            }
            Computation::Call {
                source_call,
                target_machine,
                target_state,
                call_ordinal,
                arguments,
                structural_arguments,
            } => {
                let control = &self.facts.flow.control;
                if !control.calls.is_valid(*source_call) || !structural_arguments.is_empty() {
                    return None;
                }
                let call = self.normal_return_call(source)?;
                if call.state != self.exit.state_symbol
                    || call.fact.statement_index != statement as usize
                    || call.callee.symbol != *target_machine
                    || call.entry.symbol != *target_state
                    || self
                        .program
                        .primitive_type_reference(call.entry.return_type)
                        != Some(PrimitiveType::Bool)
                    || call.fact.call_ordinal != *call_ordinal as usize
                    || !std::ptr::eq(call.fact, control.calls.get(*source_call))
                {
                    return None;
                }
                let arguments = plans.operands.span(*arguments)?;
                let parameters = self.program.state_parameters(call.entry);
                if parameters.len() != arguments.len() {
                    return None;
                }
                for ((parameter, argument), authored) in
                    parameters.iter().zip(arguments).zip(call.arguments)
                {
                    if parameter.is_mutable || !plans.nodes.is_valid(*argument) {
                        return None;
                    }
                    let node = plans.nodes.get(*argument);
                    if self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                        != Some(node.primitive_type)
                        || node.authored_root != *authored
                    {
                        return None;
                    }
                }
                self.bind_normal_call_boolean(
                    call,
                    &mut |position, remaining, depth| {
                        self.bind_boolean_computation(
                            *arguments.get(position)?,
                            *call.arguments.get(position)?,
                            statement,
                            symbols,
                            entry_position,
                            remaining,
                            depth,
                            active,
                        )
                    },
                    remaining,
                    depth + 1,
                )
            }
            Computation::Apply {
                source_expression,
                expression: Scalar::Boolean(template),
                operands,
            } => {
                if *source_expression != source
                    || !has_builtin_operators(self.program, &self.facts.operators, source)
                {
                    return None;
                }
                let operands = plans.operands.span(*operands)?;
                let parameter = |position| Box::new(Boolean::Parameter { position });
                let (expected, sources) = match self.program.expression_table.expression(source) {
                    ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                        (Boolean::Not(parameter(0)), vec![unary.operand])
                    }
                    ExpressionNode::Binary(binary)
                        if matches!(
                            binary.operator,
                            BinaryOperator::Equal | BinaryOperator::NotEqual
                        ) =>
                    {
                        let equality = Boolean::Equal {
                            left: parameter(0),
                            right: parameter(1),
                        };
                        (
                            if binary.operator == BinaryOperator::Equal {
                                equality
                            } else {
                                Boolean::Not(Box::new(equality))
                            },
                            vec![binary.left, binary.right],
                        )
                    }
                    _ => return None,
                };
                if **template != expected || operands.len() != sources.len() {
                    return None;
                }
                bind_boolean(
                    template,
                    &mut |position, local, remaining, depth| {
                        if local {
                            return None;
                        }
                        self.bind_boolean_computation(
                            *operands.get(position)?,
                            *sources.get(position)?,
                            statement,
                            symbols,
                            entry_position,
                            remaining,
                            depth,
                            active,
                        )
                    },
                    remaining,
                    depth + 1,
                )
            }
            Computation::Select {
                source_expression,
                condition,
                when_true,
                when_false,
            } => {
                if *source_expression != source
                    || !has_builtin_operators(self.program, &self.facts.operators, source)
                {
                    return None;
                }
                let ExpressionNode::Binary(binary) =
                    self.program.expression_table.expression(source)
                else {
                    return None;
                };
                let (selected, skipped, skip_value) = match binary.operator {
                    BinaryOperator::And => (*when_true, *when_false, false),
                    BinaryOperator::Or => (*when_false, *when_true, true),
                    _ => return None,
                };
                if !plans.nodes.is_valid(skipped) {
                    return None;
                }
                let skipped = plans.nodes.get(skipped);
                if skipped.primitive_type != PrimitiveType::Bool
                    || skipped.value_source.is_valid()
                    || skipped.kind
                        != Computation::Value(Scalar::Boolean(Box::new(Boolean::Constant(
                            skip_value,
                        ))))
                {
                    return None;
                }
                let left = self.bind_boolean_computation(
                    *condition,
                    binary.left,
                    statement,
                    symbols,
                    entry_position,
                    remaining,
                    depth + 1,
                    active,
                )?;
                let right = self.bind_boolean_computation(
                    selected,
                    binary.right,
                    statement,
                    symbols,
                    entry_position,
                    remaining,
                    depth + 1,
                    active,
                )?;
                let expression = if skip_value {
                    Boolean::Or {
                        left: Box::new(left),
                        right: Box::new(right),
                    }
                } else {
                    Boolean::And {
                        left: Box::new(left),
                        right: Box::new(right),
                    }
                };
                bind_boolean(
                    &expression,
                    &mut |position, local, _, _| {
                        (!local).then_some(Boolean::Parameter { position })
                    },
                    remaining,
                    depth + 1,
                )
            }
            // Storage snapshots, structural/qualified values, and dispatch
            // need their own evidence joins, not source-spelling guesses.
            _ => None,
        })();
        active.pop();
        result
    }
}
