use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

use crate::symbols::top_level::next_child_of_kind;
use crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_self_type;

pub(super) fn assign_data_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    root_children: &mut impl Iterator<Item = SymbolHandle>,
) {
    let data_type_parameters = &mut program.tables.declarations.data_type_parameters;
    let data_members = &mut program.tables.declarations.data_members;
    let state_parameters = &mut program.tables.declarations.state_parameters;
    let child_type_references = &mut program.tables.declarations.child_type_references;
    program
        .roots
        .data_definitions
        .for_each_mut(|data_definition| {
            data_definition.symbol = next_child_of_kind(root_children, symbols, SymbolKind::Data);
            let data_symbol = data_definition.symbol;
            let mut data_children = symbols.child_handles(data_symbol).into_iter().flatten();

            for type_parameter in
                data_type_parameters.span_mut_or_empty(data_definition.type_parameters)
            {
                let kind = match type_parameter.kind {
                    omega_symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
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
            for type_parameter in
                data_type_parameters.span_mut_or_empty(data_definition.type_parameters)
            {
                let omega_symbol_resolved_trees::data::TypeParameterKind::Machine { contract } =
                    &mut type_parameter.kind
                else {
                    continue;
                };
                contract.symbol = type_parameter.symbol;
                let mut contract_children = symbols
                    .child_handles(type_parameter.symbol)
                    .into_iter()
                    .flatten();
                for parameter in state_parameters.span_mut_or_empty(contract.parameters) {
                    parameter.symbol =
                        next_child_of_kind(&mut contract_children, symbols, SymbolKind::Parameter);
                    assign_type_reference_symbol_with_locals_and_self_type(
                        symbols,
                        child_type_references,
                        &local_type_parameters,
                        data_symbol,
                        &mut parameter.type_reference,
                    );
                }
                if let Some(return_type) = &mut contract.return_type {
                    assign_type_reference_symbol_with_locals_and_self_type(
                        symbols,
                        child_type_references,
                        &local_type_parameters,
                        data_symbol,
                        return_type,
                    );
                }
            }

            for member in data_members.span_mut_or_empty(data_definition.members) {
                match member {
                    omega_symbol_resolved_trees::data::DataMember::Field(field) => {
                        field.symbol =
                            next_child_of_kind(&mut data_children, symbols, SymbolKind::Field);
                    }
                    omega_symbol_resolved_trees::data::DataMember::Variant(variant) => {
                        variant.symbol =
                            next_child_of_kind(&mut data_children, symbols, SymbolKind::Variant);
                    }
                }
            }
        });
}
