use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolKind, SymbolTable};

use crate::symbols::top_level::{SignatureSymbolTables, next_child_of_kind};

pub(super) fn assign_data_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
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
                SignatureSymbolTables {
                    symbols,
                    data_type_parameters,
                    state_parameters,
                    child_type_references,
                    type_constraints,
                }
                .assign_parameter_contract_symbols(
                    &mut contract,
                    parameter_symbol,
                    &local_type_parameters,
                );
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
            data_definition.predicate_subject_symbol = if data_definition.where_facts.is_empty() {
                symbols::SymbolHandle::invalid()
            } else {
                next_child_of_kind(&mut data_children, symbols, SymbolKind::Parameter)
            };
            data_definition.predicate_subject_type = if !data_definition.predicate_subject_symbol.is_valid() {
                symbol_resolved_trees::types::TypeReference::Unit
            } else if let Some(origin) = &data_definition.generic_instance {
                origin.clone()
            } else if local_type_parameters.is_empty() {
                symbol_resolved_trees::types::TypeReference::Named {
                    symbol: data_symbol,
                    name: symbol_resolved_trees::name::DiagnosticName::generated(data_definition.name.as_str()),
                    supplied_operand_owner: Default::default(),
                }
            } else {
                let arguments = local_type_parameters.iter().map(|parameter| {
                    symbol_resolved_trees::types::TypeReference::Named {
                        symbol: parameter.symbol,
                        name: symbol_resolved_trees::name::DiagnosticName::generated(parameter.name.as_str()),
                        supplied_operand_owner: Default::default(),
                    }
                }).collect::<Vec<_>>();
                let span = match &data_definition.predicate_subject_type {
                    symbol_resolved_trees::types::TypeReference::Generic(previous)
                        if child_type_references.span_or_empty(previous.arguments) == arguments.as_slice() => previous.arguments,
                    _ => child_type_references.insert_many(arguments),
                };
                symbol_resolved_trees::types::TypeReference::Generic(symbol_resolved_trees::types::GenericTypeReference {
                    storage: symbol_resolved_trees::types::GenericTypeReferenceStorage {
                        base_symbol: data_symbol,
                        base_name: symbol_resolved_trees::name::DiagnosticName::generated(data_definition.name.as_str()),
                        lifetime_arguments: data_definition.lifetime_parameters.clone(),
                        arguments: span,
                        ..Default::default()
                    },
                })
            };
        });
}
