use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbols::{SymbolHandle, SymbolKind, SymbolTableBuilder};

use super::insert_machine_parameter_signature_children;
use crate::symbols::symbol_table::names::symbol_seed;

pub(in crate::symbols::symbol_table) fn insert_data_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    data_symbol: SymbolHandle,
    data_definition: &psi_symbol_resolved_trees::data::DataDefinition,
    has_sources: bool,
) {
    let data_children = builder.insert_children(
        data_symbol,
        program
            .data_type_parameters(data_definition.type_parameters)
            .iter()
            .map(|parameter| {
                let kind = match parameter.kind {
                    psi_symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                        SymbolKind::MachineParameter
                    }
                    _ => SymbolKind::TypeParameter,
                };
                symbol_seed(kind, &parameter.name, has_sources)
            })
            .chain(
                program
                    .data_members(data_definition.members)
                    .iter()
                    .filter_map(|member| match member {
                        psi_symbol_resolved_trees::data::DataMember::Field(field) => {
                            Some(symbol_seed(SymbolKind::Field, &field.name, has_sources))
                        }
                        psi_symbol_resolved_trees::data::DataMember::Variant(variant) => {
                            Some(symbol_seed(SymbolKind::Variant, &variant.name, has_sources))
                        }
                    }),
            ),
    );

    let mut data_children = SymbolTableBuilder::child_handles(data_children);
    for parameter in program.data_type_parameters(data_definition.type_parameters) {
        let parameter_symbol = data_children.next();
        if let (
            Some(parameter_symbol),
            psi_symbol_resolved_trees::data::TypeParameterKind::Machine { contract },
        ) = (parameter_symbol, &parameter.kind)
            && let Some(contract) = contract.structural()
        {
            insert_machine_parameter_signature_children(
                builder,
                program,
                parameter_symbol,
                contract,
                has_sources,
            );
        }
    }
    for member in program.data_members(data_definition.members) {
        let Some(member_symbol) = data_children.next() else {
            break;
        };
        let psi_symbol_resolved_trees::data::DataMember::Variant(variant) = member else {
            continue;
        };
        builder.insert_children(
            member_symbol,
            program
                .data_payload_fields(variant.payload)
                .iter()
                .map(|field| symbol_seed(SymbolKind::Field, &field.name, has_sources)),
        );
    }
}
