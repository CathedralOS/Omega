//! Returned partition invocations and the wrappers composed from them.

use crate::checks::content::content_paths::{
    content_path, content_segments_to_fact_path, projection_observations,
};
use checked_trees::{
    CheckFacts, ContentPartitionCompositionFact, ContentPartitionPlaceSubstitution,
    ContentPartitionResultRewrite,
};
use language_semantics::content::{
    ContentConservationEquation, ContentConservationOwnerKind, ContentConservationPlan,
    ContentConservationTerm, ContentPlaceRoot, ContentPlaceVersion, ContentStructuralPlace,
    conservation_report_fingerprint,
};
use language_semantics::{
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
use typed_trees::statement::{StatementNode, TransitionTargetNode};

/// An executed value-producing call or a selected tail transfer, joined to
/// one exact outgoing occurrence. Local bindings and constructors are already
/// represented by the returned claim paths and need no separate source forms.
#[derive(Debug, Clone)]
pub(crate) struct ReturnedPartitionInvocation {
    machine_symbol: SymbolHandle,
    evaluation: checked_trees::FlowPermissionEvaluation,
    statement_index: usize,
    call_ordinal: usize,
    pub(crate) target_symbol: SymbolHandle,
    returned_at: arena::Handle<checked_trees::FlowStateOutgoingFact>,
    result: InvocationResult,
}

#[derive(Debug, Clone)]
enum InvocationResult {
    Captured {
        expression: ExpressionHandle,
        claims: Vec<crate::checks::multiplicity::ReturnedClaim>,
    },
    TailTransfer,
}

#[derive(Debug, Default)]
struct PartitionCompositionEvidence {
    input_claim_identities: Vec<PermissionClaimIdentity>,
    input_claim_bindings: Vec<checked_trees::ContentPartitionInputClaimBinding>,
    result_rewrites: Vec<ContentPartitionResultRewrite>,
    substitutions: Vec<ContentPartitionPlaceSubstitution>,
    observed_entry_projection: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct AvailablePartitionSource {
    pub(crate) plan: ContentConservationPlan,
    pub(crate) derivation_depth: u32,
}

/// The flow owner records ordinary outgoing edges even when no authored
/// ensures clause exists. Keep that complete roster independently from the
/// invocations that happen to have usable theorem evidence.
pub(crate) fn ordinary_returns(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
) -> Vec<arena::Handle<checked_trees::FlowStateOutgoingFact>> {
    facts
        .flow
        .control
        .outgoing
        .iter()
        .filter_map(|(handle, outgoing)| {
            if outgoing.state_symbol != state.symbol {
                return None;
            }
            if outgoing.transition_target.is_valid() {
                let Some(StatementNode::Transition(transition)) = program
                    .statement_table
                    .statement_at(state, outgoing.statement_index)
                else {
                    return None;
                };
                if transition.exit != typed_trees::statement::TransitionExit::Ordinary {
                    return None;
                }
            }
            Some(handle)
        })
        .collect()
}

pub(crate) fn returned_partition_invocations(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
) -> Vec<ReturnedPartitionInvocation> {
    let Some(state_flow) = facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, flow)| (flow.state_symbol == state.symbol).then_some(flow))
    else {
        return Vec::new();
    };
    let Some(machine) = program.machine_by_symbol(state_flow.machine_symbol) else {
        return Vec::new();
    };
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let mut invocations = Vec::new();
    for returned_at in ordinary_returns(program, facts, state) {
        let outgoing = facts.flow.control.outgoing.get(returned_at);
        let claims = crate::checks::multiplicity::current_returned_claims(program, facts, outgoing);
        for call in facts.flow.control.calls.span_or_empty(state_flow.calls) {
            let (Some(call_position), Some(return_position)) = (
                order.position(call.statement_index),
                order.position(outgoing.statement_index).or_else(|| {
                    (outgoing.statement_index
                        == program
                            .statement_table
                            .statements(state.statement_nodes)
                            .len())
                    .then_some(order.indices().len())
                }),
            ) else {
                continue;
            };
            if call_position > return_position {
                continue;
            }
            let evaluation = crate::flow::call_permission_evaluation(
                program,
                machine,
                state,
                call.statement_index,
                call.call_ordinal,
            );
            if evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
                continue;
            }
            let Some(site) = validation::find_call_site(
                program,
                state_flow.machine_symbol,
                state.symbol,
                call.statement_index,
                call.call_ordinal,
            ) else {
                continue;
            };
            let result = match site {
                validation::CallSite::Expression { expression, .. } => {
                    // A syntax handle alone is not an invocation identity.
                    // Retain the uniquely bound flow occurrence and its lane.
                    let mut producers = facts
                        .flow
                        .control
                        .calls
                        .span_or_empty(state_flow.calls)
                        .iter()
                        .filter(|producer| {
                            producer.statement_index == call.statement_index
                                && producer.authored_expression == expression
                        });
                    if !producers
                        .next()
                        .is_some_and(|producer| std::ptr::eq(producer, call))
                        || producers.next().is_some()
                    {
                        continue;
                    }
                    let Ok(claims) = &claims else {
                        continue;
                    };
                    InvocationResult::Captured {
                        expression,
                        claims: claims.clone(),
                    }
                }
                validation::CallSite::TransitionNamed { path, .. }
                    if call.statement_index == outgoing.statement_index
                        && outgoing.transition_target.is_valid() =>
                {
                    let TransitionTargetNode::Named { path: selected, .. } = program
                        .statement_table
                        .transition_target(outgoing.transition_target)
                    else {
                        continue;
                    };
                    if !std::ptr::eq(path, selected) {
                        continue;
                    }
                    InvocationResult::TailTransfer
                }
                _ => continue,
            };
            invocations.push(ReturnedPartitionInvocation {
                machine_symbol: machine.symbol,
                evaluation,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target_symbol: call.target_symbol,
                returned_at,
                result,
            });
        }
    }
    invocations
}

pub(crate) fn equation_contains_partition(equation: &ContentConservationEquation) -> bool {
    term_contains_partition(equation.left()) || term_contains_partition(equation.right())
}

fn term_contains_partition(term: &ContentConservationTerm) -> bool {
    match term {
        ContentConservationTerm::Projection { .. } => false,
        ContentConservationTerm::Separate(_) => true,
    }
}

pub(crate) fn instantiate_partition_wrapper(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state: &typed_trees::state::State,
    invocation: &ReturnedPartitionInvocation,
    source: &ContentConservationPlan,
    source_derivation_depth: u32,
) -> Option<ContentPartitionCompositionFact> {
    let target_parameters = validation::call_target_parameters(program, invocation.target_symbol)?;
    let call_ordinal = invocation.call_ordinal;
    let mut evidence = PartitionCompositionEvidence::default();
    let left = instantiate_partition_term(
        program,
        facts,
        state,
        invocation,
        target_parameters,
        source.equation.left(),
        &mut evidence,
    )?;
    let right = instantiate_partition_term(
        program,
        facts,
        state,
        invocation,
        target_parameters,
        source.equation.right(),
        &mut evidence,
    )?;
    if !evidence.observed_entry_projection {
        return None;
    }
    evidence.input_claim_bindings.sort_by_key(|binding| {
        (
            format!("{:?}", binding.claim_identity),
            format!("{:?}", binding.entry_place),
        )
    });
    evidence.input_claim_bindings.dedup();
    if evidence
        .input_claim_bindings
        .iter()
        .enumerate()
        .any(|(index, binding)| {
            evidence.input_claim_bindings[index + 1..]
                .iter()
                .any(|later| {
                    later.claim_identity == binding.claim_identity
                        && later.entry_place != binding.entry_place
                })
        })
    {
        return None;
    }
    evidence.input_claim_identities = evidence
        .input_claim_bindings
        .iter()
        .map(|binding| binding.claim_identity)
        .collect();
    evidence.input_claim_identities.dedup();
    evidence.result_rewrites.sort_by_key(|rewrite| {
        (
            format!("{:?}", rewrite.source),
            format!("{:?}", rewrite.target),
            format!("{:?}", rewrite.claim_identity),
        )
    });
    evidence.result_rewrites.dedup();
    evidence.substitutions.sort_by_key(|substitution| {
        (
            format!("{:?}", substitution.source),
            format!("{:?}", substitution.target),
        )
    });
    evidence.substitutions.dedup();
    let equation = ContentConservationEquation::new(left, right);
    let report_fingerprint = conservation_report_fingerprint(&source.algebra, &equation);
    let plan = ContentConservationPlan {
        owner_kind: ContentConservationOwnerKind::Machine,
        owner: machine_symbol,
        callable: state.symbol,
        algebra: source.algebra.clone(),
        equation,
        report_fingerprint,
    };
    Some(ContentPartitionCompositionFact {
        machine_symbol,
        state_symbol: state.symbol,
        source_callable: source.callable,
        source_report_fingerprint: source.report_fingerprint,
        source_derivation_depth,
        source_plan: source.clone(),
        statement_index: invocation.statement_index,
        call_ordinal,
        returned_at: invocation.returned_at,
        input_claim_identities: evidence.input_claim_identities,
        input_claim_bindings: evidence.input_claim_bindings,
        result_rewrites: evidence.result_rewrites,
        substitutions: evidence.substitutions,
        plan,
    })
}

#[allow(clippy::too_many_arguments)]
fn instantiate_partition_term(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller_state: &typed_trees::state::State,
    invocation: &ReturnedPartitionInvocation,
    target_parameters: &[StateParameter],
    term: &ContentConservationTerm,
    evidence: &mut PartitionCompositionEvidence,
) -> Option<ContentConservationTerm> {
    match term {
        ContentConservationTerm::Projection {
            domain,
            semantic_domain,
            projection_machine,
            projection_report_fingerprint,
            subject,
        } => {
            let projection = facts
                .qualifications
                .content
                .plans
                .iter()
                .find(|projection| {
                    projection.domain == *domain
                        && projection.semantic_domain == *semantic_domain
                        && projection.machine == *projection_machine
                })?;
            let observations = projection_observations(&projection.expression)?;
            Some(ContentConservationTerm::Projection {
                domain: *domain,
                semantic_domain: *semantic_domain,
                projection_machine: *projection_machine,
                projection_report_fingerprint: *projection_report_fingerprint,
                subject: instantiate_partition_subject(
                    program,
                    facts,
                    caller_state,
                    invocation,
                    target_parameters,
                    subject,
                    &observations,
                    evidence,
                )?,
            })
        }
        ContentConservationTerm::Separate(children) => Some(ContentConservationTerm::separate(
            children
                .iter()
                .map(|child| {
                    instantiate_partition_term(
                        program,
                        facts,
                        caller_state,
                        invocation,
                        target_parameters,
                        child,
                        evidence,
                    )
                })
                .collect::<Option<Vec<_>>>()?,
        )),
    }
}

/// Whether `event` belongs to `caller_state` and was emitted at this
/// invocation site (the call edge itself or its containing statement).
fn permission_event_at_invocation(
    event: &checked_trees::FlowPermissionEventFact,
    caller_state_symbol: SymbolHandle,
    invocation: &ReturnedPartitionInvocation,
    call_ordinal: usize,
) -> bool {
    let source_matches = matches!(
        event.source,
        PermissionEventSource::Call {
            statement_index,
            call_ordinal: event_call_ordinal,
            target_symbol,
            ..
        } if statement_index == invocation.statement_index
            && event_call_ordinal == call_ordinal
            && target_symbol == invocation.target_symbol
    ) || event.source
        == PermissionEventSource::Statement {
            statement_index: invocation.statement_index,
        };
    event.machine_symbol == invocation.machine_symbol
        && event.state_symbol == caller_state_symbol
        && event.evaluation == invocation.evaluation
        && source_matches
}

#[allow(clippy::too_many_arguments)]
fn instantiate_partition_subject(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller_state: &typed_trees::state::State,
    invocation: &ReturnedPartitionInvocation,
    target_parameters: &[StateParameter],
    subject: &ContentStructuralPlace,
    observations: &[Vec<facts::PlaceSegment>],
    evidence: &mut PartitionCompositionEvidence,
) -> Option<ContentStructuralPlace> {
    let target = match (&subject.root, subject.version) {
        (ContentPlaceRoot::Result, ContentPlaceVersion::Current) => {
            instantiate_partition_result_subject(
                program,
                facts,
                caller_state,
                invocation,
                subject,
                observations,
                evidence,
            )?
        }
        (
            ContentPlaceRoot::Parameter {
                position, symbol, ..
            },
            ContentPlaceVersion::Entry,
        ) => {
            evidence.observed_entry_projection = true;
            let parameter = target_parameters
                .get(usize::try_from(*position).ok()?)
                .filter(|parameter| !symbol.is_valid() || parameter.symbol == *symbol)
                .or_else(|| {
                    symbol
                        .is_valid()
                        .then(|| {
                            target_parameters
                                .iter()
                                .find(|parameter| parameter.symbol == *symbol)
                        })
                        .flatten()
                })?;
            let (claim_identity, entry_place) = partition_entry_binding(
                program,
                facts,
                caller_state,
                invocation,
                target_parameters,
                parameter,
                subject,
                observations,
            )?;
            evidence
                .input_claim_bindings
                .push(checked_trees::ContentPartitionInputClaimBinding {
                    claim_identity,
                    entry_place: entry_place.clone(),
                });
            entry_place
        }
        _ => return None,
    };
    if let Some(previous) = evidence
        .substitutions
        .iter()
        .find(|substitution| substitution.source == *subject)
    {
        if previous.target != target {
            return None;
        }
    } else {
        evidence
            .substitutions
            .push(ContentPartitionPlaceSubstitution {
                source: subject.clone(),
                target: target.clone(),
            });
    }
    Some(target)
}

/// Bind the actual captured value to its entry observation. The capture owner
/// traces detached stores and constructions; the origin owner then proves that
/// the reaching observation still has its entry contents. The permission join
/// independently requires the same claim at this exact consuming occurrence.
fn partition_entry_binding(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    invocation: &ReturnedPartitionInvocation,
    parameters: &[StateParameter],
    parameter: &StateParameter,
    subject: &ContentStructuralPlace,
    observations: &[Vec<facts::PlaceSegment>],
) -> Option<(PermissionClaimIdentity, ContentStructuralPlace)> {
    let state_flow = facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, flow)| (flow.state_symbol == state.symbol).then_some(flow))?;
    let machine = program.machine_by_symbol(state_flow.machine_symbol)?;
    let call = facts
        .flow
        .control
        .calls
        .span_or_empty(state_flow.calls)
        .iter()
        .find(|call| {
            call.statement_index == invocation.statement_index
                && call.call_ordinal == invocation.call_ordinal
                && call.target_symbol == invocation.target_symbol
        })?;
    let position = typed_trees::signature::state_parameter_position(parameters, parameter.symbol)?;
    let operand = facts
        .flow
        .control
        .operands
        .span_or_empty(call.argument_operands())
        .get(position)?;
    let path = content_segments_to_fact_path(&subject.segments)?;
    let values = crate::flow::captured_operand_values(
        program,
        &facts.flow,
        &facts.semantic,
        state_flow,
        operand,
        parameter.type_reference,
        &path,
    )?;
    let mut binding = None;
    for value in &values {
        let actuals = crate::flow::captured_observation_places(program, &facts.flow, value)?;
        if actuals.is_empty() {
            return None;
        }
        for actual in actuals {
            let mut transfers = facts
                .flow
                .ownership
                .permissions
                .iter()
                .filter(|(_, event)| {
                    permission_event_at_invocation(
                        event,
                        state.symbol,
                        invocation,
                        invocation.call_ordinal,
                    ) && event.expression == value.observation.expression
                        && event.kind == PermissionEventKind::Transfer
                        && event.access == PermissionAccess::Owned
                        && event.obligation_live
                        && event.claim_identity != PermissionClaimIdentity::Unknown
                        && event.root == actual.projected.root
                        && facts.flow.ownership.segments.span_or_empty(event.segments)
                            == actual.projected.segments
                })
                .map(|(_, event)| event);
            let transfer = transfers.next()?;
            if transfers.next().is_some() {
                return None;
            }
            let identity = transfer.claim_identity;
            let mut entries = facts
                .flow
                .ownership
                .permissions
                .iter()
                .filter(|(_, event)| {
                    event.machine_symbol == state_flow.machine_symbol
                        && event.state_symbol == state.symbol
                        && event.source == PermissionEventSource::StateEntry
                        && event.kind == PermissionEventKind::Establish
                        && event.access == PermissionAccess::Owned
                        && event.obligation_live
                        && event.claim_identity == identity
                })
                .map(|(_, event)| event);
            let entry = entries.next()?;
            if entries.next().is_some() {
                return None;
            }
            let root = entry.root.as_symbol()?;
            let (position, entry_parameter) = program
                .state_parameters(state)
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.symbol == root)?;
            let entry_path = facts.flow.ownership.segments.span_or_empty(entry.segments);
            for observation in observations {
                let mut selected = path.clone();
                selected.extend_from_slice(observation);
                let captured = crate::flow::captured_operand_values(
                    program,
                    &facts.flow,
                    &facts.semantic,
                    state_flow,
                    operand,
                    parameter.type_reference,
                    &selected,
                )?;
                if captured.is_empty() {
                    return None;
                }
                let mut expected = validation::CanonicalPlace {
                    root: entry.root,
                    segments: entry_path.to_vec(),
                };
                expected.segments.extend_from_slice(observation);
                for captured in &captured {
                    let producers =
                        crate::flow::captured_value_places(program, &facts.flow, captured)?;
                    if producers.is_empty() {
                        return None;
                    }
                    for producer in producers {
                        if !crate::flow::operand_entry_origin_is_stable(
                            program,
                            &facts.flow,
                            captured.operand,
                            &producer.projected,
                        ) {
                            return None;
                        }
                        let origin = crate::flow::value_origin_before_statement(
                            program,
                            &facts.flow,
                            machine,
                            state_flow,
                            captured.operand.statement_index,
                            producer.projected,
                            None,
                            crate::flow::ValueObservation::Contents,
                        )?;
                        if origin != expected {
                            return None;
                        }
                    }
                }
            }
            let candidate = (
                identity,
                ContentStructuralPlace {
                    version: ContentPlaceVersion::Entry,
                    root: ContentPlaceRoot::Parameter {
                        position: u32::try_from(position).ok()?,
                        symbol: entry_parameter.symbol,
                        name: entry_parameter.name.as_str().to_owned(),
                    },
                    segments: content_path(program, entry_path)?,
                },
            );
            if binding
                .as_ref()
                .is_some_and(|previous| *previous != candidate)
            {
                return None;
            }
            binding = Some(candidate);
        }
    }
    binding
}

fn instantiate_partition_result_subject(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller_state: &typed_trees::state::State,
    invocation: &ReturnedPartitionInvocation,
    subject: &ContentStructuralPlace,
    observations: &[Vec<facts::PlaceSegment>],
    evidence: &mut PartitionCompositionEvidence,
) -> Option<ContentStructuralPlace> {
    let InvocationResult::Captured { expression, claims } = &invocation.result else {
        // This exact outgoing edge is the selected tail invocation. Its result
        // is the caller's result; no intermediate caller-owned value exists.
        return Some(subject.clone());
    };
    let source_path = content_segments_to_fact_path(&subject.segments)?;
    let mut identities = Vec::new();
    for (_, event) in facts.flow.ownership.permissions.iter() {
        if !permission_event_at_invocation(
            event,
            caller_state.symbol,
            invocation,
            invocation.call_ordinal,
        ) || event.kind != PermissionEventKind::Establish
            || event.access != PermissionAccess::Owned
            || !event.obligation_live
            || event.claim_identity == PermissionClaimIdentity::Unknown
        {
            continue;
        }
        let Some((statement, produced, path)) =
            validation::receiving_value(program, &facts.flow.ownership.segments, event)
        else {
            continue;
        };
        if statement == invocation.statement_index
            && produced == *expression
            && path == source_path
            && !identities.contains(&event.claim_identity)
        {
            identities.push(event.claim_identity);
        }
    }
    let [claim_identity] = identities.as_slice() else {
        return None;
    };
    let mut outputs = claims.iter().filter(|claim| {
        claim.source == crate::checks::multiplicity::ReturnedClaimSource::Live(*claim_identity)
    });
    let returned = outputs.next()?;
    if outputs.next().is_some() {
        return None;
    }
    // Current custody and unchanged content are separate obligations. The
    // retained capture trace follows stores through every receiving home and
    // stops at writes; a claim surviving a field mutation cannot donate its
    // producer's content theorem to this exit.
    let outgoing = facts.flow.control.outgoing.get(invocation.returned_at);
    let state_flow = facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, flow)| (flow.state_symbol == caller_state.symbol).then_some(flow))?;
    let operand = facts.flow.control.operands.get(outgoing.returned_value);
    for observation in observations {
        let mut selected = returned.path.clone();
        selected.extend_from_slice(observation);
        let values = crate::flow::captured_operand_values(
            program,
            &facts.flow,
            &facts.semantic,
            state_flow,
            operand,
            operand.type_reference,
            &selected,
        )?;
        let mut expected = validation::canonical_place_from_expression_in_state(
            program,
            caller_state.symbol,
            invocation.statement_index,
            *expression,
        )?;
        expected.segments.extend_from_slice(&source_path);
        expected.segments.extend_from_slice(observation);
        if values.is_empty()
            || !values.iter().all(|value| {
                crate::flow::captured_value_places(program, &facts.flow, value).is_some_and(
                    |places| {
                        !places.is_empty() && places.iter().all(|place| place.projected == expected)
                    },
                )
            })
        {
            return None;
        }
    }
    let target = ContentStructuralPlace {
        version: ContentPlaceVersion::Current,
        root: ContentPlaceRoot::Result,
        segments: content_path(program, &returned.path)?,
    };
    evidence
        .result_rewrites
        .push(ContentPartitionResultRewrite {
            claim_identity: *claim_identity,
            source: subject.clone(),
            target: target.clone(),
        });
    Some(target)
}
