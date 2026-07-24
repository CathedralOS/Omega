//! Fail-closed fence for borrow-carrying values stored beyond one graph state.
//!
//! Local loan attribution is statement/state scoped today. Until the flow plan
//! propagates a persistent owner's loan through every outgoing transition and
//! rebases state-parameter roots on each edge, accepting one of these writes
//! would silently release the source loan at state exit.

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
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
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
                let Some((name, target_type)) =
                    persistent_target_type(program, &place, &persistent)
                else {
                    continue;
                };
                if !crate::borrow::view_link::returns_borrow(program, target_type) {
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
