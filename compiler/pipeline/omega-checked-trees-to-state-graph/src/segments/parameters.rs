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
        // DEVIRTUALIZE a `dyn Trait` param to its single concrete implementation:
        // resolve both the type symbol AND the type name to the impl's data type so
        // downstream resolution (which keys on type_symbol and matches attached
        // machines by type_name) dispatches `s.method()` like a normal call.
        let base_symbol = program.type_reference_symbol(parameter.type_reference);
        let impl_symbol = program.single_trait_impl_data_symbol(base_symbol);
        let type_symbol = impl_symbol.unwrap_or(base_symbol);
        let type_name = impl_symbol
            .and_then(|symbol| {
                program
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == symbol)
            })
            .map(|data| data.name.clone())
            .unwrap_or_else(|| {
                Identifier::generated(program.display_type_reference(parameter.type_reference))
            });

        state_graph.state_parameters.append_to_span(
            &mut parameters,
            StateParameterNode {
                symbol: parameter.symbol,
                name: parameter.name.clone(),
                type_reference: parameter.type_reference,
                type_symbol,
                type_name,
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
