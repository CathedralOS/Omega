//! Scalar postconditions consume exact return and live place values. Entry
//! parameter names are substituted only through the retained exit origin map.

use checked_trees::{CheckFacts, CheckedScalarExpressionRole, FlowExitFact};
use facts::{FactContextHandle, FactPayload, PlaceRoot, PlaceSegment};
use typed_trees::{TypedTrees, expression::ExpressionHandle, machine::Machine};

use super::super::{
    prover::{
        ScalarValue, evaluate_checked_scalar, evaluate_scalar, evaluate_with_atoms,
        has_builtin_operators, scalar_value_at_place, semantic_contexts_prove_boolean_expression,
    },
    return_values::{exit_return_expression, is_result_reference},
};
use crate::flow::{canonical_place_from_expression_in_state, canonical_place_from_symbol};

mod boolean_results;
mod calls;

pub(super) fn proves<'program>(
    program: &'program TypedTrees,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    contexts: &[FactContextHandle],
    requirement: &facts::Fact,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
) -> bool {
    let FactPayload::ContractBooleanExpression {
        fact: contract,
        expression,
        ..
    } = requirement.payload
    else {
        return false;
    };
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == exit.machine_symbol)
    else {
        return false;
    };
    let evaluator = ExitScalars {
        program,
        facts,
        machine,
        exit,
        contexts,
        contract,
        call_frames,
    };
    if !has_builtin_operators(program, &facts.operators, expression) {
        return false;
    }
    if evaluator.proves_boolean_result(expression) == Some(true)
        || evaluator.proves_immutable_result_comparison(expression)
    {
        return true;
    }
    let cases = super::cases::CaseObservation::for_requirement(
        program,
        facts,
        exit,
        contexts,
        requirement,
        call_frames,
    );
    evaluate_with_atoms(
        program,
        expression,
        &mut |leaf| evaluator.contract_value(leaf),
        &|_, _| true,
        &mut |atom| {
            cases
                .as_ref()
                .and_then(|cases| cases.observe(atom))
                .or_else(|| {
                    // A current predicate can be known without fixing its scalar
                    // operands to single values. Use the same exit premises as the
                    // whole-contract prover; failed search never supplies false.
                    evaluator.current_predicate(atom).then_some(true)
                })
        },
    ) == Some(ScalarValue::Boolean(true))
}

struct ExitScalars<'program, 'facts> {
    program: &'program TypedTrees,
    facts: &'facts CheckFacts,
    machine: &'program Machine,
    exit: &'facts FlowExitFact,
    contexts: &'facts [FactContextHandle],
    contract: arena::Handle<typed_trees::domain::ProofFact>,
    call_frames: Option<&'facts validation::CallFrameResolver<'program>>,
}

impl ExitScalars<'_, '_> {
    fn current_predicate(&self, expression: ExpressionHandle) -> bool {
        use typed_trees::expression::{BinaryOperator, ExpressionNode, UnaryOperator};
        // The scalar leaf resolver can rebind entry parameters through exact
        // origins. The ordinary predicate prover does not perform that
        // substitution, so only unchanged binder identities may reach it.
        if self
            .facts
            .flow
            .control
            .exit_parameter_origins
            .span_or_empty(self.exit.parameter_origins)
            .iter()
            .any(|origin| {
                origin.contract == self.contract && origin.entry_parameter != origin.state_parameter
            })
        {
            return false;
        }
        let Some(state) = crate::find_state_in_machine(
            self.program,
            self.exit.machine_symbol,
            self.exit.state_symbol,
        ) else {
            return false;
        };
        let boolean = match self.program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => matches!(
                binary.operator,
                BinaryOperator::And
                    | BinaryOperator::Or
                    | BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::CaseMembership
            ),
            ExpressionNode::Unary(unary) => unary.operator == UnaryOperator::LogicalNot,
            _ => {
                validation::expression_result_type_reference(
                    self.program,
                    self.machine,
                    state,
                    expression,
                )
                .and_then(|reference| self.program.primitive_type_reference(reference))
                    == Some(typed_trees::types::PrimitiveType::Bool)
            }
        };
        boolean
            && semantic_contexts_prove_boolean_expression(
                self.program,
                &self.facts.semantic,
                self.contexts,
                expression,
            )
    }

    fn proves_immutable_result_comparison(&self, expression: ExpressionHandle) -> bool {
        use typed_trees::expression::{BinaryOperator, ExpressionNode};
        let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        else {
            return false;
        };
        let spelling = match binary.operator {
            BinaryOperator::And => {
                return self.proves_immutable_result_comparison(binary.left)
                    && self.proves_immutable_result_comparison(binary.right);
            }
            BinaryOperator::Or => {
                return self.proves_immutable_result_comparison(binary.left)
                    || self.proves_immutable_result_comparison(binary.right);
            }
            BinaryOperator::Equal => language_core::OperatorSpelling::Equal,
            BinaryOperator::LessOrEqual => language_core::OperatorSpelling::LessEqual,
            BinaryOperator::GreaterOrEqual => language_core::OperatorSpelling::GreaterEqual,
            _ => return false,
        };
        let argument = if is_result_reference(self.program, self.machine, binary.left) {
            binary.right
        } else if is_result_reference(self.program, self.machine, binary.right) {
            binary.left
        } else {
            return false;
        };
        let ExpressionNode::Name(path) = self.program.expression_table.expression(argument) else {
            return false;
        };
        if !path.symbol.is_valid() || path.head_symbol != path.symbol {
            return false;
        }
        let Some(entry) = self.program.machine_states(self.machine).first() else {
            return false;
        };
        if let Some(parameter) = self
            .program
            .machine_type_parameters(self.machine)
            .iter()
            .find(|parameter| parameter.symbol == path.symbol)
            && let typed_trees::data::TypeParameterKind::Const { type_reference } = parameter.kind
        {
            let primitive = self.program.primitive_type_reference(type_reference);
            if !primitive.is_some_and(|primitive| {
                primitive.accepts_integer_literal()
                    || (primitive == typed_trees::types::PrimitiveType::Bool
                        && binary.operator == BinaryOperator::Equal)
            }) || primitive != self.program.primitive_type_reference(entry.return_type)
            {
                return false;
            }
            let types = if argument == binary.right {
                [Some(entry.return_type), Some(type_reference)]
            } else {
                [Some(type_reference), Some(entry.return_type)]
            };
            // A static binder keeps one immutable identity across every state;
            // unlike runtime parameters it needs no edge-origin substitution.
            return typed_trees::operator::has_builtin_spelled_expression_meaning(
                self.program,
                self.machine.symbol,
                expression,
                spelling,
                &types,
            ) && matches!(self.program.expression_table.expression(exit_return_expression(self.program, self.exit)),
                ExpressionNode::Name(returned) if returned.symbol == parameter.symbol
                    && returned.head_symbol == parameter.symbol
                    && self.program.expression_table.name_path_members(returned.members).len() == 1);
        }
        let Some(parameter) = self
            .program
            .state_parameters(entry)
            .iter()
            .find(|parameter| parameter.symbol == path.symbol)
        else {
            return false;
        };
        // Immutable identity proves Boolean equality without choosing true or
        // false. Ordering remains integral; floating equality is not reflexive.
        if parameter.is_mutable
            || parameter.is_self
            || parameter.is_const
            || !self
                .program
                .primitive_type_reference(parameter.type_reference)
                .is_some_and(|primitive| {
                    primitive.accepts_integer_literal()
                        || (primitive == typed_trees::types::PrimitiveType::Bool
                            && binary.operator == BinaryOperator::Equal)
                })
            || self
                .program
                .primitive_type_reference(parameter.type_reference)
                != self.program.primitive_type_reference(entry.return_type)
        {
            return false;
        }
        let types = if argument == binary.right {
            [Some(entry.return_type), Some(parameter.type_reference)]
        } else {
            [Some(parameter.type_reference), Some(entry.return_type)]
        };
        if !typed_trees::operator::has_builtin_spelled_expression_meaning(
            self.program,
            self.machine.symbol,
            expression,
            spelling,
            &types,
        ) {
            return false;
        }
        let Some(origin) = self
            .facts
            .flow
            .control
            .exit_parameter_origins
            .span_or_empty(self.exit.parameter_origins)
            .iter()
            .find(|origin| {
                origin.contract == self.contract && origin.entry_parameter == parameter.symbol
            })
        else {
            return false;
        };
        let Some(state) = crate::find_state_in_machine(
            self.program,
            self.exit.machine_symbol,
            self.exit.state_symbol,
        ) else {
            return false;
        };
        if !self
            .program
            .state_parameters(state)
            .iter()
            .any(|parameter| {
                parameter.symbol == origin.state_parameter
                    && !parameter.is_mutable
                    && !parameter.is_self
                    && !parameter.is_const
            })
        {
            return false;
        }
        // The return is this immutable entry value, identified through the
        // retained state-edge origin. No initializer or storage read is replayed.
        matches!(self.program.expression_table.expression(exit_return_expression(self.program, self.exit)), ExpressionNode::Name(returned)
            if origin.state_parameter.is_valid() && returned.symbol == origin.state_parameter && returned.head_symbol == origin.state_parameter)
    }

    fn contract_value(&self, expression: ExpressionHandle) -> Option<ScalarValue> {
        if is_result_reference(self.program, self.machine, expression) {
            return self.return_value();
        }
        let entry = self.program.machine_states(self.machine).first()?;
        let mut place =
            canonical_place_from_expression_in_state(self.program, entry.symbol, 0, expression)?;
        let PlaceRoot::Symbol(root) = place.root else {
            return None;
        };
        if self
            .program
            .state_parameters(entry)
            .iter()
            .any(|parameter| parameter.symbol == root)
        {
            let origin = self
                .facts
                .flow
                .control
                .exit_parameter_origins
                .span_or_empty(self.exit.parameter_origins)
                .iter()
                .find(|origin| {
                    origin.contract == self.contract && origin.entry_parameter == root
                })?;
            if !origin.state_parameter.is_valid() {
                return None;
            }
            place.root = PlaceRoot::Symbol(origin.state_parameter);
        }
        // Dynamic selectors need their own evaluated occurrence, not a replay
        // of an entry expression under a new state or storage revision.
        if !stable_segments(&place.segments) {
            return None;
        }
        self.value_at_place(&place)
    }

    fn return_value(&self) -> Option<ScalarValue> {
        let expression = exit_return_expression(self.program, self.exit);
        if !self.return_expression_is_stable(expression) {
            return None;
        }
        self.selected_return_value(expression)
            .or_else(|| self.closed_call_value(expression))
            .or_else(|| {
                evaluate_scalar(self.program, expression, &mut |leaf| {
                    let place = canonical_place_from_expression_in_state(
                        self.program,
                        self.exit.state_symbol,
                        self.exit.statement_index,
                        leaf,
                    )?;
                    if !stable_segments(&place.segments) {
                        return None;
                    }
                    self.value_at_place(&place)
                })
            })
    }

    fn return_expression_is_stable(&self, expression: ExpressionHandle) -> bool {
        // Exit contexts describe storage after return-expression effects.
        // Even short-circuit operands cannot be reread after a later write.
        self.program
            .expression_table
            .expression_is_valid(expression)
            && has_builtin_operators(self.program, &self.facts.operators, expression)
            && self.call_frames.is_some_and(|frames| {
                frames
                    .expression_write_frame(self.machine, expression)
                    .into_complete_paths()
                    .is_some_and(|paths| paths.is_empty())
            })
    }

    fn selected_return_value(&self, expression: ExpressionHandle) -> Option<ScalarValue> {
        let (expression, symbols) = self.selected_return_expression(expression)?;
        evaluate_checked_scalar(
            expression,
            &mut crate::values::BoundScalarValues {
                symbols,
                value_at_symbol: |symbol| {
                    let place = canonical_place_from_symbol(symbol)?;
                    self.value_at_place(&place)
                },
            },
        )
    }

    fn selected_return_expression(
        &self,
        expression: ExpressionHandle,
    ) -> Option<(
        &checked_trees::CheckedScalarExpression,
        &[symbols::SymbolHandle],
    )> {
        self.selected_scalar_expression(
            u32::try_from(self.exit.statement_index).ok()?,
            self.return_expression_role()?,
            expression,
        )
    }

    fn return_expression_role(&self) -> Option<CheckedScalarExpressionRole> {
        let state = crate::find_state_in_machine(
            self.program,
            self.exit.machine_symbol,
            self.exit.state_symbol,
        )?;
        Some(
            match self
                .program
                .statement_table
                .statements(state.statement_nodes)
                .get(self.exit.statement_index)?
            {
                typed_trees::statement::StatementNode::Transition(transition)
                    if self.exit.transition_target.is_valid()
                        && self.exit.transition_target == transition.continuation =>
                {
                    CheckedScalarExpressionRole::ContinuationReturn
                }
                _ => CheckedScalarExpressionRole::Return,
            },
        )
    }

    fn selected_scalar_expression(
        &self,
        statement_ordinal: u32,
        role: CheckedScalarExpressionRole,
        expression: ExpressionHandle,
    ) -> Option<(
        &checked_trees::CheckedScalarExpression,
        &[symbols::SymbolHandle],
    )> {
        let plans = &self.facts.values.scalar_expressions;
        let mut bindings = plans.source_bindings.iter().filter(|(_, binding)| {
            binding.state == self.exit.state_symbol
                && binding.statement_ordinal == statement_ordinal
                && binding.role == role
        });
        let (_, binding) = bindings.next()?;
        if bindings.next().is_some() || binding.expression != expression {
            return None;
        }
        if matches!(role, CheckedScalarExpressionRole::CallArgument { .. })
            && binding.destination.is_valid()
        {
            return None;
        }
        let mut selected = plans.expressions.iter().filter(|plan| {
            plan.state == binding.state
                && plan.statement_ordinal == binding.statement_ordinal
                && plan.role == binding.role
        });
        let plan = selected.next()?;
        if selected.next().is_some() {
            return None;
        }
        let symbols = plans.binding_symbols.span_or_empty(binding.symbols);
        let state = crate::find_state_in_machine(
            self.program,
            self.exit.machine_symbol,
            self.exit.state_symbol,
        )?;
        // Dense positions describe the declaration roster at this occurrence,
        // not a caller-chosen substitution. Check it without reading values.
        let parameters = self
            .program
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                self.program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
            })
            .map(|parameter| parameter.symbol);
        let locals = self
            .program
            .statement_table
            .statements(state.statement_nodes)
            .get(..statement_ordinal as usize)?
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
            });
        if !parameters.chain(locals).eq(symbols.iter().copied()) {
            return None;
        }
        Some((&plan.expression, symbols))
    }
}

fn stable_segments(segments: &[PlaceSegment]) -> bool {
    segments.iter().all(|segment| match segment {
        PlaceSegment::Field { symbol } => symbol.is_valid(),
        PlaceSegment::Case { variant } => variant.is_valid(),
        PlaceSegment::FixedIndex { .. } => true,
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use checked_trees::{
        CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedOperatorUseFact,
    };
    use typed_trees::expression::{BinaryOperator, ExpressionNode, TableBinaryExpression};

    #[test]
    fn scalar_exit_evaluation_never_reinterprets_a_selected_operator() {
        let mut program = TypedTrees::default();
        let operand = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let expression =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    operator: BinaryOperator::Equal,
                    left: operand,
                    right: operand,
                }));
        let mut operators = CheckedOperatorFacts::default();
        assert!(has_builtin_operators(&program, &operators, expression));
        operators.uses.append(CheckedOperatorUseFact {
            expression,
            status: CheckedOperatorResolutionStatus::BuiltinFallback,
            ..Default::default()
        });
        assert!(has_builtin_operators(&program, &operators, expression));
        operators.uses.append(CheckedOperatorUseFact {
            expression,
            status: CheckedOperatorResolutionStatus::Resolved,
            ..Default::default()
        });
        assert!(!has_builtin_operators(&program, &operators, expression));
    }
}
