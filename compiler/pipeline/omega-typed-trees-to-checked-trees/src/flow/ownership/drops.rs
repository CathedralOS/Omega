use super::*;

pub(in crate::flow) fn append_state_exit_drop_events(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    state: &omega_typed_trees::state::State,
) {
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .rev()
    {
        let StatementNode::LocalData(local_data) = statement else {
            continue;
        };

        if !type_requires_ownership(program, local_data.type_reference) {
            continue;
        }

        append_drop_event_for_place(
            program,
            ctx,
            CanonicalPlace {
                root: omega_facts::PlaceRoot::Symbol(local_data.symbol),
                segments: Vec::new(),
            },
            FlowOwnershipEventSource::StateExit,
        );
    }

    // An owned by-value parameter is storage this state received cleanup
    // responsibility for (moving a resource into a target state transfers the
    // obligation), so its exit-edge drop must be recorded here, not at the
    // call site. Parameters are created before any local, so with locals
    // dropping in reverse creation order the parameter obligations come last.
    // Borrowed parameters (`&T`/`&mut T`, including `self`) own nothing.
    for parameter in program.state_parameters(state).iter().rev() {
        if parameter.is_self || !type_requires_ownership(program, parameter.type_reference) {
            continue;
        }

        append_drop_event_for_place(
            program,
            ctx,
            CanonicalPlace {
                root: omega_facts::PlaceRoot::Symbol(parameter.symbol),
                segments: Vec::new(),
            },
            FlowOwnershipEventSource::StateExit,
        );
    }
}
