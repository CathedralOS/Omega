//! Write-only store destination access, type, position, and dominance validation.

use crate::tests::fixtures::scalar_units::write_only_store_unit;
use crate::tests::support::{id, refresh_node_derivatives};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation;

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
    destination.access = StructuralAccess::SharedBorrow;
    refresh_node_derivatives(&mut shared, 0, 0, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&shared),
        Err(OptimizationUnitValidationError::InvalidWriteOnlyPrimitiveStore { .. })
    ));
}
