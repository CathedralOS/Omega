use psi_symbol_resolved_trees::{SymbolResolvedTrees, types::TypeReference};
use psi_symbols::SymbolHandle;

pub(super) fn all_instances_reachable_from_ordinary_data(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    instance_symbols: &[SymbolHandle],
) -> bool {
    let mut reachable = Vec::with_capacity(instance_symbols.len());
    for definition in source
        .data_definitions
        .iter()
        .skip(data_frontier)
        .filter(|definition| definition.generic_instance.is_none())
    {
        collect_member_references(source, definition, instance_symbols, &mut reachable);
    }
    let mut cursor = 0;
    while cursor < reachable.len() {
        let symbol = reachable[cursor];
        cursor += 1;
        let Some(definition) = source
            .data_definitions
            .iter()
            .find(|definition| definition.symbol == symbol)
        else {
            return false;
        };
        collect_member_references(source, definition, instance_symbols, &mut reachable);
    }
    instance_symbols
        .iter()
        .all(|symbol| reachable.contains(symbol))
}

fn collect_member_references(
    source: &SymbolResolvedTrees,
    definition: &psi_symbol_resolved_trees::data::DataDefinition,
    instance_symbols: &[SymbolHandle],
    reachable: &mut Vec<SymbolHandle>,
) {
    for member in source.data_members(definition.members) {
        match member {
            psi_symbol_resolved_trees::data::DataMember::Field(field) => {
                collect_type_references(source, &field.type_reference, instance_symbols, reachable);
            }
            psi_symbol_resolved_trees::data::DataMember::Variant(variant) => {
                for field in source.data_payload_fields(variant.payload) {
                    collect_type_references(
                        source,
                        &field.type_reference,
                        instance_symbols,
                        reachable,
                    );
                }
            }
        }
    }
}

fn collect_type_references(
    source: &SymbolResolvedTrees,
    type_reference: &TypeReference,
    instance_symbols: &[SymbolHandle],
    reachable: &mut Vec<SymbolHandle>,
) {
    match type_reference {
        TypeReference::Named {
            symbol: candidate, ..
        } => {
            if instance_symbols.contains(candidate) && !reachable.contains(candidate) {
                reachable.push(*candidate);
            }
        }
        TypeReference::Reference(reference) => collect_type_references(
            source,
            source.child_type_reference(reference.referee),
            instance_symbols,
            reachable,
        ),
        TypeReference::Slice(slice) => collect_type_references(
            source,
            source.child_type_reference(slice.element_type),
            instance_symbols,
            reachable,
        ),
        TypeReference::FixedArray(array) => collect_type_references(
            source,
            source.child_type_reference(array.element_type),
            instance_symbols,
            reachable,
        ),
        TypeReference::Generic(generic) => {
            if instance_symbols.contains(&generic.base_symbol)
                && !reachable.contains(&generic.base_symbol)
            {
                reachable.push(generic.base_symbol);
            }
            for argument in source.child_type_references(generic.arguments) {
                collect_type_references(source, argument, instance_symbols, reachable);
            }
        }
        TypeReference::Constrained(constrained) => collect_type_references(
            source,
            source.child_type_reference(constrained.base_type),
            instance_symbols,
            reachable,
        ),
        TypeReference::ConstExpression(_)
        | TypeReference::DynamicTrait { .. }
        | TypeReference::SelfType { .. }
        | TypeReference::Unit => {}
    }
}
