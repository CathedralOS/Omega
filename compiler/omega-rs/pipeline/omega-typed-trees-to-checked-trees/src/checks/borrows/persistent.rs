//! Fail-closed fence for borrow-carrying values stored beyond one graph state.
//!
//! Local loan attribution is statement/state scoped today. A borrow backed by
//! program-static storage needs no source loan, so literals and machine results
//! whose every value exit is likewise static may be stored persistently. Other
//! sources remain fenced until the flow plan propagates a persistent owner's
//! loan through every outgoing transition and rebases state-parameter roots on
//! each edge.

use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::statement::StatementNode;
use omega_typed_trees::types::TypeReferenceHandle;

pub(super) fn check_persistent_borrow_assignments(
    program: &omega_typed_trees::TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        let persistent = persistent_storage(program, machine);
        if persistent.is_empty() {
            continue;
        }

        for state in program.machine_states(machine) {
            let mut static_persistent_places = Vec::new();
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let StatementNode::Assignment(assignment) = statement else {
                    if matches!(statement, StatementNode::Call(_)) {
                        // Until call mutation summaries are joined here, do not
                        // carry a static-provenance shortcut across an opaque
                        // statement call that may replace persistent storage.
                        static_persistent_places.clear();
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
                let Some((name, target_type)) =
                    persistent_target_type(program, &place, &persistent)
                else {
                    continue;
                };
                if !crate::borrow::view_link::returns_borrow(program, target_type) {
                    continue;
                }
                let initializers = crate::borrow::borrow_initializer_expressions(
                    program,
                    target_type,
                    assignment.value,
                );
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
                        &static_persistent_places,
                    );

                static_persistent_places
                    .retain(|known| !place_contains_or_is_contained_by(known, &place));
                if has_only_static_sources {
                    static_persistent_places.push(place);
                    continue;
                }

                diagnostics.push(Diagnostic::error(format!(
                    "assignment stores a borrow-carrying value in persistent field `{name}` of \
                     machine `{}`; persistent loans must be propagated through graph-state \
                     transitions before this write can be admitted",
                    machine.name,
                )));
            }
        }
    }
}

fn source_is_known_static_persistent_place(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: omega_typed_trees::expression::ExpressionHandle,
    known_static: &[crate::flow::CanonicalPlace],
) -> bool {
    let Some(source) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    ) else {
        return false;
    };
    known_static
        .iter()
        .any(|known| place_is_prefix_of(known, &source))
}

fn place_contains_or_is_contained_by(
    left: &crate::flow::CanonicalPlace,
    right: &crate::flow::CanonicalPlace,
) -> bool {
    place_is_prefix_of(left, right) || place_is_prefix_of(right, left)
}

fn place_is_prefix_of(
    prefix: &crate::flow::CanonicalPlace,
    place: &crate::flow::CanonicalPlace,
) -> bool {
    prefix.root == place.root
        && prefix.segments.len() <= place.segments.len()
        && prefix
            .segments
            .iter()
            .zip(&place.segments)
            .all(|(left, right)| crate::flow::canonical_place_segments_equal(*left, *right))
}

fn is_state_independent_borrow_source(
    program: &omega_typed_trees::TypedTrees,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::String(_) => true,
        omega_typed_trees::expression::ExpressionNode::Cast(cast) => {
            is_state_independent_borrow_source(program, cast.value)
        }
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => {
            is_state_independent_borrow_source(program, binary.left)
                && is_state_independent_borrow_source(program, binary.right)
        }
        omega_typed_trees::expression::ExpressionNode::Call(call) => {
            let Some(state) = crate::semantic_calls::find_state(program, call.target_symbol) else {
                return false;
            };
            state_returns_only_static_borrows(program, state, &mut Vec::new())
        }
        omega_typed_trees::expression::ExpressionNode::ArrayLiteral(_)
        | omega_typed_trees::expression::ExpressionNode::Atomic(_)
        | omega_typed_trees::expression::ExpressionNode::Boolean(_)
        | omega_typed_trees::expression::ExpressionNode::Float(_)
        | omega_typed_trees::expression::ExpressionNode::Indexed(_)
        | omega_typed_trees::expression::ExpressionNode::Integer(_)
        | omega_typed_trees::expression::ExpressionNode::Member(_)
        | omega_typed_trees::expression::ExpressionNode::Mutable(_)
        | omega_typed_trees::expression::ExpressionNode::Name(_)
        | omega_typed_trees::expression::ExpressionNode::Range(_)
        | omega_typed_trees::expression::ExpressionNode::StructLiteral(_)
        | omega_typed_trees::expression::ExpressionNode::Unary(_)
        | omega_typed_trees::expression::ExpressionNode::ZeroValue(_) => false,
    }
}

fn state_returns_only_static_borrows(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
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
                omega_typed_trees::statement::TransitionTargetNode::Value(expression) => {
                    found_value_exit = true;
                    is_state_independent_borrow_source(program, *expression)
                }
                omega_typed_trees::statement::TransitionTargetNode::Named { path, .. } => {
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
                omega_typed_trees::statement::TransitionTargetNode::SelfTarget => false,
                omega_typed_trees::statement::TransitionTargetNode::Terminal => true,
            },
        );

    visiting.pop();
    all_static && found_value_exit
}

fn persistent_target_type<'program>(
    program: &omega_typed_trees::TypedTrees,
    place: &crate::flow::CanonicalPlace,
    persistent: &[(SymbolHandle, &'program str, TypeReferenceHandle)],
) -> Option<(&'program str, TypeReferenceHandle)> {
    if let omega_facts::PlaceRoot::Symbol(symbol) = place.root
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
            let omega_facts::PlaceSegment::Field { symbol } = segment else {
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
    program: &'program omega_typed_trees::TypedTrees,
    machine: &'program omega_typed_trees::machine::Machine,
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
            omega_typed_trees::data::DataMember::Field(field) => {
                Some((field.symbol, field.name.as_str(), field.type_reference))
            }
            omega_typed_trees::data::DataMember::Variant(_) => None,
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
