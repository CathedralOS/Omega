//! Declaration headers, not a shared spelling or orphan arena row, own contracts.

use super::*;
use symbols::{SymbolNameRef, SymbolTableBuilder};
use typed_trees::{
    TypedTrees, data::DataDefinition, name::Identifier, type_aliases::TypeAliasDefinition,
};

fn callable(
    symbol: SymbolHandle,
    nested: arena::HandleSpan<TypeParameter>,
    suspends: bool,
) -> TypeParameter {
    TypeParameter {
        symbol,
        name: Identifier::generated("Selected"),
        kind: TypeParameterKind::Machine {
            contract: MachineParameterContract::Structural(
                typed_trees::signature::StateSignature {
                    symbol,
                    name: Identifier::generated("Selected"),
                    type_parameters: nested,
                    suspends,
                    ..Default::default()
                },
            ),
        },
        ..Default::default()
    }
}

#[test]
fn alias_and_data_contracts_rejoin_full_owned_binders_and_nested_telescopes() {
    for alias in [false, true] {
        let mut builder = SymbolTableBuilder::default();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let kind = if alias {
            SymbolKind::TypeAlias
        } else {
            SymbolKind::Data
        };
        let owners = builder.insert_children(
            root,
            [
                (kind, SymbolNameRef::Static("Owner")),
                (kind, SymbolNameRef::Static("Other")),
            ],
        );
        let owner = owners.handle_at(0);
        let other = owners.handle_at(1);
        let selected = builder
            .insert_children(
                owner,
                [(
                    SymbolKind::MachineParameter,
                    SymbolNameRef::Static("Selected"),
                )],
            )
            .start();
        let nested = builder
            .insert_children(
                selected,
                [(SymbolKind::MachineParameter, SymbolNameRef::Static("Inner"))],
            )
            .start();
        let foreign = builder
            .insert_children(
                other,
                [(
                    SymbolKind::MachineParameter,
                    SymbolNameRef::Static("Selected"),
                )],
            )
            .start();
        let mut program = TypedTrees::default();
        program.symbols = builder.finish();
        let inner = program.tables.data_type_parameters.insert_many([callable(
            nested,
            arena::HandleSpan::empty(),
            false,
        )]);
        let own = program
            .tables
            .data_type_parameters
            .insert_many([callable(selected, inner, false)]);
        // This row exists in the shared arena but no declaration owns it yet.
        let orphan = program.tables.data_type_parameters.insert_many([callable(
            foreign,
            arena::HandleSpan::empty(),
            true,
        )]);
        let publish = |program: &mut TypedTrees, symbol, parameters| {
            if alias {
                program.push_type_alias_definition(TypeAliasDefinition {
                    symbol,
                    type_parameters: parameters,
                    ..Default::default()
                });
            } else {
                program.push_data_definition(DataDefinition {
                    symbol,
                    type_parameters: parameters,
                    ..Default::default()
                });
            }
        };
        publish(&mut program, owner, own);
        for symbol in [selected, nested] {
            let (parameter, contract) =
                machine_parameter_contract_definition(&program, symbol).expect("owned callable");
            assert_eq!(parameter.symbol, symbol);
            let MachineParameterContract::Structural(signature) = contract else {
                panic!("structural contract");
            };
            assert_eq!(signature.symbol, symbol);
            assert!(!signature.suspends);
        }
        assert!(machine_parameter_contract_definition(&program, foreign).is_none());
        assert!(machine_parameter_contract_definition(&program, SymbolHandle::invalid()).is_none());
        publish(&mut program, other, orphan);
        let (parameter, contract) = machine_parameter_contract_definition(&program, foreign)
            .expect("separately owned same-spelled callable");
        assert_eq!(parameter.symbol, foreign);
        let MachineParameterContract::Structural(signature) = contract else {
            panic!("structural contract");
        };
        assert!(signature.suspends);
    }
}
