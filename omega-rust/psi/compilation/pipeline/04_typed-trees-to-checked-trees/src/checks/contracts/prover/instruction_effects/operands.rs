//! Instruction captures and current proposition terms share the existing
//! scoped arithmetic judge. Value origins and operand-prefix preservation
//! remain flow-owned; neither aliases nor initializers are executed here.
use super::super::call_guarantees::{arithmetic, callable::Callable};
use super::{QueryArgument, QueryArguments};
use checked_trees::{CheckFacts, FlowOperandFact, FlowStateFact};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::types::PrimitiveType;
use validation::{CanonicalPlace, ScopedArithmeticValue, StrictArithmeticImplicationJudgment};

struct Capture<'facts> {
    frontier: CaptureFrontier<'facts>,
    primitive: PrimitiveType,
    place: CanonicalPlace,
    origin: Option<CanonicalPlace>,
    value: ScopedArithmeticValue,
}

enum CaptureFrontier<'facts> {
    Operand(&'facts FlowOperandFact),
    NormalReturn(&'facts checked_trees::FlowCallFact),
}

#[derive(Clone, Copy)]
struct Values<'scope> {
    program: &'scope dyn validation::CheckingInput,
    facts: &'scope CheckFacts,
    flow: &'scope FlowStateFact,
    state: &'scope typed_trees::state::State,
    statement: usize,
    contexts: &'scope [facts::FactContextHandle],
    frames: &'scope validation::CallFrameResolver<'scope>,
}
impl<'scope> Values<'scope> {
    fn captured(
        &self,
        operand: &'scope FlowOperandFact,
        projection: &[facts::PlaceSegment],
        primitive: PrimitiveType,
        captures: &mut Vec<Capture<'scope>>,
    ) -> Option<ScopedArithmeticValue> {
        let Self {
            program,
            facts,
            flow,
            state,
            frames,
            ..
        } = *self;
        let machine = program.machine_by_symbol(flow.machine_symbol)?;
        let callable = Callable::Machine { machine, state };
        let mut capture = |expression, primitive| {
            let observed = if expression == operand.expression {
                operand
            } else {
                let mut observations = facts
                    .flow
                    .control
                    .operands
                    .iter()
                    .map(|(_, value)| value)
                    .filter(|value| {
                        value.state_symbol == operand.state_symbol
                            && value.statement_index == operand.statement_index
                            && value.expression == expression
                    });
                if let Some(first) = observations.next() {
                    let place = value_place(
                        program,
                        first.state_symbol,
                        first.statement_index,
                        expression,
                        projection,
                    )?;
                    if !observations.all(|other| {
                        crate::flow::operand_observations_agree(
                            program,
                            &facts.flow,
                            first,
                            other,
                            &place,
                        )
                    }) {
                        return None;
                    }
                    first
                } else {
                    // A root's completion is also a valid leaf observation when
                    // its complete prefix preserves that leaf's contents.
                    let place = value_place(
                        program,
                        operand.state_symbol,
                        operand.statement_index,
                        expression,
                        projection,
                    )?;
                    if !crate::flow::operand_entry_origin_is_stable(
                        program,
                        &facts.flow,
                        operand,
                        &place,
                    ) {
                        return None;
                    }
                    operand
                }
            };
            let place = value_place(
                program,
                observed.state_symbol,
                observed.statement_index,
                expression,
                projection,
            )?;
            let observed_contexts = facts
                .flow
                .semantic_constraint_contexts(observed.constraints)
                .collect::<Vec<_>>();
            let origin =
                crate::flow::operand_entry_origin_is_stable(program, &facts.flow, observed, &place)
                    .then(|| {
                        arithmetic::value_origin(
                            program,
                            facts,
                            flow,
                            observed.statement_index,
                            frames,
                            place.clone(),
                        )
                    })
                    .flatten();
            let value = known_integer(
                program,
                facts,
                state,
                observed.statement_index,
                &observed_contexts,
                expression,
                projection,
                primitive,
            )
            .map(ScopedArithmeticValue::Integer)
            .unwrap_or_else(|| {
                origin
                    .clone()
                    .map(|origin| arithmetic::atom(origin, primitive))
                    .unwrap_or_else(|| ScopedArithmeticValue::Atom {
                        identity: format!(
                            "\0instruction-capture:{:?}:{}:{:?}:{:?}:{primitive:?}:{place:?}",
                            observed.state_symbol,
                            observed.statement_index,
                            observed.expression,
                            observed.entry_origin_writes
                        ),
                        unsigned: !primitive.is_signed_integer(),
                    })
            });
            captures.push(Capture {
                frontier: CaptureFrontier::Operand(observed),
                primitive,
                place,
                origin,
                value: value.clone(),
            });
            Some(value)
        };
        if !projection.is_empty()
            || matches!(
                program.expression_table.expression(operand.expression),
                ExpressionNode::Call(_)
            )
        {
            capture(operand.expression, primitive)
        } else {
            arithmetic::scalar_term(program, facts, &callable, operand.expression, &mut capture)
        }
    }
    fn current(
        &self,
        expression: ExpressionHandle,
        projection: &[facts::PlaceSegment],
        primitive: PrimitiveType,
        captures: &[Capture<'scope>],
    ) -> Option<ScopedArithmeticValue> {
        let Self {
            program,
            facts,
            flow,
            state,
            statement,
            contexts,
            frames,
        } = *self;
        let machine = program.machine_by_symbol(flow.machine_symbol)?;
        let callable = Callable::Machine { machine, state };
        let observe = |expression, remaining: &[facts::PlaceSegment], primitive| {
            if let Some(integer) = known_integer(
                program, facts, state, statement, contexts, expression, remaining, primitive,
            ) {
                return Some(ScopedArithmeticValue::Integer(integer));
            }
            let place = value_place(program, state.symbol, statement, expression, remaining)?;
            let origin =
                arithmetic::value_origin(program, facts, flow, statement, frames, place.clone());
            for capture in captures {
                if capture.primitive != primitive {
                    continue;
                }
                let query_preserves = facts
                    .flow
                    .control
                    .calls
                    .span_or_empty(flow.calls)
                    .iter()
                    .filter(|call| call.statement_index == statement)
                    .all(|call| {
                        crate::flow::entry_origin_is_stable(
                            program,
                            &facts.flow.control,
                            call.normal_return_writes,
                            !call.normal_return_writes_complete,
                            &place,
                        )
                    });
                let same_place = capture.place == place
                    && match capture.frontier {
                        CaptureFrontier::Operand(operand) => {
                            crate::flow::operand_observation_before_statement_agrees(
                                program,
                                &facts.flow,
                                operand,
                                state.symbol,
                                statement,
                                &place,
                            )
                        }
                        CaptureFrontier::NormalReturn(call) => {
                            crate::flow::call_return_before_statement_agrees(
                                program,
                                &facts.flow,
                                flow,
                                call,
                                statement,
                                &place,
                            )
                        }
                    };
                let copied_return = match capture.frontier {
                    CaptureFrontier::NormalReturn(call) => {
                        crate::flow::value_origin_matches_call_return(
                            program,
                            &facts.flow,
                            machine,
                            flow,
                            statement,
                            place.clone(),
                            call,
                            &capture.place,
                            frames,
                        )
                    }
                    CaptureFrontier::Operand(_) => false,
                };
                if query_preserves
                    && (same_place
                        || copied_return
                        || capture
                            .origin
                            .as_ref()
                            .zip(origin.as_ref())
                            .is_some_and(|(left, right)| left == right))
                {
                    return Some(capture.value.clone());
                }
            }
            Some(ScopedArithmeticValue::Atom {
                identity: format!(
                    "\0instruction-current:{:?}:{statement}:{primitive:?}:{place:?}",
                    state.symbol
                ),
                unsigned: !primitive.is_signed_integer(),
            })
        };
        if !projection.is_empty()
            || matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Call(_)
            )
        {
            observe(expression, projection, primitive)
        } else {
            arithmetic::scalar_term(
                program,
                facts,
                &callable,
                expression,
                |expression, primitive| observe(expression, &[], primitive),
            )
        }
    }
    fn equal(&self, captured: &ScopedArithmeticValue, current: &ScopedArithmeticValue) -> bool {
        let Some(machine) = self.program.machine_by_symbol(self.flow.machine_symbol) else {
            return false;
        };
        validation::scoped_arithmetic_values_relation(
            self.program,
            machine,
            &[],
            BinaryOperator::Equal,
            captured,
            current,
        ) == StrictArithmeticImplicationJudgment::Proven
    }
}
pub(super) fn operand_matches(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    flow: &FlowStateFact,
    state: &typed_trees::state::State,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    operand: &FlowOperandFact,
    arguments: &QueryArguments<'_>,
    required: QueryArgument,
    primitive: PrimitiveType,
) -> bool {
    let Some(frames) = validation::CallFrameResolver::new(program) else {
        return false;
    };
    let values = Values {
        program,
        facts,
        flow,
        state,
        statement,
        contexts,
        frames: &frames,
    };
    let mut captures = Vec::new();
    let Some(captured) = values.captured(operand, &[], primitive, &mut captures) else {
        return false;
    };
    let Some(current) = arguments.scoped_value(
        program,
        facts,
        required,
        &[],
        primitive,
        &mut |expression, path, primitive| values.current(expression, path, primitive, &captures),
    ) else {
        return false;
    };
    values.equal(&captured, &current)
}

pub(super) fn guarantee_matches(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    flow: &FlowStateFact,
    state: &typed_trees::state::State,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    call: &checked_trees::FlowCallFact,
    candidate: &typed_trees::proposition::PropositionApplication,
    arguments: &QueryArguments<'_>,
    required: &[QueryArgument],
    operation: language_core::inline_assembly::AsmRegisterOperation,
) -> bool {
    let Some(contract) = validation::instruction_effect_contract(program, candidate) else {
        return false;
    };
    if contract.operation != operation {
        return false;
    }
    let Some(supplied) = QueryArguments::new(program, &contract) else {
        return false;
    };
    let Some(frame) = supplied.frames.last() else {
        return false;
    };
    if frame.arguments.len() != required.len() {
        return false;
    }
    let Some(frames) = validation::CallFrameResolver::new(program) else {
        return false;
    };
    let values = Values {
        program,
        facts,
        flow,
        state,
        statement,
        contexts,
        frames: &frames,
    };
    frame
        .arguments
        .iter()
        .zip(required)
        .zip(operation.input_carriers())
        .all(|((supplied_argument, required), carrier)| {
            let Some(primitive) = PrimitiveType::from_name(carrier.name()) else {
                return false;
            };
            let mut captures = Vec::new();
            let Some(captured) = supplied.scoped_value(
                program,
                facts,
                *supplied_argument,
                &[],
                primitive,
                &mut |expression, projection, primitive| {
                    super::super::call_guarantees::guarantee_scalar(
                        program,
                        facts,
                        flow,
                        call,
                        expression,
                        projection,
                        primitive,
                        &frames,
                        &mut |operand, path, primitive, post_state| {
                            if post_state {
                                if !post_binding_preserved(
                                    program,
                                    facts,
                                    operand,
                                    state.symbol,
                                    statement,
                                    &frames,
                                ) {
                                    return None;
                                }
                                let place = value_place(program, state.symbol, statement, operand.expression, path)?;
                            let returned_contexts = facts.flow.semantic_constraint_contexts(call.exit_constraints).collect::<Vec<_>>();
                            let scalar = super::super::scalar_value_at_place(program, &facts.semantic,
                                returned_contexts.iter().map(|context| facts.semantic.contexts.get(*context)), &place);
                            let value = if let Some(facts::ScalarValue::Integer(value)) = scalar {
                                ScopedArithmeticValue::Integer(value)
                            } else {
                                ScopedArithmeticValue::Atom {
                                    identity: format!("\0instruction-return:{:?}:{}:{}:{primitive:?}:{place:?}",
                                        state.symbol, call.statement_index, call.call_ordinal),
                                    unsigned: !primitive.is_signed_integer(),
                                }
                            };
                            captures.push(Capture { frontier: CaptureFrontier::NormalReturn(call), primitive,
                                place, origin: None, value: value.clone() });
                            Some(value)
                            } else {
                                values.captured(operand, path, primitive, &mut captures)
                            }
                        },
                    )
                },
            ) else {
                return false;
            };
            let Some(current) = arguments.scoped_value(
                program,
                facts,
                *required,
                &[],
                primitive,
                &mut |expression, path, primitive| {
                    values.current(expression, path, primitive, &captures)
                },
            ) else {
                return false;
            };
            values.equal(&captured, &current)
        })
}

fn post_binding_preserved(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    operand: &FlowOperandFact,
    state: symbols::SymbolHandle,
    statement: usize,
    frames: &validation::CallFrameResolver<'_>,
) -> bool {
    let Some(caller) = facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.state_symbol == operand.state_symbol)
    else {
        return false;
    };
    let Some(machine) = program.machine_by_symbol(caller.machine_symbol) else {
        return false;
    };
    let Some(source_state) = program.state_by_symbol(operand.state_symbol) else {
        return false;
    };
    if frames.expression_is_fixed_incoming_reference(source_state, operand.expression) {
        return true;
    }

    // Operand destination carriers may expose a referenced value's contents.
    // Descriptor identity instead follows the actual expression's source type.
    let Some(actual_type) = validation::expression_result_type_reference(
        program,
        machine,
        source_state,
        operand.expression,
    ) else {
        return false;
    };
    let actual_type = validation::peel_declared_constraints(program, actual_type);
    let typed_trees::types::TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(actual_type)
    else {
        return false;
    };
    let fresh = || CanonicalPlace {
        root: facts::PlaceRoot::Expression(operand.expression),
        segments: Vec::new(),
    };
    let binding = if let Some(place) = validation::canonical_place_from_expression_in_state(
        program,
        operand.state_symbol,
        operand.statement_index,
        operand.expression,
    ) {
        let Some(place_type) = validation::canonical_place_type_reference(
            program,
            operand.state_symbol,
            operand.statement_index,
            &place,
        ) else {
            return false;
        };
        let place_type = validation::peel_declared_constraints(program, place_type);
        // Stored descriptors keep their own identity. Taking a new reference
        // adds exactly one reference layer to the addressed storage type.
        if program.package_qualified_type_identity(actual_type)
            == program.package_qualified_type_identity(place_type)
        {
            place
        } else if program.package_qualified_type_identity(validation::peel_declared_constraints(
            program, *referee,
        )) == program.package_qualified_type_identity(place_type)
        {
            fresh()
        } else {
            return false;
        }
    } else {
        fresh()
    };
    crate::flow::operand_binding_before_statement_agrees(
        program,
        &facts.flow,
        operand,
        state,
        statement,
        &binding,
    )
}
fn value_place(
    program: &dyn validation::CheckingInput,
    state: symbols::SymbolHandle,
    statement: usize,
    expression: ExpressionHandle,
    projection: &[facts::PlaceSegment],
) -> Option<CanonicalPlace> {
    let mut place =
        validation::canonical_place_from_expression_in_state(program, state, statement, expression)
            .unwrap_or(CanonicalPlace {
                root: facts::PlaceRoot::Expression(expression),
                segments: Vec::new(),
            });
    place.segments.extend_from_slice(projection);
    Some(place)
}
fn known_integer(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    expression: ExpressionHandle,
    projection: &[facts::PlaceSegment],
    primitive: PrimitiveType,
) -> Option<numerics::bignum::BigInt> {
    if !projection.is_empty() {
        let place = value_place(program, state.symbol, statement, expression, projection)?;
        return match super::super::scalar_value_at_place(
            program,
            &facts.semantic,
            contexts
                .iter()
                .map(|context| facts.semantic.contexts.get(*context)),
            &place,
        )? {
            facts::ScalarValue::Integer(value) => Some(value),
            _ => None,
        };
    }
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Integer(_)
    ) {
        return None;
    }
    match super::super::live_scalar_value(
        program, facts, state, statement, contexts, expression, primitive,
    )? {
        facts::ScalarValue::Integer(value) => Some(value),
        _ => None,
    }
}
