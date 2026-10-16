//! Bind invocation predicates to actual values captured in source order.
//!
//! Detached copies keep operand-time observations; reference operands use the
//! invocation's live referents. Cleanup requirements and concrete write checks
//! share this substitution law. Neither may reread an initializer or substitute
//! a later value for an operand already evaluated.

use validation::CanonicalPlace;

use checked_trees::{
    CheckFacts, CheckedBooleanExpression, CheckedScalarExpression, FlowCallFact, FlowStateFact,
};
use facts::FactContextHandle;
use symbols::SymbolHandle;
use typed_trees::{expression::ExpressionHandle, signature::StateParameter};

mod captured_predicates;

pub(crate) fn target_machine(
    program: &dyn ::validation::CheckingInput,
    call: &FlowCallFact,
) -> Option<SymbolHandle> {
    program
        .machine_by_symbol(call.target_symbol)
        .map(|machine| machine.symbol)
        .or_else(|| {
            validation::find_machine_by_entry_state(program, call.target_symbol)
                .map(|(machine, _)| machine.symbol)
        })
}

pub(crate) struct Actual<'program> {
    pub(crate) parameter: &'program StateParameter,
    pub(crate) argument: Option<ExpressionHandle>,
    pub(crate) path: Vec<facts::PlaceSegment>,
    pub(crate) operand_position: usize,
}

pub(crate) fn actual_at<'program>(
    program: &'program dyn ::validation::CheckingInput,
    state: &FlowStateFact,
    call: &FlowCallFact,
    position: usize,
    path: Vec<facts::PlaceSegment>,
) -> Option<Actual<'program>> {
    let site = validation::find_call_site(
        program,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let parameters = validation::call_target_parameters(program, call.target_symbol)?;
    let parameter = parameters.get(position)?;
    let receiver_offset = usize::from(
        matches!(&site, validation::CallSite::Expression { call, .. } if call.receiver.is_valid()),
    );
    let ordinal = parameters[..position].iter().count();
    let operand_position = { ordinal + receiver_offset };
    let argument = {
        let ordinal = parameters[..position].iter().count();
        Some(*validation::call_site_argument_expressions(program, &site).get(ordinal)?)
    };
    Some(Actual {
        parameter,
        argument,
        path,
        operand_position,
    })
}

/// Bind a callee observation to its actual operand, without assuming that the
/// formal's declaration position is a dense scalar position.
pub(crate) fn observation_actual<'program>(
    program: &'program dyn ::validation::CheckingInput,
    state: &FlowStateFact,
    call: &FlowCallFact,
    observation: &crate::facts::boolean_predicates::BoundObservation,
) -> Option<Actual<'program>> {
    let parameters = validation::call_target_parameters(program, call.target_symbol)?;
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

fn captured_operand<'facts>(
    program: &dyn ::validation::CheckingInput,
    facts: &'facts CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    actual: &Actual<'_>,
) -> Option<&'facts checked_trees::FlowOperandFact> {
    crate::flow::captured_call_operands(program, &facts.flow, state, call)?
        .0
        .get(actual.operand_position)
}

/// The complete call telescope in exact captured source order. Static namespace
/// receivers are evaluation positions but not explicit argument parameters;
/// retain the same receiver offset used by individual captured observations.
pub(crate) fn captured_argument_expressions(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
) -> Option<Vec<ExpressionHandle>> {
    if !call.authored_source_custody_valid {
        return None;
    }
    let (operands, receiver_offset) =
        crate::flow::captured_call_operands(program, &facts.flow, state, call)?;
    Some(
        operands[receiver_offset..]
            .iter()
            .map(|operand| operand.expression)
            .collect(),
    )
}

pub(crate) fn captured_values<'facts>(
    program: &dyn ::validation::CheckingInput,
    facts: &'facts CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    actual: &Actual<'_>,
) -> Option<Vec<crate::flow::CapturedValue<'facts>>> {
    let operand = captured_operand(program, facts, state, call, actual)?;
    crate::flow::captured_operand_values(
        program,
        &facts.flow,
        &facts.semantic,
        state,
        operand,
        actual.parameter.type_reference,
        &actual.path,
    )
}

fn captured_value<'facts>(
    program: &dyn ::validation::CheckingInput,
    facts: &'facts CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    actual: &Actual<'_>,
) -> Option<crate::flow::CapturedValue<'facts>> {
    let mut values = captured_values(program, facts, state, call, actual)?;
    if values.len() != 1 {
        return None;
    }
    values.pop()
}

/// Complete ordered operand evidence supplies closed arithmetic observations
/// before any source-substitution tier can reinterpret an earlier descriptor.
pub(crate) fn proves_captured_arithmetic_requirement(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    expression: ExpressionHandle,
) -> bool {
    let Some((machine, body)) = validation::find_state(program, call.target_symbol)
        .or_else(|| validation::find_machine_head(program, call.target_symbol))
    else {
        return false;
    };
    if captured_argument_expressions(program, facts, state, call).is_none() {
        return false;
    }
    validation::prove_captured_arithmetic_call_requirement(
        program,
        machine,
        body,
        expression,
        |formal, observation| {
            captured_arithmetic_observation(program, facts, state, call, formal, observation)
        },
    )
}

/// A requirement observes only its demanded formal coordinate. The complete
/// ordered call roster remains authenticated by `captured_call_operands`; this
/// projection does not suppress evaluation or availability of another actual.
/// Scalar values use the established copy/live-referent epoch. Descriptor
/// geometry closes in its own completed operand epoch, not in later invocation
/// facts about the selector or endpoints that built that descriptor.
pub(crate) fn captured_arithmetic_observation(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    formal: SymbolHandle,
    observation: validation::ArithmeticCallObservation<'_>,
) -> Option<numerics::bignum::BigInt> {
    if !call.authored_source_custody_valid {
        return None;
    }
    let parameters = validation::call_target_parameters(program, call.target_symbol)?;
    let position = parameters
        .iter()
        .position(|parameter| parameter.symbol == formal)?;
    let path = match observation {
        validation::ArithmeticCallObservation::Value(path)
        | validation::ArithmeticCallObservation::CollectionLength(path) => path
            .iter()
            .map(|symbol| facts::PlaceSegment::Field { symbol: *symbol })
            .collect(),
    };
    let actual = actual_at(program, state, call, position, path)?;
    let captured = captured_value(program, facts, state, call, &actual)?;
    if captured.observation.state_symbol != state.state_symbol
        || captured.observation.statement_index != call.statement_index
        || captured.operand.state_symbol != state.state_symbol
        || captured.operand.statement_index > call.statement_index
    {
        return None;
    }
    let (machine, body) =
        validation::machine_state_by_symbols(program, state.machine_symbol, state.state_symbol)?;
    match observation {
        validation::ArithmeticCallObservation::Value(_) => {
            let reference = validation::project_type_reference_from_segments(
                program,
                actual.parameter.type_reference,
                &actual.path,
            )?;
            let primitive = program.primitive_type_reference(reference)?;
            if !primitive.is_fixed_integer() {
                return None;
            }
            let contexts = actual_contexts(program, facts, state, call, &actual);
            let value = current_actual(
                program,
                facts,
                state,
                call,
                &captured,
                primitive,
                &contexts,
                &mut Vec::new(),
            )?;
            match crate::values::evaluate_checked_scalar(&value, &mut |_| None)? {
                facts::ScalarValue::Integer(value) => Some(value),
                _ => None,
            }
        }
        validation::ArithmeticCallObservation::CollectionLength(_) => {
            // Range operands and stored descriptors retain one formed address.
            // Element writes cannot resize it; endpoint variables are not read
            // again at invocation. Borrow admission remains independent.
            if let Some(length) = crate::checks::collection_extents::from_captured_value(
                program,
                &facts.borrow,
                &facts.flow,
                &captured,
                facts
                    .flow
                    .contexts
                    .constraint_refs
                    .span_or_empty(call.entry_constraints),
            ) {
                return Some(numerics::bignum::BigInt::from_u64(
                    u64::try_from(length).ok()?,
                ));
            }
            // This closes produced geometry only: the empty-symbol length
            // engine has no live descriptor/formal length hypotheses. Stored
            // descriptor references remain unresolved unless the ordinary
            // immutable initializer owner authenticates their saved value.
            if !captured.projection.remaining.is_empty() {
                return None;
            }
            let contexts = facts
                .flow
                .semantic_constraint_contexts(captured.operand.constraints)
                .collect::<Vec<_>>();
            let length = validation::captured_collection_length(
                program,
                machine,
                body,
                captured.projection.expression,
                |place| {
                    let reference = validation::canonical_place_type_reference(
                        program,
                        state.state_symbol,
                        captured.operand.statement_index,
                        place,
                    )?;
                    if !program
                        .primitive_type_reference(reference)?
                        .is_fixed_integer()
                    {
                        return None;
                    }
                    match scalar_at(program, facts, &contexts, place)? {
                        facts::ScalarValue::Integer(value) => Some(value),
                        _ => None,
                    }
                },
            )?;
            Some(numerics::bignum::BigInt::from_u64(length))
        }
    }
}

#[derive(Default)]
pub(super) struct MembershipEvidence {
    pub contexts: Vec<FactContextHandle>,
    pub referents: Option<Vec<CanonicalPlace>>,
}

/// Bind the subject and evidence epoch of a declared membership's exact formal. A
/// copied argument retains completion facts even when a later operand writes
/// its source; borrowed arguments retain the ordinary invocation-live rules.
/// The declaration's subject and its substituted place must agree with the
/// same argument occurrence. A missing capture supplies no evidence.
pub(super) fn membership_evidence(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &facts::Fact,
) -> Option<MembershipEvidence> {
    let facts::FactPayload::ContractDomainMembership {
        value,
        fact: source_fact,
        ..
    } = requirement.payload
    else {
        return None;
    };
    let declared = validation::canonical_place_from_expression(program, value)?;
    let symbol = declared.root.as_symbol()?;
    let Some(contract_call) = crate::flow::proof_contract_call(
        &facts.proof,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    ) else {
        return Some(MembershipEvidence::default());
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
            Err(()) => return Some(MembershipEvidence::default()),
        };
        if position.is_some_and(|position| position != bound) {
            return Some(MembershipEvidence::default());
        }
        position = Some(bound);
    }
    let Some(position) = position else {
        return Some(MembershipEvidence::default());
    };
    let Some(actual) = actual_at(program, state, call, position, declared.segments) else {
        return Some(MembershipEvidence::default());
    };
    let Some(place) = requirement.place.as_place() else {
        return Some(MembershipEvidence::default());
    };
    let Some(required) = crate::flow::canonical_place_from_semantic_place(
        program,
        &facts.semantic,
        facts.semantic.places.get(place),
    ) else {
        return Some(MembershipEvidence::default());
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
        return Some(MembershipEvidence::default());
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
        return Some(MembershipEvidence::default());
    }
    let Some(values) = captured_values(program, facts, state, call, &actual) else {
        return Some(MembershipEvidence::default());
    };
    let contexts = match values.as_slice() {
        [captured] => crate::flow::operand_contexts(
            program,
            &facts.flow,
            &facts.semantic,
            captured.observation,
            captured.observation_type,
            call.entry_constraints,
        ),
        _ => Vec::new(),
    };
    let referents = (!values.is_empty()
        && values
            .iter()
            .all(|value| crate::flow::operand_binds_live_view(program, value.type_reference)))
    .then(|| {
        values
            .iter()
            .map(|value| {
                // Missing address capture is not an authenticated backing
                // place. Do not reconstruct it from initializer syntax.
                if value.operand.referents.is_empty() {
                    return None;
                }
                crate::flow::captured_value_places(program, &facts.flow, value)
            })
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default()
            .into_iter()
            .flatten()
            .map(|place| place.projected)
            .collect()
    });
    // A stored view carries the address captured when it was formed, while
    // its membership is tested in the invocation's live context. Rebinding
    // only the context leaves record-held views stranded at the descriptor
    // slot and misses elementwise evidence on their actual backing storage.
    Some(MembershipEvidence {
        contexts,
        referents,
    })
}

/// A projected field observes its own completed child. A stored aggregate
/// retains its whole-value capture; its initializer is never replayed.
pub(crate) fn actual_contexts(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    actual: &Actual<'_>,
) -> Vec<FactContextHandle> {
    let Some(captured) = captured_value(program, facts, state, call, actual) else {
        return Vec::new();
    };
    crate::flow::operand_contexts(
        program,
        &facts.flow,
        &facts.semantic,
        captured.observation,
        captured.observation_type,
        call.entry_constraints,
    )
}

/// Rebind a selected actual expression to exact current storage. Local slots
/// denote stored values; their initializer expressions are never replayed.
#[allow(clippy::too_many_arguments)]
fn current_actual(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    captured: &crate::flow::CapturedValue<'_>,
    primitive: typed_trees::types::PrimitiveType,
    contexts: &[FactContextHandle],
    bindings: &mut Vec<crate::facts::boolean_predicates::BoundObservation>,
) -> Option<CheckedScalarExpression> {
    let (machine, body) =
        validation::machine_state_by_symbols(program, state.machine_symbol, state.state_symbol)?;
    let capture = captured.observation;
    let projection = &captured.observation_projection;
    let expression = projection.expression;
    if projection.remaining.is_empty() {
        if !validation::has_builtin_bound_expression_meaning(
            program,
            machine,
            Some(body),
            expression,
        ) {
            return None;
        }
        if let Some(value) = crate::values::lower_unit_scalar_argument_for_places(
            program,
            &facts.operators,
            body,
            capture.statement_index,
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
    // Facts established on stored values belong to the observed storage, not
    // its earlier producer. A reaching match/call may explain the value but
    // cannot replace the local's later guard namespace. Constructed children
    // already carry their own exact observation and completion epoch.
    let mut place = validation::canonical_place_from_expression_in_state(
        program,
        capture.state_symbol,
        capture.statement_index,
        expression,
    )?;
    place.extend_segments(&projection.remaining);
    bind_current(
        program, facts, state, call, place, primitive, contexts, bindings,
    )
}

fn bind_current(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    place: CanonicalPlace,
    primitive: typed_trees::types::PrimitiveType,
    contexts: &[FactContextHandle],
    bindings: &mut Vec<crate::facts::boolean_predicates::BoundObservation>,
) -> Option<CheckedScalarExpression> {
    let type_reference = validation::canonical_place_type_reference(
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[FactContextHandle],
    target: SymbolHandle,
    expression: CheckedBooleanExpression,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some(predicate) = crate::facts::boolean_predicates::bind_entry(program, target, expression)
    else {
        return false;
    };
    if target_machine(program, call) != Some(target) {
        return false;
    }
    proves_bound_predicate(
        program,
        facts,
        state,
        call,
        contexts,
        &predicate,
        frames,
        &|observation| observation_actual(program, state, call, observation),
    )
}

/// The source row owns its formal-to-actual correspondence. Entry cleanup,
/// ordinary declarations, named states and inherited rows share one captured
/// Boolean algebra after supplying that exact correspondence.
pub(crate) fn proves_bound_predicate<'program>(
    program: &'program dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[FactContextHandle],
    predicate: &crate::facts::boolean_predicates::BoundPredicate,
    frames: Option<&validation::CallFrameResolver<'_>>,
    actual: &impl Fn(&crate::facts::boolean_predicates::BoundObservation) -> Option<Actual<'program>>,
) -> bool {
    // Judge one formula in the observed-value namespace. Producer evidence
    // joins each exact copied value independently, rather than requiring all
    // operands to retain the same context or choosing one global proof mode.
    let mut selected_contexts: Option<Vec<FactContextHandle>> = None;
    let mut binding_epochs: Vec<&checked_trees::FlowOperandFact> = Vec::new();
    let mut captures = Vec::new();
    let mut current = predicate.clone();
    current.bindings.clear();
    if crate::facts::boolean_predicates::map_observations(&mut current.expression, &mut |operand| {
        let observation = predicate.binding(&operand)?;
        let actual = actual(observation)?;
        let captured = captured_value(program, facts, state, call, &actual)?;
        let capture = captured.observation;
        let capture_type = captured.observation_type;
        let operand_contexts = crate::flow::operand_contexts(
            program,
            &facts.flow,
            &facts.semantic,
            capture,
            capture_type,
            call.entry_constraints,
        );
        if let Some(selected) = &mut selected_contexts {
            selected.retain(|context| operand_contexts.contains(context));
        } else {
            selected_contexts = Some(operand_contexts.clone());
        }
        let epoch = if crate::flow::operand_binds_live_view(program, capture_type) {
            facts.flow.control.operands.span(call.operands)?.last()?
        } else {
            capture
        };
        let first_binding = current.bindings.len();
        *operand = current_actual(
            program,
            facts,
            state,
            call,
            &captured,
            operand.primitive_type()?,
            &operand_contexts,
            &mut current.bindings,
        )?;
        for binding in &current.bindings[first_binding..] {
            for (previous, previous_epoch) in current.bindings[..first_binding]
                .iter()
                .zip(&binding_epochs)
            {
                if previous.place == binding.place
                    && !crate::flow::operand_observations_agree(
                        program,
                        &facts.flow,
                        previous_epoch,
                        epoch,
                        &binding.place,
                    )
                {
                    return None;
                }
            }
        }
        binding_epochs.resize(current.bindings.len(), epoch);
        captured_predicates::retain_bindings(
            program,
            facts,
            &captured,
            epoch,
            &current.bindings[first_binding..],
            first_binding,
            &mut captures,
        );
        Some(())
    })
    .is_none()
    {
        return false;
    }
    let selected_contexts = selected_contexts.unwrap_or_else(|| contexts.to_vec());
    let mut retained =
        crate::facts::boolean_predicates::retained(program, &facts.semantic, &selected_contexts);
    // Common observation contexts retain guards on current storage. Per-value
    // producer contexts are joined below, with all dependencies epoch-checked;
    // a later write cannot donate a new value to an earlier capture.
    if let Some(frames) = frames {
        retained.extend(super::prover::call_guarantees::current_predicates(
            program,
            facts,
            state,
            call.statement_index + 1,
            &selected_contexts,
            frames,
        ));
    }
    retained.extend(captured_predicates::transport(
        program,
        facts,
        state,
        &mut current,
        &captures,
        frames,
    ));
    crate::facts::boolean_predicates::proves(&retained, &current, |place| {
        scalar_at(program, facts, &selected_contexts, place)
    })
}

/// Transport only entry-custodied observations. A callee selector remains a
/// formula; it never becomes an unconditional caller cleanup demand.
pub(crate) fn entry_predicate(
    program: &dyn ::validation::CheckingInput,
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
        let observation = predicate.binding(&operand)?;
        let actual = observation_actual(program, state, call, observation)?;
        let primitive = operand.primitive_type()?;
        let captured = captured_value(program, facts, state, call, &actual)?;
        let capture = captured.operand;
        let (machine, body) = validation::machine_state_by_symbols(
            program,
            state.machine_symbol,
            state.state_symbol,
        )?;
        if captured.projection.remaining.is_empty()
            && let Some(value) = crate::values::lower_entry_scalar(
                program,
                &facts.operators,
                machine,
                body,
                capture.statement_index,
                captured.projection.expression,
                primitive,
                &|place| {
                    crate::flow::operand_entry_origin_is_stable(
                        program,
                        &facts.flow,
                        capture,
                        place,
                    )
                    .then_some(())?;
                    crate::facts::entry_parameter_place_before_statement(
                        program,
                        state.machine_symbol,
                        state.state_symbol,
                        capture.statement_index,
                        place,
                    )
                },
            )
        {
            *operand = value;
            return Some(());
        }
        let mut place = validation::canonical_place_from_expression_in_state(
            program,
            capture.state_symbol,
            capture.statement_index,
            captured.projection.expression,
        )?;
        place.extend_segments(&captured.projection.remaining);
        crate::flow::operand_entry_origin_is_stable(program, &facts.flow, capture, &place)
            .then_some(())?;
        let (position, path) = crate::facts::entry_parameter_place_before_statement(
            program,
            state.machine_symbol,
            state.state_symbol,
            capture.statement_index,
            &place,
        )?;
        *operand = crate::facts::boolean_predicates::entry_scalar(
            program,
            state.machine_symbol,
            position,
            path,
            primitive,
        )?;
        Some(())
    })?;
    Some(expression)
}
