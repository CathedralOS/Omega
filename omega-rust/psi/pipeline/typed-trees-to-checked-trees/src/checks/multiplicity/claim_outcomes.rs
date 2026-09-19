//! Deriving claim outcomes: which claim each state result, named
//! transition and call argument settles, and the rewrites that follow call
//! result origins.

mod joins;
pub(crate) use joins::{publish_conditional_claim_joins, validate_conditional_claim_joins};

use crate::checks::multiplicity::linear_obligations::{
    CheckedClaimOutcomeEntry, CheckedClaimOutcomeMap, CheckedClaimOutcomeSource,
};
use crate::checks::multiplicity::linear_validation::{
    established_provenance, linear_claim_frontier, permission_event_statement_index,
};
use crate::checks::multiplicity::type_multiplicity::{data_field_name, literal_variant};
use arena::HandleSpan;
use checked_trees::{
    CheckFacts, FlowClaimOutcomeEntryFact, FlowClaimOutcomeMapFact, FlowClaimOutcomeSource,
    FlowPermissionEventFact,
};
use language_semantics::{
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    PermissionProvenance,
};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;

pub(crate) fn derive_checked_claim_outcome_maps(
    program: &typed_trees::TypedTrees,
    ownership: &checked_trees::FlowOwnershipFacts,
    permission_events: &[FlowPermissionEventFact],
) -> Vec<CheckedClaimOutcomeMap> {
    let state_count = program
        .machines()
        .iter()
        .map(|machine| program.machine_states(machine).len())
        .sum::<usize>();
    let mut maps = Vec::new();
    for _ in 0..=state_count {
        let next = program
            .machines()
            .iter()
            .flat_map(|machine| {
                program.machine_states(machine).iter().filter_map(|state| {
                    derive_checked_claim_outcome_map(
                        program,
                        machine.symbol,
                        state,
                        ownership,
                        permission_events,
                        &maps,
                    )
                })
            })
            .collect::<Vec<_>>();
        if next == maps {
            return maps;
        }
        maps = next;
    }
    maps
}

#[allow(clippy::too_many_arguments)]
fn derive_checked_claim_outcome_map(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state: &typed_trees::state::State,
    ownership: &checked_trees::FlowOwnershipFacts,
    permission_events: &[FlowPermissionEventFact],
    known_maps: &[CheckedClaimOutcomeMap],
) -> Option<CheckedClaimOutcomeMap> {
    let segments = &ownership.segments;
    let expected_paths = linear_claim_frontier(program, state.return_type)
        .into_iter()
        .map(|claim| claim.path)
        .collect::<Vec<_>>();
    if expected_paths.is_empty() {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    // Owned receivers are terminal-consumer inputs and do not enter the
    // ordinary parameter permission ledger. The exact identity body still
    // supplies an input/result correspondence: it performs no intervening
    // operation that could replace or consume the receiver. Do not infer this
    // from a call's single argument or from its result carrier.
    if let [StatementNode::Expression(expression)] = statements
        && let Some(place) = crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            0,
            *expression,
        )
        && place.segments.is_empty()
        && let facts::PlaceRoot::Symbol(parameter_symbol) = place.root
        && program.state_parameters(state).iter().any(|parameter| {
            parameter.is_self
                && parameter.symbol == parameter_symbol
                && (program.normalized_type_identity(parameter.type_reference)
                    == program.normalized_type_identity(state.return_type)
                    || program.machines().iter().any(|machine| {
                        machine.symbol == machine_symbol
                            && machine.attached_data_symbol.is_valid()
                            && matches!(program.type_reference_table.type_reference(parameter.type_reference),
                                typed_trees::types::TypeReferenceNode::Named { symbol, .. } if *symbol == machine_symbol)
                            && matches!(program.type_reference_table.type_reference(state.return_type),
                                typed_trees::types::TypeReferenceNode::Named { symbol, .. } if *symbol == machine.attached_data_symbol)
                    }))
        })
    {
        return Some(CheckedClaimOutcomeMap {
            machine_symbol,
            state_symbol: state.symbol,
            entries: expected_paths
                .into_iter()
                .map(|path| CheckedClaimOutcomeEntry {
                    output_path: path.clone(),
                    source: CheckedClaimOutcomeSource::Input {
                        parameter_symbol,
                        path,
                    },
                })
                .collect(),
        });
    }
    let result_expressions = state_result_expressions(program, statements);
    let named_transitions = state_result_named_transitions(program, statements);
    let mut entries = result_expressions
        .iter()
        .copied()
        .flat_map(|(statement_index, expression)| {
            claim_outcomes_for_expression(
                program,
                state,
                statement_index,
                expression,
                ownership,
                permission_events,
                known_maps,
                &[],
            )
        })
        .chain(named_transitions.iter().copied().flat_map(
            |(statement_index, target_symbol, arguments)| {
                claim_outcomes_for_named_transition(
                    program,
                    state,
                    statement_index,
                    target_symbol,
                    arguments,
                    segments,
                    permission_events,
                    known_maps,
                )
            },
        ))
        .fold(Vec::new(), |mut entries, entry| {
            if !entries.contains(&entry) {
                entries.push(entry);
            }
            entries
        });
    entries.retain(|entry| expected_paths.contains(&entry.output_path));
    for path in &expected_paths {
        let sources = entries
            .iter()
            .filter(|entry| entry.output_path == *path)
            .map(|entry| &entry.source)
            .fold(Vec::new(), |mut sources, source| {
                if !sources.contains(&source) {
                    sources.push(source);
                }
                sources
            });
        if sources.len() != 1 {
            if sources.is_empty()
                && claim_path_is_statically_inactive(
                    program,
                    path,
                    &result_expressions,
                    &named_transitions,
                    known_maps,
                )
            {
                continue;
            }
            return None;
        }
    }
    entries.retain(|entry| {
        expected_paths
            .iter()
            .filter(|path| **path == entry.output_path)
            .count()
            == 1
    });
    entries.sort_by_key(|entry| {
        expected_paths
            .iter()
            .position(|path| *path == entry.output_path)
            .unwrap_or(usize::MAX)
    });
    for (index, entry) in entries.iter().enumerate() {
        if entries[index + 1..].iter().any(|candidate| {
            candidate.source == entry.source
                && !claim_paths_are_case_alternatives(&entry.output_path, &candidate.output_path)
        }) {
            return None;
        }
    }
    Some(CheckedClaimOutcomeMap {
        machine_symbol,
        state_symbol: state.symbol,
        entries,
    })
}

pub(crate) fn claim_paths_are_case_alternatives(
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    left.iter()
        .zip(right)
        .find_map(|(left, right)| {
            if left == right {
                return None;
            }
            Some(matches!(
                (left, right),
                (
                    facts::PlaceSegment::Case {
                        variant: left_variant
                    },
                    facts::PlaceSegment::Case {
                        variant: right_variant
                    }
                ) if left_variant != right_variant
            ))
        })
        .unwrap_or(false)
}

fn claim_path_is_statically_inactive(
    program: &typed_trees::TypedTrees,
    path: &[facts::PlaceSegment],
    result_expressions: &[(usize, typed_trees::expression::ExpressionHandle)],
    named_transitions: &[(
        usize,
        SymbolHandle,
        HandleSpan<typed_trees::expression::ExpressionHandle>,
    )],
    known_maps: &[CheckedClaimOutcomeMap],
) -> bool {
    if result_expressions.is_empty() && named_transitions.is_empty() {
        return false;
    }
    result_expressions.iter().all(|(_, expression)| {
        expression_statically_excludes_claim_path(program, *expression, path, known_maps)
    }) && named_transitions.iter().all(|(_, target_symbol, _)| {
        crate::semantic_calls::find_state(program, *target_symbol)
            .and_then(|target| {
                known_maps
                    .iter()
                    .find(|map| map.state_symbol == target.symbol)
            })
            .is_some_and(|map| !map.entries.iter().any(|entry| entry.output_path == path))
    })
}

fn expression_statically_excludes_claim_path(
    program: &typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    path: &[facts::PlaceSegment],
    known_maps: &[CheckedClaimOutcomeMap],
) -> bool {
    match program.expression_table.expression(expression) {
        typed_trees::expression::ExpressionNode::StructLiteral(literal) => {
            let mut remaining = path;
            if literal.case_name.is_some() {
                let Some(facts::PlaceSegment::Case { variant }) = remaining.first() else {
                    return false;
                };
                if literal_variant(program, literal).map(|candidate| candidate.symbol)
                    != Some(*variant)
                {
                    return true;
                }
                remaining = &remaining[1..];
            }
            let Some(facts::PlaceSegment::Field { symbol }) = remaining.first() else {
                return false;
            };
            let Some(field_name) = data_field_name(program, *symbol) else {
                return false;
            };
            program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .find(|field| field.name.as_str() == field_name)
                .is_some_and(|field| {
                    expression_statically_excludes_claim_path(
                        program,
                        field.value,
                        &remaining[1..],
                        known_maps,
                    )
                })
        }
        typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            let Some(facts::PlaceSegment::FixedIndex { index }) = path.first() else {
                return false;
            };
            program
                .expression_table
                .expression_handles(*values)
                .get(*index)
                .is_some_and(|value| {
                    expression_statically_excludes_claim_path(
                        program,
                        *value,
                        &path[1..],
                        known_maps,
                    )
                })
        }
        typed_trees::expression::ExpressionNode::Call(call) => {
            crate::semantic_calls::find_state(program, call.target_symbol)
                .and_then(|target| {
                    known_maps
                        .iter()
                        .find(|map| map.state_symbol == target.symbol)
                })
                .is_some_and(|map| !map.entries.iter().any(|entry| entry.output_path == path))
        }
        _ => false,
    }
}

fn state_result_expressions(
    program: &typed_trees::TypedTrees,
    statements: &[StatementNode],
) -> Vec<(usize, typed_trees::expression::ExpressionHandle)> {
    statements
        .iter()
        .enumerate()
        .flat_map(|(statement_index, statement)| match statement {
            StatementNode::Expression(expression) if statement_index + 1 == statements.len() => {
                vec![(statement_index, *expression)]
            }
            StatementNode::Transition(transition) => [transition.target, transition.continuation]
                .into_iter()
                .filter(|handle| handle.is_valid())
                .filter_map(|handle| {
                    let typed_trees::statement::TransitionTargetNode::Value(expression) =
                        program.statement_table.transition_target(handle)
                    else {
                        return None;
                    };
                    Some((statement_index, *expression))
                })
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

fn state_result_named_transitions(
    program: &typed_trees::TypedTrees,
    statements: &[StatementNode],
) -> Vec<(
    usize,
    SymbolHandle,
    HandleSpan<typed_trees::expression::ExpressionHandle>,
)> {
    statements
        .iter()
        .enumerate()
        .flat_map(|(statement_index, statement)| {
            let StatementNode::Transition(transition) = statement else {
                return Vec::new();
            };
            [transition.target, transition.continuation]
                .into_iter()
                .filter(|handle| handle.is_valid())
                .filter_map(|handle| {
                    let typed_trees::statement::TransitionTargetNode::Named {
                        path, arguments, ..
                    } = program.statement_table.transition_target(handle)
                    else {
                        return None;
                    };
                    Some((statement_index, path.symbol, *arguments))
                })
                .collect()
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn claim_outcomes_for_named_transition(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    target_symbol: SymbolHandle,
    arguments: HandleSpan<typed_trees::expression::ExpressionHandle>,
    segments: &arena::Arena<facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
    known_maps: &[CheckedClaimOutcomeMap],
) -> Vec<CheckedClaimOutcomeEntry> {
    let Some(target_state) = crate::semantic_calls::find_state(program, target_symbol) else {
        return Vec::new();
    };
    let Some(target_map) = known_maps
        .iter()
        .find(|map| map.state_symbol == target_state.symbol)
    else {
        return Vec::new();
    };
    let arguments = program.statement_table.expression_handles(arguments);
    target_map
        .entries
        .iter()
        .filter_map(|entry| {
            let source = bind_claim_outcome_source_at_arguments(
                program,
                state,
                statement_index,
                arguments,
                None,
                target_state,
                &entry.source,
                segments,
                permission_events,
            )?;
            Some(CheckedClaimOutcomeEntry {
                output_path: entry.output_path.clone(),
                source,
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn claim_outcomes_for_expression(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    ownership: &checked_trees::FlowOwnershipFacts,
    permission_events: &[FlowPermissionEventFact],
    known_maps: &[CheckedClaimOutcomeMap],
    output_prefix: &[facts::PlaceSegment],
) -> Vec<CheckedClaimOutcomeEntry> {
    let segments = &ownership.segments;
    if let Some(place) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    ) {
        let found = permission_events
            .iter()
            .filter(|event| {
                event.state_symbol == state.symbol
                    && permission_event_statement_index(event.source) == Some(statement_index)
                    && event.kind == PermissionEventKind::Transfer
                    && event.access == PermissionAccess::Owned
                    && event.obligation_live
                    && event.root == place.root
            })
            .filter_map(|event| {
                let event_path = segments.span_or_empty(event.segments);
                if !event_path.starts_with(place.segments.as_slice()) {
                    return None;
                }
                let mut output_path = output_prefix.to_vec();
                output_path.extend_from_slice(&event_path[place.segments.len()..]);
                Some(CheckedClaimOutcomeEntry {
                    output_path,
                    source: claim_outcome_source_for_event(
                        program,
                        state,
                        event,
                        segments,
                        permission_events,
                    ),
                })
            })
            .collect::<Vec<_>>();
        if !found.is_empty() {
            return found;
        }
    }

    match program.expression_table.expression(expression) {
        typed_trees::expression::ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .enumerate()
            .flat_map(|(index, value)| {
                let mut element_prefix = output_prefix.to_vec();
                element_prefix.push(facts::PlaceSegment::FixedIndex { index });
                claim_outcomes_for_expression(
                    program,
                    state,
                    statement_index,
                    *value,
                    ownership,
                    permission_events,
                    known_maps,
                    &element_prefix,
                )
            })
            .collect(),
        typed_trees::expression::ExpressionNode::StructLiteral(literal)
            if literal.case_name.is_some() =>
        {
            let Some(variant) = literal_variant(program, literal) else {
                return Vec::new();
            };
            let mut case_prefix = output_prefix.to_vec();
            case_prefix.push(facts::PlaceSegment::Case {
                variant: variant.symbol,
            });
            program
                .data_payload_fields(variant)
                .iter()
                .filter_map(|field| {
                    let value = program
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .find(|literal_field| literal_field.name == field.name)?
                        .value;
                    let mut field_prefix = case_prefix.clone();
                    field_prefix.push(facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                    Some(claim_outcomes_for_expression(
                        program,
                        state,
                        statement_index,
                        value,
                        ownership,
                        permission_events,
                        known_maps,
                        &field_prefix,
                    ))
                })
                .flatten()
                .collect()
        }
        typed_trees::expression::ExpressionNode::StructLiteral(literal)
            if literal.case_name.is_none() =>
        {
            let Some(definition) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == literal.type_name.as_str())
            else {
                return Vec::new();
            };
            program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .flat_map(|literal_field| {
                    let Some(field) =
                        program
                            .data_members(definition)
                            .iter()
                            .find_map(|member| match member {
                                typed_trees::data::DataMember::Field(field)
                                    if field.name.as_str() == literal_field.name.as_str() =>
                                {
                                    Some(field)
                                }
                                _ => None,
                            })
                    else {
                        return Vec::new();
                    };
                    let mut field_prefix = output_prefix.to_vec();
                    field_prefix.push(facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                    claim_outcomes_for_expression(
                        program,
                        state,
                        statement_index,
                        literal_field.value,
                        ownership,
                        permission_events,
                        known_maps,
                        &field_prefix,
                    )
                })
                .collect()
        }
        typed_trees::expression::ExpressionNode::Call(call) => {
            let Some(target_state) = crate::semantic_calls::find_state(program, call.target_symbol)
            else {
                return Vec::new();
            };
            let Some(target_map) = known_maps
                .iter()
                .find(|map| map.state_symbol == target_state.symbol)
            else {
                return Vec::new();
            };
            target_map
                .entries
                .iter()
                .filter_map(|entry| {
                    let source = bind_claim_outcome_source_at_call(
                        program,
                        state,
                        statement_index,
                        call,
                        target_state,
                        &entry.source,
                        segments,
                        permission_events,
                    )?;
                    let mut output_path = output_prefix.to_vec();
                    output_path.extend_from_slice(&entry.output_path);
                    Some(CheckedClaimOutcomeEntry {
                        output_path,
                        source,
                    })
                })
                .collect()
        }
        // A receipt'd selection joins one result claim at the destination;
        // the receipt's transfers name the consumed source claim per edge
        // rather than a permission event.
        typed_trees::expression::ExpressionNode::Match(_) => claim_outcomes_for_owned_selection(
            program,
            state,
            statement_index,
            expression,
            ownership,
            output_prefix,
        ),
        _ => Vec::new(),
    }
}

/// A receipt'd selection produces its result claims at the join edge: each
/// transfer names the consumed source claims that become the result's claims
/// on that edge, and uniform linear consumption means every edge maps the
/// result to the same source places. A whole affine carrier contributes one
/// entry per frontier claim, landing at the claim's position below the moved
/// leaf. A parameter source binds the caller's claim at its exact input path;
/// a local source publishes the established identity the roster recorded. A
/// fresh per-edge product establishes its claim inside the selection, so its
/// origin is intentionally untracked.
fn claim_outcomes_for_owned_selection(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    ownership: &checked_trees::FlowOwnershipFacts,
    output_prefix: &[facts::PlaceSegment],
) -> Vec<CheckedClaimOutcomeEntry> {
    let Some((_, receipt)) = ownership.owned_selection_at(state.symbol, statement_index as u32)
    else {
        return Vec::new();
    };
    if receipt.expression != expression {
        return Vec::new();
    }
    let mut entries = Vec::new();
    for transfer in ownership
        .selection_transfers
        .span_or_empty(receipt.transfers)
    {
        if !transfer.source.is_valid() {
            continue;
        }
        let source = ownership.selection_sources.get(transfer.source);
        let leaf_path = ownership.segments.span_or_empty(transfer.path);
        let is_parameter = program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.symbol == source.symbol);
        for claim in ownership
            .selection_transfer_claims
            .span_or_empty(transfer.claims)
        {
            // The claim's consumed path is the leaf's moved path plus the
            // claim's position below it; stripping the leaf prefix yields the
            // result-relative path the claim lands at.
            let claim_path = ownership.segments.span_or_empty(claim.path);
            let suffix = claim_path.strip_prefix(leaf_path).unwrap_or(claim_path);
            let mut output_path = output_prefix.to_vec();
            output_path.extend_from_slice(suffix);
            let source = if is_parameter {
                CheckedClaimOutcomeSource::Input {
                    parameter_symbol: source.symbol,
                    path: claim_path.to_vec(),
                }
            } else {
                CheckedClaimOutcomeSource::Established {
                    claim_identity: claim.claim_identity,
                    provenance: claim.provenance,
                }
            };
            let entry = CheckedClaimOutcomeEntry {
                output_path,
                source,
            };
            if !entries.contains(&entry) {
                entries.push(entry);
            }
        }
    }
    if entries.is_empty() {
        entries.push(CheckedClaimOutcomeEntry {
            output_path: output_prefix.to_vec(),
            source: CheckedClaimOutcomeSource::Established {
                claim_identity: PermissionClaimIdentity::Unknown,
                provenance: PermissionProvenance::Unknown,
            },
        });
    }
    entries
}

fn claim_outcome_source_for_event(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    event: &FlowPermissionEventFact,
    segments: &arena::Arena<facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
) -> CheckedClaimOutcomeSource {
    if let Some(entry) = permission_events.iter().find(|candidate| {
        candidate.state_symbol == state.symbol
            && candidate.source == PermissionEventSource::StateEntry
            && candidate.kind == PermissionEventKind::Establish
            && candidate.access == PermissionAccess::Owned
            && candidate.claim_identity == event.claim_identity
            && candidate.provenance == event.provenance
    }) && let facts::PlaceRoot::Symbol(parameter_symbol) = entry.root
        && program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.symbol == parameter_symbol)
    {
        return CheckedClaimOutcomeSource::Input {
            parameter_symbol,
            path: segments.span_or_empty(entry.segments).to_vec(),
        };
    }
    CheckedClaimOutcomeSource::Established {
        claim_identity: event.claim_identity,
        provenance: event.provenance,
    }
}

fn claim_origin_for_source(
    state: &typed_trees::state::State,
    source: &CheckedClaimOutcomeSource,
    segments: &arena::Arena<facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
) -> Option<(PermissionProvenance, PermissionClaimIdentity)> {
    match source {
        CheckedClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        } => Some((*provenance, *claim_identity)),
        CheckedClaimOutcomeSource::Input {
            parameter_symbol,
            path,
        } => {
            let origins = permission_events
                .iter()
                .filter(|event| {
                    event.state_symbol == state.symbol
                        && event.source == PermissionEventSource::StateEntry
                        && event.kind == PermissionEventKind::Establish
                        && event.access == PermissionAccess::Owned
                        && event.obligation_live
                        && event.root == facts::PlaceRoot::Symbol(*parameter_symbol)
                        && segments.span_or_empty(event.segments) == path
                        && event.claim_identity != PermissionClaimIdentity::Unknown
                        && event.provenance != PermissionProvenance::Unknown
                })
                .map(|event| (event.provenance, event.claim_identity))
                .fold(Vec::new(), |mut origins, origin| {
                    if !origins.contains(&origin) {
                        origins.push(origin);
                    }
                    origins
                });
            let [origin] = origins.as_slice() else {
                return None;
            };
            Some(*origin)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn bind_claim_outcome_source_at_call(
    program: &typed_trees::TypedTrees,
    caller_state: &typed_trees::state::State,
    statement_index: usize,
    call: &typed_trees::expression::TableCallExpression,
    target_state: &typed_trees::state::State,
    source: &CheckedClaimOutcomeSource,
    segments: &arena::Arena<facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
) -> Option<CheckedClaimOutcomeSource> {
    bind_claim_outcome_source_at_arguments(
        program,
        caller_state,
        statement_index,
        program.expression_table.expression_handles(call.arguments),
        call.receiver.is_valid().then_some(call.receiver),
        target_state,
        source,
        segments,
        permission_events,
    )
}

#[allow(clippy::too_many_arguments)]
fn bind_claim_outcome_source_at_arguments(
    program: &typed_trees::TypedTrees,
    caller_state: &typed_trees::state::State,
    statement_index: usize,
    arguments: &[typed_trees::expression::ExpressionHandle],
    receiver: Option<typed_trees::expression::ExpressionHandle>,
    target_state: &typed_trees::state::State,
    source: &CheckedClaimOutcomeSource,
    segments: &arena::Arena<facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
) -> Option<CheckedClaimOutcomeSource> {
    let CheckedClaimOutcomeSource::Input {
        parameter_symbol,
        path,
    } = source
    else {
        return Some(source.clone());
    };
    let argument = argument_for_parameter(
        program,
        arguments,
        receiver,
        target_state,
        *parameter_symbol,
    )?;
    let parameter = program
        .state_parameters(target_state)
        .iter()
        .find(|parameter| parameter.symbol == *parameter_symbol)?;
    // Constructor operands already name the transferred claims. Resolve the
    // exact typed projection instead of inventing storage below a literal.
    let mut projections = crate::flow::literal_value_projections(
        program,
        argument,
        parameter.type_reference,
        path,
        false,
    )?;
    let projection = projections.pop()?;
    if !projections.is_empty() {
        return None;
    }
    let mut place = crate::flow::canonical_place_from_expression_in_state(
        program,
        caller_state.symbol,
        statement_index,
        projection.expression,
    )?;
    place.segments.extend_from_slice(&projection.remaining);
    let sources = permission_events
        .iter()
        .filter(|event| {
            event.state_symbol == caller_state.symbol
                && permission_event_statement_index(event.source) == Some(statement_index)
                && event.kind == PermissionEventKind::Transfer
                && event.access == PermissionAccess::Owned
                && event.obligation_live
                && event.root == place.root
                && segments.span_or_empty(event.segments) == place.segments
        })
        .map(|event| {
            claim_outcome_source_for_event(
                program,
                caller_state,
                event,
                segments,
                permission_events,
            )
        })
        .fold(Vec::new(), |mut sources, source| {
            if !sources.contains(&source) {
                sources.push(source);
            }
            sources
        });
    let [source] = sources.as_slice() else {
        return None;
    };
    Some(source.clone())
}

fn argument_for_parameter(
    program: &typed_trees::TypedTrees,
    arguments: &[typed_trees::expression::ExpressionHandle],
    receiver: Option<typed_trees::expression::ExpressionHandle>,
    target_state: &typed_trees::state::State,
    parameter_symbol: SymbolHandle,
) -> Option<typed_trees::expression::ExpressionHandle> {
    let parameters = program.state_parameters(target_state);
    let includes_explicit_self =
        parameters.iter().any(|parameter| parameter.is_self) && arguments.len() == parameters.len();
    let mut argument_index = 0usize;
    for parameter in parameters {
        let argument = if parameter.is_self && !includes_explicit_self {
            receiver
        } else {
            let argument = arguments.get(argument_index).copied();
            argument_index = argument_index.saturating_add(1);
            argument
        };
        if parameter.symbol == parameter_symbol {
            return argument;
        }
    }
    None
}

pub(crate) fn call_result_origin_rewrites(
    program: &typed_trees::TypedTrees,
    segments: &arena::Arena<facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
    maps: &[CheckedClaimOutcomeMap],
) -> Vec<(
    PermissionProvenance,
    PermissionClaimIdentity,
    PermissionProvenance,
    PermissionClaimIdentity,
)> {
    let mut rewrites = Vec::new();
    for event in permission_events.iter() {
        if event.kind != PermissionEventKind::Establish
            || event.access != PermissionAccess::Owned
            || !event.obligation_live
        {
            continue;
        }
        let PermissionEventSource::Statement { statement_index } = event.source else {
            continue;
        };
        let locally_minted =
            established_provenance(event.machine_symbol, event.state_symbol, event.source);
        if event.provenance != locally_minted
            || event.claim_identity == PermissionClaimIdentity::Unknown
        {
            continue;
        }
        let Some(state) = crate::semantic_calls::find_state(program, event.state_symbol) else {
            continue;
        };
        let Some(statement) = program
            .statement_table
            .statements(state.statement_nodes)
            .get(statement_index)
        else {
            continue;
        };
        let result_expression = match statement {
            StatementNode::LocalData(local) => local.initial_value,
            StatementNode::Assignment(assignment) => assignment.value,
            _ => continue,
        };
        let typed_trees::expression::ExpressionNode::Call(call) =
            program.expression_table.expression(result_expression)
        else {
            continue;
        };
        let Some(target_state) = crate::semantic_calls::find_state(program, call.target_symbol)
        else {
            continue;
        };
        let Some(map) = maps
            .iter()
            .find(|map| map.state_symbol == target_state.symbol)
        else {
            continue;
        };
        let receiving_path = segments.span_or_empty(event.segments);
        let matching = map
            .entries
            .iter()
            .filter(|entry| entry.output_path == receiving_path)
            .filter_map(|entry| {
                bind_claim_outcome_source_at_call(
                    program,
                    state,
                    statement_index,
                    call,
                    target_state,
                    &entry.source,
                    segments,
                    permission_events,
                )
            })
            .fold(Vec::new(), |mut sources, source| {
                if !sources.contains(&source) {
                    sources.push(source);
                }
                sources
            });
        let [source] = matching.as_slice() else {
            continue;
        };
        let Some((provenance, claim_identity)) =
            claim_origin_for_source(state, source, segments, permission_events)
        else {
            continue;
        };
        // A callee-local result of an unresolved checked call is not an
        // established root. Wait for its exact correspondence; otherwise a
        // wrapper would share that unresolved occurrence across invocations.
        if let CheckedClaimOutcomeSource::Established { provenance, .. } = source {
            match provenance {
                PermissionProvenance::Joined { .. } => continue,
                PermissionProvenance::Established {
                    state_symbol,
                    source: PermissionEventSource::Statement { statement_index },
                    ..
                } => {
                    if let Some(source_state) =
                        crate::semantic_calls::find_state(program, *state_symbol)
                        && let Some(statement) = program
                            .statement_table
                            .statements(source_state.statement_nodes)
                            .get(*statement_index)
                    {
                        let expression = match statement {
                            StatementNode::LocalData(local) => local.initial_value,
                            StatementNode::Assignment(assignment) => assignment.value,
                            _ => typed_trees::expression::ExpressionHandle::invalid(),
                        };
                        if expression.is_valid()
                            && let typed_trees::expression::ExpressionNode::Call(call) =
                                program.expression_table.expression(expression)
                            && crate::semantic_calls::find_state(program, call.target_symbol)
                                .is_some()
                            && !maps
                                .iter()
                                .any(|map| map.state_symbol == call.target_symbol)
                        {
                            continue;
                        }
                    }
                }
                _ => {}
            }
        }
        rewrites.push((
            locally_minted,
            event.claim_identity,
            provenance,
            claim_identity,
        ));
    }
    rewrites
}

pub(crate) fn apply_claim_origin_rewrites(
    permission_events: &mut [FlowPermissionEventFact],
    rewrites: &[(
        PermissionProvenance,
        PermissionClaimIdentity,
        PermissionProvenance,
        PermissionClaimIdentity,
    )],
) -> bool {
    let mut changed = false;
    for event in permission_events
        .iter_mut()
        .filter(|event| event.access == PermissionAccess::Owned)
    {
        let mut provenance = event.provenance;
        let mut claim_identity = event.claim_identity;
        for _ in 0..rewrites.len() {
            let Some((_, _, replacement_provenance, replacement_identity)) =
                rewrites
                    .iter()
                    .find(|(source_provenance, source_identity, _, _)| {
                        *source_provenance == provenance && *source_identity == claim_identity
                    })
            else {
                break;
            };
            if *replacement_provenance == provenance && *replacement_identity == claim_identity {
                break;
            }
            provenance = *replacement_provenance;
            claim_identity = *replacement_identity;
        }
        changed |= event.provenance != provenance || event.claim_identity != claim_identity;
        event.provenance = provenance;
        event.claim_identity = claim_identity;
    }
    changed
}

pub(crate) fn publish_claim_outcome_maps(
    facts: &mut CheckFacts,
    maps: Vec<CheckedClaimOutcomeMap>,
) {
    facts.flow.ownership.claim_outcome_entries = arena::Arena::default();
    facts.flow.ownership.claim_outcome_maps = arena::Arena::default();
    for map in maps {
        let mut entries = Vec::new();
        for entry in map.entries {
            let output_segments = facts.flow.ownership.segments.insert_many(entry.output_path);
            let source = match entry.source {
                CheckedClaimOutcomeSource::Input {
                    parameter_symbol,
                    path,
                } => FlowClaimOutcomeSource::Input {
                    parameter_symbol,
                    segments: facts.flow.ownership.segments.insert_many(path),
                },
                CheckedClaimOutcomeSource::Established {
                    claim_identity,
                    provenance,
                } => FlowClaimOutcomeSource::Established {
                    claim_identity,
                    provenance,
                },
            };
            entries.push(FlowClaimOutcomeEntryFact {
                output_segments,
                source,
            });
        }
        let entries = facts
            .flow
            .ownership
            .claim_outcome_entries
            .insert_many(entries);
        facts
            .flow
            .ownership
            .claim_outcome_maps
            .insert(FlowClaimOutcomeMapFact {
                machine_symbol: map.machine_symbol,
                state_symbol: map.state_symbol,
                entries,
            });
    }
}
