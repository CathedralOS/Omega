use omega_state_graph::{StateGraph, StateParameterNode};
use psi_arena::HandleSpan;
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::name::Identifier;
use psi_checked_trees::state::State;

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
        let dyn_conformance_rows =
            selected_dynamic_conformance_rows(program, parameter.type_reference);
        // DEVIRTUALIZE a `dyn Trait` param to its single concrete implementation:
        // resolve both the type symbol AND the type name to the impl's data type so
        // downstream resolution (which keys on type_symbol and matches attached
        // machines by type_name) dispatches `s.method()` like a normal call.
        // With MULTIPLE impls the param keeps the trait symbol/name and instead
        // records every impl's data type name -- the trait's closed world -- so a
        // method call through it can be monomorphized per call site (the
        // receiver's static type at each site picks the impl).
        let base_symbol = program.type_reference_symbol(parameter.type_reference);
        let impl_symbols = if dyn_conformance_rows.is_empty() {
            program.trait_impl_data_symbols(base_symbol)
        } else {
            Vec::new()
        };
        let impl_symbol = match impl_symbols.as_slice() {
            [single] => Some(*single),
            _ => None,
        };
        let type_symbol = impl_symbol.unwrap_or(base_symbol);
        let data_name_for = |symbol: psi_symbols::SymbolHandle| {
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
                dyn_conformance_rows,
                is_mutable_reference: matches!(
                    program
                        .type_reference_table
                        .type_reference(parameter.type_reference),
                    psi_checked_trees::types::TypeReferenceNode::Reference {
                        is_mutable: true,
                        ..
                    }
                ),
            },
        );
    }

    parameters
}

fn selected_dynamic_conformance_rows(
    program: &CheckedTrees,
    type_reference: psi_checked_trees::types::TypeReferenceHandle,
) -> Vec<psi_checked_trees::DynamicConformanceRowFact> {
    let Some(conformance_symbol) = dynamic_conformance_symbol(program, type_reference) else {
        return Vec::new();
    };
    let Some(conformance) = program
        .data_conformances()
        .iter()
        .find(|conformance| conformance.symbol == conformance_symbol)
    else {
        return Vec::new();
    };
    program
        .closed_conformance_rows(conformance)
        .unwrap_or_default()
        .iter()
        .filter(|row| row.realization_machine.is_valid() && row.realization_state.is_valid())
        .map(|row| psi_checked_trees::DynamicConformanceRowFact {
            declaring_trait: row.declaring_trait,
            requirement: row.requirement,
            realization_machine: row.realization_machine,
            realization_state: row.realization_state,
            source: match row.source {
                psi_checked_trees::trait_definition::ConformanceRowSource::Inline => {
                    psi_checked_trees::DynamicConformanceRowSource::Inline
                }
                psi_checked_trees::trait_definition::ConformanceRowSource::Reference => {
                    psi_checked_trees::DynamicConformanceRowSource::Reference
                }
                psi_checked_trees::trait_definition::ConformanceRowSource::TraitDefault => {
                    psi_checked_trees::DynamicConformanceRowSource::TraitDefault
                }
            },
        })
        .collect()
}

fn dynamic_conformance_symbol(
    program: &CheckedTrees,
    type_reference: psi_checked_trees::types::TypeReferenceHandle,
) -> Option<psi_symbols::SymbolHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        psi_checked_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            dynamic_conformance_symbol(program, *referee)
        }
        psi_checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            dynamic_conformance_symbol(program, *base_type)
        }
        psi_checked_trees::types::TypeReferenceNode::DynamicTrait { conformance, .. } => {
            *conformance
        }
        _ => None,
    }
}
