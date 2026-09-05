//! Scalar postconditions consume exact return and live place values. Entry
//! parameter names are substituted only through the retained exit origin map.

use checked_trees::{CheckFacts, CheckedScalarExpressionRole, FlowExitFact};
use facts::{FactContextHandle, FactPayload, PlaceRoot, PlaceSegment};
use typed_trees::{TypedTrees, expression::ExpressionHandle, machine::Machine};

use super::super::{
    prover::{
        ScalarValue, evaluate_checked_scalar, evaluate_scalar, has_builtin_operators,
        scalar_value_at_place,
    },
    return_values::{exit_return_expression, is_result_reference},
};
use crate::flow::{canonical_place_from_expression_in_state, canonical_place_from_symbol};

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
    evaluate_scalar(program, expression, &mut |leaf| {
        evaluator.contract_value(leaf)
    }) == Some(ScalarValue::Boolean(true))
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
        scalar_value_at_place(
            self.program,
            &self.facts.semantic,
            self.contexts
                .iter()
                .map(|context| self.facts.semantic.contexts.get(*context)),
            &place,
        )
    }

    fn return_value(&self) -> Option<ScalarValue> {
        let expression = exit_return_expression(self.program, self.exit);
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
            || !has_builtin_operators(self.program, &self.facts.operators, expression)
            || !self.call_frames.is_some_and(|frames| {
                frames
                    .expression_write_frame(self.machine, expression)
                    .into_complete_paths()
                    .is_some_and(|paths| paths.is_empty())
            })
        {
            // Exit contexts describe storage after return-expression effects.
            // Even a short-circuit expression cannot reread its left operand
            // here if evaluating the right operand may have changed it.
            return None;
        }
        self.selected_return_value(expression).or_else(|| {
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
                scalar_value_at_place(
                    self.program,
                    &self.facts.semantic,
                    self.contexts
                        .iter()
                        .map(|context| self.facts.semantic.contexts.get(*context)),
                    &place,
                )
            })
        })
    }

    fn selected_return_value(&self, expression: ExpressionHandle) -> Option<ScalarValue> {
        let plans = &self.facts.values.scalar_expressions;
        let statement_ordinal = u32::try_from(self.exit.statement_index).ok()?;
        let state = crate::find_state_in_machine(
            self.program,
            self.exit.machine_symbol,
            self.exit.state_symbol,
        )?;
        let role = match self
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
        };
        let mut bindings = plans.source_bindings.iter().filter(|(_, binding)| {
            binding.state == self.exit.state_symbol
                && binding.statement_ordinal == statement_ordinal
                && binding.role == role
                && binding.expression == expression
        });
        let (_, binding) = bindings.next()?;
        if bindings.next().is_some() {
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
        evaluate_checked_scalar(
            &plan.expression,
            &mut crate::values::BoundScalarValues {
                symbols,
                value_at_symbol: |symbol| {
                    let place = canonical_place_from_symbol(symbol)?;
                    scalar_value_at_place(
                        self.program,
                        &self.facts.semantic,
                        self.contexts
                            .iter()
                            .map(|context| self.facts.semantic.contexts.get(*context)),
                        &place,
                    )
                },
            },
        )
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
