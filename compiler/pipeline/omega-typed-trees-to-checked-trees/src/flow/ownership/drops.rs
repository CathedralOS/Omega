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
            ctx,
            CanonicalPlace {
                root: omega_facts::PlaceRoot::Symbol(local_data.symbol),
                segments: Vec::new(),
            },
            FlowOwnershipEventSource::StateExit,
        );
    }
}
