use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind, SymbolTableAppender, SymbolTableBuilder};

use super::insert_machine_type_parameter_children;
use crate::symbols::symbol_table::names::{symbol_seed, type_parameter_seeds};

pub(in crate::symbols::symbol_table) fn insert_data_symbol_children(
    builder: &mut impl SymbolTableAppender,
    program: &SymbolResolvedTrees,
    data_symbol: SymbolHandle,
    data_definition: &symbol_resolved_trees::data::DataDefinition,
    has_sources: bool,
) {
    let data_children =
        builder.insert_children(
            data_symbol,
            type_parameter_seeds(
                program.data_type_parameters(data_definition.type_parameters),
                false,
                has_sources,
            )
            .chain(program.data_members(data_definition.members).iter().map(
                |member| match member {
                    symbol_resolved_trees::data::DataMember::Field(field) => {
                        symbol_seed(SymbolKind::Field, &field.name, has_sources)
                    }
                    symbol_resolved_trees::data::DataMember::Variant(variant) => {
                        symbol_seed(SymbolKind::Variant, &variant.name, has_sources)
                    }
                },
            )),
        );

    let mut data_children = SymbolTableBuilder::child_handles(data_children);
    insert_machine_type_parameter_children(
        builder,
        program,
        program.data_type_parameters(data_definition.type_parameters),
        &mut data_children,
        has_sources,
    );
    for member in program.data_members(data_definition.members) {
        let Some(member_symbol) = data_children.next() else {
            break;
        };
        let symbol_resolved_trees::data::DataMember::Variant(variant) = member else {
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
