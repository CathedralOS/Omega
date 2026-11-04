use super::*;
use crate::data::{TypeParameter, TypeParameterKind};
use crate::name::Identifier;
use crate::types::{DomainConstraint, TypeConstraintNode, TypeReferenceNode};
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

#[test]
fn applied_domain_carrier_matches_its_exact_argument_telescope() {
    let mut symbols = SymbolTableBuilder::new();
    let pair = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Pair"));
    let other = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Other"));
    let first = symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("T"));
    let second = symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("U"));
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let operands = [first, second].map(|symbol| {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: Identifier::generated("parameter"),
            })
    });
    let parameters = program
        .data_type_parameters
        .insert_many([first, second].map(|symbol| TypeParameter {
            symbol,
            name: Identifier::generated("parameter"),
            kind: TypeParameterKind::Type,
            ..Default::default()
        }));
    let application =
        |program: &mut TypedTrees, base_symbol, arguments: &[types::TypeReferenceHandle]| {
            let arguments = program
                .type_reference_table
                .insert_type_reference_handles(arguments.iter().copied());
            program
                .type_reference_table
                .insert(TypeReferenceNode::Generic {
                    base_symbol,
                    base_name: Identifier::generated("Pair"),
                    lifetime_arguments: Vec::new(),
                    arguments,
                })
        };
    let target = application(&mut program, pair, &operands);
    let reversed = application(&mut program, pair, &[operands[1], operands[0]]);
    let unrelated = application(&mut program, other, &operands);
    let domain = domain::DomainDefinition {
        type_parameters: parameters,
        target_type: target,
        ..Default::default()
    };
    assert!(domain::domain_accepts_carrier(
        &program, &domain, target, &operands
    ));
    assert!(domain::domain_accepts_carrier(
        &program,
        &domain,
        reversed,
        &[operands[1], operands[0]]
    ));
    assert!(!domain::domain_accepts_carrier(
        &program, &domain, reversed, &operands
    ));
    assert!(!domain::domain_accepts_carrier(
        &program, &domain, unrelated, &operands
    ));
}

#[test]
fn applied_domain_constraint_keeps_arguments_identity_and_roles_together() {
    let mut symbols = SymbolTableBuilder::new();
    let domain_symbol = symbols.insert_root(SymbolKind::Domain, SymbolNameRef::Static("Marked"));
    let parameter = symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("T"));
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let carrier = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let operand = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: parameter,
            name: Identifier::generated("T"),
        });
    let parameters = program.data_type_parameters.insert_many([TypeParameter {
        symbol: parameter,
        name: Identifier::generated("T"),
        kind: TypeParameterKind::Type,
        ..Default::default()
    }]);
    let declaration_id = program.semantic_domains.intern("Marked");
    program.push_domain_definition(domain::DomainDefinition {
        symbol: domain_symbol,
        name: Identifier::generated("Marked"),
        type_parameters: parameters,
        target_type: carrier,
        semantic_id: declaration_id,
        ..Default::default()
    });
    let original = CallableApplication::domain_identity(
        &mut program,
        domain_symbol,
        &[operand],
        declaration_id,
    )
    .unwrap();
    let constraints =
        program
            .type_reference_table
            .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                symbol: domain_symbol,
                arguments: vec![operand],
                semantic_id: original,
                semantic_roles: language_semantics::DomainSemanticRoles {
                    denotation_dimension: Some(original),
                    arithmetic_policy: Some(original),
                },
                ..Default::default()
            })]);
    let source = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: carrier,
            constraints,
        });
    let applied = program
        .apply_callable_type(source, &[(parameter, carrier)], &[], parameter)
        .unwrap();
    let TypeReferenceNode::Constrained {
        constraints: applied_constraints,
        ..
    } = program.type_reference_node(applied)
    else {
        panic!("lost domain constraint");
    };
    let TypeConstraintNode::Domain(applied) = &program
        .type_reference_table
        .constraints(*applied_constraints)[0]
    else {
        panic!("lost domain instance");
    };
    let applied = applied.clone();
    let expected = CallableApplication::domain_identity(
        &mut program,
        domain_symbol,
        &[carrier],
        declaration_id,
    )
    .unwrap();
    assert_ne!(original, expected);
    assert_eq!(applied.arguments, [carrier]);
    assert_eq!(applied.semantic_id, expected);
    assert_eq!(applied.semantic_roles.denotation_dimension, Some(expected));
    assert_eq!(applied.semantic_roles.arithmetic_policy, Some(expected));
    let TypeConstraintNode::Domain(source) =
        &program.type_reference_table.constraints(constraints)[0]
    else {
        panic!("source declaration changed kind");
    };
    assert_eq!(source.arguments, [operand]);
    assert_eq!(
        source.semantic_id, original,
        "application must not mutate its source contract"
    );
}
