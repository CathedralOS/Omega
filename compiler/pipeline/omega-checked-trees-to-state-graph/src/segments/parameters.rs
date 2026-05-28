use omega_checked_trees::CheckedTrees;
use omega_checked_trees::name::Identifier;
use omega_checked_trees::state::State;
use omega_core::arena::HandleSpan;
use omega_state_graph::{StateGraph, StateParameterNode};

pub(super) fn state_parameters_for_segment(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    state: &State,
    segment_index: usize,
) -> HandleSpan<StateParameterNode> {
    if segment_index > 0 {
        return HandleSpan::empty();
    }

    let mut parameters = HandleSpan::empty();
    for parameter in program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
    {
        state_graph.state_parameters.append_to_span(
            &mut parameters,
            StateParameterNode {
                symbol: parameter.symbol,
                name: parameter.name.clone(),
                type_reference: parameter.type_reference,
                type_symbol: program.type_reference_symbol(parameter.type_reference),
                type_name: Identifier::generated(
                    program.display_type_reference(parameter.type_reference),
                ),
                is_mutable_reference: matches!(
                    program
                        .type_reference_table
                        .type_reference(parameter.type_reference),
                    omega_checked_trees::types::TypeReferenceNode::Reference {
                        is_mutable: true,
                        ..
                    }
                ),
            },
        );
    }

    parameters
}
