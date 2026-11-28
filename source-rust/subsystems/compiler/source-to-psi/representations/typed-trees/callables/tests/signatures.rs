use super::*;
use crate::source_state::{name::Identifier, types::TypeReferenceNode};
use crate::source_symbols::SymbolTableBuilder;
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef};

#[test]
fn applied_signature_binders_keep_their_owner_and_independent_substitutions() {
    let mut symbols = SymbolTableBuilder::new();
    let declared_owner = symbols.insert_root(SymbolKind::State, SymbolNameRef::Static("bind"));
    let children = symbols.insert_children(
        declared_owner,
        [
            (SymbolKind::TypeParameter, SymbolNameRef::Static("Locator")),
            (SymbolKind::Parameter, SymbolNameRef::Static("locator")),
        ],
    );
    let locator_type = children.handle_at(0);
    let locator_value = children.handle_at(1);
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: locator_type,
            name: Identifier::generated("Locator"),
        });
    let declared = signature::StateSignature {
        symbol: declared_owner,
        type_parameters: program
            .data_type_parameters
            .insert_many([data::TypeParameter {
                symbol: locator_type,
                name: Identifier::generated("Locator"),
                ..Default::default()
            }]),
        parameters: program
            .state_parameters
            .insert_many([signature::StateParameter {
                symbol: locator_value,
                name: Identifier::generated("locator"),
                type_reference: reference,
                is_const: false,
                is_mutable: false,
                relevance: language_core::BindingRelevance::Relevant,
            }]),
        return_type: reference,
        ..Default::default()
    };
    let mut previous_type = None;
    for generated_owner in [false, false, true] {
        let mut application = CallableApplication::default();
        let owner = if generated_owner {
            let owner = program.symbols.insert_generated_root_from(
                declared_owner,
                SymbolKind::State,
                "$applied",
            );
            application.symbols.push((declared_owner, owner));
            application.owned_signature = owner;
            owner
        } else {
            declared_owner
        };
        let applied = application.signature(&mut program, &declared).unwrap();
        let applied_type = program.state_signature_type_parameters(&applied)[0].symbol;
        let applied_value = &program.state_signature_parameters(&applied)[0];
        assert_eq!(applied.symbol, owner);
        assert_ne!(applied_type, locator_type);
        assert_ne!(Some(applied_type), previous_type);
        previous_type = Some(applied_type);
        for (fresh, original) in [
            (applied_type, locator_type),
            (applied_value.symbol, locator_value),
        ] {
            assert_eq!(program.symbols.get(fresh).parent, owner);
            assert_eq!(program.symbols.get(fresh).generated_from, original);
        }
        for reference in [applied_value.type_reference, applied.return_type] {
            assert!(matches!(
                program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Named { symbol, .. } if *symbol == applied_type
            ));
        }
        assert_eq!(program.symbols.get(declared_owner).children, children);
        assert_eq!(program.symbols.get(locator_type).parent, declared_owner);
    }
    let invalid = signature::StateSignature {
        symbol: SymbolHandle::invalid(),
        ..declared
    };
    assert_eq!(
        CallableApplication::default().signature(&mut program, &invalid),
        Err("applied callable signature has no retained declaration owner")
    );
}
