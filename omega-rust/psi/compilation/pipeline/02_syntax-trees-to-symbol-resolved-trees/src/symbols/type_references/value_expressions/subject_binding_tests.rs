//! Alias arguments bind source subjects under the same frontier as data.

use super::*;
use crate::symbols::scope::{LexicalRequirements, attached_machines};
use source::SourceSpan;
use symbol_resolved_trees::{
    SymbolResolvedTrees,
    data::{DataDefinition, DataDefinitionStorage, TypeParameter, TypeParameterKind},
    statement::{LocalData, LocalDataStorage, Statement},
    type_aliases::TypeAliasDefinition,
    types::{GenericTypeReference, GenericTypeReferenceStorage},
};
use symbols::{SymbolNameRef, SymbolTableBuilder};

fn name(value: &str) -> DiagnosticName {
    DiagnosticName::from_str(value, SourceSpan::default())
}

fn named(symbol: SymbolHandle, value: &str) -> TypeReference {
    TypeReference::Named {
        supplied_operand_owner: Default::default(),
        symbol,
        name: name(value),
    }
}

#[test]
fn alias_subjects_use_exact_headers_roles_and_lexical_frontiers() {
    // Data is the independent existing control; alias missing/duplicate headers
    // and another declaration with the same leaf name cannot supply its roles.
    for (data, copies, other_alias) in [
        (true, 1, false),
        (false, 0, false),
        (false, 1, false),
        (false, 2, false),
        (false, 1, true),
    ] {
        let mut builder = SymbolTableBuilder::default();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let roots = builder.insert_children(
            root,
            symbols::builtin_type_symbols().into_iter().chain([
                (SymbolKind::Data, SymbolNameRef::Static("Record")),
                (SymbolKind::TypeAlias, SymbolNameRef::Static("Alias")),
                (SymbolKind::Module, SymbolNameRef::Static("other")),
                (SymbolKind::Const, SymbolNameRef::Static("index")),
            ]),
        );
        let u64_symbol = roots.handle_at(symbols::BuiltinTypeAtom::U64.ordinal() as u32);
        let record = roots.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
        let alias = roots.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 1);
        let other = roots.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 2);
        let descendants = builder.insert_children(
            other,
            [
                (SymbolKind::TypeAlias, SymbolNameRef::Static("Alias")),
                (SymbolKind::Machine, SymbolNameRef::Static("owner")),
            ],
        );
        let absent_alias = descendants.handle_at(0);
        let owner = descendants.handle_at(1);
        let state = builder
            .insert_children(owner, [(SymbolKind::State, SymbolNameRef::Static("entry"))])
            .start();
        let subjects = builder.insert_children(
            state,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("index")),
                (SymbolKind::Local, SymbolNameRef::Static("index")),
            ],
        );
        let parameter = subjects.handle_at(0);
        let local = subjects.handle_at(1);
        let mut header_symbols = Vec::new();
        for declaration in [record, alias] {
            header_symbols.push(builder.insert_children(
                declaration,
                [
                    (
                        SymbolKind::TypeParameter,
                        SymbolNameRef::Static("ValueSlot"),
                    ),
                    (
                        SymbolKind::TypeParameter,
                        SymbolNameRef::Static("ConstSlot"),
                    ),
                    (SymbolKind::TypeParameter, SymbolNameRef::Static("TypeSlot")),
                ],
            ));
        }
        let symbols = builder.finish();
        let mut program = SymbolResolvedTrees::default();
        program.symbols = symbols.clone();
        let carrier = named(u64_symbol, "u64");
        let mut spans = Vec::new();
        for symbols in header_symbols {
            spans.push(
                program
                    .tables
                    .declarations
                    .data_type_parameters
                    .insert_many([
                        TypeParameter {
                            symbol: symbols.handle_at(0),
                            name: name("ValueSlot"),
                            kind: TypeParameterKind::Value {
                                type_reference: carrier.clone(),
                            },
                            ..Default::default()
                        },
                        TypeParameter {
                            symbol: symbols.handle_at(1),
                            name: name("ConstSlot"),
                            kind: TypeParameterKind::Const {
                                type_reference: carrier.clone(),
                            },
                            ..Default::default()
                        },
                        TypeParameter {
                            symbol: symbols.handle_at(2),
                            name: name("TypeSlot"),
                            ..Default::default()
                        },
                    ]),
            );
        }
        program.roots.data_definitions.push(DataDefinition {
            symbol: record,
            name: name("Record"),
            storage: DataDefinitionStorage {
                type_parameters: spans[0],
                ..Default::default()
            },
            ..Default::default()
        });
        for _ in 0..copies {
            program.roots.type_aliases.push(TypeAliasDefinition {
                symbol: alias,
                name: name("Alias"),
                type_parameters: spans[1],
                ..Default::default()
            });
        }
        let attached = attached_machines(&program);
        let parameter = StateParameter {
            symbol: parameter,
            name: name("index"),
            type_reference: carrier.clone(),
            ..Default::default()
        };
        let prior = [Statement::LocalData(LocalData {
            symbol: local,
            name: name("index"),
            storage: LocalDataStorage {
                type_reference: carrier.clone(),
                ..Default::default()
            },
        })];
        for has_prior in [false, true] {
            let arguments = program
                .tables
                .declarations
                .child_type_references
                .insert_many([
                    named(SymbolHandle::invalid(), "index"),
                    named(SymbolHandle::invalid(), "index"),
                    named(SymbolHandle::invalid(), "index"),
                ]);
            let target = if data {
                record
            } else if other_alias {
                absent_alias
            } else {
                alias
            };
            let mut application = TypeReference::Generic(GenericTypeReference {
                storage: GenericTypeReferenceStorage {
                    base_symbol: target,
                    base_name: name(if data { "Record" } else { "Alias" }),
                    arguments,
                    ..Default::default()
                },
            });
            let scope = MachineScope {
                symbol: owner,
                lexical_requirements: LexicalRequirements::none(),
                attached_machines: &attached,
                resolution_indexes: &program.resolution_indexes,
                type_parameters: &[],
                attached_data_symbol: SymbolHandle::invalid(),
                inherited_data_members: None,
                owned_data: &[],
                prior_statements: if has_prior { &prior } else { &[] },
                data_definitions: &program.roots.data_definitions,
                type_aliases: &program.roots.type_aliases,
                conformances: &program.roots.conformances,
                operand_owners: &program.tables.types.operand_lifetime_owners,
                data_members: &program.tables.declarations.data_members,
                data_type_parameters: &program.tables.declarations.data_type_parameters,
                data_payload_fields: &program.tables.declarations.data_payload_fields,
                type_constraints: &program.tables.types.constraints,
            };
            assign_type_value_expression_symbols(
                &symbols,
                &scope,
                std::slice::from_ref(&parameter),
                state,
                &mut program.tables.bodies.expressions,
                &mut program.tables.declarations.child_type_references,
                &mut application,
            );
            let expected = if data || (copies == 1 && !other_alias) {
                if has_prior { local } else { parameter.symbol }
            } else {
                SymbolHandle::invalid()
            };
            for (ordinal, expected) in [expected, expected, SymbolHandle::invalid()]
                .into_iter()
                .enumerate()
            {
                assert!(
                    matches!(program.tables.declarations.child_type_references.get(arguments.handle_at(ordinal as u32)),
                    TypeReference::Named { symbol, .. } if *symbol == expected)
                );
            }
        }
    }
}
