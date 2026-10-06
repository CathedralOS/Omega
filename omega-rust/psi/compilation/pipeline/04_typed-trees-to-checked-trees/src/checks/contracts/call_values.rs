//! Bind invocation predicates to actual values captured in source order.
//!
//! Detached copies keep operand-time observations; reference operands use the
//! invocation's live referents. Cleanup requirements and concrete write checks
//! share this substitution law. Neither may reread an initializer or substitute
//! a later value for an operand already evaluated.
use crate::flow::CanonicalPlace;
use checked_trees::{
    CheckFacts, CheckedBooleanExpression, CheckedScalarExpression, FlowCallFact, FlowStateFact,
};
use facts::FactContextHandle;
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, expression::ExpressionHandle, signature::StateParameter};

pub(crate) fn target_machine(program: &TypedTrees, call: &FlowCallFact) -> Option<SymbolHandle> {
    crate::lookup::machine_by_symbol(program, call.target_symbol)
        .map(|machine| machine.symbol)
        .or_else(|| {
            crate::semantic::calls::find_machine_by_entry_state(program, call.target_symbol)
                .map(|(machine, _)| machine.symbol)
        })
}

pub(crate) struct Actual<'program> {
    pub(crate) parameter: &'program StateParameter,
    pub(crate) argument: Option<ExpressionHandle>,
    pub(crate) place: CanonicalPlace,
    pub(crate) path: Vec<facts::PlaceSegment>,
    pub(crate) operand_position: usize,
}

pub(crate) fn actual_at<'program>(
    program: &'program TypedTrees,
    state: &FlowStateFact,
    call: &FlowCallFact,
    position: usize,
    path: Vec<facts::PlaceSegment>,
) -> Option<Actual<'program>> {
    let site = crate::semantic::calls::find_call_site(
        program,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let parameters = crate::semantic::calls::call_target_parameters(program, call.target_symbol)?;
    let parameter = parameters.get(position)?;
    let receiver_offset = usize::from(
        matches!(&site, crate::semantic::calls::CallSite::Expression { call, .. } if call.receiver.is_valid()),
    );
    let ordinal = parameters[..position].iter().count();
    let operand_position = { ordinal + receiver_offset };
    let argument = {
        let ordinal = parameters[..position].iter().count();
        Some(*crate::semantic::calls::call_site_argument_expressions(program, &site).get(ordinal)?)
    };
    let place = if let Some(argument) = argument {
        crate::flow::canonical_place_from_expression_in_state(
            program,
            state.state_symbol,
            call.statement_index,
            argument,
        )?
    } else {
        crate::flow::canonical_receiver_place_for_call_site(
            program,
            state.machine_symbol,
            state.state_symbol,
            &site,
            call.statement_index,
        )?
    };
    Some(Actual {
        parameter,
        argument,
        place,
        path,
        operand_position,
    })
}

/// Bind a callee observation to its actual operand, without assuming that the
/// formal's declaration position is a dense scalar position.
pub(crate) fn observation_actual<'program>(
    program: &'program TypedTrees,
    state: &FlowStateFact,
    call: &FlowCallFact,
    observation: &crate::facts::boolean_predicates::BoundObservation,
) -> Option<Actual<'program>> {
    let parameters = crate::semantic::calls::call_target_parameters(program, call.target_symbol)?;
    let position = parameters.iter().position(|parameter| {
        observation.place.root == facts::PlaceRoot::Symbol(parameter.symbol)
    })?;
    actual_at(
        program,
        state,
        call,
        position,
        observation.place.segments.clone(),
    )
}

pub(crate) fn projected_expression(
    program: &TypedTrees,
    actual: &Actual<'_>,
) -> Option<ExpressionHandle> {
    let expression = actual.argument?;
    if actual.path.is_empty() {
        return Some(expression);
    }
    let projections = crate::flow::literal_value_projections(
        program,
        expression,
        actual.parameter.type_reference,
        &actual.path,
        false,
    )?;
    let [projection] = projections.as_slice() else {
        return None;
    };
    projection
        .remaining
        .is_empty()
        .then_some(projection.expression)
}

pub(crate) fn captured_actual<'facts>(
    program: &TypedTrees,
    facts: &'facts CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    actual: &Actual<'_>,
) -> Option<&'facts checked_trees::FlowOperandFact> {
    let site = crate::semantic::calls::find_call_site(
        program,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let receiver = match &site {
        crate::semantic::calls::CallSite::Expression { call, .. } if call.receiver.is_valid() => {
            Some(call.receiver)
        }
        _ => None,
    };
    let expected = receiver.into_iter().chain(
        crate::semantic::calls::call_site_argument_expressions(program, &site)
            .iter()
            .copied(),
    );
    let operands = facts.flow.control.operands.span(call.operands)?;
    if !operands
        .iter()
        .map(|operand| operand.expression)
        .eq(expected)
    {
        return None;
    }
    operands.get(actual.operand_position)
}

/// Select the evidence epoch of a declared membership's exact formal. A
/// copied argument retains completion facts even when a later operand writes
/// its source; borrowed arguments retain the ordinary invocation-live rules.
/// The declaration's subject and its substituted place must agree with the
/// same argument occurrence. A missing capture supplies no evidence.
pub(crate) fn membership_contexts(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &facts::Fact,
) -> Option<Vec<FactContextHandle>> {
    let facts::FactPayload::ContractDomainMembership {
        value,
        fact: source_fact,
        ..
    } = requirement.payload
    else {
        return None;
    };
    let declared = crate::flow::canonical_place_from_expression(program, value)?;
    let facts::PlaceRoot::Symbol(symbol) = declared.root else {
        return None;
    };
    let Some(contract_call) = crate::flow::proof_contract_call(
        &facts.proof,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    ) else {
        return Some(Vec::new());
    };
    let mut position = None;
    for reference in facts
        .proof
        .contract_fact_refs
        .span_or_empty(contract_call.requires)
    {
        let contract = facts.proof.contract_facts.get(reference.fact);
        if contract.fact != source_fact {
            continue;
        }
        let bound = match crate::semantic::facts::contract_parameter_position(
            program,
            &facts.proof,
            contract,
            contract_call.target_state_symbol,
            symbol,
        ) {
            Ok(Some(bound)) => bound,
            Ok(None) => return None,
            Err(()) => return Some(Vec::new()),
        };
        if position.is_some_and(|position| position != bound) {
            return Some(Vec::new());
        }
        position = Some(bound);
    }
    let Some(position) = position else {
        return Some(Vec::new());
    };
    let Some(actual) = actual_at(program, state, call, position, declared.segments) else {
        return Some(Vec::new());
    };
    let facts::FactPlace::Place(place) = requirement.place else {
        return Some(Vec::new());
    };
    let Some(required) = crate::flow::canonical_place_from_semantic_place(
        program,
        &facts.semantic,
        facts.semantic.places.get(place),
    ) else {
        return Some(Vec::new());
    };
    let Some(expected) = actual.argument.and_then(|argument| {
        crate::semantic::facts::projected_argument_place(
            program,
            state.state_symbol,
            call.statement_index,
            argument,
            actual.parameter.type_reference,
            &actual.path,
        )
    }) else {
        return Some(Vec::new());
    };
    if required.root != expected.root
        || required.segments.len() != expected.segments.len()
        || !required
            .segments
            .iter()
            .zip(&expected.segments)
            .all(|(left, right)| {
                crate::flow::canonical_place_segments_equal(program, *left, *right)
            })
    {
        return Some(Vec::new());
    }
    Some(actual_contexts(program, facts, state, call, &actual))
}

/// Memberships of an actual and its declared fields share one evaluation
/// epoch. Absent or mismatched occurrence custody cannot provide evidence.
pub(crate) fn actual_contexts(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    actual: &Actual<'_>,
) -> Vec<FactContextHandle> {
    let Some(captured) = captured_actual(program, facts, state, call, actual) else {
        return Vec::new();
    };
    crate::flow::operand_contexts(
        program,
        &facts.flow,
        &facts.semantic,
        captured,
        actual.parameter.type_reference,
        call.entry_constraints,
    )
}

/// Rebind a selected actual expression to exact current storage. Local slots
/// denote stored values; their initializer expressions are never replayed.
fn current_actual(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    actual: &Actual<'_>,
    primitive: typed_trees::types::PrimitiveType,
    contexts: &[FactContextHandle],
    bindings: &mut Vec<crate::facts::boolean_predicates::BoundObservation>,
) -> Option<CheckedScalarExpression> {
    let (machine, body) = crate::lookup::symbols::machine_state_by_symbol(
        program,
        state.machine_symbol,
        state.state_symbol,
    )?;
    if let Some(expression) = projected_expression(program, actual) {
        if !validation::has_builtin_bound_expression_meaning(
            program,
            machine,
            Some(body),
            expression,
        ) {
            return None;
        }
        if let Some(value) = crate::values::lower_unit_scalar_argument_with_places(
            program,
            &facts.operators,
            body,
            call.statement_index,
            expression,
            primitive,
            |place, primitive| {
                bind_current(
                    program,
                    facts,
                    state,
                    call,
                    place.clone(),
                    primitive,
                    contexts,
                    bindings,
                )
            },
        ) {
            return Some(value);
        }
    }
    let mut place = actual.place.clone();
    place.segments.extend_from_slice(&actual.path);
    bind_current(
        program, facts, state, call, place, primitive, contexts, bindings,
    )
}

fn bind_current(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    place: CanonicalPlace,
    primitive: typed_trees::types::PrimitiveType,
    contexts: &[FactContextHandle],
    bindings: &mut Vec<crate::facts::boolean_predicates::BoundObservation>,
) -> Option<CheckedScalarExpression> {
    let type_reference = crate::flow::canonical_place_type_reference(
        program,
        state.state_symbol,
        call.statement_index,
        &place,
    )?;
    if program.primitive_type_reference(type_reference) != Some(primitive) {
        return None;
    }
    if let Some(value) = scalar_at(program, facts, contexts, &place) {
        if let Some(literal) = observed_literal(value, primitive) {
            return Some(literal);
        }
    }
    let operand = CheckedScalarExpression::Parameter {
        position: bindings.len(),
        primitive_type: primitive,
    };
    bindings.push(crate::facts::boolean_predicates::BoundObservation {
        operand: operand.clone(),
        place,
        type_reference,
    });
    Some(operand)
}

fn scalar_at(
    program: &TypedTrees,
    facts: &CheckFacts,
    contexts: &[FactContextHandle],
    place: &CanonicalPlace,
) -> Option<facts::ScalarValue> {
    super::prover::boolean_value_at_place(program, &facts.semantic, contexts, place)
        .map(facts::ScalarValue::Boolean)
        .or_else(|| {
            crate::values::scalar_value_at_place(
                program,
                &facts.semantic,
                contexts
                    .iter()
                    .map(|handle| facts.semantic.contexts.get(*handle)),
                place,
            )
        })
}

fn observed_literal(
    value: facts::ScalarValue,
    primitive: typed_trees::types::PrimitiveType,
) -> Option<CheckedScalarExpression> {
    match value {
        facts::ScalarValue::Boolean(value)
            if primitive == typed_trees::types::PrimitiveType::Bool =>
        {
            Some(CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::Constant(value),
            )))
        }
        facts::ScalarValue::Integer(value) => {
            let text = value.to_string();
            let digits = text.strip_prefix('-').unwrap_or(&text);
            let literal = numerics::literals::IntegerLiteral::from_parts(
                text.starts_with('-'),
                numerics::literals::IntegerRadix::Decimal,
                digits,
            )
            .ok()?
            .with_landing(numerics::literals::IntegerLanding {
                landed_type: crate::values::landed_for_primitive(primitive)?,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            });
            Some(CheckedScalarExpression::IntegerLiteral { literal })
        }
        _ => None,
    }
}

pub(crate) fn proves_entry_predicate(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[FactContextHandle],
    target: SymbolHandle,
    expression: CheckedBooleanExpression,
) -> bool {
    let Some(predicate) = crate::facts::boolean_predicates::bind_entry(program, target, expression)
    else {
        return false;
    };
    if target_machine(program, call) != Some(target) {
        return false;
    }
    let mut selected_contexts: Option<Vec<FactContextHandle>> = None;
    let mut binding_epochs: Vec<usize> = Vec::new();
    let mut current = predicate.clone();
    current.bindings.clear();
    if crate::facts::boolean_predicates::map_observations(&mut current.expression, &mut |operand| {
        let observation = predicate
            .bindings
            .iter()
            .find(|binding| binding.operand == *operand)?;
        let actual = observation_actual(program, state, call, observation)?;
        let capture = captured_actual(program, facts, state, call, &actual)?;
        let operand_contexts = crate::flow::operand_contexts(
            program,
            &facts.flow,
            &facts.semantic,
            capture,
            actual.parameter.type_reference,
            call.entry_constraints,
        );
        if let Some(selected) = &mut selected_contexts {
            selected.retain(|context| operand_contexts.contains(context));
        } else {
            selected_contexts = Some(operand_contexts.clone());
        }
        let epoch =
            if crate::flow::operand_binds_stable_copy(program, actual.parameter.type_reference) {
                actual.operand_position
            } else {
                facts
                    .flow
                    .control
                    .operands
                    .span(call.operands)?
                    .len()
                    .checked_sub(1)?
            };
        let first_binding = current.bindings.len();
        *operand = current_actual(
            program,
            facts,
            state,
            call,
            &actual,
            operand.primitive_type()?,
            &operand_contexts,
            &mut current.bindings,
        )?;
        let operands = facts.flow.control.operands.span(call.operands)?;
        for binding in &current.bindings[first_binding..] {
            for (previous, previous_epoch) in current.bindings[..first_binding]
                .iter()
                .zip(&binding_epochs)
            {
                if previous.place == binding.place
                    && !crate::flow::operand_observations_agree(
                        program,
                        &facts.flow,
                        operands.get(*previous_epoch)?,
                        operands.get(epoch)?,
                        &binding.place,
                    )
                {
                    return None;
                }
            }
        }
        binding_epochs.resize(current.bindings.len(), epoch);
        Some(())
    })
    .is_none()
    {
        return false;
    }
    let selected_contexts = selected_contexts.unwrap_or_else(|| contexts.to_vec());
    let retained =
        crate::facts::boolean_predicates::retained(program, &facts.semantic, &selected_contexts);
    crate::facts::boolean_predicates::proves(&retained, &current, |place| {
        scalar_at(program, facts, &selected_contexts, place)
    })
}

/// Transport only entry-custodied observations. A callee selector remains a
/// formula; it never becomes an unconditional caller cleanup demand.
pub(crate) fn entry_predicate(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    target: SymbolHandle,
    predicate: CheckedBooleanExpression,
) -> Option<CheckedBooleanExpression> {
    if target_machine(program, call)? != target {
        return None;
    }
    let predicate = crate::facts::boolean_predicates::bind_entry(program, target, predicate)?;
    let mut expression = predicate.expression.clone();
    crate::facts::boolean_predicates::map_observations(&mut expression, &mut |operand| {
        let observation = predicate
            .bindings
            .iter()
            .find(|binding| binding.operand == *operand)?;
        let actual = observation_actual(program, state, call, observation)?;
        let primitive = operand.primitive_type()?;
        let capture = captured_actual(program, facts, state, call, &actual)?;
        let (machine, body) = crate::lookup::symbols::machine_state_by_symbol(
            program,
            state.machine_symbol,
            state.state_symbol,
        )?;
        if let Some(expression) = projected_expression(program, &actual)
            && let Some(value) = crate::values::lower_entry_scalar_with_subjects(
                program,
                &facts.operators,
                machine,
                body,
                call.statement_index,
                expression,
                primitive,
                &|place| {
                    crate::flow::operand_entry_origin_is_stable(
                        program,
                        &facts.flow,
                        capture,
                        &place,
                    )
                    .then_some(())?;
                    crate::facts::entry_parameter_place_before_statement(
                        program,
                        state.machine_symbol,
                        state.state_symbol,
                        call.statement_index,
                        &place,
                    )
                },
            )
        {
            *operand = value;
            return Some(());
        }
        let mut place = actual.place;
        place.segments.extend(actual.path);
        crate::flow::operand_entry_origin_is_stable(program, &facts.flow, capture, &place)
            .then_some(())?;
        let (position, path) = crate::facts::entry_parameter_place_before_statement(
            program,
            state.machine_symbol,
            state.state_symbol,
            call.statement_index,
            &place,
        )?;
        *operand = if path.is_empty() {
            let machine = program.machine_by_symbol(state.machine_symbol)?;
            let index = crate::checks::termination::named_transition_target_state_index(
                program,
                machine,
                machine.symbol,
            )?;
            let entry = program.machine_states(machine).get(index)?;
            let parameters = program.state_parameters(entry);
            if program.primitive_type_reference(parameters.get(position as usize)?.type_reference)
                != Some(primitive)
            {
                return None;
            }
            CheckedScalarExpression::Parameter {
                position: parameters[..position as usize]
                    .iter()
                    .filter(|parameter| crate::values::occupies_scalar_position(program, parameter))
                    .count(),
                primitive_type: primitive,
            }
        } else {
            CheckedScalarExpression::StructuralParameterField {
                parameter_position: position,
                path,
                primitive_type: primitive,
            }
        };
        Some(())
    })?;
    Some(expression)
}
