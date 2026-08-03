//! Fail-closed fence for borrow-carrying values stored beyond one graph state.
//!
//! Local loan attribution is statement/state scoped today. A borrow backed by
//! program-static storage needs no source loan, so literals and machine results
//! whose every value exit is likewise static may be stored persistently. Other
//! sources remain fenced until the flow plan propagates a persistent owner's
//! loan through every outgoing transition and rebases state-parameter roots on
//! each edge.

use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::statement::StatementNode;
use psi_typed_trees::types::TypeReferenceHandle;

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
        let entry_fields =
            static_persistent_fields_at_state_entries(program, machine, &persistent, states);
        for (state, entry_fields) in states.iter().zip(entry_fields) {
            let (_, state_diagnostics) =
                analyze_persistent_state(program, machine, state, &persistent, &entry_fields, true);
            diagnostics.extend(state_diagnostics);
        }
    }
}

fn static_persistent_fields_at_state_entries(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    persistent: &[(SymbolHandle, &str, TypeReferenceHandle)],
    states: &[psi_typed_trees::state::State],
) -> Vec<Vec<SymbolHandle>> {
    let mut entries = vec![None::<Vec<SymbolHandle>>; states.len()];
    if !states.is_empty() {
        entries[0] = Some(Vec::new());
    }

    loop {
        let mut changed = false;
        for (state_index, state) in states.iter().enumerate() {
            let Some(entry) = entries[state_index].clone() else {
                continue;
            };
            let (exit, _) =
                analyze_persistent_state(program, machine, state, persistent, &entry, false);
            for target in state_transition_targets(program, state) {
                let Some(target_index) = states
                    .iter()
                    .position(|candidate| candidate.symbol == target)
                else {
                    continue;
                };
                let merged = entries[target_index].as_ref().map_or_else(
                    || exit.clone(),
                    |prior| {
                        prior
                            .iter()
                            .copied()
                            .filter(|field| exit.contains(field))
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

fn state_transition_targets(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
) -> Vec<SymbolHandle> {
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
            let psi_typed_trees::statement::TransitionTargetNode::Named { path, .. } =
                program.statement_table.transition_target(target)
            else {
                return None;
            };
            path.symbol.is_valid().then_some(path.symbol)
        })
        .collect()
}

fn analyze_persistent_state(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    persistent: &[(SymbolHandle, &str, TypeReferenceHandle)],
    entry_fields: &[SymbolHandle],
    report_diagnostics: bool,
) -> (Vec<SymbolHandle>, Vec<Diagnostic>) {
    let mut static_persistent_places = Vec::new();
    let mut static_persistent_fields = entry_fields.to_vec();
    let mut diagnostics = Vec::new();
    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let StatementNode::Assignment(assignment) = statement else {
            if matches!(statement, StatementNode::Call(_)) {
                // Until call mutation summaries are joined here, do not carry
                // static provenance across an opaque statement call.
                static_persistent_places.clear();
                static_persistent_fields.clear();
            }
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
        let Some((name, target_type)) = persistent_target_type(program, &place, persistent) else {
            static_persistent_places
                .retain(|known| !static_provenance_invalidated_by_mutation(program, known, &place));
            continue;
        };
        if !crate::borrow::view_link::returns_borrow(program, target_type) {
            static_persistent_places
                .retain(|known| !static_provenance_invalidated_by_mutation(program, known, &place));
            if let Some((field, _)) = target_field {
                static_persistent_fields.retain(|known| *known != field);
            }
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
                &static_persistent_fields,
                persistent,
            );

        // Assignment reads its source before replacing the target.
        static_persistent_places
            .retain(|known| !static_provenance_invalidated_by_mutation(program, known, &place));
        if let Some((field, _)) = target_field {
            static_persistent_fields.retain(|known| *known != field);
        }
        if has_only_static_sources {
            static_persistent_places.push(place);
            if let Some((field, tail)) = target_field
                && tail
                    == static_persistent_places
                        .last()
                        .expect("just inserted static place")
                        .segments
                        .len()
                && !static_persistent_fields.contains(&field)
            {
                static_persistent_fields.push(field);
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
    (static_persistent_fields, diagnostics)
}

fn source_is_known_static_persistent_place(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: psi_typed_trees::expression::ExpressionHandle,
    source_type: TypeReferenceHandle,
    known_static: &[crate::flow::CanonicalPlace],
    known_static_fields: &[SymbolHandle],
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

    if persistent_field_and_tail(&source, persistent)
        .is_some_and(|(field, _)| known_static_fields.contains(&field))
    {
        return true;
    }

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
        | psi_typed_trees::expression::ExpressionNode::Mutable(_)
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
