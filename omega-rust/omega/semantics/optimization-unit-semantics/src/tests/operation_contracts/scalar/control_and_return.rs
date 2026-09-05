//! Conditional-input and scalar-return type corruption rejection.

use crate::tests::{
    redundant_parameter_region_fixture, refresh_identity, refresh_node_derivatives, unit,
};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};

#[test]
fn rejects_self_consistent_control_and_return_type_corruption() {
    let mut conditional = redundant_parameter_region_fixture().0;
    conditional.functions[0].parameters[0].scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("valid integer"));
    refresh_identity(&mut conditional);
    assert!(matches!(
        validate_psi_optimization_unit(&conditional),
        Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 0, .. })
    ));

    let mut scalar_return = unit();
    let (psi_operation, result) = match &scalar_return.functions[0].blocks[0].nodes[0].operation {
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            ..
        } => (*psi_operation, *result),
        _ => panic!("fixture begins with an integer constant"),
    };
    scalar_return.functions[0].blocks[0].nodes[0].operation = AbstractOperation::BooleanConstant {
        psi_operation,
        result,
        value: true,
    };
    refresh_node_derivatives(&mut scalar_return, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&scalar_return),
        Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 1, .. })
    ));
}
