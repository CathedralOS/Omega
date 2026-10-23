//! Deriving claim outcomes: which claim each state result, named
//! transition and call argument settles, and the rewrites that follow call
//! result origins.
//!
//! Input outcomes forward the caller's held identity. Callee establishments
//! instead name templates: each receiving occurrence keeps its locally
//! allocated identity, while `joins` retains the complete call/return
//! correspondence. A result binding is not new resource authority.
//! Argument correspondence uses the completed actual's current ownership at
//! the exact invocation. Descending into a constructor's historical children
//! loses claims already rebased beneath that completed argument.

mod joins;
pub(crate) use joins::{publish_call_claim_joins, validate_call_claim_joins};

use crate::checks::multiplicity::linear_obligations::{
    CheckedClaimOutcomeEntry, CheckedClaimOutcomeMap, CheckedClaimOutcomeSource,
};
use crate::checks::multiplicity::linear_validation::{
    established_provenance, linear_claim_frontier, permission_event_statement_index,
};
use crate::checks::multiplicity::type_multiplicity::data_field_name;
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

/// Invocation correspondence is an input to permission preparation, not a
/// correction to custody after cleanup has already observed it. Only a changed
/// summary actually read by recording schedules another preparation iteration.
#[derive(Default)]
pub(crate) struct ClaimOutcomePreparation {
    maps: Vec<CheckedClaimOutcomeMap>,
    dependencies: Vec<SymbolHandle>,
    joins: joins::CallJoinPreparation,
    revisions: usize,
    revision_limit: usize,
}

impl ClaimOutcomePreparation {
    pub(crate) fn begin(&mut self) {
        self.dependencies.clear();
        self.joins.begin();
    }

    pub(crate) fn update(
        &mut self,
        maps: Vec<CheckedClaimOutcomeMap>,
        joins: Vec<joins::PreparedCallClaimJoin>,
    ) -> Result<bool, &'static str> {
        let dependency_count = maps.len().saturating_add(joins.len());
        // A corrected summary can remove bindings. That shrinks the current
        // approximation, not the dependency universe already traversed.
        self.revision_limit = self
            .revision_limit
            .max(dependency_count.saturating_mul(2).saturating_add(2));
        let maps_changed = self.dependencies.iter().any(|state| {
            self.maps.iter().find(|map| map.state_symbol == *state)
                != maps.iter().find(|map| map.state_symbol == *state)
        });
        let joins_changed = self.joins.update(joins);
        let changed = maps_changed || joins_changed;
        if changed {
            self.revisions += 1;
            if self.revisions > self.revision_limit {
                return Err("call-result custody preparation did not converge");
            }
        }
        self.maps = maps;
        Ok(changed)
    }

    pub(crate) fn resolve(
        &mut self,
        program: &dyn validation::CheckingInput,
        segments: &arena::PagedArenaSet<facts::PlaceSegment>,
        event: &mut FlowPermissionEventFact,
        permission_events: &[FlowPermissionEventFact],
    ) {
        let Some((_, expression, path)) = receiving_value(program, segments, event) else {
            return;
        };
        let Some(call) = program.expression_table.call_node(expression) else {
            return;
        };
        let Some(target) = validation::call_target_state(program, call.target_symbol) else {
            return;
        };
        if !self.dependencies.contains(&target.symbol) {
            self.dependencies.push(target.symbol);
        }
        if let Some(map) = self
            .maps
            .iter()
            .find(|map| map.state_symbol == target.symbol)
            && !map.entries.iter().any(|entry| entry.output_path == path)
        {
            event.obligation_live = false;
            event.claim_identity = PermissionClaimIdentity::Unknown;
            return;
        }
        if let Some((_, _, provenance, identity)) =
            call_result_origin_rewrite(program, segments, event, permission_events, &self.maps)
        {
            event.provenance = provenance;
            event.claim_identity = identity;
        }
        // Different returning sources can have a complete join without an
        // invariant outcome map. Missing either summary never means absence.
        // Forward inputs first; only still-local receiving occurrences join.
        self.joins.resolve(segments, expression, event);
    }
}

pub(crate) fn derive_checked_claim_outcome_maps(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    ownership: &checked_trees::FlowOwnershipFacts,
    permission_events: &[FlowPermissionEventFact],
) -> Result<Vec<CheckedClaimOutcomeMap>, &'static str> {
    let candidate_states = program
        .machines()
        .iter()
        .flat_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .filter_map(move |state| {
                    let expected_paths = linear_claim_frontier(program, state.return_type)
                        .into_iter()
                        .map(|claim| claim.path)
                        .collect::<Vec<_>>();
                    (!expected_paths.is_empty()).then_some((machine.symbol, state, expected_paths))
                })
        })
        .collect::<Vec<_>>();
    if candidate_states.is_empty() {
        return Ok(Vec::new());
    }
    let state_count = candidate_states.len();
    let derive = |maps: &[CheckedClaimOutcomeMap]| {
        candidate_states
            .iter()
            .filter_map(|(machine_symbol, state, expected_paths)| {
                derive_claim_outcome_hypothesis(
                    program,
                    borrow,
                    *machine_symbol,
                    state,
                    expected_paths.clone(),
                    ownership,
                    permission_events,
                    maps,
                )
            })
            .collect::<Vec<_>>()
    };
    let mut maps = Vec::new();
    for _ in 0..=state_count {
        let next = derive(&maps)
            .into_iter()
            .map(|hypothesis| hypothesis.map)
            .collect::<Vec<_>>();
        if next == maps {
            // A direct return can seed a recursive invariant, but a missing
            // alternative is not evidence that the invariant holds. Close
            // every return against the retained hypotheses before publishing
            // any map. Removing a dependency also invalidates its callers;
            // this decreasing phase cannot invent or resurrect a summary.
            loop {
                let supported = derive(&maps)
                    .into_iter()
                    .filter(|hypothesis| {
                        hypothesis.all_returns_covered && maps.contains(&hypothesis.map)
                    })
                    .map(|hypothesis| hypothesis.map)
                    .collect::<Vec<_>>();
                if supported.len() == maps.len() {
                    return Ok(supported);
                }
                maps = supported;
            }
        }
        maps = next;
    }
    Err("claim outcome equations did not converge")
}

/// Private fixed-point hypotheses are not permission or lineage evidence.
/// Normal-return coverage is checked again after removing unsupported maps.
struct ClaimOutcomeHypothesis {
    map: CheckedClaimOutcomeMap,
    all_returns_covered: bool,
}

#[allow(clippy::too_many_arguments)]
fn derive_claim_outcome_hypothesis(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    machine_symbol: SymbolHandle,
    state: &typed_trees::state::State,
    expected_paths: Vec<Vec<facts::PlaceSegment>>,
    ownership: &checked_trees::FlowOwnershipFacts,
    permission_events: &[FlowPermissionEventFact],
    known_maps: &[CheckedClaimOutcomeMap],
) -> Option<ClaimOutcomeHypothesis> {
    let segments = &ownership.segments;
    if expected_paths.is_empty() {
        return None;
    }
    let absence = ClaimAbsence {
        program,
        state,
        ownership,
        events: permission_events,
        maps: known_maps,
        writes: joins::InactivePlaceWrites::prepare(program, borrow, state),
    };
    let statements = typed_trees::statement::reachable_state_statements(
        program.statement_table.statements(state.statement_nodes),
    );
    let result_expressions = state_result_expressions(program, statements);
    let named_transitions = state_result_named_transitions(program, statements);
    let return_entries = result_expressions
        .iter()
        .copied()
        .map(|(statement_index, expression)| {
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
        .chain(named_transitions.iter().copied().map(
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
        .collect::<Vec<_>>();
    let mut entries =
        return_entries
            .iter()
            .flatten()
            .cloned()
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
                    &absence,
                    path,
                    &result_expressions,
                    &named_transitions,
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
    let inactive_paths = expected_paths
        .iter()
        .filter(|path| !entries.iter().any(|entry| entry.output_path == **path))
        .cloned()
        .collect::<Vec<_>>();
    if !joins::complete_frontier(program, state.return_type, &entries, &inactive_paths) {
        return None;
    }
    // Each return accounts for its own selected case. A path may be absent
    // from one return only when that return independently excludes it; another
    // return's constructor is not absence evidence. All live occurrences still
    // agree on the invariant source, so swapped or unresolved maps cannot be
    // hidden by flattening alternatives.
    let all_returns_covered = !return_entries.is_empty()
        && return_entries
            .iter()
            .enumerate()
            .all(|(return_index, returned)| {
                let (expressions, transitions) = if return_index < result_expressions.len() {
                    (
                        &result_expressions[return_index..return_index + 1],
                        &named_transitions[0..0],
                    )
                } else {
                    let transition_index = return_index - result_expressions.len();
                    (
                        &result_expressions[0..0],
                        &named_transitions[transition_index..transition_index + 1],
                    )
                };
                let absent = expected_paths
                    .iter()
                    .filter(|path| {
                        !returned.iter().any(|entry| entry.output_path == **path)
                            && claim_path_is_statically_inactive(
                                &absence,
                                path,
                                expressions,
                                transitions,
                            )
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                joins::complete_frontier(program, state.return_type, returned, &absent)
                    && returned.iter().all(|entry| entries.contains(entry))
            });
    Some(ClaimOutcomeHypothesis {
        map: CheckedClaimOutcomeMap {
            machine_symbol,
            state_symbol: state.symbol,
            entries,
        },
        all_returns_covered,
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
    absence: &ClaimAbsence<'_>,
    path: &[facts::PlaceSegment],
    result_expressions: &[(usize, typed_trees::expression::ExpressionHandle)],
    named_transitions: &[(
        usize,
        SymbolHandle,
        HandleSpan<typed_trees::expression::ExpressionHandle>,
    )],
) -> bool {
    if (result_expressions.is_empty() && named_transitions.is_empty())
        || !path
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
    {
        return false;
    }
    result_expressions.iter().all(|(statement, expression)| {
        absence.expression(*statement, *expression, path, absence.state.return_type)
    }) && named_transitions
        .iter()
        .all(|(statement, target, arguments)| {
            absence.arguments(
                *statement,
                *target,
                absence
                    .program
                    .statement_table
                    .expression_handles(*arguments),
                path,
            )
        })
}

/// Source-owned exclusion of a conditional claim. Complete outcome maps and
/// preserved inactive storage are evidence; a failed identity lookup is not.
struct ClaimAbsence<'a> {
    program: &'a dyn ::validation::CheckingInput,
    state: &'a typed_trees::state::State,
    ownership: &'a checked_trees::FlowOwnershipFacts,
    events: &'a [FlowPermissionEventFact],
    maps: &'a [CheckedClaimOutcomeMap],
    writes: Option<joins::InactivePlaceWrites>,
}

impl ClaimAbsence<'_> {
    fn expression(
        &self,
        statement: usize,
        expression: typed_trees::expression::ExpressionHandle,
        path: &[facts::PlaceSegment],
        expected: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        if let Some(mut place) = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state.symbol,
            statement,
            expression,
        ) {
            place.segments.extend_from_slice(path);
            if self
                .writes
                .as_ref()
                .and_then(|writes| {
                    writes.preserved(
                        self.program,
                        self.state,
                        statement,
                        &place,
                        self.ownership,
                        self.events,
                    )
                })
                .is_some()
            {
                return true;
            }
        }
        match self.program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::StructLiteral(literal) => {
                let mut remaining = path;
                if literal.case_name.is_some() {
                    let Some(facts::PlaceSegment::Case { variant }) = remaining.first() else {
                        return false;
                    };
                    let Some(selected) = literal.case_symbol.and_then(|selected| {
                        self.program
                            .closed_data_constructor_case(expected, selected)
                    }) else {
                        return false;
                    };
                    // Unknown or unrelated classifiers never establish absence.
                    if self
                        .program
                        .closed_data_constructor_case(expected, *variant)
                        .is_none()
                    {
                        return false;
                    }
                    if selected.symbol != *variant {
                        return true;
                    }
                    remaining = &remaining[1..];
                }
                let Some(facts::PlaceSegment::Field { symbol }) = remaining.first() else {
                    return false;
                };
                let Some(field_name) = data_field_name(self.program, *symbol) else {
                    return false;
                };
                self.program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .find(|field| field.name.as_str() == field_name)
                    .is_some_and(|field| {
                        let consumed = path.len() - remaining.len() + 1;
                        output_path_type(
                            self.program,
                            self.state,
                            statement,
                            expected,
                            &path[..consumed],
                        )
                        .is_some_and(|expected| {
                            self.expression(statement, field.value, &remaining[1..], expected)
                        })
                    })
            }
            typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
                let Some(facts::PlaceSegment::FixedIndex { index }) = path.first() else {
                    return false;
                };
                self.program
                    .expression_table
                    .expression_handles(*values)
                    .get(*index)
                    .is_some_and(|value| {
                        output_path_type(self.program, self.state, statement, expected, &path[..1])
                            .is_some_and(|expected| {
                                self.expression(statement, *value, &path[1..], expected)
                            })
                    })
            }
            typed_trees::expression::ExpressionNode::Call(call) => self.arguments(
                statement,
                call.target_symbol,
                self.program
                    .expression_table
                    .expression_handles(call.arguments),
                path,
            ),
            _ => false,
        }
    }

    fn arguments(
        &self,
        statement: usize,
        target_symbol: SymbolHandle,
        arguments: &[typed_trees::expression::ExpressionHandle],
        path: &[facts::PlaceSegment],
    ) -> bool {
        let Some(target) = validation::call_target_state(self.program, target_symbol) else {
            return false;
        };
        let mut maps = self
            .maps
            .iter()
            .filter(|map| map.state_symbol == target.symbol);
        let Some(map) = maps.next() else {
            return false;
        };
        if maps.next().is_some() {
            return false;
        }
        let mut entries = map.entries.iter().filter(|entry| entry.output_path == path);
        let Some(entry) = entries.next() else {
            return path
                .iter()
                .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }));
        };
        if entries.next().is_some() {
            return false;
        }
        let CheckedClaimOutcomeSource::Input {
            parameter_symbol,
            path: input_path,
        } = &entry.source
        else {
            return false;
        };
        let Some(argument) =
            argument_for_parameter(self.program, arguments, target, *parameter_symbol)
        else {
            return false;
        };
        // Substitute the exact input path into its actual once. The same walk
        // handles constructor exclusion, nested calls and preserved storage.
        let Some(parameter) = parameter_for_symbol(self.program, target, *parameter_symbol) else {
            return false;
        };
        self.expression(statement, argument, input_path, parameter.type_reference)
    }
}

fn state_result_expressions(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
            let Some(transition) = statement.as_transition() else {
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
                    if validation::call_target_state(program, path.symbol).is_some_and(|target| {
                        validation::normal_completion_possible(program, target) == Some(false)
                    }) {
                        return None;
                    }
                    Some((statement_index, path.symbol, *arguments))
                })
                .collect()
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn claim_outcomes_for_named_transition(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    target_symbol: SymbolHandle,
    arguments: HandleSpan<typed_trees::expression::ExpressionHandle>,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
    known_maps: &[CheckedClaimOutcomeMap],
) -> Vec<CheckedClaimOutcomeEntry> {
    let Some(target_state) = validation::call_target_state(program, target_symbol) else {
        return Vec::new();
    };
    let Some(target_map) = known_maps
        .iter()
        .find(|map| map.state_symbol == target_state.symbol)
    else {
        return Vec::new();
    };
    let Some(invocation) =
        named_claim_invocation(program, state, statement_index, target_symbol, arguments)
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
                target_state,
                invocation,
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
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    ownership: &checked_trees::FlowOwnershipFacts,
    permission_events: &[FlowPermissionEventFact],
    known_maps: &[CheckedClaimOutcomeMap],
    output_prefix: &[facts::PlaceSegment],
) -> Vec<CheckedClaimOutcomeEntry> {
    let segments = &ownership.segments;
    if let Some(place) = validation::canonical_place_from_expression_in_state(
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
            let Some(variant) = output_path_type(
                program,
                state,
                statement_index,
                state.return_type,
                output_prefix,
            )
            .and_then(|reference| {
                program.closed_data_constructor_case(reference, literal.case_symbol?)
            }) else {
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
            let Some(definition) = output_path_type(
                program,
                state,
                statement_index,
                state.return_type,
                output_prefix,
            )
            .and_then(|reference| {
                program.closed_data_constructor_definition(reference, literal.type_symbol)
            }) else {
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
            let Some(target_state) = validation::call_target_state(program, call.target_symbol)
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
        typed_trees::expression::ExpressionNode::Match(_) => {
            let Some((_, receipt)) =
                ownership.owned_selection_at(state.symbol, statement_index as u32)
            else {
                return Vec::new();
            };
            if receipt.expression != expression {
                return Vec::new();
            }
            let Some(alternatives) =
                selection_alternatives(program, state, ownership, permission_events, receipt)
            else {
                return Vec::new();
            };
            let mut arms = alternatives.into_iter().map(|(_, entries)| entries);
            let Some(mut entries) = arms.next() else {
                return Vec::new();
            };
            // This summary names an invariant source, not an arbitrary arm's
            // source. Every reachable arm must supply the same complete map;
            // an absent arm mapping cannot prove conservation by omission.
            if arms.all(|other| {
                other.len() == entries.len() && other.iter().all(|entry| entries.contains(entry))
            }) {
                for entry in &mut entries {
                    let mut path = output_prefix.to_vec();
                    path.append(&mut entry.output_path);
                    entry.output_path = path;
                }
                entries
            } else {
                Vec::new()
            }
        }
        // A qualification-only `as` returns the value it was handed: the
        // suffix moves, the storage does not. `consume_granted(root: Extent in
        // Granted) -> Extent { root as Extent }` conserves the caller's claim
        // exactly as a bare `root` would, and the erasure is not optional --
        // a routed atom cannot weaken implicitly, so the cast is the only way
        // to spell that return. Without this the state published no outcome
        // map and every caller of such a machine was told its linear result
        // "has no unique conserved claim mapping".
        typed_trees::expression::ExpressionNode::Cast(cast)
            if validation::value_preserving_cast_operand(
                program,
                state.symbol,
                statement_index,
                expression,
            )
            .is_some() =>
        {
            claim_outcomes_for_expression(
                program,
                state,
                statement_index,
                cast.value,
                ownership,
                permission_events,
                known_maps,
                output_prefix,
            )
        }
        _ => Vec::new(),
    }
}

/// Resolve the receiving namespace before interpreting a constructor. A shared
/// generic body retains template selectors while each result owns its closed
/// case and field identities; display names cannot join those namespaces.
fn output_path_type(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    statement: usize,
    root_type: typed_trees::types::TypeReferenceHandle,
    path: &[facts::PlaceSegment],
) -> Option<typed_trees::types::TypeReferenceHandle> {
    validation::canonical_place_type_reference(
        program,
        state.symbol,
        statement,
        &validation::CanonicalPlace {
            root: facts::PlaceRoot::TypeReference(root_type),
            segments: path.to_vec(),
        },
    )
}

/// Complete, correlated source maps for the receipt's authored terminal arms.
/// Result paths were resolved by the producer once; consumers do not recover
/// field correspondence from source order or independently combine claims.
fn selection_alternatives(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    ownership: &checked_trees::FlowOwnershipFacts,
    events: &[FlowPermissionEventFact],
    receipt: &checked_trees::FlowOwnedSelectionReceipt,
) -> Option<
    Vec<(
        arena::Handle<typed_trees::expression::TableMatchArm>,
        Vec<CheckedClaimOutcomeEntry>,
    )>,
> {
    super::owned_selection::terminal_arms(program, receipt.expression)
        .into_iter()
        .map(|(arm, _)| {
            let mut entries = Vec::new();
            for transfer in ownership
                .selection_transfers
                .span_or_empty(receipt.transfers)
                .iter()
                .filter(|transfer| transfer.source_arm == arm)
            {
                if !transfer.source.is_valid() {
                    continue;
                }
                for claim in ownership
                    .selection_transfer_claims
                    .span_or_empty(transfer.claims)
                {
                    let claim_path = ownership.segments.span_or_empty(claim.path);
                    let suffix =
                        claim_path.strip_prefix(ownership.segments.span_or_empty(transfer.path))?;
                    let mut output_path = ownership
                        .segments
                        .span_or_empty(transfer.result_path)
                        .to_vec();
                    output_path.extend_from_slice(suffix);
                    // The source home may still be a parameter after its
                    // old claim was consumed and replaced. Only the actual
                    // transferred identity/provenance can name an entry input.
                    let origin = claim_outcome_source_for_identity(
                        program,
                        state,
                        claim.claim_identity,
                        claim.provenance,
                        &ownership.segments,
                        events,
                    );
                    entries.push(CheckedClaimOutcomeEntry {
                        output_path,
                        source: origin,
                    });
                }
            }
            for produced in ownership
                .selection_produced_claims
                .span_or_empty(receipt.produced_claims)
                .iter()
                .filter(|produced| produced.source_arm == arm)
            {
                entries.push(CheckedClaimOutcomeEntry {
                    output_path: ownership
                        .segments
                        .span_or_empty(produced.result_path)
                        .to_vec(),
                    source: CheckedClaimOutcomeSource::Established {
                        claim_identity: produced.claim_identity,
                        provenance: produced.provenance,
                    },
                });
            }
            Some((arm, entries))
        })
        .collect()
}

fn claim_outcome_source_for_event(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    event: &FlowPermissionEventFact,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
) -> CheckedClaimOutcomeSource {
    claim_outcome_source_for_identity(
        program,
        state,
        event.claim_identity,
        event.provenance,
        segments,
        permission_events,
    )
}

fn claim_outcome_source_for_identity(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    claim_identity: PermissionClaimIdentity,
    provenance: PermissionProvenance,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
) -> CheckedClaimOutcomeSource {
    if let Some(entry) = permission_events.iter().find(|candidate| {
        candidate.state_symbol == state.symbol
            && candidate.source == PermissionEventSource::StateEntry
            && candidate.kind == PermissionEventKind::Establish
            && candidate.access == PermissionAccess::Owned
            && candidate.claim_identity == claim_identity
            && candidate.provenance == provenance
    }) && let Some(parameter_symbol) = entry.root.as_symbol()
        && parameter_for_symbol(program, state, parameter_symbol).is_some()
    {
        return CheckedClaimOutcomeSource::Input {
            parameter_symbol,
            path: segments.span_or_empty(entry.segments).to_vec(),
        };
    }
    CheckedClaimOutcomeSource::Established {
        claim_identity,
        provenance,
    }
}

fn claim_origin_for_source(
    state: &typed_trees::state::State,
    source: &CheckedClaimOutcomeSource,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
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
    program: &dyn ::validation::CheckingInput,
    caller_state: &typed_trees::state::State,
    statement_index: usize,
    call: &typed_trees::expression::TableCallExpression,
    target_state: &typed_trees::state::State,
    source: &CheckedClaimOutcomeSource,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
) -> Option<CheckedClaimOutcomeSource> {
    let (machine, _) = validation::find_state(program, caller_state.symbol)?;
    let invocation = validation::collect_call_sites_in_statement(program, machine, caller_state, statement_index)
        .into_iter().enumerate().find_map(|(call_ordinal, site)| {
            matches!(site, validation::CallSite::Expression { call: found, .. } if std::ptr::eq(found, call))
                .then_some(PermissionEventSource::Call { statement_index, call_ordinal, target_symbol: call.target_symbol })
        })?;
    bind_claim_outcome_source_at_arguments(
        program,
        caller_state,
        statement_index,
        program.expression_table.expression_handles(call.arguments),
        target_state,
        invocation,
        source,
        segments,
        permission_events,
    )
}

fn named_claim_invocation(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    target_symbol: SymbolHandle,
    arguments: HandleSpan<typed_trees::expression::ExpressionHandle>,
) -> Option<PermissionEventSource> {
    let (machine, _) = validation::find_state(program, state.symbol)?;
    let resolved_target = validation::call_target_state(program, target_symbol)
        .map_or(target_symbol, |target| target.symbol);
    let mut matching =
        validation::collect_call_sites_in_statement(program, machine, state, statement_index)
            .into_iter()
            .enumerate()
            .filter_map(|(call_ordinal, site)| {
                matches!(site, validation::CallSite::TransitionNamed { path, arguments: found, .. }
                if path.symbol == target_symbol && found == arguments)
                .then_some(PermissionEventSource::Call {
                    statement_index,
                    call_ordinal,
                    target_symbol: resolved_target,
                })
            });
    let source = matching.next()?;
    matching.next().is_none().then_some(source)
}

#[allow(clippy::too_many_arguments)]
fn bind_claim_outcome_source_at_arguments(
    program: &dyn ::validation::CheckingInput,
    caller_state: &typed_trees::state::State,
    statement_index: usize,
    arguments: &[typed_trees::expression::ExpressionHandle],
    target_state: &typed_trees::state::State,
    invocation: PermissionEventSource,
    source: &CheckedClaimOutcomeSource,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
) -> Option<CheckedClaimOutcomeSource> {
    let CheckedClaimOutcomeSource::Input {
        parameter_symbol,
        path,
    } = source
    else {
        return Some(source.clone());
    };
    let argument = argument_for_parameter(program, arguments, target_state, *parameter_symbol)?;
    // Invocation custody belongs to the completed actual, not to the earlier
    // births of its constructed children. Resolve the formal-relative path
    // beneath that current owner, just as structural call transfer does.
    let mut place = validation::canonical_place_from_expression_in_state(
        program,
        caller_state.symbol,
        statement_index,
        argument,
    )?;
    let transferred_at = |event: &&FlowPermissionEventFact| {
        event.state_symbol == caller_state.symbol
            && event.source == invocation
            && event.kind == PermissionEventKind::Transfer
            && event.access == PermissionAccess::Owned
            && event.obligation_live
    };
    let transfers = permission_events
        .iter()
        .filter(transferred_at)
        .collect::<Vec<_>>();
    // An owned extraction can publish anonymous custody distinct from its
    // borrowed source. Both roots at this invocation are ambiguous, not a
    // reason to select whichever historical claim happens to match.
    if place.root.as_symbol().is_some()
        && transfers
            .iter()
            .any(|event| event.root == facts::PlaceRoot::Expression(argument))
    {
        if transfers.iter().any(|event| {
            event.root == place.root
                && segments
                    .span_or_empty(event.segments)
                    .starts_with(&place.segments)
        }) {
            return None;
        }
        place = validation::CanonicalPlace {
            root: facts::PlaceRoot::Expression(argument),
            segments: Vec::new(),
        };
    }
    place.segments.extend_from_slice(path);
    let sources = permission_events
        .iter()
        .filter(|event| {
            event.state_symbol == caller_state.symbol
                && event.source == invocation
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

/// Resolve `parameter_symbol` to its authored state parameter.
fn parameter_for_symbol<'a>(
    program: &'a dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    parameter_symbol: SymbolHandle,
) -> Option<&'a typed_trees::signature::StateParameter> {
    let parameters = program.state_parameters(state);
    parameters
        .iter()
        .find(|parameter| parameter.symbol == parameter_symbol)
}

fn argument_for_parameter(
    program: &dyn ::validation::CheckingInput,
    arguments: &[typed_trees::expression::ExpressionHandle],
    target_state: &typed_trees::state::State,
    parameter_symbol: SymbolHandle,
) -> Option<typed_trees::expression::ExpressionHandle> {
    let parameters = program.state_parameters(target_state);
    let matched = parameter_for_symbol(program, target_state, parameter_symbol)?;
    let position = typed_trees::signature::state_parameter_position(parameters, matched.symbol)?;
    arguments.get(position).copied()
}

pub(crate) use validation::receiving_value;

pub(crate) fn call_result_origin_rewrites(
    program: &dyn ::validation::CheckingInput,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
    maps: &[CheckedClaimOutcomeMap],
) -> Vec<(
    PermissionProvenance,
    PermissionClaimIdentity,
    PermissionProvenance,
    PermissionClaimIdentity,
)> {
    if maps.is_empty() {
        return Vec::new();
    }
    permission_events
        .iter()
        .filter_map(|event| {
            call_result_origin_rewrite(program, segments, event, permission_events, maps)
        })
        .collect()
}

fn call_result_origin_rewrite(
    program: &dyn validation::CheckingInput,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    event: &FlowPermissionEventFact,
    permission_events: &[FlowPermissionEventFact],
    maps: &[CheckedClaimOutcomeMap],
) -> Option<(
    PermissionProvenance,
    PermissionClaimIdentity,
    PermissionProvenance,
    PermissionClaimIdentity,
)> {
    if event.kind != PermissionEventKind::Establish
        || event.access != PermissionAccess::Owned
        || !event.obligation_live
    {
        return None;
    }
    let (statement_index, result_expression, receiving_path) =
        receiving_value(program, segments, event)?;
    let locally_minted =
        established_provenance(event.machine_symbol, event.state_symbol, event.source);
    if event.provenance != locally_minted
        || event.claim_identity == PermissionClaimIdentity::Unknown
    {
        return None;
    }
    let state = program.state_by_symbol(event.state_symbol)?;
    let typed_trees::expression::ExpressionNode::Call(call) =
        program.expression_table.expression(result_expression)
    else {
        return None;
    };
    let target_state = validation::call_target_state(program, call.target_symbol)?;
    let map = maps
        .iter()
        .find(|map| map.state_symbol == target_state.symbol)?;
    let matching = map
        .entries
        .iter()
        .filter(|entry| entry.output_path == receiving_path)
        // Only an input correspondence forwards a caller-held identity.
        // A callee establishment is a template shared by all invocations;
        // its completed result keeps the receiving occurrence's identity
        // and joins its exact return source through a call receipt.
        .filter(|entry| matches!(entry.source, CheckedClaimOutcomeSource::Input { .. }))
        // Forwarding preserves one identity only when it names one receiving
        // location. A case-dependent location instead needs the existing
        // occurrence-local join, whose alternatives authenticate the live path.
        .filter(|entry| {
            !map.entries
                .iter()
                .any(|other| other.source == entry.source && other.output_path != entry.output_path)
        })
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
        return None;
    };
    let (provenance, claim_identity) =
        claim_origin_for_source(state, source, segments, permission_events)?;
    Some((
        locally_minted,
        event.claim_identity,
        provenance,
        claim_identity,
    ))
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
    facts.flow.ownership.claim_outcome_entries = Default::default();
    facts.flow.ownership.claim_outcome_maps = Default::default();
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
