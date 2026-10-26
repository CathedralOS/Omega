//! The anonymous owners one statement's consumers receive. A producer completes
//! before its consumer: a construction establishes its value, and a call
//! result is established by its call's completion. One source evaluation
//! inventory supplies named/anonymous permission production and the exact loan
//! continuation; executable admission consumes that published roster.
use super::CheckFacts;
use crate::checks::multiplicity::linear_obligations::type_reference_is_reference;
use crate::checks::type_carries_linear_obligation;
use language_semantics::PermissionEventSource;

/// How the consuming operation uses the anonymous owner.
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

pub(crate) enum TemporaryDestination {
    CallArgument,
    ConstructedChild {
        production: arena::HandleSpan<checked_trees::FlowOperandChild>,
        expression: typed_trees::expression::ExpressionHandle,
        path: Vec<facts::PlaceSegment>,
    },
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
    pub(crate) destination: TemporaryDestination,
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
    owned_value_type(program, state_symbol, statement_index, argument)
}

fn owned_value_type(
    program: &dyn ::validation::CheckingInput,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    argument: typed_trees::expression::ExpressionHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
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
            let statements = program.statement_table.statements(state.statement_nodes);
            let walk = super::super::borrowed_windows::StorageWalk::new(
                program, machine, state, statements,
            );
            super::super::borrowed_windows::authored_route_exclusive(
                &walk,
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
    // Owned arguments consume storage even when their frontier contains no
    // linear claim. Keep one completed owner and transfer its actual custody;
    // claim-free constructions do not need a separate synthetic event lane.
    (type_carries_linear_obligation(program, type_reference)
        || (anonymous_value
            && validation::reference_result_custody::binding_multiplicity(program, type_reference)
                == language_semantics::Multiplicity::Affine))
        .then_some(type_reference)
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
            let Some(producer) = producer_source(
                program,
                machine,
                state,
                calls,
                consumer.statement_index,
                evaluation,
                expression,
            ) else {
                continue;
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
                destination: TemporaryDestination::CallArgument,
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
    append_constructed_owners(
        program,
        facts,
        machine,
        state,
        calls,
        &completed,
        &mut owners,
    );
    // Cleanup later filters one exact consumer and reverses this shared
    // completion order, without grouping borrowed and projected owners.
    owners.sort_by_key(|owner| owner.completion_ordinal);
    Ok(owners)
}

/// Simultaneous children bind actual anonymous owners to their typed receiving
/// paths. Alternatives are not children: the selection's guarded receipt owns
/// those transfers. Both kinds use the same temporary-owner transfer ledger.
#[allow(clippy::too_many_arguments)]
fn append_constructed_owners(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    calls: &[checked_trees::FlowCallFact],
    completed: &[Vec<(
        typed_trees::expression::ExpressionHandle,
        Vec<crate::checks::ownership_occurrences::EvaluationBranch>,
    )>],
    owners: &mut Vec<TemporaryOwner>,
) {
    use crate::checks::multiplicity::permission_schedule::{PermissionStep, walk};
    use crate::checks::ownership_occurrences::{StatementStep, statement_steps_at_evaluation};
    let retained =
        retained_operand_productions(facts, machine.symbol, state.symbol, calls, completed);
    walk(program, state, calls, (), |step, _| {
        let PermissionStep::Evaluate {
            statement,
            evaluation,
        } = step
        else {
            return;
        };
        let node = &program.statement_table.statements(state.statement_nodes)[statement];
        let occurrences = statement_steps_at_evaluation(
            program,
            machine,
            state,
            statement,
            node,
            &[],
            &facts.operators,
            Some(evaluation),
        );
        for (_, parent) in retained.iter().copied().map(|handle| (handle, facts.flow.control.operands.get(handle))).filter(|(_, operand)| {
            operand.state_symbol == state.symbol
                && operand.statement_index == statement
                && occurrences.iter().any(|occurrence| {
                    matches!(occurrence.step, StatementStep::Complete(expression) if expression == operand.expression)
                })
        }) {
            // Copies of one completed product retain its exact child span;
            // separately evaluated products allocate distinct spans. Invocation
            // evidence and the actual call can both retain that one production.
            let production = parent.constructed_children();
            for child in facts.flow.control.operand_children.span_or_empty(production) {
                if !child.type_reference.is_valid()
                    || type_reference_is_reference(program, child.type_reference)
                {
                    continue;
                }
                let value = facts.flow.control.operands.get(child.value);
                let Some(reference) = owned_value_type(program, state.symbol, statement, value.expression) else {
                    continue;
                };
                let Some(completion_ordinal) = completed.get(statement)
                    .and_then(|values| values.iter().position(|(expression, _)| *expression == value.expression))
                else {
                    continue;
                };
                if !occurrences.iter().any(|occurrence| {
                    matches!(occurrence.step, StatementStep::Complete(expression) if expression == value.expression)
                }) {
                    continue;
                }
                let path = facts.flow.control.operand_child_paths.span_or_empty(child.path);
                if owners.iter().any(|owner| {
                    owner.expression == value.expression && owner.evaluation == evaluation
                        && matches!(&owner.destination, TemporaryDestination::ConstructedChild { production: existing_production, path: existing, .. }
                            if *existing_production == production && existing == path)
                }) {
                    continue;
                }
                let Some(producer) = producer_source(
                    program, machine, state, calls, statement, evaluation, value.expression,
                ) else {
                    continue;
                };
                owners.push(TemporaryOwner {
                    expression: value.expression,
                    type_reference: reference,
                    consumption: TemporaryConsumption::OwnedTransfer,
                    evaluation,
                    producer,
                    consumer: PermissionEventSource::Statement { statement_index: statement },
                    destination: TemporaryDestination::ConstructedChild {
                        production,
                        expression: parent.expression,
                        path: path.to_vec(),
                    },
                    completion_ordinal,
                    completion: typed_trees::expression::ExpressionHandle::invalid(),
                    completion_constraints: None,
                });
            }
        }
    });
}

/// Walk only value graphs retained by this state's actual consumers. In
/// particular a contextual borrow keeps its completed owned production through
/// retention.stored_value; allocated child rows alone are not a parent receipt.
/// Handles deduplicate shared captures without conflating repeated expressions.
fn retained_operand_productions(
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    calls: &[checked_trees::FlowCallFact],
    completed: &[Vec<(
        typed_trees::expression::ExpressionHandle,
        Vec<crate::checks::ownership_occurrences::EvaluationBranch>,
    )>],
) -> Vec<arena::Handle<checked_trees::FlowOperandFact>> {
    let control = &facts.flow.control;
    let Some(scope) = control.state(machine, state) else {
        return Vec::new();
    };
    let mut pending = calls
        .iter()
        .filter(|call| {
            !control.is_retired(state, call)
                && (!call.authored_expression.is_valid()
                    || completed.get(call.statement_index).is_some_and(|rows| {
                        rows.iter()
                            .any(|(expression, _)| *expression == call.authored_expression)
                    }))
        })
        .flat_map(|call| call.operands.handles())
        .collect::<Vec<_>>();
    for statement in control.statements.span_or_empty(scope.statements) {
        pending.extend([statement.stored_value, statement.store_target]);
        pending.extend(
            control
                .value_observations
                .span_or_empty(statement.value_observations)
                .iter()
                .map(|row| row.receiver),
        );
    }
    pending.extend(
        control
            .outgoing
            .span_or_empty(scope.outgoing)
            .iter()
            .map(|row| row.returned_value),
    );
    // Invocation rows are emitted only by the state expression traversal, but
    // their arena also contains declaration contracts and other states. Join
    // each row's selected-use owner to this actual completion schedule first.
    for (_, invocation) in control.operator_invocations.iter() {
        let selected_use = if facts.operators.uses.is_valid(invocation.operator_use) {
            let usage = facts.operators.uses.get(invocation.operator_use);
            Some((usage.origin, usage.expression))
        } else if facts.operators.named_uses.is_valid(invocation.named_use) {
            let usage = facts.operators.named_uses.get(invocation.named_use);
            Some((usage.origin, usage.expression))
        } else {
            None
        };
        let Some((
            checked_trees::CheckedValueOrigin::StateStatement {
                machine_symbol,
                state_symbol,
                statement_index,
                ..
            },
            expression,
        )) = selected_use
        else {
            continue;
        };
        if machine_symbol != machine
            || state_symbol != state
            || !completed
                .get(statement_index)
                .is_some_and(|rows| rows.iter().any(|(completed, _)| *completed == expression))
        {
            continue;
        }
        pending.extend(invocation.operands.handles());
    }
    let mut retained = Vec::new();
    let mut retentions = Vec::new();
    let mut visited_retentions = Vec::new();
    while let Some(handle) = pending.pop() {
        if !control.operands.is_valid(handle) || retained.contains(&handle) {
            continue;
        }
        let operand = control.operands.get(handle);
        if operand.state_symbol != state {
            continue;
        }
        retained.push(handle);
        // Address formation retains the exact evaluated receiver even when
        // the selected scalar/owned child has no reference lifetime leaves.
        // Value and referent queries do not consume this production edge.
        pending.push(operand.backing);
        match operand.completion {
            checked_trees::FlowOperandCompletion::Constructed(children) => pending.extend(
                control
                    .operand_children
                    .span_or_empty(children)
                    .iter()
                    .map(|child| child.value),
            ),
            checked_trees::FlowOperandCompletion::Selected(alternatives) => pending.extend(
                control
                    .operand_alternatives
                    .span_or_empty(alternatives)
                    .iter()
                    .map(|arm| arm.value),
            ),
            checked_trees::FlowOperandCompletion::Copied { copy, .. } => {
                let copy = control.copied_values.get(copy);
                pending.push(copy.source);
                retentions.extend(
                    control
                        .captured_reference_leaves
                        .span_or_empty(copy.references)
                        .iter()
                        .map(|leaf| leaf.retention),
                );
            }
            checked_trees::FlowOperandCompletion::Observation => {}
        }
        retentions.push(operand.retention);
        while let Some(retention) = retentions.pop() {
            if !control.operand_retentions.is_valid(retention)
                || visited_retentions.contains(&retention)
            {
                continue;
            }
            visited_retentions.push(retention);
            let profile = control.operand_retentions.get(retention);
            pending.push(profile.stored_value);
            retentions.extend(
                control
                    .operand_retained_leaves
                    .span_or_empty(profile.leaves)
                    .iter()
                    .map(|leaf| leaf.retention),
            );
        }
    }
    retained
}

#[allow(clippy::too_many_arguments)]
fn producer_source(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    calls: &[checked_trees::FlowCallFact],
    statement: usize,
    evaluation: checked_trees::FlowPermissionEvaluation,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<PermissionEventSource> {
    if program.expression_table.call_node(expression).is_none() {
        return Some(PermissionEventSource::Statement {
            statement_index: statement,
        });
    }
    let mut producers = calls.iter().filter(|producer| {
        producer.statement_index == statement && producer.authored_expression == expression
    });
    let producer = producers.next()?;
    if producers.next().is_some()
        || crate::flow::call_permission_evaluation(
            program,
            machine,
            state,
            statement,
            producer.call_ordinal,
        ) != evaluation
    {
        return None;
    }
    Some(PermissionEventSource::Call {
        statement_index: statement,
        call_ordinal: producer.call_ordinal,
        target_symbol: producer.target_symbol,
    })
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
