//! Fail-closed fence for borrow-carrying values stored beyond one graph state.
//!
//! Local loan attribution is statement/state scoped today. A borrow backed by
//! program-static storage needs no source loan, so literals and machine results
//! whose every value exit is likewise static may be stored persistently. Stable
//! fixed-index paths and runtime indexes named by immutable state parameters or
//! locals preserve that provenance across named edges when the same symbol, or
//! a direct immutable local-copy alias, is forwarded into an immutable target
//! parameter. Non-static sources remain fenced until the flow plan propagates
//! a persistent owner's loan through every outgoing transition.

use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::statement::StatementNode;
use psi_typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
struct StaticPersistentPath {
    field: SymbolHandle,
    segments: Vec<StaticPersistentSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StaticPersistentSegment {
    Field(SymbolHandle),
    Case(SymbolHandle),
    FixedIndex(usize),
    StableIndex(SymbolHandle),
}

#[derive(Debug, Clone, Copy)]
struct StateTransitionEdge {
    target: SymbolHandle,
    arguments: psi_arena::HandleSpan<psi_typed_trees::expression::ExpressionHandle>,
}

pub(super) fn check_persistent_borrow_assignments(
    program: &psi_typed_trees::TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        let persistent = persistent_storage(program, machine);
        if persistent.is_empty() {
            continue;
        }

        let states = program.machine_states(machine);
        let call_frames = psi_validation::CallFrameResolver::new(program);
        let entry_paths = static_persistent_paths_at_state_entries(
            program,
            machine,
            &persistent,
            states,
            call_frames.as_ref(),
        );
        for (state, entry_paths) in states.iter().zip(entry_paths) {
            let (_, state_diagnostics) = analyze_persistent_state(
                program,
                machine,
                state,
                &persistent,
                &entry_paths,
                call_frames.as_ref(),
                true,
            );
            diagnostics.extend(state_diagnostics);
        }
    }
}

fn static_persistent_paths_at_state_entries(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    persistent: &[(SymbolHandle, &str, TypeReferenceHandle)],
    states: &[psi_typed_trees::state::State],
    call_frames: Option<&psi_validation::CallFrameResolver<'_>>,
) -> Vec<Vec<StaticPersistentPath>> {
    let mut entries = vec![None::<Vec<StaticPersistentPath>>; states.len()];
    if !states.is_empty() {
        entries[0] = Some(Vec::new());
    }

    loop {
        let mut changed = false;
        for (state_index, state) in states.iter().enumerate() {
            let Some(entry) = entries[state_index].clone() else {
                continue;
            };
            let (exit, _) = analyze_persistent_state(
                program,
                machine,
                state,
                persistent,
                &entry,
                call_frames,
                false,
            );
            for edge in state_transition_edges(program, state) {
                let Some(target_index) = states
                    .iter()
                    .position(|candidate| candidate.symbol == edge.target)
                else {
                    continue;
                };
                let exit = rebase_static_paths_for_transition(
                    program,
                    state,
                    &states[target_index],
                    edge.arguments,
                    &exit,
                );
                let merged = entries[target_index].as_ref().map_or_else(
                    || exit.clone(),
                    |prior| {
                        prior
                            .iter()
                            .filter(|&path| exit.contains(path))
                            .cloned()
                            .collect()
                    },
                );
                if entries[target_index].as_ref() != Some(&merged) {
                    entries[target_index] = Some(merged);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    entries
        .into_iter()
        .map(|entry| entry.unwrap_or_default())
        .collect()
}

fn state_transition_edges(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
) -> Vec<StateTransitionEdge> {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| {
            let StatementNode::Transition(transition) = statement else {
                return None;
            };
            Some([transition.target, transition.continuation])
        })
        .flatten()
        .filter(|target| target.is_valid())
        .filter_map(|target| {
            let psi_typed_trees::statement::TransitionTargetNode::Named {
                path, arguments, ..
            } = program.statement_table.transition_target(target)
            else {
                return None;
            };
            path.symbol.is_valid().then_some(StateTransitionEdge {
                target: path.symbol,
                arguments: *arguments,
            })
        })
        .collect()
}

fn rebase_static_paths_for_transition(
    program: &psi_typed_trees::TypedTrees,
    source: &psi_typed_trees::state::State,
    target: &psi_typed_trees::state::State,
    arguments: psi_arena::HandleSpan<psi_typed_trees::expression::ExpressionHandle>,
    paths: &[StaticPersistentPath],
) -> Vec<StaticPersistentPath> {
    let arguments = program.statement_table.expression_handles(arguments);
    let target_parameters = program
        .state_parameters(target)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    if arguments.len() != target_parameters.len() {
        return paths
            .iter()
            .filter(|path| {
                !path
                    .segments
                    .iter()
                    .any(|segment| matches!(segment, StaticPersistentSegment::StableIndex(_)))
            })
            .cloned()
            .collect();
    }

    let mut rebased = Vec::new();
    for path in paths {
        let mut path = path.clone();
        let mut complete = true;
        for segment in &mut path.segments {
            let StaticPersistentSegment::StableIndex(source_symbol) = *segment else {
                continue;
            };
            let replacement =
                arguments
                    .iter()
                    .zip(&target_parameters)
                    .find_map(|(argument, parameter)| {
                        (!parameter.is_mutable
                            && expression_is_exact_stable_index(
                                program,
                                source,
                                *argument,
                                source_symbol,
                            ))
                        .then_some(parameter.symbol)
                    });
            let Some(replacement) = replacement else {
                complete = false;
                break;
            };
            *segment = StaticPersistentSegment::StableIndex(replacement);
        }
        if complete && !rebased.contains(&path) {
            rebased.push(path);
        }
    }
    rebased
}

fn expression_is_exact_stable_index(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    expression: psi_typed_trees::expression::ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    let Some(candidate) = exact_name_symbol(program, state, expression) else {
        return false;
    };
    stable_index_origin_symbol(program, state, symbol, &mut Vec::new())
        .zip(stable_index_origin_symbol(
            program,
            state,
            candidate,
            &mut Vec::new(),
        ))
        .is_some_and(|(source, candidate)| source == candidate)
}

fn stable_index_origin_symbol(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    symbol: SymbolHandle,
    visiting: &mut Vec<SymbolHandle>,
) -> Option<SymbolHandle> {
    if program
        .state_parameters(state)
        .iter()
        .any(|parameter| !parameter.is_self && !parameter.is_mutable && parameter.symbol == symbol)
    {
        return Some(symbol);
    }
    let local = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            (!local.is_mutable && local.symbol == symbol).then_some(local)
        })?;
    if visiting.contains(&symbol) {
        return None;
    }
    visiting.push(symbol);
    let origin = exact_name_symbol(program, state, local.initial_value)
        .and_then(|source| stable_index_origin_symbol(program, state, source, visiting))
        .unwrap_or(symbol);
    visiting.pop();
    Some(origin)
}

fn analyze_persistent_state(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    persistent: &[(SymbolHandle, &str, TypeReferenceHandle)],
    entry_paths: &[StaticPersistentPath],
    call_frames: Option<&psi_validation::CallFrameResolver<'_>>,
    report_diagnostics: bool,
) -> (Vec<StaticPersistentPath>, Vec<Diagnostic>) {
    let mut static_persistent_places = Vec::new();
    let mut static_persistent_paths = entry_paths.to_vec();
    let mut diagnostics = Vec::new();
    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let value_writes = call_frames
            .and_then(|frames| frames.statement_value_may_write_paths(machine, statement));
        if retain_static_paths_across_call_frame(
            program,
            state,
            persistent,
            &mut static_persistent_paths,
            value_writes,
        ) {
            static_persistent_places.clear();
        }

        if let StatementNode::Call(call) = statement {
            let statement_writes =
                call_frames.and_then(|frames| frames.may_write_paths(machine, call));
            if retain_static_paths_across_call_frame(
                program,
                state,
                persistent,
                &mut static_persistent_paths,
                statement_writes,
            ) {
                static_persistent_places.clear();
            }
            continue;
        }

        let StatementNode::Assignment(assignment) = statement else {
            continue;
        };
        let Some(place) = crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            statement_index,
            assignment.target,
        ) else {
            continue;
        };
        let target_field = persistent_field_and_tail(&place, persistent);
        let target_path = stable_persistent_path(program, state, &place, persistent);
        let Some((name, target_type)) = persistent_target_type(program, &place, persistent) else {
            static_persistent_places
                .retain(|known| !static_provenance_invalidated_by_mutation(program, known, &place));
            invalidate_static_persistent_paths(
                &mut static_persistent_paths,
                target_field,
                target_path.as_ref(),
            );
            continue;
        };
        if !crate::borrow::view_link::returns_borrow(program, target_type) {
            static_persistent_places
                .retain(|known| !static_provenance_invalidated_by_mutation(program, known, &place));
            invalidate_static_persistent_paths(
                &mut static_persistent_paths,
                target_field,
                target_path.as_ref(),
            );
            continue;
        }
        let initializers =
            crate::borrow::borrow_initializer_expressions(program, target_type, assignment.value);
        let has_only_static_sources = if initializers.is_empty() {
            is_state_independent_borrow_source(program, assignment.value)
        } else {
            initializers
                .into_iter()
                .all(|initializer| is_state_independent_borrow_source(program, initializer))
        };
        let has_only_static_sources = has_only_static_sources
            || source_is_known_static_persistent_place(
                program,
                state.symbol,
                statement_index,
                assignment.value,
                target_type,
                &static_persistent_places,
                &static_persistent_paths,
                persistent,
            );

        // Assignment reads its source before replacing the target.
        static_persistent_places
            .retain(|known| !static_provenance_invalidated_by_mutation(program, known, &place));
        invalidate_static_persistent_paths(
            &mut static_persistent_paths,
            target_field,
            target_path.as_ref(),
        );
        if has_only_static_sources {
            static_persistent_places.push(place);
            if let Some(target_path) = target_path {
                add_static_borrow_frontier(
                    program,
                    target_path,
                    target_type,
                    &mut static_persistent_paths,
                );
            }
            continue;
        }

        if report_diagnostics {
            diagnostics.push(Diagnostic::error(format!(
                "assignment stores a borrow-carrying value in persistent field `{name}` of \
                 machine `{}`; persistent loans must be propagated through graph-state \
                 transitions before this write can be admitted",
                machine.name,
            )));
        }
    }
    (static_persistent_paths, diagnostics)
}

fn source_is_known_static_persistent_place(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: psi_typed_trees::expression::ExpressionHandle,
    source_type: TypeReferenceHandle,
    known_static: &[crate::flow::CanonicalPlace],
    known_static_paths: &[StaticPersistentPath],
    persistent: &[(SymbolHandle, &str, TypeReferenceHandle)],
) -> bool {
    let Some(source) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    ) else {
        return false;
    };

    let Some(state) = crate::find_state(program, state_symbol) else {
        return false;
    };
    if known_static
        .iter()
        .any(|known| place_is_proven_prefix_of(program, state, known, &source))
    {
        return true;
    }

    let Some(borrow_paths) =
        crate::borrow::view_link::borrow_carrying_owner_paths(program, source_type)
    else {
        return false;
    };
    !borrow_paths.is_empty()
        && borrow_paths.into_iter().all(|path| {
            let Some(segments) = owner_path_place_segments(&path) else {
                return false;
            };
            let mut leaf = source.clone();
            leaf.extend_segments(&segments);
            known_static
                .iter()
                .any(|known| place_is_proven_prefix_of(program, state, known, &leaf))
                || stable_persistent_path(program, state, &leaf, persistent).is_some_and(|leaf| {
                    known_static_paths.iter().any(|known| {
                        known.field == leaf.field
                            && known.segments.len() <= leaf.segments.len()
                            && known
                                .segments
                                .iter()
                                .zip(&leaf.segments)
                                .all(|(left, right)| left == right)
                    })
                })
        })
}

/// Return the persistent field selected by a canonical place and the segment
/// index immediately below that field. Attached-data places carry the field as
/// a segment below the state-local receiver root; machine-owned data can use
/// the field symbol as the root directly.
fn persistent_field_and_tail(
    place: &crate::flow::CanonicalPlace,
    persistent: &[(SymbolHandle, &str, TypeReferenceHandle)],
) -> Option<(SymbolHandle, usize)> {
    if let psi_facts::PlaceRoot::Symbol(symbol) = place.root
        && persistent
            .iter()
            .any(|(candidate, _, _)| *candidate == symbol)
    {
        return Some((symbol, 0));
    }

    place
        .segments
        .iter()
        .enumerate()
        .find_map(|(index, segment)| {
            let psi_facts::PlaceSegment::Field { symbol } = segment else {
                return None;
            };
            persistent
                .iter()
                .any(|(candidate, _, _)| candidate == symbol)
                .then_some((*symbol, index + 1))
        })
}

fn stable_persistent_path(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    place: &crate::flow::CanonicalPlace,
    persistent: &[(SymbolHandle, &str, TypeReferenceHandle)],
) -> Option<StaticPersistentPath> {
    let (field, tail) = persistent_field_and_tail(place, persistent)?;
    let segments = place.segments[tail..]
        .iter()
        .map(|segment| static_persistent_segment(program, state, *segment))
        .collect::<Option<Vec<_>>>()?;
    Some(StaticPersistentPath { field, segments })
}

fn static_persistent_segment(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    segment: psi_facts::PlaceSegment,
) -> Option<StaticPersistentSegment> {
    match segment {
        psi_facts::PlaceSegment::Field { symbol } => Some(StaticPersistentSegment::Field(symbol)),
        psi_facts::PlaceSegment::Case { variant } => Some(StaticPersistentSegment::Case(variant)),
        psi_facts::PlaceSegment::FixedIndex { index } => {
            Some(StaticPersistentSegment::FixedIndex(index))
        }
        psi_facts::PlaceSegment::FixedRange { .. } => None,
        psi_facts::PlaceSegment::Index { expression } => {
            immutable_state_index_symbol(program, state, expression)
                .map(StaticPersistentSegment::StableIndex)
        }
    }
}

fn immutable_state_index_symbol(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<SymbolHandle> {
    let symbol = exact_name_symbol(program, state, expression)?;
    let parameter_matches = program.state_parameters(state).iter().filter(|parameter| {
        !parameter.is_self && !parameter.is_mutable && parameter.symbol == symbol
    });
    let local_matches = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            (!local.is_mutable && local.symbol == symbol).then_some(local.symbol)
        });
    let mut matching = parameter_matches
        .map(|parameter| parameter.symbol)
        .chain(local_matches);
    let matched = matching.next()?;
    matching.next().is_none().then_some(matched)
}

fn exact_name_symbol(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<SymbolHandle> {
    let psi_typed_trees::expression::ExpressionNode::Name(path) =
        program.expression_table.expression(expression)
    else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    if path.symbol.is_valid() {
        return Some(path.symbol);
    }
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            (local.name == *name).then_some(local.symbol)
        })
}

fn invalidate_static_persistent_paths(
    paths: &mut Vec<StaticPersistentPath>,
    target_field: Option<(SymbolHandle, usize)>,
    target_path: Option<&StaticPersistentPath>,
) {
    let Some((field, _)) = target_field else {
        return;
    };
    paths.retain(|known| {
        if known.field != field {
            return true;
        }
        let Some(target) = target_path else {
            // A dynamic or otherwise unstable persistent projection may select
            // any leaf beneath the field.
            return false;
        };
        !static_persistent_segments_may_overlap(&known.segments, &target.segments)
    });
}

fn static_persistent_segments_may_overlap(
    left: &[StaticPersistentSegment],
    right: &[StaticPersistentSegment],
) -> bool {
    for (left, right) in left.iter().zip(right) {
        match (*left, *right) {
            (StaticPersistentSegment::Field(left), StaticPersistentSegment::Field(right))
            | (StaticPersistentSegment::Case(left), StaticPersistentSegment::Case(right))
                if left != right =>
            {
                return false;
            }
            (
                StaticPersistentSegment::FixedIndex(left),
                StaticPersistentSegment::FixedIndex(right),
            ) if left != right => return false,
            (
                StaticPersistentSegment::StableIndex(left),
                StaticPersistentSegment::StableIndex(right),
            ) if left == right => {}
            (StaticPersistentSegment::Field(_), StaticPersistentSegment::Field(_))
            | (StaticPersistentSegment::Case(_), StaticPersistentSegment::Case(_))
            | (StaticPersistentSegment::FixedIndex(_), StaticPersistentSegment::FixedIndex(_))
            | (StaticPersistentSegment::FixedIndex(_), StaticPersistentSegment::StableIndex(_))
            | (StaticPersistentSegment::StableIndex(_), StaticPersistentSegment::FixedIndex(_))
            | (StaticPersistentSegment::StableIndex(_), StaticPersistentSegment::StableIndex(_)) => {
            }
            // Different segment classes at one structural depth are not a
            // stable proof of disjointness.
            _ => return true,
        }
    }
    true
}

fn add_static_borrow_frontier(
    program: &psi_typed_trees::TypedTrees,
    target: StaticPersistentPath,
    target_type: TypeReferenceHandle,
    paths: &mut Vec<StaticPersistentPath>,
) {
    let Some(frontier) =
        crate::borrow::view_link::borrow_carrying_owner_paths(program, target_type)
    else {
        return;
    };
    for owner_path in frontier {
        let Some(owner_segments) = owner_path_place_segments(&owner_path) else {
            continue;
        };
        let mut path = target.clone();
        path.segments
            .extend(owner_segments.into_iter().map(|segment| match segment {
                psi_facts::PlaceSegment::Field { symbol } => StaticPersistentSegment::Field(symbol),
                psi_facts::PlaceSegment::Case { variant } => StaticPersistentSegment::Case(variant),
                psi_facts::PlaceSegment::FixedIndex { index } => {
                    StaticPersistentSegment::FixedIndex(index)
                }
                psi_facts::PlaceSegment::FixedRange { .. }
                | psi_facts::PlaceSegment::Index { .. } => unreachable!(
                    "range/dynamic borrow-owner paths are rejected before persistent propagation"
                ),
            }));
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
}

/// Apply one complete R5 may-write frame to the stable provenance paths.
/// Returns whether any call was opaque or may write something, in which case
/// state-local canonical markers are conservatively retired as well.
fn retain_static_paths_across_call_frame(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    persistent: &[(SymbolHandle, &str, TypeReferenceHandle)],
    paths: &mut Vec<StaticPersistentPath>,
    written: Option<Vec<String>>,
) -> bool {
    let Some(written) = written else {
        paths.clear();
        return true;
    };
    if written.is_empty() {
        return false;
    }
    paths.retain(|path| {
        let aliases = static_path_frame_aliases(program, state, persistent, path);
        !aliases.is_empty()
            && written.iter().all(|written| {
                aliases
                    .iter()
                    .all(|path| !psi_validation::frame_paths_overlap(path, written))
            })
    });
    true
}

fn static_path_frame_aliases(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    persistent: &[(SymbolHandle, &str, TypeReferenceHandle)],
    path: &StaticPersistentPath,
) -> Vec<String> {
    let Some((_, root_name, _)) = persistent.iter().find(|(field, _, _)| *field == path.field)
    else {
        return Vec::new();
    };
    let mut suffix = String::new();
    for segment in &path.segments {
        match *segment {
            StaticPersistentSegment::Field(symbol) => {
                let Some(name) = data_field_name(program, symbol) else {
                    break;
                };
                suffix.push('.');
                suffix.push_str(name);
            }
            StaticPersistentSegment::FixedIndex(index) => {
                suffix.push('[');
                suffix.push_str(&index.to_string());
                suffix.push(']');
            }
            StaticPersistentSegment::StableIndex(symbol) => {
                let parameter_name = program
                    .state_parameters(state)
                    .iter()
                    .find_map(|parameter| {
                        (parameter.symbol == symbol).then_some(parameter.name.as_str())
                    });
                let local_name = program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .find_map(|statement| {
                        let StatementNode::LocalData(local) = statement else {
                            return None;
                        };
                        (local.symbol == symbol).then_some(local.name.as_str())
                    });
                let Some(name) = parameter_name.or(local_name) else {
                    return Vec::new();
                };
                suffix.push('[');
                suffix.push_str(name);
                suffix.push(']');
            }
            // Source frame strings do not retain normalized case identity.
            // Stop at the containing sum so any payload write invalidates the
            // fact rather than conflating equal field names across variants.
            StaticPersistentSegment::Case(_) => break,
        }
    }
    vec![
        format!("{root_name}{suffix}"),
        format!("self.{root_name}{suffix}"),
    ]
}

fn data_field_name(program: &psi_typed_trees::TypedTrees, symbol: SymbolHandle) -> Option<&str> {
    for definition in program.data_definitions() {
        for member in program.data_members(definition) {
            match member {
                psi_typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    return Some(field.name.as_str());
                }
                psi_typed_trees::data::DataMember::Variant(variant) => {
                    if let Some(field) = program
                        .data_payload_fields(variant)
                        .iter()
                        .find(|field| field.symbol == symbol)
                    {
                        return Some(field.name.as_str());
                    }
                }
                psi_typed_trees::data::DataMember::Field(_) => {}
            }
        }
    }
    None
}

fn owner_path_place_segments(
    path: &[crate::borrow::BorrowOwnerSegment],
) -> Option<Vec<psi_facts::PlaceSegment>> {
    path.iter()
        .map(|segment| match segment {
            crate::borrow::BorrowOwnerSegment::Field(symbol) => {
                Some(psi_facts::PlaceSegment::Field { symbol: *symbol })
            }
            crate::borrow::BorrowOwnerSegment::Case(variant) => {
                Some(psi_facts::PlaceSegment::Case { variant: *variant })
            }
            crate::borrow::BorrowOwnerSegment::FixedIndex(index) => {
                Some(psi_facts::PlaceSegment::FixedIndex { index: *index })
            }
            crate::borrow::BorrowOwnerSegment::DynamicIndex => None,
        })
        .collect()
}

fn static_provenance_invalidated_by_mutation(
    program: &psi_typed_trees::TypedTrees,
    known: &crate::flow::CanonicalPlace,
    mutated: &crate::flow::CanonicalPlace,
) -> bool {
    if known.root == mutated.root
        && crate::flow::canonical_place_segments_may_overlap(
            program,
            &known.segments,
            &mutated.segments,
        )
    {
        return true;
    }

    known.segments.iter().any(|segment| {
        let psi_facts::PlaceSegment::Index { expression } = segment else {
            return false;
        };
        crate::flow::canonical_place_from_expression(program, *expression).is_some_and(
            |dependency| {
                dependency.root == mutated.root
                    && crate::flow::canonical_place_segments_may_overlap(
                        program,
                        &dependency.segments,
                        &mutated.segments,
                    )
            },
        )
    })
}

fn place_is_proven_prefix_of(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    prefix: &crate::flow::CanonicalPlace,
    place: &crate::flow::CanonicalPlace,
) -> bool {
    prefix.root == place.root
        && prefix.segments.len() <= place.segments.len()
        && prefix
            .segments
            .iter()
            .zip(&place.segments)
            .all(|(left, right)| place_segments_proven_equal(program, state, *left, *right))
}

fn place_segments_proven_equal(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    left: psi_facts::PlaceSegment,
    right: psi_facts::PlaceSegment,
) -> bool {
    match (left, right) {
        (
            psi_facts::PlaceSegment::Index {
                expression: left_expression,
            },
            psi_facts::PlaceSegment::Index {
                expression: right_expression,
            },
        ) => {
            if let (Some(left), Some(right)) = (
                program
                    .expression_table
                    .constant_integer_value(left_expression),
                program
                    .expression_table
                    .constant_integer_value(right_expression),
            ) {
                return left == right;
            }
            immutable_local_index_symbol(program, state, left_expression).is_some_and(|left| {
                immutable_local_index_symbol(program, state, right_expression)
                    .is_some_and(|right| left == right)
            })
        }
        _ => crate::flow::canonical_place_segments_equal(left, right),
    }
}

fn immutable_local_index_symbol(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<SymbolHandle> {
    let psi_typed_trees::expression::ExpressionNode::Name(path) =
        program.expression_table.expression(expression)
    else {
        return None;
    };
    let members = program.expression_table.name_path_members(path.members);
    let [name] = members else {
        return None;
    };
    let resolved = path
        .head_symbol
        .is_valid()
        .then_some(path.head_symbol)
        .or_else(|| path.symbol.is_valid().then_some(path.symbol));
    let mut matches = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            (!local.is_mutable
                && resolved
                    .map(|symbol| local.symbol == symbol)
                    .unwrap_or_else(|| local.name == *name))
            .then_some(local.symbol)
        });
    let symbol = matches.next()?;
    matches.next().is_none().then_some(symbol)
}

fn is_state_independent_borrow_source(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        psi_typed_trees::expression::ExpressionNode::String(_) => true,
        psi_typed_trees::expression::ExpressionNode::Cast(cast) => {
            is_state_independent_borrow_source(program, cast.value)
        }
        psi_typed_trees::expression::ExpressionNode::Binary(binary) => {
            is_state_independent_borrow_source(program, binary.left)
                && is_state_independent_borrow_source(program, binary.right)
        }
        psi_typed_trees::expression::ExpressionNode::Call(call) => {
            let Some(state) = crate::semantic_calls::find_state(program, call.target_symbol) else {
                return false;
            };
            state_returns_only_static_borrows(program, state, &mut Vec::new())
        }
        psi_typed_trees::expression::ExpressionNode::ArrayLiteral(_)
        | psi_typed_trees::expression::ExpressionNode::Atomic(_)
        | psi_typed_trees::expression::ExpressionNode::Boolean(_)
        | psi_typed_trees::expression::ExpressionNode::Float(_)
        | psi_typed_trees::expression::ExpressionNode::Indexed(_)
        | psi_typed_trees::expression::ExpressionNode::Integer(_)
        | psi_typed_trees::expression::ExpressionNode::Member(_)
        | psi_typed_trees::expression::ExpressionNode::Borrow(_)
        | psi_typed_trees::expression::ExpressionNode::Name(_)
        | psi_typed_trees::expression::ExpressionNode::Range(_)
        | psi_typed_trees::expression::ExpressionNode::StructLiteral(_)
        | psi_typed_trees::expression::ExpressionNode::Unary(_)
        | psi_typed_trees::expression::ExpressionNode::ZeroValue(_) => false,
    }
}

fn state_returns_only_static_borrows(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    visiting: &mut Vec<SymbolHandle>,
) -> bool {
    if visiting.contains(&state.symbol) {
        return false;
    }
    visiting.push(state.symbol);

    let mut found_value_exit = false;
    let all_static = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| {
            let StatementNode::Transition(transition) = statement else {
                return None;
            };
            Some([transition.target, transition.continuation])
        })
        .flatten()
        .filter(|target| target.is_valid())
        .all(
            |target| match program.statement_table.transition_target(target) {
                psi_typed_trees::statement::TransitionTargetNode::Value(expression) => {
                    found_value_exit = true;
                    is_state_independent_borrow_source(program, *expression)
                }
                psi_typed_trees::statement::TransitionTargetNode::Named { path, .. } => {
                    let Some(target_state) =
                        crate::semantic_calls::find_state(program, path.symbol)
                    else {
                        return false;
                    };
                    let is_static =
                        state_returns_only_static_borrows(program, target_state, visiting);
                    found_value_exit |= is_static;
                    is_static
                }
                psi_typed_trees::statement::TransitionTargetNode::SelfTarget => false,
                psi_typed_trees::statement::TransitionTargetNode::Terminal => true,
            },
        );

    visiting.pop();
    all_static && found_value_exit
}

fn persistent_target_type<'program>(
    program: &psi_typed_trees::TypedTrees,
    place: &crate::flow::CanonicalPlace,
    persistent: &[(SymbolHandle, &'program str, TypeReferenceHandle)],
) -> Option<(&'program str, TypeReferenceHandle)> {
    if let psi_facts::PlaceRoot::Symbol(symbol) = place.root
        && let Some((_, name, root_type)) = persistent
            .iter()
            .find(|(candidate, _, _)| *candidate == symbol)
    {
        let target_type = crate::flow::project_type_reference_from_segments(
            program,
            *root_type,
            &place.segments,
        )?;
        return Some((*name, target_type));
    }

    place
        .segments
        .iter()
        .enumerate()
        .find_map(|(index, segment)| {
            let psi_facts::PlaceSegment::Field { symbol } = segment else {
                return None;
            };
            let (_, name, root_type) = persistent
                .iter()
                .find(|(candidate, _, _)| candidate == symbol)?;
            let target_type = crate::flow::project_type_reference_from_segments(
                program,
                *root_type,
                &place.segments[index + 1..],
            )?;
            Some((*name, target_type))
        })
}

fn persistent_storage<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    machine: &'program psi_typed_trees::machine::Machine,
) -> Vec<(SymbolHandle, &'program str, TypeReferenceHandle)> {
    let attached = machine
        .attached_data
        .as_ref()
        .and_then(|name| {
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.name == *name)
        })
        .into_iter()
        .flat_map(|definition| program.data_members(definition).iter())
        .filter_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) => {
                Some((field.symbol, field.name.as_str(), field.type_reference))
            }
            psi_typed_trees::data::DataMember::Variant(_) => None,
        });
    attached
        .chain(
            program
                .machine_owned_data(machine)
                .iter()
                .map(|owned| (owned.symbol, owned.name.as_str(), owned.type_reference)),
        )
        .collect()
}
