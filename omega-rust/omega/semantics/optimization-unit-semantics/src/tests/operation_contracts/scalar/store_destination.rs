//! Write-only store destination access, type, position, and dominance validation.

use crate::tests::{
    id, refresh_identity, refresh_node_derivatives, write_only_store_before_value_unit,
    write_only_store_unit,
};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};

#[test]
fn validates_exact_write_only_store_destination_access_type_and_dominance() {
    let baseline = write_only_store_unit();
    assert_eq!(validate_psi_optimization_unit(&baseline), Ok(()));

    let mut access_drift = baseline.clone();
    let AbstractOperation::WriteOnlyPrimitiveStore { destination, .. } =
        &mut access_drift.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("fixture second node is the write-only store")
    };
    destination.access = terminal_psi::StructuralAccess::SharedBorrow;
    refresh_node_derivatives(&mut access_drift, 0, 0, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&access_drift),
        Err(OptimizationUnitValidationError::InvalidWriteOnlyPrimitiveStore { node: 1, .. })
    ));

    let mut destination_drift = baseline.clone();
    let AbstractOperation::WriteOnlyPrimitiveStore { destination, .. } =
        &mut destination_drift.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("fixture second node is the write-only store")
    };
    destination.position = 1;
    refresh_node_derivatives(&mut destination_drift, 0, 0, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&destination_drift),
        Err(OptimizationUnitValidationError::InvalidWriteOnlyPrimitiveStore { node: 1, .. })
    ));

    let mut type_drift = baseline.clone();
    let structural_type = type_drift.structural_types[0].id;
    type_drift.structural_types[0].shape = terminal_psi::StructuralTypeShape::PrimitiveScalar(
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).unwrap()),
    );
    refresh_identity(&mut type_drift);
    assert!(
        matches!(
            validate_psi_optimization_unit(&type_drift),
            Err(OptimizationUnitValidationError::InvalidWriteOnlyPrimitiveStore { node: 1, .. })
        ),
        "type {structural_type:?} drift must reject at the store"
    );

    let before_value = write_only_store_before_value_unit();
    assert!(matches!(
        validate_psi_optimization_unit(&before_value),
        Err(OptimizationUnitValidationError::UseBeforeDefinition { value, .. })
            if value == id(53, ValueId::new)
    ));
}
