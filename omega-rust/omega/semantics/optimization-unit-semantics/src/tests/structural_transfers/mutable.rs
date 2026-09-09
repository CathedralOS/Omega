//! Exclusive transfers consume source names even when their descriptor still exists.
use super::*;

fn mutable_transfer_unit() -> PsiOptimizationUnit {
    let mut candidate = transfer_unit();
    let function = &mut candidate.functions[0];
    function.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    function.blocks[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    let AbstractOperation::Jump {
        structural_bindings,
        ..
    } = &mut function.blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    structural_bindings[0].argument.access = StructuralAccess::MutableBorrow;
    refresh_node_derivatives(&mut candidate, 0, 0, 0);
    candidate
}

#[test]
fn transferred_mutable_view_exposes_only_its_current_binding() {
    let mut candidate = mutable_transfer_unit();
    validate_psi_optimization_unit(&candidate).unwrap();
    let AbstractOperation::ByteSequenceLength { source, .. } =
        &mut candidate.functions[0].blocks[1].nodes[0].operation
    else {
        unreachable!()
    };
    *source = id(10, PlaceId::new);
    refresh_node_derivatives(&mut candidate, 0, 1, 0);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(
            OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                machine: id(1, MachineId::new),
                block: id(3, BlockId::new),
                node: 0,
                place: id(10, PlaceId::new),
            }
        )
    );
}

#[test]
fn mutable_binding_cannot_be_relabelled_as_a_shared_arrival() {
    let mut candidate = mutable_transfer_unit();
    candidate.functions[0].blocks[1].structural_parameters[0].access =
        StructuralAccess::SharedBorrow;
    let AbstractOperation::Jump {
        structural_bindings,
        ..
    } = &mut candidate.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    structural_bindings[0].argument.access = StructuralAccess::SharedBorrow;
    refresh_node_derivatives(&mut candidate, 0, 0, 0);
    assert!(validate_psi_optimization_unit(&candidate).is_err());
}
