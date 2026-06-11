use super::{
    FixedArrayLength, PrimitiveType, TypeConstraintNode, TypeReferenceNode, TypeReferenceTable,
};
use crate::expression::{ExpressionNode, ExpressionTable};
use crate::name::Identifier;
use omega_core::symbols::SymbolHandle;

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
    let minimum = expressions.insert(ExpressionNode::Integer(0));
    let maximum = expressions.insert(ExpressionNode::Integer(10));
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
        "i32[range<0, 10>]"
    );
    assert_eq!(types.primitive_type(root), Some(PrimitiveType::I32));
    assert_eq!(types.type_symbol(root), SymbolHandle::invalid());
}

#[test]
fn type_reference_table_copies_table_payloads_without_tree_roundtrip() {
    let mut source_expressions = ExpressionTable::new();
    let minimum = source_expressions.insert(ExpressionNode::Integer(1));
    let maximum = source_expressions.insert(ExpressionNode::Integer(8));
    let mut source_types = TypeReferenceTable::new();
    let u8_reference = source_types.insert(TypeReferenceNode::Named {
        symbol: SymbolHandle::from_arena_index(11),
        name: Identifier::generated("u8"),
    });
    let fixed_array_reference = source_types.insert(TypeReferenceNode::FixedArray {
        element_type: u8_reference,
        length: FixedArrayLength::Literal(8),
    });
    let constraints =
        source_types.insert_constraints([TypeConstraintNode::Range { minimum, maximum }]);
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
        "[u8; 8][range<1, 8>]"
    );
    assert_eq!(
        copied_types.type_reference_count(),
        source_types.type_reference_count()
    );
    assert_eq!(
        copied_expressions.expression_count(),
        source_expressions.expression_count()
    );
}
