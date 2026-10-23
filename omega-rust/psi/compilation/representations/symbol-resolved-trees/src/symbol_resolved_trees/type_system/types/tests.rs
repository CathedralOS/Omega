use super::{
    ConstrainedTypeReference, ConstrainedTypeReferenceStorage, FixedArrayLength,
    FixedArrayTypeReference, FixedArrayTypeReferenceStorage, GenericTypeReference,
    GenericTypeReferenceStorage, TypeConstraint, TypeConstraintNode, TypeReference,
    TypeReferenceNode, TypeReferenceTable,
};
use crate::expression::ExpressionTable;
use crate::name::DiagnosticName;
use arena::Arena;
use symbols::SymbolHandle;

#[test]
fn type_reference_table_stores_nested_typed_references_as_handles() {
    let mut source_arguments = Arena::<TypeReference>::new();
    let fixed_array_element = source_arguments.append(TypeReference::Named {
        supplied_operand_owner: Default::default(),
        symbol: SymbolHandle::invalid(),
        name: DiagnosticName::generated("u8"),
    });
    let arguments = source_arguments.insert_many([
        TypeReference::Named {
            supplied_operand_owner: Default::default(),
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::generated("usize"),
        },
        TypeReference::FixedArray(FixedArrayTypeReference {
            storage: FixedArrayTypeReferenceStorage {
                supplied_operand_owner: Default::default(),
                element_type: fixed_array_element,
                length: FixedArrayLength::Literal(16),
            },
        }),
    ]);
    let type_reference = TypeReference::Generic(GenericTypeReference {
        storage: GenericTypeReferenceStorage {
            supplied_operand_owner: Default::default(),
            base_symbol: SymbolHandle::invalid(),
            base_name: DiagnosticName::generated("Result"),
            lifetime_arguments: Vec::new(),
            arguments,
        },
    });

    let source_constraints = Arena::<TypeConstraint>::new();
    let source_expressions = ExpressionTable::new();
    let mut expressions = ExpressionTable::new();
    let mut types = TypeReferenceTable::new();
    let root = types.insert_tree(
        &type_reference,
        &mut expressions,
        &source_arguments,
        &source_constraints,
        &source_expressions,
    );

    assert_eq!(types.type_reference_count(), 4);
    let TypeReferenceNode::Generic { arguments, .. } = types.type_reference(root) else {
        panic!("root type reference should be generic");
    };

    assert_eq!(arguments.count(), 2);
}

#[test]
fn type_reference_table_stores_typed_constraints_as_expression_handles() {
    for end_inclusive in [false, true] {
        check_range_tree_to_table(end_inclusive);
    }
}

fn check_range_tree_to_table(end_inclusive: bool) {
    let mut source_constraints = Arena::<TypeConstraint>::new();
    let mut source_expressions = ExpressionTable::new();
    let mut source_arguments = Arena::<TypeReference>::new();
    let base_type = source_arguments.append(TypeReference::Named {
        supplied_operand_owner: Default::default(),
        symbol: SymbolHandle::invalid(),
        name: DiagnosticName::generated("i32"),
    });
    let minimum = source_expressions.insert(crate::expression::ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(0),
    ));
    let maximum = source_expressions.insert(crate::expression::ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(10),
    ));
    let source_constraint = TypeConstraint::Range {
        minimum,
        maximum,
        end_inclusive,
    };
    assert_eq!(
        source_constraint.display_name(),
        if end_inclusive {
            "expression..=expression"
        } else {
            "expression..expression"
        }
    );
    let constraints = source_constraints.insert_many([source_constraint]);
    let type_reference = TypeReference::Constrained(ConstrainedTypeReference {
        storage: ConstrainedTypeReferenceStorage {
            supplied_operand_owner: Default::default(),
            base_type,
            constraints,
        },
    });

    let mut expressions = ExpressionTable::new();
    let mut types = TypeReferenceTable::new();
    let root = types.insert_tree(
        &type_reference,
        &mut expressions,
        &source_arguments,
        &source_constraints,
        &source_expressions,
    );

    assert_eq!(types.type_reference_count(), 2);
    assert_eq!(expressions.expression_count(), 2);

    let TypeReferenceNode::Constrained { constraints, .. } = types.type_reference(root) else {
        panic!("root type reference should be constrained");
    };
    let [
        TypeConstraintNode::Range {
            minimum,
            maximum,
            end_inclusive: copied_end_inclusive,
        },
    ] = types.constraints(*constraints)
    else {
        panic!("expected one range constraint");
    };

    assert!(minimum.is_valid());
    assert!(maximum.is_valid());
    assert_eq!(*copied_end_inclusive, end_inclusive);
}

/// Normalization appends constrained and constant-expression type copies to
/// the existing expression arena. Compare every row with the former snapshot
/// route, including the exact original and appended handles.
#[test]
fn normalization_with_owned_expressions_matches_snapshot_copies() {
    use crate::expression::{ExpressionNode, ExpressionSource};
    let mut source = ExpressionTable::new();
    let minimum = source.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(0),
    ));
    let maximum = source.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(42),
    ));
    source.set_source_span(
        maximum,
        source::SourceSpan::new(source::SourceId(3), source::Span::new(7, 9)),
    );
    let mut children = Arena::new();
    let base_type = children.insert(TypeReference::Unit);
    let mut constraints = Arena::new();
    let range = constraints.insert_many([TypeConstraint::Range {
        minimum,
        maximum,
        end_inclusive: true,
    }]);
    let arguments = children.insert_many([
        TypeReference::Constrained(ConstrainedTypeReference {
            storage: ConstrainedTypeReferenceStorage {
                supplied_operand_owner: Default::default(),
                base_type,
                constraints: range,
            },
        }),
        TypeReference::ConstExpression(maximum),
    ]);
    let reference = TypeReference::Generic(GenericTypeReference {
        storage: GenericTypeReferenceStorage {
            supplied_operand_owner: Default::default(),
            base_symbol: SymbolHandle::invalid(),
            base_name: DiagnosticName::generated("Window"),
            lifetime_arguments: Vec::new(),
            arguments,
        },
    });
    let mut copied_expressions = source.clone();
    let mut copied_types = TypeReferenceTable::new();
    let copied = copied_types.insert_tree(
        &reference,
        &mut copied_expressions,
        &children,
        &constraints,
        &source,
    );
    let mut moved_expressions = source;
    let mut moved_types = TypeReferenceTable::new();
    let moved = moved_types.insert_tree_from_source(
        &reference,
        &mut moved_expressions,
        &children,
        &constraints,
        ExpressionSource::Own,
    );
    assert_eq!(copied, moved);
    assert_eq!(copied_types, moved_types);
    assert_eq!(copied_expressions, moved_expressions);
    assert_eq!(moved_expressions.expression_count(), 5);
}
