use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTableBuilder};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

use super::insert_machine_parameter_signature_children;
use crate::symbols::symbol_table::names::symbol_seed;

pub(in crate::symbols::symbol_table) fn insert_data_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    data_symbol: SymbolHandle,
    data_definition: &omega_symbol_resolved_trees::data::DataDefinition,
    has_sources: bool,
) {
    let data_children = builder.insert_children(
        data_symbol,
        program
            .data_type_parameters(data_definition.type_parameters)
            .iter()
            .map(|parameter| {
                let kind = match parameter.kind {
                    omega_symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                        SymbolKind::MachineParameter
                    }
                    _ => SymbolKind::TypeParameter,
                };
                symbol_seed(kind, &parameter.name, has_sources)
            })
            .chain(program.data_members(data_definition.members).iter().map(
                |member| match member {
                    omega_symbol_resolved_trees::data::DataMember::Field(field) => {
                        symbol_seed(SymbolKind::Field, &field.name, has_sources)
                    }
                    omega_symbol_resolved_trees::data::DataMember::Variant(variant) => {
                        symbol_seed(SymbolKind::Variant, &variant.name, has_sources)
                    }
                },
            )),
    );

    let mut data_children = SymbolTableBuilder::child_handles(data_children);
    for parameter in program.data_type_parameters(data_definition.type_parameters) {
        let parameter_symbol = data_children.next();
        if let (
            Some(parameter_symbol),
            omega_symbol_resolved_trees::data::TypeParameterKind::Machine { contract },
        ) = (parameter_symbol, &parameter.kind)
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
}
