use super::*;
use crate::source_state::data::{TypeParameter, TypeParameterKind};
use crate::source_state::name::Identifier;
use crate::source_state::types::{DomainConstraint, TypeConstraintNode, TypeReferenceNode};
use crate::source_symbols::SymbolTableBuilder;
use symbols::{SymbolKind, SymbolNameRef};

#[test]
fn domain_application_lookup_is_exact_and_rebuilt_from_stored_rows() {
    use record_storage::{RecordLimits, decode, encode};
    use semantic_vocabulary::SemanticDomainId;

    let mut applications = domain::AppliedDomainApplications::default();
    for (declaration, identity) in [
        (SymbolHandle::from_parts(7, 2), SemanticDomainId(42)),
        (SymbolHandle::from_parts(7, 3), SemanticDomainId(42)),
        (SymbolHandle::from_parts(7, 2), SemanticDomainId(41)),
    ] {
        assert!(applications.insert(domain::AppliedDomainApplication {
            declaration,
            identity,
            arguments: Vec::new(),
            is_closed: true,
            theory: Some(Err(format!("refusal for {}", identity.0))),
        }));
    }
    let rows = applications.iter().cloned().collect::<Vec<_>>();
    assert!(!applications.insert(rows[0].clone()));
    let bytes = encode(&applications, RecordLimits::default()).unwrap();
    let restored: domain::AppliedDomainApplications =
        decode(&bytes, RecordLimits::default()).unwrap();
    assert_eq!(restored.iter().cloned().collect::<Vec<_>>(), rows);
    for row in restored.iter() {
        assert_eq!(restored.find(row.declaration, row.identity), Some(row));
    }
    assert!(
        restored
            .find(rows[0].declaration, SemanticDomainId(40))
            .is_none()
    );
    let malformed = encode(
        &vec![rows[0].clone(), rows[0].clone()],
        RecordLimits::default(),
    )
    .unwrap();
    let error = decode::<domain::AppliedDomainApplications>(&malformed, RecordLimits::default())
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("duplicate exact domain application"),
        "{error}"
    );
}

#[test]
fn a_generic_domain_carrier_is_its_binder_not_a_matching_diagnostic_name() {
    let mut symbols = SymbolTableBuilder::new();
    let nominal = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("T"));
    let binder = symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("Binder"));
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let parameters = program.data_type_parameters.insert_many([TypeParameter {
        symbol: binder,
        name: Identifier::generated("T"),
        kind: TypeParameterKind::Type,
        ..Default::default()
    }]);
    let mut definition = domain::DomainDefinition {
        type_parameters: parameters,
        ..Default::default()
    };
    for (symbol, expected) in [
        (binder, true),
        (nominal, false),
        (SymbolHandle::invalid(), false),
    ] {
        definition.target_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: Identifier::generated("T"),
            });
        assert_eq!(domain::has_generic_carrier(&program, &definition), expected);
    }
}

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
    let original = program
        .retain_domain_application(domain_symbol, carrier, &[operand])
        .unwrap();
    let constraints =
        program
            .type_reference_table
            .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                symbol: domain_symbol,
                arguments: vec![operand],
                semantic_id: original,
                semantic_roles: source_declarations::DomainSemanticRoles {
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
    let expected = program
        .retain_domain_application(domain_symbol, carrier, &[carrier])
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
