use crate::TypedTrees;
use crate::data::{DataDefinition, TypeParameter, TypeParameterKind};
use crate::expression::ExpressionNode;
use crate::name::Identifier;
use crate::trait_definition::Conformance;
use crate::type_identity::TypeIdentityRequest;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::{IntegerLanding, IntegerLiteral, LandedIntegerType};
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};

fn named(program: &mut TypedTrees, symbol: SymbolHandle, name: &str) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol,
            name: Identifier::generated(name),
        })
}

fn application(
    program: &mut TypedTrees,
    symbol: SymbolHandle,
    name: &str,
    arguments: Vec<TypeReferenceHandle>,
) -> TypeReferenceHandle {
    let arguments = program
        .type_reference_table
        .insert_type_reference_handles(arguments);
    program
        .type_reference_table
        .insert(TypeReferenceNode::Generic {
            base_symbol: symbol,
            base_name: Identifier::generated(name),
            lifetime_arguments: Vec::new(),
            arguments,
        })
}

#[test]
fn ordinary_nested_implementation_const_roles_preserve_values_and_policy_identity() {
    let mut symbols = SymbolTableBuilder::default();
    let u64_symbol = symbols.insert_root(SymbolKind::BuiltinType, SymbolNameRef::Static("u64"));
    let payload = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Payload"));
    let policy = symbols.insert_root(SymbolKind::Conformance, SymbolNameRef::Static("Policy"));
    let other_policy = symbols.insert_root(
        SymbolKind::Conformance,
        SymbolNameRef::Static("OtherPolicy"),
    );
    let storage = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Storage"));
    let indexed = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Indexed"));
    let policy_slot = symbols
        .insert_children(
            policy,
            [(SymbolKind::TypeParameter, SymbolNameRef::Static("N"))],
        )
        .handles()
        .next()
        .unwrap();
    let other_slot = symbols
        .insert_children(
            other_policy,
            [(SymbolKind::TypeParameter, SymbolNameRef::Static("N"))],
        )
        .handles()
        .next()
        .unwrap();
    let storage_slots = symbols
        .insert_children(
            storage,
            [
                (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                (SymbolKind::TypeParameter, SymbolNameRef::Static("Policy")),
            ],
        )
        .handles()
        .collect::<Vec<_>>();
    let value_slot = symbols
        .insert_children(
            indexed,
            [(SymbolKind::TypeParameter, SymbolNameRef::Static("Subject"))],
        )
        .handles()
        .next()
        .unwrap();
    let caller = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("caller"));
    let subjects = symbols
        .insert_children(
            caller,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("left")),
                (SymbolKind::Parameter, SymbolNameRef::Static("right")),
            ],
        )
        .handles()
        .collect::<Vec<_>>();
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..TypedTrees::default()
    };
    let u64_type = named(&mut program, u64_symbol, "u64");
    let payload_type = named(&mut program, payload, "Payload");
    program.push_data_definition(DataDefinition {
        symbol: payload,
        name: Identifier::generated("Payload"),
        ..DataDefinition::default()
    });
    for (symbol, slot, name) in [
        (policy, policy_slot, "Policy"),
        (other_policy, other_slot, "OtherPolicy"),
    ] {
        let type_parameters = program.data_type_parameters.insert_many([TypeParameter {
            symbol: slot,
            name: Identifier::generated("N"),
            kind: TypeParameterKind::Const {
                type_reference: u64_type,
            },
            ..TypeParameter::default()
        }]);
        program.push_conformance(Conformance {
            symbol,
            alias: Some(Identifier::generated(name)),
            type_parameters,
            ..Conformance::default()
        });
        assert!(matches!(
            program.declaration_type_parameters(symbol)[0].kind,
            TypeParameterKind::Const { .. }
        ));
    }
    let type_parameters = program.data_type_parameters.insert_many([
        TypeParameter {
            symbol: storage_slots[0],
            name: Identifier::generated("T"),
            kind: TypeParameterKind::Type,
            ..TypeParameter::default()
        },
        TypeParameter {
            symbol: storage_slots[1],
            name: Identifier::generated("Policy"),
            kind: TypeParameterKind::Implementation,
            ..TypeParameter::default()
        },
    ]);
    program.push_data_definition(DataDefinition {
        symbol: storage,
        name: Identifier::generated("Storage"),
        type_parameters,
        ..DataDefinition::default()
    });

    // These are the actual two representations: specialization's remembered
    // integer type and an authored, landed const expression in Policy<N>.
    let remembered_seven = named(&mut program, SymbolHandle::invalid(), "7");
    let integer = program.expression_table.insert(ExpressionNode::Integer(
        IntegerLiteral::from_value(7).with_landing(IntegerLanding {
            landed_type: LandedIntegerType::U64,
            domain: ArithmeticDomain::Exact,
        }),
    ));
    let authored_seven = program
        .type_reference_table
        .insert(TypeReferenceNode::ConstExpression(integer));
    let eight = named(&mut program, SymbolHandle::invalid(), "8");
    let remembered_policy = application(&mut program, policy, "Policy", vec![remembered_seven]);
    let authored_policy = application(&mut program, policy, "Policy", vec![authored_seven]);
    let wrong_value_policy = application(&mut program, policy, "Policy", vec![eight]);
    let wrong_declaration_policy = application(
        &mut program,
        other_policy,
        "OtherPolicy",
        vec![remembered_seven],
    );
    let remembered = application(
        &mut program,
        storage,
        "Storage",
        vec![payload_type, remembered_policy],
    );
    let authored = application(
        &mut program,
        storage,
        "Storage",
        vec![payload_type, authored_policy],
    );
    let wrong_value = application(
        &mut program,
        storage,
        "Storage",
        vec![payload_type, wrong_value_policy],
    );
    let wrong_declaration = application(
        &mut program,
        storage,
        "Storage",
        vec![payload_type, wrong_declaration_policy],
    );
    let ordinary = |program: &TypedTrees, reference| {
        program.type_identity(TypeIdentityRequest::ordinary(reference))
    };
    assert_ne!(
        ordinary(&program, remembered),
        ordinary(&program, wrong_value)
    );
    assert_ne!(
        ordinary(&program, remembered),
        ordinary(&program, wrong_declaration)
    );

    // An independent runtime Value coordinate is still a source identity.
    // Only storage-plan identity may erase it after proving field independence.
    let type_parameters = program.data_type_parameters.insert_many([TypeParameter {
        symbol: value_slot,
        name: Identifier::generated("Subject"),
        kind: TypeParameterKind::Value {
            type_reference: u64_type,
        },
        ..TypeParameter::default()
    }]);
    program.push_data_definition(DataDefinition {
        symbol: indexed,
        name: Identifier::generated("Indexed"),
        type_parameters,
        ..DataDefinition::default()
    });
    let left = named(&mut program, subjects[0], "left");
    let right = named(&mut program, subjects[1], "right");
    let left = application(&mut program, indexed, "Indexed", vec![left]);
    let right = application(&mut program, indexed, "Indexed", vec![right]);
    assert_ne!(ordinary(&program, left), ordinary(&program, right));
    assert_eq!(
        program.storage_type_identity(TypeIdentityRequest::ordinary(left)),
        program.storage_type_identity(TypeIdentityRequest::ordinary(right))
    );

    let borrow = |program: &mut TypedTrees, referee| {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee,
                access: language_semantics::ReferenceAccess::Shared,
                lifetime: None,
            })
    };
    let remembered_borrow = borrow(&mut program, remembered);
    let authored_borrow = borrow(&mut program, authored);
    let actual = program.type_identity_request_text(TypeIdentityRequest::ordinary(remembered));
    let expected = program.type_identity_request_text(TypeIdentityRequest::ordinary(authored));
    eprintln!("specialized carrier: {actual}\nauthored carrier: {expected}");
    let same_carrier = ordinary(&program, remembered) == ordinary(&program, authored);
    let same_borrow = ordinary(&program, remembered_borrow) == ordinary(&program, authored_borrow);
    assert!(
        same_carrier && same_borrow,
        "same declaration-owned Const value must preserve ordinary and borrowed carrier identity\nspecialized: {actual}\nauthored: {expected}"
    );
}
