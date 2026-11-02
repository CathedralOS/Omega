//! A literal layout must keep the exact computation graph across arena copies.
use super::*;
use crate::expression::{ExpressionNode, ExpressionTable};
use numerics::literals::{IntegerLiteral, IntegerRadix};

fn integer(expressions: &mut ExpressionTable, value: &str) -> ExpressionHandle {
    expressions.insert(ExpressionNode::Integer(
        IntegerLiteral::from_parts(false, IntegerRadix::Decimal, value).unwrap(),
    ))
}

#[test]
fn completed_extent_custody_survives_distinct_arenas_and_replacement() {
    let mut expressions = ExpressionTable::new();
    let authored = integer(&mut expressions, "1");
    let materialized = integer(&mut expressions, "1");
    expressions.retain_materialization_source(materialized, authored);
    let mut source = TypeReferenceTable::new();
    let argument = source.insert(TypeReferenceNode::ConstExpression(materialized));
    let element_type = source.insert(TypeReferenceNode::Unit);
    let array = source.insert(TypeReferenceNode::FixedArray {
        element_type,
        length: FixedArrayLength::Expression(argument),
    });
    source.set_fixed_array_length(array, 1);
    let fresh = source.fresh_occurrence(array);
    assert_eq!(source.array_extent_argument(fresh), argument);

    let mut receiving_expressions = ExpressionTable::new();
    for _ in 0..4 {
        integer(&mut receiving_expressions, "99");
    }
    let mut receiving = TypeReferenceTable::new();
    for _ in 0..4 {
        receiving.insert(TypeReferenceNode::Unit);
    }
    let copied = receiving.copy_from(&source, &expressions, &mut receiving_expressions, fresh);
    let copied_argument = receiving.array_extent_argument(copied);
    assert_ne!(copied_argument, argument);
    let TypeReferenceNode::ConstExpression(value) = receiving.type_reference(copied_argument)
    else {
        panic!("completed extent lost its value child");
    };
    assert_ne!(*value, materialized);
    assert_eq!(
        receiving_expressions.constant_integer_value(*value),
        Some(1)
    );
    let original = receiving_expressions.materialization_source(*value);
    assert!(original.is_valid());
    assert_ne!(original, authored);
    assert_eq!(
        receiving_expressions.constant_integer_value(original),
        Some(1)
    );
    assert!(matches!(
        receiving.type_reference(copied),
        TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(1),
            ..
        }
    ));

    let replacement = receiving.insert(TypeReferenceNode::Unit);
    receiving.substitute_from(copied, replacement).unwrap();
    assert!(!receiving.array_extent_argument(copied).is_valid());
    // Replacement cannot erase a different occurrence's retained computation.
    assert_eq!(source.array_extent_argument(fresh), argument);

    let mut program = crate::TypedTrees::default();
    program.type_reference_table = source;
    program.expression_table = expressions;
    let applied = program
        .apply_callable_type(fresh, &[], &[], SymbolHandle::invalid())
        .unwrap();
    let applied_argument = program.type_reference_table.array_extent_argument(applied);
    let TypeReferenceNode::ConstExpression(applied_value) = program
        .type_reference_table
        .type_reference(applied_argument)
    else {
        panic!("callable application lost the completed extent child");
    };
    assert!(
        program
            .expression_table
            .materialization_source(*applied_value)
            .is_valid()
    );

    let binder = SymbolHandle::from_parts(19, 0);
    let template = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::ConstParameter {
                symbol: binder,
                name: Identifier::generated("N"),
            },
        });
    let supplied = program
        .apply_callable_type(
            template,
            &[(binder, argument)],
            &[],
            SymbolHandle::invalid(),
        )
        .unwrap();
    assert_eq!(
        program.type_reference_table.array_extent_argument(supplied),
        argument,
        "an already supplied actual must retain its own telescope"
    );
}
