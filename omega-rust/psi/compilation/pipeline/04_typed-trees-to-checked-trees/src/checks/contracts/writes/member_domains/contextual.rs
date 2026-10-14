//! Rejoin a record predicate to its captured input observations. Constructor
//! children retain their own evaluation facts; projected storage uses the same
//! flow-owned capture query. Every substituted leaf must retain the exact
//! premise context, so a later sibling write cannot replace an earlier value.
//! Raw callee contracts and source initializers are not evidence. Selected
//! operator meaning remains declaration-owned.

use super::{CheckFacts, ExpressionHandle, ExpressionNode, FlowStateFact};

mod premises;

/// Bounds belong to the observed child, even when its reaching origin was a
/// computation. The stored snapshot has already selected arithmetic and casts;
/// replaying the initializer would read the wrong epoch after a sibling write.
pub(super) fn captured_integer_bounds(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    capture: &checked_trees::FlowOperandFact,
    path: &[facts::PlaceSegment],
) -> Option<facts::IntegerRange> {
    let values = crate::flow::captured_operand_values(
        program,
        &facts.flow,
        &facts.semantic,
        state,
        capture,
        capture.type_reference,
        path,
    )?;
    let mut bounds: Option<facts::IntegerRange> = None;
    for value in values {
        let contexts = crate::flow::operand_contexts(
            program,
            &facts.flow,
            &facts.semantic,
            value.observation,
            value.observation_type,
            capture.constraints,
        );
        // The pure selected algebra retains operator identity and exact cast
        // admission. It cannot contain effectful calls or replay local
        // initializers, so its reads share this observation's completion epoch.
        // Calls instead supply the completed-result snapshot below, never a
        // declaration-only reading of their raw ensures clauses.
        let selected = value
            .observation_projection
            .remaining
            .is_empty()
            .then(|| {
                let (_, body) = validation::machine_state_by_symbols(
                    program,
                    state.machine_symbol,
                    value.observation.state_symbol,
                )?;
                let primitive = program.primitive_type_reference(value.observation_type)?;
                let expression = crate::values::lower_unit_scalar_argument(
                    program,
                    &facts.operators,
                    body,
                    value.observation.statement_index,
                    value.observation_projection.expression,
                    primitive,
                )?;
                let symbols = crate::values::unit_scalar_binding_symbols(
                    program,
                    body,
                    value.observation.statement_index,
                )?;
                validation::selected_scalar_interval(
                    &expression,
                    &mut crate::values::LivePlaceIntervals {
                        program,
                        semantic: &facts.semantic,
                        contexts: &contexts,
                        parameters: program.state_parameters(body),
                        symbols: &symbols,
                        state: body.symbol,
                    },
                )
            })
            .flatten();
        for place in crate::flow::captured_observation_places(program, &facts.flow, &value)? {
            let snapshot = crate::values::integer_bounds_at_place(
                program,
                &facts.semantic,
                contexts
                    .iter()
                    .map(|context| facts.semantic.contexts.get(*context)),
                &place.projected,
            );
            let observed = match (snapshot, selected.clone()) {
                (Some(snapshot), Some(selected)) => facts::IntegerRange {
                    minimum: snapshot.minimum.max(selected.minimum),
                    maximum: snapshot.maximum.min(selected.maximum),
                },
                (snapshot, selected) => snapshot.or(selected)?,
            };
            if observed.minimum > observed.maximum {
                return None;
            }
            bounds = Some(match bounds {
                Some(previous) => facts::IntegerRange {
                    minimum: previous.minimum.min(observed.minimum),
                    maximum: previous.maximum.max(observed.maximum),
                },
                None => observed,
            });
        }
    }
    bounds
}

/// A constructed child observes a completed call at its own capture frontier,
/// not at the containing return's final storage. All possible captured values
/// must establish the same scalar; their premise sets are never unioned.
pub(super) fn captured_scalar(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    capture: &checked_trees::FlowOperandFact,
    path: &[facts::PlaceSegment],
    frames: &validation::CallFrameResolver<'_>,
) -> Option<facts::ScalarValue> {
    let values = crate::flow::captured_operand_values(
        program,
        &facts.flow,
        &facts.semantic,
        state,
        capture,
        capture.type_reference,
        path,
    )?;
    let mut agreed = None;
    for value in values {
        let contexts = crate::flow::operand_contexts(
            program,
            &facts.flow,
            &facts.semantic,
            value.operand,
            value.type_reference,
            capture.constraints,
        );
        for place in crate::flow::captured_value_places(program, &facts.flow, &value)? {
            let scalar = super::super::super::prover::call_guarantees::scalar_at_place(
                program,
                facts,
                state,
                value.operand.statement_index.checked_add(1)?,
                &contexts,
                frames,
                &place.projected,
            )?;
            if agreed.as_ref().is_some_and(|previous| *previous != scalar) {
                return None;
            }
            agreed = Some(scalar);
        }
    }
    agreed
}

pub(super) fn proves(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    predicate: ExpressionHandle,
    domain: &typed_trees::domain::DomainDefinition,
    capture: Option<&checked_trees::FlowOperandFact>,
) -> bool {
    let Some(machine) = program.machine_by_symbol(state.machine_symbol) else {
        return false;
    };
    let declaration = program.state_by_symbol(state.state_symbol);
    let mut offered_contexts = contexts.to_vec();
    let mut pending = capture.into_iter().collect::<Vec<_>>();
    while let Some(operand) = pending.pop() {
        for context in facts.flow.semantic_constraint_contexts(operand.constraints) {
            if !offered_contexts.contains(&context) {
                offered_contexts.push(context);
            }
        }
        for child in facts
            .flow
            .control
            .operand_children
            .span_or_empty(operand.children)
        {
            if facts.flow.control.operands.is_valid(child.value) {
                pending.push(facts.flow.control.operands.get(child.value));
            }
        }
    }
    offered_contexts.iter().any(|context| {
        facts
            .semantic
            .context_view(facts.semantic.contexts.get(*context))
            .facts()
            .any(|fact| {
                premises::from_fact(program, &facts.semantic, fact)
                    .iter()
                    .any(|premise| {
                        let equal = |actual, required| {
                            corresponds(
                                program, facts, state, statement, contexts, *context, value,
                                actual, required, domain, capture, premise,
                            )
                        };
                        premise.builtin(program, machine, declaration, premise.expression)
                            && super::super::super::direct::guard_value_proves_with(
                                program,
                                premise.expression,
                                premise.holds,
                                predicate,
                                true,
                                &equal,
                            )
                    })
            })
    })
}

fn corresponds(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    premise: facts::FactContextHandle,
    value: ExpressionHandle,
    actual: ExpressionHandle,
    required: ExpressionHandle,
    domain: &typed_trees::domain::DomainDefinition,
    capture: Option<&checked_trees::FlowOperandFact>,
    offered: &premises::Premise<'_>,
) -> bool {
    if let Some(segments) =
        crate::flow::relative_place_segments_from_expression(program, required, domain)
    {
        if let Some(capture) = capture {
            let Some(values) = crate::flow::captured_operand_values(
                program,
                &facts.flow,
                &facts.semantic,
                state,
                capture,
                capture.type_reference,
                &segments,
            ) else {
                return false;
            };
            // Every leaf must share this exact premise's lifetime. Combining
            // facts from different child epochs would correlate values across
            // a sibling write. The ordinary capture owner supplies projection
            // and view-versus-copy semantics, not syntax substitution.
            return !values.is_empty()
                && values.iter().all(|value| {
                    crate::flow::operand_contexts(
                        program,
                        &facts.flow,
                        &facts.semantic,
                        value.operand,
                        value.type_reference,
                        capture.constraints,
                    )
                    .contains(&premise)
                        && crate::flow::captured_value_places(program, &facts.flow, value)
                            .is_some_and(|places| {
                                !places.is_empty()
                                    && places.iter().all(|place| {
                                        offered
                                            .place(
                                                program,
                                                value.operand.state_symbol,
                                                value.operand.statement_index,
                                                actual,
                                            )
                                            .is_some_and(|actual| {
                                                crate::flow::canonical_places_agree(
                                                    program,
                                                    &actual,
                                                    &place.projected,
                                                )
                                            })
                                    })
                            })
                });
        }
        if !contexts.contains(&premise) {
            return false;
        }
        return super::member_place(program, state, statement, value, &segments)
            .zip(offered.place(program, state.state_symbol, statement, actual))
            .is_some_and(|(required, actual)| {
                crate::flow::canonical_places_agree(program, &required, &actual)
            });
    }
    if let Some(required) = super::expression_constant_scalar(program, required, domain.symbol) {
        return super::expression_constant_scalar(
            program,
            actual,
            offered.owner(state.machine_symbol),
        )
        .as_ref()
            == Some(&required);
    }
    if let Some(required) = super::case_value_symbol(program, required) {
        return super::case_value_symbol(program, actual) == Some(required);
    }
    let (ExpressionNode::Binary(actual), ExpressionNode::Binary(required_binary)) = (
        program.expression_table.expression(actual),
        program.expression_table.expression(required),
    ) else {
        return false;
    };
    // This routine substitutes leaves; it never assigns semantics from an
    // operator's spelling. The premise was checked by its machine owner and
    // every required node is checked by the domain owner.
    if !validation::has_builtin_domain_decomposed_guard_meaning(program, domain, required) {
        return false;
    }
    let same = |actual, required| {
        corresponds(
            program, facts, state, statement, contexts, premise, value, actual, required, domain,
            capture, offered,
        )
    };
    (actual.operator == required_binary.operator
        && same(actual.left, required_binary.left)
        && same(actual.right, required_binary.right))
        || (matches!(
            actual.operator,
            typed_trees::expression::BinaryOperator::Equal
                | typed_trees::expression::BinaryOperator::NotEqual
                | typed_trees::expression::BinaryOperator::Less
                | typed_trees::expression::BinaryOperator::LessOrEqual
                | typed_trees::expression::BinaryOperator::Greater
                | typed_trees::expression::BinaryOperator::GreaterOrEqual
        ) && actual.operator.mirrored() == required_binary.operator
            && same(actual.right, required_binary.left)
            && same(actual.left, required_binary.right))
}
