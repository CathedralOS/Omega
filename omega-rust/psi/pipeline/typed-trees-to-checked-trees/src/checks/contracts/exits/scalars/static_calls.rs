//! Declared normal-return equalities under exact const-argument substitution.
//! A callee's binder spelling is not a caller value identity. Match its retained
//! binder position to the exact static selection instead. Ordinary runtime-call
//! readers keep their no-machine-arguments fence; this reader separately checks
//! the complete const roster before using the shared call/guarantee custody.

use super::*;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::expression::{BinaryOperator, ExpressionNode, StaticMachineArgument};

#[derive(PartialEq, Eq)]
enum StaticResult {
    CallerBinder(symbols::SymbolHandle),
    Value(ScalarValue),
}

impl ExitScalars<'_, '_> {
    pub(super) fn static_call_returns_symbol(
        &self,
        expression: ExpressionHandle,
        symbol: symbols::SymbolHandle,
    ) -> bool {
        self.static_call_result(expression) == Some(StaticResult::CallerBinder(symbol))
    }

    pub(super) fn closed_static_call_value(
        &self,
        expression: ExpressionHandle,
    ) -> Option<ScalarValue> {
        match self.static_call_result(expression)? {
            StaticResult::Value(value) => Some(value),
            StaticResult::CallerBinder(_) => None,
        }
    }

    fn static_call_result(&self, expression: ExpressionHandle) -> Option<StaticResult> {
        let expression = if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Call(_)
        ) {
            expression
        } else {
            let place = canonical_place_from_expression_in_state(
                self.program,
                self.exit.state_symbol,
                self.exit.statement_index,
                expression,
            )?;
            if !stable_segments(&place.segments) {
                return None;
            }
            self.assigned_call_at_place(&place)?
        };
        let selected = self.normal_return_call_target(expression)?;
        let ExpressionNode::Call(authored) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        let parameters = self.program.machine_type_parameters(selected.callee);
        if parameters.len() != authored.machine_arguments.len() {
            return None;
        }
        // Static operands are immutable, so they need no runtime storage
        // snapshot. Every binder must still have one exact, typed selection.
        let bindings = parameters
            .iter()
            .zip(authored.machine_arguments.iter())
            .map(|(parameter, argument)| self.static_argument_result(parameter, argument))
            .collect::<Option<Vec<_>>>()?;
        let result_type = self
            .program
            .primitive_type_reference(selected.entry.return_type)?;
        if !result_type.accepts_integer_literal()
            && result_type != typed_trees::types::PrimitiveType::Bool
        {
            return None;
        }
        let mut result = None;
        for guarantee in self.normal_call_guarantees(selected) {
            let ExpressionNode::Binary(binary) =
                self.program.expression_table.expression(guarantee)
            else {
                continue;
            };
            if binary.operator != BinaryOperator::Equal {
                continue;
            }
            // A const formal can itself be named `result`. Only an unresolved
            // reserved occurrence owned by this exact ensures clause denotes
            // the returned value; the declaration's ordinary binders do not.
            let is_returned_value = |expression| {
                validation::reserved_result_owner(self.program, expression)
                    .is_some_and(|(owner, _)| owner == selected.callee.symbol)
            };
            let value = if is_returned_value(binary.left) {
                binary.right
            } else if is_returned_value(binary.right) {
                binary.left
            } else {
                continue;
            };
            let (candidate, value_type) = match self.program.expression_table.expression(value) {
                ExpressionNode::Name(path) => {
                    if path.symbol != path.head_symbol
                        || self
                            .program
                            .expression_table
                            .name_path_members(path.members)
                            .len()
                            != 1
                    {
                        continue;
                    }
                    let Some(position) = parameters.iter().position(|parameter| {
                        parameter.symbol.is_valid() && parameter.symbol == path.symbol
                    }) else {
                        continue;
                    };
                    let TypeParameterKind::Const { type_reference } = parameters[position].kind
                    else {
                        continue;
                    };
                    (&bindings[position], type_reference)
                }
                ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {
                    // Specialization has already replaced the const binder by
                    // a typed closed leaf in the declaration's guarantee.
                    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        self.program,
                        selected.callee.symbol,
                        guarantee,
                        language_core::OperatorSpelling::Equal,
                        &[
                            Some(selected.entry.return_type),
                            Some(selected.entry.return_type),
                        ],
                    ) {
                        continue;
                    }
                    let candidate =
                        StaticResult::Value(evaluate_scalar(self.program, value, &mut |_| None)?);
                    if result.as_ref().is_some_and(|prior| prior != &candidate) {
                        return None;
                    }
                    result = Some(candidate);
                    continue;
                }
                _ => continue,
            };
            if self.program.primitive_type_reference(value_type) != Some(result_type)
                || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    self.program,
                    selected.callee.symbol,
                    guarantee,
                    language_core::OperatorSpelling::Equal,
                    &[Some(selected.entry.return_type), Some(value_type)],
                )
            {
                continue;
            }
            if result.as_ref().is_some_and(|prior| prior != candidate) {
                return None;
            }
            result = Some(match candidate {
                StaticResult::CallerBinder(symbol) => StaticResult::CallerBinder(*symbol),
                StaticResult::Value(value) => StaticResult::Value(value.clone()),
            });
        }
        result
    }

    fn static_argument_result(
        &self,
        parameter: &TypeParameter,
        argument: &StaticMachineArgument,
    ) -> Option<StaticResult> {
        let TypeParameterKind::Const { type_reference } = parameter.kind else {
            return None;
        };
        if argument.application.is_some() || argument.evidence_projection.is_some() {
            return None;
        }
        let primitive = self.program.primitive_type_reference(type_reference)?;
        if let Some(literal) = &argument.const_literal {
            return primitive
                .accepts_integer_literal()
                .then(|| literal.value_bignum())
                .flatten()
                .map(|value| StaticResult::Value(ScalarValue::Integer(value)));
        }
        let caller = self
            .program
            .machine_type_parameters(self.machine)
            .iter()
            .find(|caller| caller.symbol.is_valid() && caller.symbol == argument.symbol)?;
        let TypeParameterKind::Const {
            type_reference: caller_type,
        } = caller.kind
        else {
            return None;
        };
        (self.program.primitive_type_reference(caller_type) == Some(primitive))
            .then_some(StaticResult::CallerBinder(caller.symbol))
    }
}
