//! Integer consequences of one completed operand and all its retained alternatives.
//! The value's own observation and producer epochs remain separate; neither
//! rereads a source initializer or imports facts established after completion.

use checked_trees::{FlowConstraintRef, FlowFacts, FlowOperandFact, FlowStateFact};
use facts::{FactPlan, IntegerRange};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

pub(crate) struct CapturedIntegerIntervals<'a> {
    program: &'a dyn validation::CheckingInput,
    operators: &'a checked_trees::CheckedOperatorFacts,
    semantic: &'a FactPlan,
    flow: &'a FlowFacts,
    state: &'a FlowStateFact,
}

impl<'a> CapturedIntegerIntervals<'a> {
    pub(crate) fn new(
        program: &'a dyn validation::CheckingInput,
        operators: &'a checked_trees::CheckedOperatorFacts,
        semantic: &'a FactPlan,
        flow: &'a FlowFacts,
        state: &'a FlowStateFact,
    ) -> Self {
        Self {
            program,
            operators,
            semantic,
            flow,
            state,
        }
    }

    pub(crate) fn bounds(
        &self,
        operand: crate::flow::CapturedOperand<'_>,
        reference: TypeReferenceHandle,
        path: &[facts::PlaceSegment],
        invocation: arena::HandleSpan<FlowConstraintRef>,
        epoch: &FlowOperandFact,
        primitive: PrimitiveType,
    ) -> Option<IntegerRange> {
        let values = crate::flow::captured_operand_values(
            self.program,
            self.flow,
            self.semantic,
            self.state,
            operand,
            reference,
            path,
        )?;
        let values = crate::flow::observe_captured_values(
            self.program,
            self.flow,
            self.semantic,
            self.state,
            values,
            epoch,
        );
        let mut hull: Option<IntegerRange> = None;
        for value in values {
            let mut known: Option<IntegerRange> = None;
            for (capture, reference, projection, places, invocation) in [
                (
                    value.observation,
                    value.observation_type,
                    &value.observation_projection,
                    crate::flow::captured_observation_places(self.program, self.flow, &value),
                    invocation,
                ),
                (
                    value.operand,
                    value.type_reference,
                    &value.projection,
                    crate::flow::captured_value_places(self.program, self.flow, &value),
                    value.producer_invocation(invocation),
                ),
            ] {
                let contexts = crate::flow::operand_contexts(
                    self.program,
                    &self.flow.contexts,
                    &self.flow.control,
                    self.semantic,
                    capture.row(),
                    reference,
                    invocation,
                );
                let literal = projection
                    .remaining
                    .is_empty()
                    .then(|| {
                        let expression = capture.row().expression;
                        let integer = match self.program.expression_table.expression(expression) {
                            typed_trees::expression::ExpressionNode::Integer(literal) => {
                                literal.value_bignum()
                            }
                            _ => None,
                        }
                        .or_else(|| {
                            validation::completed_integer_constant(self.program, expression)
                                .map(|constant| constant.value)
                        });
                        integer.map(|value| IntegerRange {
                            minimum: value.clone(),
                            maximum: value,
                        })
                    })
                    .flatten();
                let bounds = literal.or_else(|| {
                    let places = places?;
                    if places.is_empty() {
                        return None;
                    }
                    let mut current = super::CurrentIntegerBounds::at_statement(
                        self.program,
                        self.operators,
                        self.semantic,
                        &contexts,
                        capture.row().state_symbol,
                        capture.row().statement_index,
                    )?;
                    let mut range: Option<IntegerRange> = None;
                    for place in places {
                        let next = current.at_place(&place.projected, primitive)?;
                        range = Some(match range {
                            Some(previous) => IntegerRange {
                                minimum: previous.minimum.min(next.minimum),
                                maximum: previous.maximum.max(next.maximum),
                            },
                            None => next,
                        });
                    }
                    range
                });
                if let Some(bounds) = bounds {
                    // Both observations describe this one captured value,
                    // so it must satisfy both bounds.
                    known = Some(match known {
                        Some(previous) => IntegerRange {
                            minimum: previous.minimum.max(bounds.minimum),
                            maximum: previous.maximum.min(bounds.maximum),
                        },
                        None => bounds,
                    });
                }
            }
            let bounds = known?;
            if bounds.minimum > bounds.maximum {
                return None;
            }
            hull = Some(match hull {
                Some(previous) => IntegerRange {
                    minimum: previous.minimum.min(bounds.minimum),
                    maximum: previous.maximum.max(bounds.maximum),
                },
                None => bounds,
            });
        }
        hull
    }
}
