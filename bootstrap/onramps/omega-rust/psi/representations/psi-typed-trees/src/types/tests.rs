use super::{
    DomainConstraint, DomainConstraintSubject, FixedArrayLength, OmegaLayoutGrammar, PrimitiveType,
    TypeConstraintNode, TypeReferenceNode, TypeReferenceTable,
};
use crate::expression::{ExpressionNode, ExpressionTable};
use crate::name::Identifier;
use psi_symbols::SymbolHandle;

#[test]
fn type_reference_table_stores_nested_typed_references_as_handles() {
    let mut types = TypeReferenceTable::new();
    let usize_reference = types.insert(TypeReferenceNode::Named {
        symbol: SymbolHandle::invalid(),
        name: Identifier::generated("usize"),
    });
    let u8_reference = types.insert(TypeReferenceNode::Named {
        symbol: SymbolHandle::invalid(),
        name: Identifier::generated("u8"),
    });
    let fixed_array_reference = types.insert(TypeReferenceNode::FixedArray {
        element_type: u8_reference,
        length: FixedArrayLength::Literal(16),
    });
    let arguments = types.insert_type_reference_handles([usize_reference, fixed_array_reference]);
    let root = types.insert(TypeReferenceNode::Generic {
        base_symbol: SymbolHandle::invalid(),
        base_name: Identifier::generated("Result"),
        lifetime_arguments: Vec::new(),
        arguments,
    });

    assert_eq!(types.type_reference_count(), 4);
    let TypeReferenceNode::Generic { arguments, .. } = types.type_reference(root) else {
        panic!("root type reference should be generic");
    };

    assert_eq!(arguments.count(), 2);
    assert_eq!(types.display_name(root), "Result<usize, [u8; 16]>");
}

#[test]
fn type_reference_table_stores_typed_constraints_as_expression_handles() {
    let mut expressions = ExpressionTable::new();
    let minimum = expressions.insert(ExpressionNode::Integer(
        psi_numerics::literals::IntegerLiteral::from_value(0),
    ));
    let maximum = expressions.insert(ExpressionNode::Integer(
        psi_numerics::literals::IntegerLiteral::from_value(10),
    ));
    let mut types = TypeReferenceTable::new();
    let base_type = types.insert(TypeReferenceNode::Named {
        symbol: SymbolHandle::invalid(),
        name: Identifier::generated("i32"),
    });
    let constraints = types.insert_constraints([TypeConstraintNode::Range { minimum, maximum }]);
    let root = types.insert(TypeReferenceNode::Constrained {
        base_type,
        constraints,
    });

    assert_eq!(types.type_reference_count(), 2);
    assert_eq!(expressions.expression_count(), 2);

    let TypeReferenceNode::Constrained { constraints, .. } = types.type_reference(root) else {
        panic!("root type reference should be constrained");
    };
    let [TypeConstraintNode::Range { minimum, maximum }] = types.constraints(*constraints) else {
        panic!("expected one range constraint");
    };

    assert!(minimum.is_valid());
    assert!(maximum.is_valid());
    assert_eq!(
        types.display_name_with_constraints(root, &expressions),
        "i32[0..=10]"
    );
    assert_eq!(types.primitive_type(root), Some(PrimitiveType::I32));
    assert_eq!(types.type_symbol(root), SymbolHandle::invalid());
}

#[test]
fn type_reference_table_copies_table_payloads_without_tree_roundtrip() {
    let mut source_expressions = ExpressionTable::new();
    let minimum = source_expressions.insert(ExpressionNode::Integer(
        psi_numerics::literals::IntegerLiteral::from_value(1),
    ));
    let maximum = source_expressions.insert(ExpressionNode::Integer(
        psi_numerics::literals::IntegerLiteral::from_value(8),
    ));
    let mut source_types = TypeReferenceTable::new();
    let u8_reference = source_types.insert(TypeReferenceNode::Named {
        symbol: SymbolHandle::from_arena_index(11),
        name: Identifier::generated("u8"),
    });
    let fixed_array_reference = source_types.insert(TypeReferenceNode::FixedArray {
        element_type: u8_reference,
        length: FixedArrayLength::Literal(8),
    });
    let domain_symbol = SymbolHandle::from_arena_index(12);
    let semantic_id = psi_language_semantics::SemanticDomainId(9);
    let predicate_body = psi_language_semantics::DomainPredicateBody::Present;
    let semantic_roles = psi_language_semantics::DomainSemanticRoles {
        denotation_dimension: Some(semantic_id),
        arithmetic_policy: None,
    };
    let establishment_routes = vec![
        psi_language_semantics::DomainEstablishmentRoute::CheckedRequirement {
            trait_definition: SymbolHandle::from_arena_index(13),
            requirement: SymbolHandle::from_arena_index(14),
        },
    ];
    let constraints = source_types.insert_constraints([
        TypeConstraintNode::Range { minimum, maximum },
        TypeConstraintNode::Domain(DomainConstraint {
            name: Identifier::generated("Utf8"),
            arguments: Vec::new(),
            subject: DomainConstraintSubject::Declared,
            symbol: domain_symbol,
            semantic_id,
            classification: None,
            predicate_body,
            semantic_roles,
            establishment_routes: establishment_routes.clone(),
        }),
    ]);
    let source_root = source_types.insert(TypeReferenceNode::Constrained {
        base_type: fixed_array_reference,
        constraints,
    });

    let mut copied_expressions = ExpressionTable::new();
    let mut copied_types = TypeReferenceTable::new();
    let copied_root = copied_types.copy_from(
        &source_types,
        &source_expressions,
        &mut copied_expressions,
        source_root,
    );

    assert_eq!(
        copied_types.display_name_with_constraints(copied_root, &copied_expressions),
        "[u8; 8][1..=8, in Utf8]"
    );
    assert_eq!(
        copied_types.type_reference_count(),
        source_types.type_reference_count()
    );
    assert_eq!(
        copied_expressions.expression_count(),
        source_expressions.expression_count()
    );
    let TypeReferenceNode::Constrained { constraints, .. } =
        copied_types.type_reference(copied_root)
    else {
        panic!("copied constrained type")
    };
    let [_, TypeConstraintNode::Domain(copied_domain)] = copied_types.constraints(*constraints)
    else {
        panic!("copied normalized domain constraint")
    };
    assert_eq!(copied_domain.symbol, domain_symbol);
    assert_eq!(copied_domain.subject, DomainConstraintSubject::Declared);
    assert_eq!(copied_domain.semantic_id, semantic_id);
    assert_eq!(copied_domain.predicate_body, predicate_body);
    assert_eq!(copied_domain.semantic_roles, semantic_roles);
    assert_eq!(copied_domain.establishment_routes, establishment_routes);
}

#[test]
fn type_reference_table_copy_preserves_closed_domain_subjects_and_arguments() {
    let source_expressions = ExpressionTable::new();
    let mut source_types = TypeReferenceTable::new();
    let byte = source_types.insert(TypeReferenceNode::Named {
        symbol: SymbolHandle::invalid(),
        name: Identifier::generated("u8"),
    });
    let carrier = source_types.insert(TypeReferenceNode::Slice { element_type: byte });
    let schema_symbol = SymbolHandle::from_arena_index(21);
    let schema = source_types.insert(TypeReferenceNode::Named {
        symbol: schema_symbol,
        name: Identifier::generated("Save"),
    });
    let constraints =
        source_types.insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
            name: Identifier::generated("display only"),
            arguments: vec![schema],
            subject: DomainConstraintSubject::OmegaLayout {
                grammar: OmegaLayoutGrammar::Derived,
            },
            ..DomainConstraint::default()
        })]);
    let source_root = source_types.insert(TypeReferenceNode::Constrained {
        base_type: carrier,
        constraints,
    });

    let mut copied_expressions = ExpressionTable::new();
    let mut copied_types = TypeReferenceTable::new();
    let copied_root = copied_types.copy_from(
        &source_types,
        &source_expressions,
        &mut copied_expressions,
        source_root,
    );
    let TypeReferenceNode::Constrained { constraints, .. } =
        copied_types.type_reference(copied_root)
    else {
        panic!("copied constrained type")
    };
    let [TypeConstraintNode::Domain(copied)] = copied_types.constraints(*constraints) else {
        panic!("copied layout constraint")
    };

    assert_eq!(
        copied.subject,
        DomainConstraintSubject::OmegaLayout {
            grammar: OmegaLayoutGrammar::Derived,
        }
    );
    let [copied_schema] = copied.arguments.as_slice() else {
        panic!("copied schema argument")
    };
    assert!(matches!(
        copied_types.type_reference(*copied_schema),
        TypeReferenceNode::Named { symbol, name }
            if *symbol == schema_symbol && name.as_str() == "Save"
    ));
}

#[test]
fn type_reference_symbol_remap_reaches_nested_types_and_constraints() {
    let old = SymbolHandle::from_arena_index(41);
    let new = SymbolHandle::from_arena_index(42);
    let mut expressions = ExpressionTable::new();
    let mut members = psi_arena::HandleSpan::empty();
    expressions.push_name_path_member(&mut members, Identifier::generated("n"));
    let mut member_symbols = psi_arena::HandleSpan::empty();
    expressions.push_name_path_member_symbol(&mut member_symbols, old);
    let subject = expressions.insert(ExpressionNode::Name(crate::expression::TableNamePath {
        members,
        member_symbols,
        head_symbol: old,
        symbol: old,
    }));

    let mut types = TypeReferenceTable::new();
    let element = types.insert(TypeReferenceNode::Named {
        symbol: old,
        name: Identifier::generated("Element"),
    });
    let array = types.insert(TypeReferenceNode::FixedArray {
        element_type: element,
        length: FixedArrayLength::ConstParameter {
            symbol: old,
            name: Identifier::generated("n"),
        },
    });
    let arguments = types.insert_type_reference_handles([array]);
    let generic = types.insert(TypeReferenceNode::Generic {
        base_symbol: old,
        base_name: Identifier::generated("Box"),
        lifetime_arguments: Vec::new(),
        arguments,
    });
    let constraints = types.insert_constraints([TypeConstraintNode::Range {
        minimum: subject,
        maximum: subject,
    }]);
    let root = types.insert(TypeReferenceNode::Constrained {
        base_type: generic,
        constraints,
    });

    types.remap_symbols_in(root, &mut expressions, &[(old, new)]);

    let TypeReferenceNode::Generic { base_symbol, .. } = types.type_reference(generic) else {
        panic!("expected generic type");
    };
    assert_eq!(*base_symbol, new);
    let TypeReferenceNode::FixedArray { length, .. } = types.type_reference(array) else {
        panic!("expected fixed array");
    };
    assert!(matches!(
        length,
        FixedArrayLength::ConstParameter { symbol, .. } if *symbol == new
    ));
    let TypeReferenceNode::Named { symbol, .. } = types.type_reference(element) else {
        panic!("expected named element type");
    };
    assert_eq!(*symbol, new);
    let ExpressionNode::Name(path) = expressions.expression(subject) else {
        panic!("expected constraint subject name");
    };
    assert_eq!(path.head_symbol, new);
    assert_eq!(path.symbol, new);
    assert_eq!(
        expressions.name_path_member_symbols(path.member_symbols),
        &[new]
    );
}
