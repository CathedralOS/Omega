//! Write-only store destination access, type, position, and dominance validation.

use crate::tests::fixtures::scalar_units::write_only_store_unit;
use crate::tests::support::{id, refresh_node_derivatives};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation;

fn owned_array_store_unit() -> optimization_unit::PsiOptimizationUnit {
    use abstract_operations::StructuralScalarStoreDestination;
    use semantic_vocabulary::{OperationId, StructuralTypeId};
    use terminal_psi::{StructuralOperationResult, StructuralPathSegment, StructuralTypeShape};

    let mut plan = crate::tests::fixtures::scalar_units::write_only_store_plan(false);
    let mut element = plan.structural_types[0].clone();
    element.id = id(9001, StructuralTypeId::new);
    element.identity = "test::OwnedArrayElement".into();
    plan.structural_types.make_mut()[0].identity = "test::OwnedArray".into();
    plan.structural_types.make_mut()[0].shape = StructuralTypeShape::FixedArray {
        element: element.id,
        length: 2,
    };
    plan.structural_types.make_mut().push(element);
    let function = &mut plan.functions[0];
    let parameter = function.structural_parameters.remove(0);
    let producer = id(9002, OperationId::new);
    let result = StructuralOperationResult {
        place: parameter.place,
        structural_type: parameter.structural_type,
        multiplicity: parameter.multiplicity,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    };
    let AbstractOperation::WriteOnlyPrimitiveStore {
        destination, path, ..
    } = &mut function.operations[1]
    else {
        panic!("fixture store");
    };
    *destination = StructuralScalarStoreDestination::OwnedResult {
        producer,
        result: result.clone(),
    };
    *path = vec![StructuralPathSegment::FixedIndex(1)];
    function.operations.insert(
        1,
        AbstractOperation::EstablishZeroedStructural {
            psi_operation: producer,
            result,
        },
    );
    optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("owned array store reconstructs")
}

#[test]
fn owned_array_primitive_store_reconstructs_exact_producer_and_bounds() {
    use abstract_operations::StructuralScalarStoreDestination;
    use semantic_vocabulary::OperationId;
    use terminal_psi::StructuralPathSegment;

    let valid = owned_array_store_unit();
    assert_eq!(validate_psi_optimization_unit(&valid), Ok(()));
    let mut wrong_producer = valid.clone();
    let AbstractOperation::WriteOnlyPrimitiveStore {
        destination: StructuralScalarStoreDestination::OwnedResult { producer, .. },
        ..
    } = &mut wrong_producer.functions[0].blocks[0].nodes[2].operation
    else {
        panic!("owned destination");
    };
    *producer = id(9003, OperationId::new);
    refresh_node_derivatives(&mut wrong_producer, 0, 0, 2);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_producer),
        Err(OptimizationUnitValidationError::InvalidWriteOnlyPrimitiveStore { .. })
    ));

    for path in [
        Vec::new(),
        vec![StructuralPathSegment::FixedIndex(2)],
        vec![StructuralPathSegment::FixedIndex(u64::MAX)],
    ] {
        let mut invalid = valid.clone();
        let AbstractOperation::WriteOnlyPrimitiveStore {
            path: destination_path,
            ..
        } = &mut invalid.functions[0].blocks[0].nodes[2].operation
        else {
            panic!("store");
        };
        *destination_path = path;
        refresh_node_derivatives(&mut invalid, 0, 0, 2);
        assert!(matches!(
            validate_psi_optimization_unit(&invalid),
            Err(OptimizationUnitValidationError::InvalidWriteOnlyPrimitiveStore { .. })
        ));
    }
}

#[test]
fn owned_array_store_cannot_claim_shared_parameter_authority() {
    use abstract_operations::StructuralScalarStoreDestination;
    use terminal_psi::{StructuralAccess, StructuralParameterDeclaration};

    let mut invalid = owned_array_store_unit();
    let AbstractOperation::WriteOnlyPrimitiveStore { destination, .. } =
        &mut invalid.functions[0].blocks[0].nodes[2].operation
    else {
        panic!("store");
    };
    let StructuralScalarStoreDestination::OwnedResult { result, .. } = destination else {
        panic!("owned destination");
    };
    *destination = StructuralScalarStoreDestination::Parameter(StructuralParameterDeclaration {
        place: result.place,
        position: 0,
        is_self: false,
        structural_type: result.structural_type,
        multiplicity: result.multiplicity,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    refresh_node_derivatives(&mut invalid, 0, 0, 2);
    assert!(matches!(
        validate_psi_optimization_unit(&invalid),
        Err(OptimizationUnitValidationError::InvalidWriteOnlyPrimitiveStore { .. })
    ));
}

#[test]
fn indexed_primitive_store_reconstructs_leaf_bounds_and_root_access() {
    use semantic_vocabulary::StructuralTypeId;
    use terminal_psi::StructuralPathSegment as Segment;
    use terminal_psi::{StructuralAccess, StructuralTypeShape};

    let mut valid = write_only_store_unit();
    let mut element = valid.structural_types[0].clone();
    element.id = id(9001, StructuralTypeId::new);
    element.identity = "test::IndexedElement".into();
    valid.structural_types.make_mut()[0].shape = StructuralTypeShape::FixedArray {
        element: element.id,
        length: 2,
    };
    valid.structural_types.make_mut().push(element);
    let AbstractOperation::WriteOnlyPrimitiveStore { path, .. } =
        &mut valid.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("store");
    };
    *path = vec![Segment::FixedIndex(1)];
    refresh_node_derivatives(&mut valid, 0, 0, 1);
    assert_eq!(validate_psi_optimization_unit(&valid), Ok(()));

    for replacement in [
        Vec::new(),
        vec![Segment::FixedIndex(2)],
        vec![Segment::FixedIndex(u64::MAX)],
    ] {
        let mut invalid = valid.clone();
        let AbstractOperation::WriteOnlyPrimitiveStore { path, .. } =
            &mut invalid.functions[0].blocks[0].nodes[1].operation
        else {
            panic!("store");
        };
        *path = replacement;
        refresh_node_derivatives(&mut invalid, 0, 0, 1);
        assert!(matches!(
            validate_psi_optimization_unit(&invalid),
            Err(OptimizationUnitValidationError::InvalidWriteOnlyPrimitiveStore { .. })
        ));
    }
    // Matching the retained operation to a shared declaration still cannot
    // authorize a store; declaration correspondence alone is insufficient.
    let mut shared = valid;
    shared.functions[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    let AbstractOperation::WriteOnlyPrimitiveStore { destination, .. } =
        &mut shared.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("store");
    };
    let abstract_operations::StructuralScalarStoreDestination::Parameter(destination) = destination
    else {
        panic!("parameter destination");
    };
    destination.access = StructuralAccess::SharedBorrow;
    refresh_node_derivatives(&mut shared, 0, 0, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&shared),
        Err(OptimizationUnitValidationError::InvalidWriteOnlyPrimitiveStore { .. })
    ));
}
