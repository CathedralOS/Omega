//! Scalar operation result and literal-domain corruption rejection.

use crate::tests::{exact_add_unit, id, refresh_node_derivatives, unit};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use omega_abstract_operations::AbstractOperation;
use psi_core::{BlockId, IntegerValue, MachineId};

#[test]
fn rejects_self_consistent_scalar_operation_contract_corruption() {
    let mut arithmetic = exact_add_unit();
    let (psi_operation, result) = match &arithmetic.functions[0].blocks[0].nodes[1].operation {
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            ..
        } => (*psi_operation, *result),
        _ => panic!("fixture right operand is an integer constant"),
    };
    arithmetic.functions[0].blocks[0].nodes[1].operation = AbstractOperation::BooleanConstant {
        psi_operation,
        result,
        value: true,
    };
    refresh_node_derivatives(&mut arithmetic, 0, 0, 1);
    assert_eq!(
        validate_psi_optimization_unit(&arithmetic),
        Err(
            OptimizationUnitValidationError::ScalarOperationContractMismatch {
                machine: id(201, MachineId::new),
                block: id(202, BlockId::new),
                node: 2,
            }
        )
    );

    let mut out_of_range = unit();
    let AbstractOperation::IntegerConstant { value, .. } =
        &mut out_of_range.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with an integer constant")
    };
    *value = IntegerValue::Unsigned(256);
    refresh_node_derivatives(&mut out_of_range, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&out_of_range),
        Err(OptimizationUnitValidationError::ScalarOperationContractMismatch { node: 0, .. })
    ));
}
