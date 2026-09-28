use super::{
    DomainConstraint, DomainConstraintSubject, FixedArrayLength, TypeConstraintNode,
    TypeReferenceNode, TypeReferenceTable,
};
use crate::expression::{ExpressionNode, ExpressionTable};
use crate::name::Identifier;
use symbols::SymbolHandle;

#[test]
fn qualified_result_lookup_preserves_the_exact_predicate_target() {
    use numerics::arithmetic::ArithmeticDomain;
    let mut types = TypeReferenceTable::new();
    let carrier = SymbolHandle::from_parts(1, 0);
    let base = types.insert(TypeReferenceNode::Named {
        symbol: carrier,
        name: Identifier::generated("u64"),
    });
    let bounds = types.insert_constraints([TypeConstraintNode::Range {
        minimum: crate::expression::ExpressionHandle::from_parts(1, 0),
        maximum: crate::expression::ExpressionHandle::from_parts(2, 0),
        end_inclusive: true,
    }]);
    let target = types.insert(TypeReferenceNode::Constrained {
        base_type: base,
        constraints: bounds,
    });
    let other_target = types.insert(TypeReferenceNode::Constrained {
        base_type: base,
        constraints: bounds,
    });
    let policy = types.insert_constraints([TypeConstraintNode::ArithmeticDomain(
        ArithmeticDomain::Wrapping,
    )]);
    let result = types.insert(TypeReferenceNode::Constrained {
        base_type: target,
        constraints: policy,
    });
    assert_eq!(
        types.find_policy_qualified_type_reference(target, ArithmeticDomain::Wrapping),
        Some(result)
    );
    for missing in [base, other_target, super::TypeReferenceHandle::invalid()] {
        assert!(
            types
                .find_policy_qualified_type_reference(missing, ArithmeticDomain::Wrapping)
                .is_none()
        );
    }
    assert!(
        types
            .find_policy_qualified_type_reference(target, ArithmeticDomain::Saturating)
            .is_none()
    );
    assert!(
        types
            .find_arithmetic_result_type_reference(carrier, ArithmeticDomain::Wrapping)
            .is_none()
    );
    assert_eq!(
        types
            .clone()
            .find_policy_qualified_type_reference(target, ArithmeticDomain::Wrapping),
        Some(result)
    );
}

#[test]
fn arithmetic_result_lookup_requires_exact_carrier_and_policy_only_shape() {
    use numerics::arithmetic::ArithmeticDomain;
    let mut types = TypeReferenceTable::new();
    let carrier = SymbolHandle::from_parts(1, 0);
    let base = types.insert(TypeReferenceNode::Named {
        symbol: carrier,
        name: Identifier::generated("u64"),
    });
    let constraints = types.insert_constraints([
        TypeConstraintNode::ArithmeticDomain(ArithmeticDomain::Wrapping),
        TypeConstraintNode::Range {
            minimum: crate::expression::ExpressionHandle::invalid(),
            maximum: crate::expression::ExpressionHandle::invalid(),
            end_inclusive: true,
        },
    ]);
    types.insert(TypeReferenceNode::Constrained {
        base_type: base,
        constraints,
    });
    assert_eq!(
        types.find_arithmetic_result_type_reference(carrier, ArithmeticDomain::Exact),
        Some(base)
    );
    assert!(
        types
            .find_arithmetic_result_type_reference(carrier, ArithmeticDomain::Wrapping)
            .is_none()
    );
    let constraints = types.insert_constraints([TypeConstraintNode::ArithmeticDomain(
        ArithmeticDomain::Wrapping,
    )]);
    let result = types.insert(TypeReferenceNode::Constrained {
        base_type: base,
        constraints,
    });
    assert_eq!(
        types.find_arithmetic_result_type_reference(carrier, ArithmeticDomain::Wrapping),
        Some(result)
    );
    assert!(
        types
            .find_arithmetic_result_type_reference(carrier, ArithmeticDomain::Saturating)
            .is_none()
    );
    assert!(
        types
            .find_arithmetic_result_type_reference(
                SymbolHandle::from_parts(2, 0),
                ArithmeticDomain::Wrapping
            )
            .is_none()
    );
    assert_eq!(
        types
            .clone()
            .find_arithmetic_result_type_reference(carrier, ArithmeticDomain::Wrapping),
        Some(result)
    );
}

#[test]
fn type_reference_table_copies_table_payloads_without_tree_roundtrip() {
    let mut source_expressions = ExpressionTable::new();
    let minimum = source_expressions.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(1),
    ));
    let maximum = source_expressions.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(8),
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
    let semantic_id = language_semantics::SemanticDomainId(9);
    let predicate_body = language_semantics::DomainPredicateBody::Present;
    let semantic_roles = language_semantics::DomainSemanticRoles {
        denotation_dimension: Some(semantic_id),
        arithmetic_policy: None,
    };
    let establishment_routes = vec![
        language_semantics::DomainEstablishmentRoute::CheckedRequirement {
            trait_definition: SymbolHandle::from_arena_index(13),
            requirement: SymbolHandle::from_arena_index(14),
        },
    ];
    let constraints = source_types.insert_constraints([
        TypeConstraintNode::Range {
            minimum,
            maximum,
            end_inclusive: true,
        },
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
            authored_selection: None,
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
