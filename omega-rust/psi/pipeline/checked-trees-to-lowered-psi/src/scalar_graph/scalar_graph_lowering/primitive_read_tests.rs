//! Primitive read lowering tests.

use super::{IntegerValue, KnownDirectScalar, PlaceId, terminal_scalar_type};
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::scalar_graph::PrimitiveType;
use crate::scalar_graph::scalar_graph_lowering::known_evaluation::{
    evaluate_compile_known_boolean_expression, evaluate_direct_expression,
};

#[test]
fn primitive_read_values_cannot_be_recovered_from_known_ssa_inputs() {
    let source = PlaceId::new(1).unwrap();
    let scalar_type = terminal_scalar_type(PrimitiveType::U64).unwrap();
    let integer_read = LoweredDirectExpression::PrimitiveRead {
        source,
        path: Vec::new(),
        scalar_type,
    };
    assert_eq!(
        evaluate_direct_expression(
            &integer_read,
            &[Some(KnownDirectScalar::Integer(IntegerValue::Unsigned(7)))],
        ),
        None,
    );
    let boolean_read = LoweredBooleanReturnExpression::PrimitiveRead {
        source,
        path: Vec::new(),
    };
    assert_eq!(
        evaluate_compile_known_boolean_expression(
            &boolean_read,
            &[Some(KnownDirectScalar::Boolean(true))],
        ),
        None,
    );
    let equality = LoweredBooleanReturnExpression::Equal {
        left: Box::new(boolean_read.clone()),
        right: Box::new(boolean_read),
    };
    assert_eq!(
        evaluate_compile_known_boolean_expression(&equality, &[]),
        None
    );
    assert!(
        crate::scalar_graph::shared_runtime_parameters::shared_boolean_runtime_parameters(
            &equality
        )
        .is_none()
    );
    assert!(
        crate::scalar_graph::shared_runtime_parameters::normalize_shared_boolean_comparison_leaves(
            &equality
        )
        .is_none()
    );
}
