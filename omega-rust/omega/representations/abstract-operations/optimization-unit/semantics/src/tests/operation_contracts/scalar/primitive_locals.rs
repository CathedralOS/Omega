//! Receiving checks must reject forged current IR after derived metadata is refreshed.

use crate::tests::fixtures::dominance::refresh_function_derivatives;
use crate::tests::fixtures::scalar_units::write_only_store_unit;
use crate::tests::support::id;
use crate::validate_psi_optimization_unit;
use abstract_operations::{AbstractOperation as O, AbstractResult};
use semantic_vocabulary::{OperationId, ValueId};
use terminal_psi::StructuralAccess;

#[test]
fn incoming_primitive_reads_require_readable_access() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
        StructuralAccess::Owned,
    ] {
        let mut unit = write_only_store_unit();
        let source = unit.functions[0].structural_parameters[0].place;
        unit.functions[0].structural_parameters[0].access = access;
        let O::WriteOnlyPrimitiveStore { value, .. } =
            unit.functions[0].blocks[0].nodes[1].operation
        else {
            panic!("store")
        };
        unit.functions[0].blocks[0].nodes[1].operation = O::PrimitiveScalarRead {
            path: Vec::new(),
            psi_operation: id(57, OperationId::new),
            source,
            result: AbstractResult {
                value: id(60, ValueId::new),
                scalar_type: value.scalar_type,
            },
        };
        refresh_function_derivatives(&mut unit, 0);
        assert_eq!(
            validate_psi_optimization_unit(&unit).is_ok(),
            matches!(
                access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            )
        );
    }
}

#[test]
fn owned_aggregate_primitive_reads_validate_projection_custody() {
    use semantic_vocabulary::{PlaceId, ScalarType, StructuralTypeId};
    use terminal_psi::{StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape};

    let mut valid = write_only_store_unit();
    let mut element = valid.structural_types[0].clone();
    element.id = id(9001, StructuralTypeId::new);
    element.identity = "test::OwnedArrayElement".into();
    valid.structural_types.make_mut()[0].shape = StructuralTypeShape::FixedArray {
        element: element.id,
        length: 2,
    };
    valid.structural_types.make_mut().push(element);
    let parameter = &mut valid.functions[0].structural_parameters[0];
    parameter.access = StructuralAccess::Owned;
    let source = parameter.place;
    let O::WriteOnlyPrimitiveStore { value, .. } = valid.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("store");
    };
    valid.functions[0].blocks[0].nodes[1].operation = O::PrimitiveScalarRead {
        psi_operation: id(57, OperationId::new),
        source,
        path: vec![StructuralPathSegment::FixedIndex(1)],
        result: AbstractResult {
            value: id(60, ValueId::new),
            scalar_type: value.scalar_type,
        },
    };
    refresh_function_derivatives(&mut valid, 0);
    assert_eq!(validate_psi_optimization_unit(&valid), Ok(()));

    // Refresh the derived operands too: admission must reconstruct the current
    // source's access, leaf type, and bounds instead of trusting cached facts.
    for mutation in 0..6 {
        let mut invalid = valid.clone();
        let function = &mut invalid.functions[0];
        let O::PrimitiveScalarRead {
            source,
            path,
            result,
            ..
        } = &mut function.blocks[0].nodes[1].operation
        else {
            panic!("read");
        };
        match mutation {
            0 => path.clear(),
            1 => path[0] = StructuralPathSegment::FixedIndex(2),
            2 => *source = id(9002, PlaceId::new),
            3 => result.scalar_type = ScalarType::Boolean,
            4 => function.structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow,
            _ => function.structural_parameters[0].multiplicity = StructuralMultiplicity::Linear,
        }
        refresh_function_derivatives(&mut invalid, 0);
        assert!(
            validate_psi_optimization_unit(&invalid).is_err(),
            "mutation {mutation}"
        );
    }
}
