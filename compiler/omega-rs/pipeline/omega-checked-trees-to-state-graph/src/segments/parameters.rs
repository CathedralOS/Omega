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
        // With MULTIPLE impls the param keeps the trait symbol/name and instead
        // records every impl's data type name -- the trait's closed world -- so a
        // method call through it can be monomorphized per call site (the
        // receiver's static type at each site picks the impl).
        let base_symbol = program.type_reference_symbol(parameter.type_reference);
        let impl_symbols = program.trait_impl_data_symbols(base_symbol);
        let impl_symbol = match impl_symbols.as_slice() {
            [single] => Some(*single),
            _ => None,
        };
        let type_symbol = impl_symbol.unwrap_or(base_symbol);
        let data_name_for = |symbol: omega_core::symbols::SymbolHandle| {
            program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == symbol)
                .map(|data| data.name.clone())
        };
        let type_name = impl_symbol.and_then(data_name_for).unwrap_or_else(|| {
            Identifier::generated(program.display_type_reference(parameter.type_reference))
        });
        let dyn_impl_type_names = if impl_symbols.len() > 1 {
            impl_symbols
                .iter()
                .filter_map(|symbol| data_name_for(*symbol))
                .collect()
        } else {
            Vec::new()
        };

        state_graph.state_parameters.append_to_span(
            &mut parameters,
            StateParameterNode {
                symbol: parameter.symbol,
                name: parameter.name.clone(),
                type_reference: parameter.type_reference,
                type_symbol,
                type_name,
                dyn_impl_type_names,
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
