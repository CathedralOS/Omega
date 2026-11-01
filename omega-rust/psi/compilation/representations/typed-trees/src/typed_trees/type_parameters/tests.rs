use super::*;
use crate::data::{MachineParameterContract, TypeParameterKind};
use crate::name::Identifier;
use crate::signature::StateSignature;
use crate::{FrozenTypedTrees, TypedTrees};

fn parameter(symbol: SymbolHandle, name: &str, kind: TypeParameterKind) -> TypeParameter {
    TypeParameter {
        symbol,
        name: Identifier::generated(name),
        kind,
        bounds: Default::default(),
    }
}
fn callable(symbol: SymbolHandle, name: &str) -> TypeParameter {
    parameter(
        symbol,
        name,
        TypeParameterKind::Machine {
            contract: MachineParameterContract::Structural(StateSignature {
                name: Identifier::generated(name),
                ..Default::default()
            }),
        },
    )
}
fn assert_queries(program: &TypedTrees, symbols: &[SymbolHandle]) {
    for &symbol in symbols {
        let rows = program
            .data_type_parameters
            .iter()
            .filter(|(_, p)| p.symbol == symbol && symbol.is_valid())
            .map(|(_, p)| p)
            .collect::<Vec<_>>();
        assert_eq!(
            program.first_data_type_parameter_by_symbol(symbol),
            rows.first().copied()
        );
        assert_eq!(
            program.data_type_parameter_by_symbol(symbol),
            rows.last().copied()
        );
        // The independent original arena scan is the semantic oracle: skip
        // non-machine and unresolved nominal/identity rows, retaining order.
        let retained = rows.iter().find_map(|p| {
            let TypeParameterKind::Machine { contract } = &p.kind else {
                return None;
            };
            program.machine_parameter_contract_view(contract)
        });
        assert_eq!(
            program
                .retained_static_machine_contract(symbol)
                .map(|v| v.signature()),
            retained.map(|v| v.signature())
        );
    }
}

#[test]
fn contract_index_preserves_generations_duplicates_and_kind_selection() {
    let first = SymbolHandle::from_parts(7, 1);
    let next = SymbolHandle::from_parts(7, 2);
    let absent = SymbolHandle::from_parts(8, 1);
    let symbols = [first, next, absent, SymbolHandle::invalid()];
    let mut p = TypedTrees::default();
    assert_queries(&p, &symbols);
    for row in [
        parameter(first, "type", TypeParameterKind::Type),
        parameter(
            first,
            "identity",
            TypeParameterKind::Machine {
                contract: MachineParameterContract::RequirementIdentity,
            },
        ),
        parameter(
            first,
            "unresolved",
            TypeParameterKind::Machine {
                contract: MachineParameterContract::Nominal {
                    trait_definition: SymbolHandle::invalid(),
                    requirement: SymbolHandle::invalid(),
                    trait_arguments: vec![],
                    instantiated: None,
                },
            },
        ),
        callable(next, "generation-two"),
        callable(first, "first-contract"),
        callable(first, "later-contract"),
    ] {
        p.tables.data_type_parameters.insert(row);
        assert_queries(&p, &symbols);
    }
    assert_eq!(
        p.retained_static_machine_contract(first)
            .unwrap()
            .signature()
            .name
            .as_str(),
        "first-contract"
    );
    assert_eq!(
        p.retained_static_machine_contract(next)
            .unwrap()
            .signature()
            .name
            .as_str(),
        "generation-two"
    );
    assert_queries(&FrozenTypedTrees::from(p), &symbols);
}

#[test]
fn contract_index_follows_mutation_clone_and_equal_revision_replacement() {
    let first = SymbolHandle::from_parts(11, 1);
    let other = SymbolHandle::from_parts(12, 1);
    let symbols = [first, other];
    let mut p = TypedTrees::default();
    let handle = p
        .tables
        .data_type_parameters
        .insert(callable(first, "original"));
    assert_queries(&p, &symbols);
    let mut cloned = p.clone();
    cloned.tables.data_type_parameters.get_mut(handle).symbol = other;
    assert_queries(&cloned, &symbols);
    assert_queries(&p, &symbols);
    let mut replacement = Arena::default();
    replacement.insert(callable(other, "replacement"));
    assert_eq!(
        replacement.revision(),
        p.tables.data_type_parameters.revision()
    );
    *p.tables.data_type_parameters = replacement;
    assert_queries(&p, &symbols);
    let frozen = FrozenTypedTrees::from(p);
    assert_queries(&frozen, &symbols);
    let mut p = frozen.into_inner();
    p.tables.data_type_parameters.get_mut(handle).symbol = first;
    assert_queries(&p, &symbols);
    p.tables.data_type_parameters = cloned.tables.data_type_parameters.clone();
    assert_queries(&p, &symbols);
    assert_queries(&cloned, &symbols);
}
