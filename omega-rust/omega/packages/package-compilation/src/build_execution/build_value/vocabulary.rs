// Exact toolchain build vocabulary queries: canonical `build` machine identity and prelude facets.
//
// `is_build_machine` verifies that a `build` machine symbol originates from the exact
// companion `build.omg` `SourceId` selected by project discovery, rather than trusting
// a filename scan or machine name alone. `has_exact_toolchain_build_facet` checks that
// a facet data definition belongs to the compiler's build prelude.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;

const BUILD_MACHINE: &str = "build";

pub(crate) fn is_build_machine(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    build_source_id: Option<source::SourceId>,
) -> bool {
    if machine.name.as_str() != BUILD_MACHINE {
        return false;
    }
    let Some(build_source_id) = build_source_id else {
        return false;
    };
    typed
        .symbols
        .symbol_source_span(machine.symbol)
        .is_some_and(|span| span.source_id == build_source_id)
}

pub(super) fn has_exact_toolchain_build_facet(typed: &TypedTrees, name: &str) -> bool {
    typed.data_definitions().iter().any(|definition| {
        definition.name.as_str() == name && typed.symbols.is_build_prelude_symbol(definition.symbol)
    })
}

fn named_data_symbol(
    typed: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<SymbolHandle> {
    match typed.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            named_data_symbol(typed, *referee)
        }
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            named_data_symbol(typed, *base_type)
        }
        typed_trees::types::TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        _ => None,
    }
}

pub(super) fn build_machine_declares_identifier_field(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> bool {
    typed
        .machine_states(machine)
        .iter()
        .flat_map(|state| typed.state_parameters(state).iter())
        .filter_map(|parameter| named_data_symbol(typed, parameter.type_reference))
        .filter_map(|symbol| typed.data_definition_by_symbol(symbol))
        .filter(|definition| definition.name.as_str() == "Build")
        .flat_map(|definition| typed.data_members(definition).iter())
        .any(|member| {
            matches!(
                member,
                typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == "identifier"
            )
        })
}
