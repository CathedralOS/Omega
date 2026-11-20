//! Identify compiler-owned data declarations in the admitted Build prelude.
use source_compiler::source_state::TypedTrees;
use symbols::SymbolHandle;

pub(crate) fn is_exact_toolchain_build_prelude_data(
    typed: &TypedTrees,
    symbol: SymbolHandle,
    expected_name: &str,
) -> bool {
    typed.is_exact_toolchain_build_prelude_data(symbol, expected_name)
}

pub(super) fn has_exact_toolchain_build_facet(typed: &TypedTrees, name: &str) -> bool {
    typed.data_definitions().iter().any(|definition| {
        definition.name.as_str() == name && typed.symbols.is_build_prelude_symbol(definition.symbol)
    })
}

fn named_data_symbol(
    typed: &TypedTrees,
    type_reference: source_compiler::source_state::types::TypeReferenceHandle,
) -> Option<SymbolHandle> {
    match typed.type_reference_table.type_reference(type_reference) {
        source_compiler::source_state::types::TypeReferenceNode::Reference { referee, .. } => {
            named_data_symbol(typed, *referee)
        }
        source_compiler::source_state::types::TypeReferenceNode::Constrained {
            base_type, ..
        } => named_data_symbol(typed, *base_type),
        source_compiler::source_state::types::TypeReferenceNode::Named { symbol, .. } => {
            Some(*symbol)
        }
        _ => None,
    }
}

pub(super) fn build_machine_declares_identifier_field(
    typed: &TypedTrees,
    machine: &source_compiler::source_state::machine::Machine,
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
                source_compiler::source_state::data::DataMember::Field(field)
                    if field.name.as_str() == "identifier"
            )
        })
}
