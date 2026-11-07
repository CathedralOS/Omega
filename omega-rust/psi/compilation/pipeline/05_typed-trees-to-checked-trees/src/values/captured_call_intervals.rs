//! Check integer arguments using their values when they were evaluated.
//! Copies keep the bounds known at that time; references use the storage
//! observations valid at the call. Every possible value contributes to the range.

use checked_trees::{FlowCallFact, FlowFacts, FlowOperandFact, FlowStateFact};
use facts::{FactPlan, IntegerRange};
use symbols::SymbolHandle;
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;

pub(crate) struct CapturedCallIntervals<'a> {
    program: &'a dyn validation::CheckingInput,
    operators: &'a checked_trees::CheckedOperatorFacts,
    semantic: &'a FactPlan,
    flow: &'a FlowFacts,
    state: &'a FlowStateFact,
    call: &'a FlowCallFact,
    parameters: &'a [StateParameter],
    arguments: &'a [FlowOperandFact],
    epoch: &'a FlowOperandFact,
}

impl<'a> CapturedCallIntervals<'a> {
    pub(crate) fn new(
        program: &'a dyn validation::CheckingInput,
        operators: &'a checked_trees::CheckedOperatorFacts,
        semantic: &'a FactPlan,
        flow: &'a FlowFacts,
        state: &'a FlowStateFact,
        call: &'a FlowCallFact,
        parameters: &'a [StateParameter],
    ) -> Option<Self> {
        if !call.authored_source_custody_valid {
            return None;
        }
        let (operands, receiver_offset) =
            crate::flow::captured_call_operands(program, flow.view(), state, call)?;
        let arguments = crate::shared::call_formals::arguments_aligned_to_formals(
            program,
            call.target_symbol,
            parameters.len(),
            operands.get(receiver_offset..)?,
        );
        if arguments.len() != parameters.len() {
            return None;
        }
        Some(Self {
            program,
            operators,
            semantic,
            flow,
            state,
            call,
            parameters,
            arguments,
            epoch: operands.last()?,
        })
    }

    fn parameter_bounds(
        &self,
        ordinal: usize,
        path: &[facts::PlaceSegment],
        primitive: PrimitiveType,
    ) -> Option<IntegerRange> {
        let parameter = self.parameters.get(ordinal)?;
        let operand = self.arguments.get(ordinal)?;
        let values = crate::flow::captured_operand_values(
            self.program,
            self.flow.view(),
            self.semantic,
            self.state,
            crate::flow::CapturedOperand::unrecorded(operand),
            parameter.type_reference,
            path,
        )?;
        let values = crate::flow::observe_captured_values(
            self.program,
            self.flow,
            self.semantic,
            self.state,
            values,
            self.epoch,
        );
        let mut hull: Option<IntegerRange> = None;
        for value in values {
            let mut known: Option<IntegerRange> = None;
            for (capture, reference, projection, places, invocation) in [
                (
                    value.observation,
                    value.observation_type,
                    &value.observation_projection,
                    crate::flow::captured_observation_places(
                        self.program,
                        self.flow.view(),
                        &value,
                    ),
                    self.call.entry_constraints,
                ),
                (
                    value.operand,
                    value.type_reference,
                    &value.projection,
                    crate::flow::captured_value_places(self.program, self.flow.view(), &value),
                    value.producer_invocation(self.call.entry_constraints),
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

impl validation::SelectedScalarIntervalSource for CapturedCallIntervals<'_> {
    fn binding(&mut self, position: usize, primitive: PrimitiveType) -> Option<IntegerRange> {
        let (ordinal, parameter) = self
            .parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| super::occupies_scalar_position(self.program, parameter))
            .nth(position)?;
        if self
            .program
            .primitive_type_reference(parameter.type_reference)
            != Some(primitive)
        {
            return None;
        }
        self.parameter_bounds(ordinal, &[], primitive)
    }

    fn storage(
        &mut self,
        _symbol: SymbolHandle,
        _primitive: PrimitiveType,
    ) -> Option<IntegerRange> {
        None
    }

    fn structural_field(
        &mut self,
        position: u32,
        path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    ) -> Option<IntegerRange> {
        let (_, segments, reference, _) = super::resolve_structural_parameter_path(
            self.program,
            self.parameters,
            position,
            path,
        )?;
        let primitive = self.program.primitive_type_reference(reference)?;
        self.parameter_bounds(position as usize, &segments, primitive)
    }

    fn indexed_read(
        &mut self,
        _root: checked_trees::CheckedStorageRoot,
        _path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        _index: &checked_trees::CheckedScalarSelector,
        _element_path: &[checked_trees::CheckedScalarReadProjection],
        _primitive: PrimitiveType,
    ) -> Option<IntegerRange> {
        None
    }
}
