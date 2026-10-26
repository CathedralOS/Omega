//! IEEE bound implications for invocation preconditions, independent of the
//! called requirement. A positive ordered comparison excludes NaN and can
//! imply a weaker bound; its negation cannot. Only builtin or sealed IEEE
//! meanings supply these laws. Canonical places identify the subject, while
//! InvocationContexts selects its captured value's facts before this query:
//! current storage facts must never replace a detached operand's snapshot.

use validation::{CanonicalPlace, canonical_place_from_expression};

use crate::checks::operators::closed_operators_are_builtin;
use checked_trees::CheckedOperatorFacts;
use facts::{FactPayload, FactPlace, FactPlan, FloatPredicate, ProgramPoint};
use numerics::literals::FloatFormat;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

use crate::values::float_predicates::{comparison, has_ieee_meaning, literal, reverse};

struct RequiredComparison {
    position: usize,
    place: CanonicalPlace,
    reference: TypeReferenceHandle,
    predicate: FloatPredicate,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn contexts_prove_comparison(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    operators: &CheckedOperatorFacts,
    invocation_contexts: &crate::checks::operators::InvocationContexts<'_>,
    contexts: &[facts::FactContextHandle],
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> bool {
    if let Some(predicates) =
        validation::mathematical_applications::float_predicates(program, expression)
    {
        return !predicates.is_empty()
            && predicates.into_iter().all(|projection| {
                let Some(path) = program.expression_table.name_path_node(projection.subject) else {
                    return false;
                };
                if program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    != 1
                {
                    return false;
                }
                let Some(parameter) = parameters.iter().find(|parameter| {
                    parameter.symbol == path.symbol || parameter.symbol == path.head_symbol
                }) else {
                    return false;
                };
                let format = match program.primitive_type_reference(parameter.type_reference) {
                    Some(PrimitiveType::F32) => FloatFormat::F32,
                    Some(PrimitiveType::F64) => FloatFormat::F64,
                    _ => return false,
                };
                if format != projection.predicate.format() {
                    return false;
                }
                let Some(position) =
                    typed_trees::signature::state_parameter_position(parameters, parameter.symbol)
                else {
                    return false;
                };
                let Some(place) = operands
                    .get(position)
                    .and_then(|operand| canonical_place_from_expression(program, *operand))
                else {
                    return false;
                };
                let required = RequiredComparison {
                    position,
                    place,
                    reference: parameter.type_reference,
                    predicate: projection.predicate,
                };
                contexts_prove_required(
                    program,
                    semantic,
                    operators,
                    invocation_contexts,
                    contexts,
                    &required,
                )
            });
    }
    let Some(required) = required_comparison(program, parameters, operands, expression) else {
        return false;
    };
    contexts_prove_required(
        program,
        semantic,
        operators,
        invocation_contexts,
        contexts,
        &required,
    )
}

fn contexts_prove_required(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    operators: &CheckedOperatorFacts,
    invocation_contexts: &crate::checks::operators::InvocationContexts<'_>,
    contexts: &[facts::FactContextHandle],
    required: &RequiredComparison,
) -> bool {
    if comparison_holds_for_place(
        program,
        semantic,
        operators,
        contexts,
        required.reference,
        required.predicate,
        &required.place,
    ) {
        return true;
    }
    let Some(captured_operand) = invocation_contexts.operand_at(required.position) else {
        return false;
    };
    let flow = invocation_contexts.flow();
    let Some(state) = flow.control.state_by_symbol(captured_operand.state_symbol) else {
        return false;
    };
    reaching_store_proves_comparison(
        program,
        flow,
        semantic,
        operators,
        state,
        captured_operand,
        captured_operand,
        &required.place,
        contexts,
        required,
        0,
    )
}

fn comparison_holds_for_place(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    operators: &CheckedOperatorFacts,
    contexts: &[facts::FactContextHandle],
    reference: TypeReferenceHandle,
    predicate: FloatPredicate,
    place: &CanonicalPlace,
) -> bool {
    contexts.iter().any(|context| {
        semantic
            .context_view(semantic.contexts.get(*context))
            .facts()
            .any(|fact| match fact.payload {
                FactPayload::BooleanExpression(expression)
                | FactPayload::BooleanValue {
                    expression,
                    value: true,
                } => expression_proves_comparison(
                    program,
                    operators,
                    point_owner(fact.point),
                    expression,
                    place,
                    reference,
                    predicate,
                ),
                FactPayload::ContractBooleanExpression {
                    expression,
                    instantiated,
                    ..
                } if !instantiated.is_valid() => expression_proves_comparison(
                    program,
                    operators,
                    point_owner(fact.point),
                    expression,
                    place,
                    reference,
                    predicate,
                ),
                FactPayload::FloatPredicate {
                    predicate: observed,
                } if fact_has_place(program, semantic, fact.place, place) => {
                    observed.implies(predicate)
                }
                FactPayload::AssignedValue { value }
                    if fact_has_place(program, semantic, fact.place, place) =>
                {
                    literal(program, value, predicate.format())
                        .and_then(|bound| {
                            FloatPredicate::comparison(
                                predicate.format(),
                                BinaryOperator::Equal,
                                &bound,
                            )
                        })
                        .is_some_and(|observed| observed.implies(predicate))
                }
                _ => false,
            })
    })
}

#[allow(clippy::too_many_arguments)]
fn reaching_store_proves_comparison(
    program: &dyn ::validation::CheckingInput,
    flow: &checked_trees::FlowFacts,
    semantic: &FactPlan,
    operators: &CheckedOperatorFacts,
    state: &checked_trees::FlowStateFact,
    current_operand: &checked_trees::FlowOperandFact,
    invocation_operand: &checked_trees::FlowOperandFact,
    place: &CanonicalPlace,
    contexts: &[facts::FactContextHandle],
    required: &RequiredComparison,
    depth: usize,
) -> bool {
    if depth >= 16
        || current_operand.state_symbol != state.state_symbol
        || !matches!(place.root, facts::PlaceRoot::Symbol(_))
        || !crate::flow::operand_entry_origin_is_stable(program, flow, current_operand, place)
    {
        return false;
    }
    let Some(typed_state) = program.state_by_symbol(state.state_symbol) else {
        return false;
    };
    let order = program
        .statement_table
        .admitted_execution_order(typed_state.statement_nodes);
    let Some(end) = order.position(current_operand.statement_index).or_else(|| {
        (current_operand.statement_index
            == program
                .statement_table
                .statements(typed_state.statement_nodes)
                .len())
        .then_some(order.indices().len())
    }) else {
        return false;
    };
    for &index in order.indices()[..end].iter().rev() {
        let Some(statement) = flow.state_statement(state, index) else {
            return false;
        };
        if semantic.places.is_valid(statement.stored_destination)
            && flow.control.operands.is_valid(statement.stored_value)
        {
            let Some(destination) = crate::flow::canonical_place_from_semantic_place(
                program,
                semantic,
                semantic.places.get(statement.stored_destination),
            ) else {
                return false;
            };
            if destination.root == place.root
                && destination.segments.len() <= place.segments.len()
                && destination
                    .segments
                    .iter()
                    .zip(&place.segments)
                    .all(|(left, right)| {
                        crate::flow::canonical_place_segments_equal(program, *left, *right)
                    })
            {
                let stored = flow.control.operands.get(statement.stored_value);
                if !place_selectors_agree(
                    program,
                    flow,
                    state.state_symbol,
                    stored,
                    invocation_operand,
                    &destination.segments,
                ) {
                    return false;
                }
                return stored_operand_proves_comparison(
                    program,
                    flow,
                    semantic,
                    operators,
                    state,
                    invocation_operand,
                    stored,
                    &place.segments[destination.segments.len()..],
                    contexts,
                    required,
                    depth + 1,
                );
            }
        }
        if !crate::flow::entry_origin_is_stable(
            program,
            &flow.control,
            statement.completed_content_writes,
            statement.completed_content_unknown_write,
            place,
        ) {
            return false;
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn stored_operand_proves_comparison(
    program: &dyn ::validation::CheckingInput,
    flow: &checked_trees::FlowFacts,
    semantic: &FactPlan,
    operators: &CheckedOperatorFacts,
    state: &checked_trees::FlowStateFact,
    invocation_operand: &checked_trees::FlowOperandFact,
    stored: &checked_trees::FlowOperandFact,
    suffix: &[facts::PlaceSegment],
    contexts: &[facts::FactContextHandle],
    required: &RequiredComparison,
    depth: usize,
) -> bool {
    if depth >= 16 {
        return false;
    }
    if let checked_trees::FlowOperandCompletion::Selected(alternatives) = stored.completion {
        let Some(alternatives) = flow.control.operand_alternatives.span(alternatives) else {
            return false;
        };
        return !alternatives.is_empty()
            && alternatives.iter().all(|alternative| {
                if !alternative.source_arm.is_valid()
                    || !flow.control.operands.is_valid(alternative.value)
                {
                    return false;
                }
                stored_operand_proves_comparison(
                    program,
                    flow,
                    semantic,
                    operators,
                    state,
                    invocation_operand,
                    flow.control.operands.get(alternative.value),
                    suffix,
                    contexts,
                    required,
                    depth + 1,
                )
            });
    }
    if !program
        .expression_table
        .expression_is_valid(stored.expression)
        || !stored.type_reference.is_valid()
    {
        return false;
    }
    let Some(projections) = crate::flow::literal_expression_projections(
        program,
        stored.expression,
        stored.type_reference,
        suffix,
        false,
    ) else {
        return false;
    };
    !projections.is_empty()
        && projections.iter().all(|projection| {
            let Some((child, child_type)) = crate::flow::captured_operand_at_path(
                program,
                &flow.control,
                stored,
                stored.type_reference,
                &projection.destination,
            ) else {
                return false;
            };
            if child.expression != projection.expression
                || !crate::flow::operand_binds_stable_copy(program, child_type)
            {
                return false;
            }
            let Some(mut source_place) = validation::canonical_place_from_expression_in_state(
                program,
                child.state_symbol,
                child.statement_index,
                projection.expression,
            ) else {
                return false;
            };
            source_place.extend_segments(&projection.remaining);
            if crate::flow::operand_observations_agree(
                program,
                flow,
                child,
                invocation_operand,
                &source_place,
            ) && place_selectors_agree(
                program,
                flow,
                child.state_symbol,
                child,
                invocation_operand,
                &source_place.segments,
            ) && comparison_holds_for_place(
                program,
                semantic,
                operators,
                contexts,
                required.reference,
                required.predicate,
                &source_place,
            ) {
                return true;
            }
            reaching_store_proves_comparison(
                program,
                flow,
                semantic,
                operators,
                state,
                child,
                invocation_operand,
                &source_place,
                contexts,
                required,
                depth + 1,
            )
        })
}

fn place_selectors_agree(
    program: &dyn ::validation::CheckingInput,
    flow: &checked_trees::FlowFacts,
    state_symbol: SymbolHandle,
    first: &checked_trees::FlowOperandFact,
    second: &checked_trees::FlowOperandFact,
    segments: &[facts::PlaceSegment],
) -> bool {
    let mut reads = Vec::new();
    for segment in segments {
        if let facts::PlaceSegment::Index { expression } = segment {
            crate::facts::contract_occurrences::append_expression_occurrences(
                program,
                *expression,
                &mut reads,
            );
        }
    }
    reads.into_iter().all(|read| {
        validation::canonical_place_from_expression_in_state(
            program,
            state_symbol,
            first.statement_index,
            read,
        )
        .is_some_and(|place| {
            crate::flow::operand_observations_agree(program, flow, first, second, &place)
        })
    })
}

fn required_comparison(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> Option<RequiredComparison> {
    let binary = comparison(program, expression)?;
    for (subject, other, operator) in [
        (binary.left, binary.right, binary.operator),
        (binary.right, binary.left, reverse(binary.operator)),
    ] {
        let Some(path) = program.expression_table.name_path_node(subject) else {
            continue;
        };
        let Some(parameter) = parameters.iter().find(|parameter| {
            parameter.symbol == path.symbol || parameter.symbol == path.head_symbol
        }) else {
            continue;
        };
        let format = match program
            .type_reference_table
            .primitive_type(parameter.type_reference)
        {
            Some(PrimitiveType::F32) => FloatFormat::F32,
            Some(PrimitiveType::F64) => FloatFormat::F64,
            _ => continue,
        };
        let owner = program.symbols.get(parameter.symbol).parent;
        if !has_ieee_meaning(
            program,
            owner,
            expression,
            binary.operator,
            parameter.type_reference,
        ) {
            continue;
        }
        if program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
        {
            continue;
        }
        let position =
            typed_trees::signature::state_parameter_position(parameters, parameter.symbol)?;
        let operand = *operands.get(position)?;
        let place = canonical_place_from_expression(program, operand)?;
        let predicate = if operator == BinaryOperator::Equal
            && canonical_place_from_expression(program, subject)
                == canonical_place_from_expression(program, other)
        {
            FloatPredicate::non_nan(format)
        } else {
            FloatPredicate::comparison(format, operator, &literal(program, other, format)?)?
        };
        return Some(RequiredComparison {
            position,
            place,
            reference: parameter.type_reference,
            predicate,
        });
    }
    None
}

fn expression_proves_comparison(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    owner: SymbolHandle,
    expression: ExpressionHandle,
    place: &CanonicalPlace,
    reference: TypeReferenceHandle,
    predicate: FloatPredicate,
) -> bool {
    if let Some(borrow) = program.expression_table.borrow_expression(expression) {
        return expression_proves_comparison(
            program,
            operators,
            owner,
            borrow.target,
            place,
            reference,
            predicate,
        );
    }
    let Some(binary) = comparison(program, expression) else {
        return false;
    };
    if binary.operator == BinaryOperator::And
        && closed_operators_are_builtin(program, operators, expression)
    {
        return expression_proves_comparison(
            program,
            operators,
            owner,
            binary.left,
            place,
            reference,
            predicate,
        ) || expression_proves_comparison(
            program,
            operators,
            owner,
            binary.right,
            place,
            reference,
            predicate,
        );
    }
    if binary.operator == BinaryOperator::Equal
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(true)
        )
        && closed_operators_are_builtin(program, operators, expression)
    {
        return expression_proves_comparison(
            program,
            operators,
            owner,
            binary.left,
            place,
            reference,
            predicate,
        );
    }
    if !has_ieee_meaning(program, owner, expression, binary.operator, reference) {
        return false;
    }
    let left_is_subject = expression_has_place(program, binary.left, place);
    let right_is_subject = expression_has_place(program, binary.right, place);
    if left_is_subject && right_is_subject {
        return binary.operator == BinaryOperator::Equal
            && FloatPredicate::non_nan(predicate.format()).implies(predicate);
    }
    let (operator, bound) = if left_is_subject {
        (binary.operator, binary.right)
    } else if right_is_subject {
        (reverse(binary.operator), binary.left)
    } else {
        return false;
    };
    literal(program, bound, predicate.format())
        .and_then(|bound| FloatPredicate::comparison(predicate.format(), operator, &bound))
        .is_some_and(|observed| observed.implies(predicate))
}

fn expression_has_place(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    expected: &CanonicalPlace,
) -> bool {
    canonical_place_from_expression(program, expression)
        .is_some_and(|place| crate::flow::canonical_places_agree(program, &place, expected))
}

fn fact_has_place(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    place: FactPlace,
    expected: &CanonicalPlace,
) -> bool {
    let Some(place) = place.as_place() else {
        return false;
    };
    crate::flow::canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))
        .is_some_and(|place| crate::flow::canonical_places_agree(program, &place, expected))
}

fn point_owner(point: ProgramPoint) -> SymbolHandle {
    match point {
        ProgramPoint::Global => SymbolHandle::default(),
        ProgramPoint::Definition { symbol } => symbol,
        ProgramPoint::Machine { machine_symbol }
        | ProgramPoint::State { machine_symbol, .. }
        | ProgramPoint::Statement { machine_symbol, .. }
        | facts::call_point_triple_pattern!(machine_symbol, ..)
        | ProgramPoint::TransitionArm { machine_symbol, .. }
        | ProgramPoint::Exit { machine_symbol, .. } => machine_symbol,
    }
}
