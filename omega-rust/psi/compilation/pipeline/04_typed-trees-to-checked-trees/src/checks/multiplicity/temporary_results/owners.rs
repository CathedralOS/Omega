//! The anonymous owners one statement's calls consume. A producer completes
//! before its consumer: a construction establishes its value, and a call
//! result is established by its call's completion. One source evaluation
//! inventory supplies named/anonymous permission production and the exact loan
//! continuation; executable admission consumes that published roster.
use super::CheckFacts;
use crate::checks::multiplicity::linear_obligations::type_reference_is_reference;
use crate::checks::type_carries_linear_obligation;
use language_semantics::PermissionEventSource;

/// How the consuming call uses the anonymous owner.
pub(crate) enum TemporaryConsumption {
    /// An evaluated loan retains the whole producing owner, even when its
    /// operand selects a child; the owner dies after the call.
    Borrow {
        access: language_semantics::PermissionAccess,
        borrowed_path: Vec<facts::PlaceSegment>,
    },
    /// The call receives the completed owner whole, with every claim its
    /// producer established or its operands moved into it.
    OwnedTransfer,
    /// The call receives one owned subtree; its normal continuation disposes
    /// the original owner's untouched complement.
    OwnedProjection { path: Vec<facts::PlaceSegment> },
}

pub(crate) struct TemporaryOwner {
    pub(crate) expression: typed_trees::expression::ExpressionHandle,
    pub(crate) type_reference: typed_trees::types::TypeReferenceHandle,
    pub(crate) consumption: TemporaryConsumption,
    pub(crate) evaluation: checked_trees::FlowPermissionEvaluation,
    pub(crate) producer: PermissionEventSource,
    pub(crate) consumer: PermissionEventSource,
    /// A noncall observation can be the last user of returned backing. Zero
    /// retains the exact call continuation named by `consumer`.
    pub(crate) completion: typed_trees::expression::ExpressionHandle,
    pub(crate) completion_constraints: Option<arena::HandleSpan<checked_trees::FlowConstraintRef>>,
    completion_ordinal: usize,
}

/// The claim-bearing type of a by-value operand an anonymous producer
/// completes. Existing owned storage keeps its claim identities; an exclusive
/// borrowed extraction establishes machine-local custody, because the caller's
/// claims stay suspended with its loan. Canonical storage, not source spelling,
/// distinguishes those producers from an ordinary owned transfer.
pub(crate) fn owned_temporary_type(
    program: &dyn ::validation::CheckingInput,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    parameter: &typed_trees::signature::StateParameter,
    argument: typed_trees::expression::ExpressionHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if parameter.is_const
        || parameter.relevance.is_erased()
        || type_reference_is_reference(program, parameter.type_reference)
    {
        return None;
    }
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        argument,
    )?;
    let anonymous_value =
        place.root == facts::PlaceRoot::Expression(argument) && place.segments.is_empty();
    let borrowed_value =
        super::super::projected_affine::has_reference_prefix(
            program,
            state_symbol,
            statement_index,
            &place,
        ) && validation::find_state(program, state_symbol).is_some_and(|(machine, state)| {
            super::super::borrowed_windows::authored_route_exclusive(
                program,
                machine,
                state,
                statement_index,
                place.root,
                &place.segments,
            )
        });
    if !anonymous_value && !borrowed_value {
        return None;
    }
    let type_reference =
        validation::canonical_place_type_reference(program, state_symbol, statement_index, &place)?;
    let type_reference = if borrowed_value {
        validation::assignment_value_type(program, type_reference)
    } else {
        type_reference
    };
    type_carries_linear_obligation(program, type_reference).then_some(type_reference)
}

pub(crate) fn temporary_owners(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    calls: &[checked_trees::FlowCallFact],
) -> Result<Vec<TemporaryOwner>, diagnostics::Diagnostic> {
    let completed = program
        .statement_table
        .indexed_statements(state.statement_nodes)
        .map(|(statement_index, statement)| {
            crate::checks::ownership_occurrences::statement_steps(
                program,
                machine,
                state,
                statement_index,
                statement,
                &[],
                &facts.operators,
            )
            .into_iter()
            .filter_map(|occurrence| match occurrence.step {
                crate::checks::ownership_occurrences::StatementStep::Complete(expression) => {
                    Some((expression, occurrence.route))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut owners = Vec::new();
    for consumer in calls {
        if facts.flow.control.is_retired(state.symbol, consumer)
            || (consumer.authored_expression.is_valid()
                && !completed
                    .get(consumer.statement_index)
                    .is_some_and(|expressions| {
                        expressions
                            .iter()
                            .any(|(expression, _)| *expression == consumer.authored_expression)
                    }))
        {
            continue;
        }
        let Some(site) = validation::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            consumer.statement_index,
            consumer.call_ordinal,
        ) else {
            continue;
        };
        let Some(parameters) = validation::call_target_parameters(program, consumer.target_symbol)
        else {
            continue;
        };
        let arguments = validation::call_site_argument_expressions(program, &site);
        if parameters.len() != arguments.len() {
            continue;
        }
        for (parameter, argument) in parameters.iter().zip(arguments) {
            let (expression, type_reference, consumption) = if let Some(type_reference) =
                owned_temporary_type(
                    program,
                    state.symbol,
                    consumer.statement_index,
                    parameter,
                    *argument,
                ) {
                (
                    *argument,
                    type_reference,
                    TemporaryConsumption::OwnedTransfer,
                )
            } else if !site.is_internal_state_transfer(program, machine.symbol)
                && let Some((expression, type_reference, path)) =
                    owned_projection(program, state.symbol, consumer, parameter, *argument)
            {
                (
                    expression,
                    type_reference,
                    TemporaryConsumption::OwnedProjection { path },
                )
            } else {
                if parameter.is_const || parameter.is_mutable || parameter.relevance.is_erased() {
                    continue;
                }
                let descriptor_access =
                    validation::slice_view_shell(program, parameter.type_reference, &[]).and_then(
                        |shell| match shell.access {
                            language_semantics::ReferenceAccess::Shared => {
                                Some(language_semantics::PermissionAccess::Shared)
                            }
                            language_semantics::ReferenceAccess::Mutable => {
                                Some(language_semantics::PermissionAccess::Exclusive)
                            }
                            _ => None,
                        },
                    );
                let (borrowed, access) = if let Some(access) = descriptor_access {
                    (*argument, access)
                } else if let Some((borrowed, _)) = crate::semantic::calls::shared_argument_referent(
                    program,
                    *argument,
                    parameter.type_reference,
                ) {
                    (borrowed, language_semantics::PermissionAccess::Shared)
                } else {
                    continue;
                };
                let Some(place) = validation::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    consumer.statement_index,
                    borrowed,
                ) else {
                    continue;
                };
                let Some(expression) = place.root.as_expression() else {
                    continue;
                };
                let root = validation::CanonicalPlace {
                    root: place.root,
                    segments: Vec::new(),
                };
                let Some(type_reference) = validation::canonical_place_type_reference(
                    program,
                    state.symbol,
                    consumer.statement_index,
                    &root,
                ) else {
                    continue;
                };
                (
                    expression,
                    type_reference,
                    TemporaryConsumption::Borrow {
                        access,
                        borrowed_path: place.segments,
                    },
                )
            };
            let evaluation = crate::flow::call_permission_evaluation(
                program,
                machine,
                state,
                consumer.statement_index,
                consumer.call_ordinal,
            );
            if evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
                continue;
            }
            let producer = if program.expression_table.call_node(expression).is_some() {
                let mut producers = calls.iter().filter(|producer| {
                    producer.statement_index == consumer.statement_index
                        && producer.authored_expression == expression
                });
                let Some(producer) = producers.next() else {
                    continue;
                };
                if producers.next().is_some()
                    || crate::flow::call_permission_evaluation(
                        program,
                        machine,
                        state,
                        producer.statement_index,
                        producer.call_ordinal,
                    ) != evaluation
                {
                    continue;
                }
                PermissionEventSource::Call {
                    statement_index: producer.statement_index,
                    call_ordinal: producer.call_ordinal,
                    target_symbol: producer.target_symbol,
                }
            } else {
                PermissionEventSource::Statement {
                    statement_index: consumer.statement_index,
                }
            };
            let mut completions = completed
                .get(consumer.statement_index)
                .into_iter()
                .flatten()
                .enumerate()
                .filter(|(_, (completed, _))| *completed == expression);
            let Some((completion_ordinal, _)) = completions.next() else {
                continue;
            };
            if completions.next().is_some() {
                continue;
            }
            owners.push(TemporaryOwner {
                expression,
                type_reference,
                consumption,
                evaluation,
                producer,
                consumer: PermissionEventSource::Call {
                    statement_index: consumer.statement_index,
                    call_ordinal: consumer.call_ordinal,
                    target_symbol: consumer.target_symbol,
                },
                completion_ordinal,
                completion: typed_trees::expression::ExpressionHandle::invalid(),
                completion_constraints: None,
            });
        }
    }
    // A returned descriptor can lend the original anonymous owner to a later
    // call in this evaluation. Operand capture already retains its actual
    // lifetime dependencies, independently of returned address precision. Keep the original
    // establishment and borrowed region, but give its normal death to the
    // last completing consumer that still holds that exact backing.
    for owner in &mut owners {
        if !matches!(owner.consumption, TemporaryConsumption::Borrow { .. }) {
            continue;
        }
        let PermissionEventSource::Call {
            statement_index, ..
        } = owner.consumer
        else {
            continue;
        };
        let Some(completions) = completed.get(statement_index) else {
            continue;
        };
        let completion_at = |call: &checked_trees::FlowCallFact| {
            if !call.authored_expression.is_valid() {
                return Some((completions.len(), &[][..]));
            }
            let mut matching = completions
                .iter()
                .enumerate()
                .filter(|(_, (expression, _))| *expression == call.authored_expression);
            let (ordinal, (_, route)) = matching.next()?;
            matching
                .next()
                .is_none()
                .then_some((ordinal, route.as_slice()))
        };
        let mut original = calls.iter().filter(|call| {
            owner.consumer
                == PermissionEventSource::Call {
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    target_symbol: call.target_symbol,
                }
        });
        let Some(original_call) = original.next() else {
            continue;
        };
        if original.next().is_some() {
            continue;
        }
        let Some((mut last_completion, original_route)) = completion_at(original_call) else {
            continue;
        };
        for consumer in calls
            .iter()
            .filter(|call| call.statement_index == statement_index)
        {
            if facts.flow.control.is_retired(state.symbol, consumer) {
                continue;
            }
            if crate::flow::call_permission_evaluation(
                program,
                machine,
                state,
                statement_index,
                consumer.call_ordinal,
            ) != owner.evaluation
            {
                continue;
            }
            let Some((completion, route)) = completion_at(consumer) else {
                continue;
            };
            // The later completion must execute whenever the original did.
            // A joined enclosing call is valid; a sibling or newly conditional
            // call cannot take unconditional responsibility for this death.
            if completion < last_completion
                || !route
                    .iter()
                    .all(|decision| original_route.contains(decision))
            {
                continue;
            }
            let mut retains_owner = false;
            for operand in consumer.argument_operands().handles() {
                let capture = facts.flow.control.operands.get(operand);
                let roots = crate::flow::captured_lifetime_roots(program, &facts.flow.control, capture)
                    .ok_or_else(|| diagnostics::Diagnostic::error(format!(
                        "temporary backing lifetime coverage is unavailable for an operand of `{}` in state `{}` at statement {}",
                        program.symbols.name(consumer.target_symbol), state.name, statement_index,
                        )).with_source_span(program.expression_table.source_span(capture.expression)))?;
                retains_owner |= roots.contains(&facts::PlaceRoot::Expression(owner.expression));
            }
            if retains_owner {
                owner.consumer = PermissionEventSource::Call {
                    statement_index,
                    call_ordinal: consumer.call_ordinal,
                    target_symbol: consumer.target_symbol,
                };
                last_completion = completion;
            }
        }
        let Some(flow_state) = facts.flow.control.state(machine.symbol, state.symbol) else {
            continue;
        };
        let Some(statement) = facts.flow.state_statement(flow_state, statement_index) else {
            continue;
        };
        for observation in facts
            .flow
            .control
            .value_observations
            .span_or_empty(statement.value_observations)
            .iter()
            .filter(|observation| observation.evaluation == owner.evaluation)
        {
            let mut matching = completions
                .iter()
                .enumerate()
                .filter(|(_, (expression, _))| *expression == observation.expression);
            let Some((completion, (_, route))) = matching.next() else {
                continue;
            };
            if matching.next().is_some()
                || completion < last_completion
                || !route
                    .iter()
                    .all(|decision| original_route.contains(decision))
            {
                continue;
            }
            let capture = facts.flow.control.operands.get(observation.receiver);
            let roots = crate::flow::captured_observation_roots(program, &facts.flow.control, capture)
                .ok_or_else(|| diagnostics::Diagnostic::error(format!(
                    "temporary backing observation coverage is unavailable for an observed receiver in state `{}` at statement {}",
                    state.name, statement_index,
                )).with_source_span(program.expression_table.source_span(observation.expression)))?;
            if roots.contains(&facts::PlaceRoot::Expression(owner.expression)) {
                if facts
                    .flow
                    .control
                    .value_observations
                    .span_or_empty(statement.value_observations)
                    .iter()
                    .filter(|candidate| {
                        candidate.expression == observation.expression
                            && candidate.evaluation == owner.evaluation
                    })
                    .count()
                    != 1
                {
                    return Err(diagnostics::Diagnostic::error(format!(
                        "temporary owner has ambiguous value completion in state `{}` at statement {}",
                        state.name, statement_index,
                    )).with_source_span(program.expression_table.source_span(observation.expression)));
                }
                owner.consumer = PermissionEventSource::Statement { statement_index };
                owner.completion = observation.expression;
                owner.completion_constraints = Some(observation.constraints);
                last_completion = completion;
            }
        }
    }
    // Cleanup later filters one exact consumer and reverses this shared
    // completion order, without grouping borrowed and projected owners.
    owners.sort_by_key(|owner| owner.completion_ordinal);
    Ok(owners)
}

fn owned_projection(
    program: &dyn validation::CheckingInput,
    state: symbols::SymbolHandle,
    consumer: &checked_trees::FlowCallFact,
    parameter: &typed_trees::signature::StateParameter,
    argument: typed_trees::expression::ExpressionHandle,
) -> Option<(
    typed_trees::expression::ExpressionHandle,
    typed_trees::types::TypeReferenceHandle,
    Vec<facts::PlaceSegment>,
)> {
    // An internal state transfer has no call continuation: its selected exit
    // owns residual disposal. Ordinary machine invocations retain this death
    // beside their other anonymous argument owners.
    if parameter.is_const
        || parameter.relevance.is_erased()
        || type_reference_is_reference(program, parameter.type_reference)
    {
        return None;
    }
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state,
        consumer.statement_index,
        argument,
    )?;
    let expression = place.root.as_expression()?;
    if place.segments.is_empty() {
        return None;
    }
    let endpoint = validation::canonical_place_type_reference(
        program,
        state,
        consumer.statement_index,
        &place,
    )?;
    if program.type_multiplicity(endpoint) == language_semantics::Multiplicity::Unrestricted
        || type_reference_is_reference(program, endpoint)
        || super::super::projected_affine::has_reference_prefix(
            program,
            state,
            consumer.statement_index,
            &place,
        )
    {
        return None;
    }
    let reference = projectable_owner(program, state, consumer.statement_index, expression)?;
    Some((expression, reference, place.segments))
}

/// The type of an anonymous affine owner whose untouched complement may die
/// after one subtree moves out. Its completing consumer must separately prove
/// authorized disposal of every claim left under the original owner.
pub(in crate::checks::multiplicity) fn projectable_owner(
    program: &dyn validation::CheckingInput,
    state: symbols::SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let reference = validation::canonical_place_type_reference(
        program,
        state,
        statement_index,
        &validation::CanonicalPlace {
            root: facts::PlaceRoot::Expression(expression),
            segments: Vec::new(),
        },
    )?;
    // An affine containing home supplies no authority to settle its contained
    // linear claims. Continuation checking authenticates their exact remaining
    // custody and owner-authorized dispositions after the selected transfer.
    (program.type_multiplicity(reference) == language_semantics::Multiplicity::Affine)
        .then_some(reference)
}
