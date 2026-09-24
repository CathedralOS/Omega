use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::top_level::{assign_machine_parameter_signature_symbols, next_child_of_kind};

pub(super) fn assign_data_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    root_children: &mut impl Iterator<Item = SymbolHandle>,
) {
    let type_constraints = &program.tables.types.constraints;
    let data_type_parameters = &mut program.tables.declarations.data_type_parameters;
    let data_members = &mut program.tables.declarations.data_members;
    let data_payload_fields = &mut program.tables.declarations.data_payload_fields;
    let state_parameters = &mut program.tables.declarations.state_parameters;
    let child_type_references = &mut program.tables.declarations.child_type_references;
    program
        .roots
        .data_definitions
        .for_each_mut(|data_definition| {
            if !data_definition.symbol.is_valid() {
                data_definition.symbol =
                    next_child_of_kind(root_children, symbols, SymbolKind::Data);
            }
            let data_symbol = data_definition.symbol;
            let mut data_children = symbols.child_handles(data_symbol).into_iter().flatten();

            for type_parameter in
                data_type_parameters.span_mut_or_empty(data_definition.type_parameters)
            {
                let kind = match type_parameter.kind {
                    symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                        SymbolKind::MachineParameter
                    }
                    _ => SymbolKind::TypeParameter,
                };
                type_parameter.symbol = next_child_of_kind(&mut data_children, symbols, kind);
            }
            let local_type_parameters = data_type_parameters
                .span_or_empty(data_definition.type_parameters)
                .to_vec();

            // N7: data-family machine contracts participate in symbol identity
            // exactly like machine-template contracts. Their value parameters
            // are children of the static parameter and their types resolve in
            // the data family's generic context.
            for index in 0..data_definition.type_parameters.len() {
                let (parameter_symbol, kind) = {
                    let parameter =
                        &data_type_parameters.span_or_empty(data_definition.type_parameters)[index];
                    (parameter.symbol, parameter.kind.clone())
                };
                let symbol_resolved_trees::data::TypeParameterKind::Machine { mut contract } = kind
                else {
                    continue;
                };
                if let Some(signature) = contract.structural_mut() {
                    assign_machine_parameter_signature_symbols(
                        symbols,
                        data_type_parameters,
                        state_parameters,
                        child_type_references,
                        type_constraints,
                        signature,
                        parameter_symbol,
                        &local_type_parameters,
                        data_symbol,
                    );
                }
                data_type_parameters.span_mut_or_empty(data_definition.type_parameters)[index]
                    .kind = symbol_resolved_trees::data::TypeParameterKind::Machine { contract };
            }

            for member in data_members.span_mut_or_empty(data_definition.members) {
                match member {
                    symbol_resolved_trees::data::DataMember::Field(field) => {
                        field.symbol =
                            next_child_of_kind(&mut data_children, symbols, SymbolKind::Field);
                    }
                    symbol_resolved_trees::data::DataMember::Variant(variant) => {
                        variant.symbol =
                            next_child_of_kind(&mut data_children, symbols, SymbolKind::Variant);
                        let mut variant_children =
                            symbols.child_handles(variant.symbol).into_iter().flatten();
                        for field in data_payload_fields.span_mut_or_empty(variant.payload) {
                            field.symbol = next_child_of_kind(
                                &mut variant_children,
                                symbols,
                                SymbolKind::Field,
                            );
                        }
                    }
                }
            }
        });
}
