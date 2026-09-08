//! Receiving checks must reject forged current IR after derived metadata is refreshed.

use crate::tests::*;
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation as O;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

#[test]
fn local_establishment_store_and_fresh_reads_validate() {
    let unit = primitive_local_unit();
    assert_eq!(validate_psi_optimization_unit(&unit), Ok(()));
    let nodes = &unit.functions[0].blocks[0].nodes;
    assert!(unit.functions[0].structural_parameters.is_empty());
    assert_eq!(nodes[1].uses[0].value, nodes[0].definitions[0].value);
    assert_ne!(nodes[2].definitions[0].value, nodes[4].definitions[0].value);
    for node in &nodes[1..5] {
        assert_eq!(
            node.fuel
                .iter()
                .map(|settlement| settlement.units)
                .sum::<u64>(),
            1
        );
        assert!(node.ownership.is_empty());
    }
}

#[test]
fn local_store_and_read_require_dominating_establishment() {
    for use_node in [2, 3] {
        let mut unit = primitive_local_unit();
        unit.functions[0].blocks[0].nodes.swap(1, use_node);
        refresh_function_derivatives(&mut unit, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&unit),
            Err(OptimizationUnitValidationError::StructuralPlaceNotAvailable { .. })
        ));
    }
}

#[test]
fn local_initializer_requires_available_exact_scalar_type() {
    let mut unit = primitive_local_unit();
    let O::EstablishPrimitiveLocal { value, .. } =
        &mut unit.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("local")
    };
    value.value = id(60, ValueId::new);
    refresh_function_derivatives(&mut unit, 0);
    assert!(validate_psi_optimization_unit(&unit).is_err());

    let mut unit = primitive_local_unit();
    let O::EstablishPrimitiveLocal { value, .. } =
        &mut unit.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("local")
    };
    value.scalar_type = ScalarType::Boolean;
    refresh_function_derivatives(&mut unit, 0);
    assert!(validate_psi_optimization_unit(&unit).is_err());
}

#[test]
fn local_root_multiplicity_and_read_type_are_independently_checked() {
    for corruption in 0..3 {
        let mut unit = primitive_local_unit();
        match corruption {
            0 => {
                unit.functions[0].structural_places[0].kind = StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                }
            }
            1 => {
                let O::EstablishPrimitiveLocal { result, .. } =
                    &mut unit.functions[0].blocks[0].nodes[1].operation
                else {
                    panic!("local")
                };
                result.multiplicity = StructuralMultiplicity::Affine;
            }
            _ => {
                let O::PrimitiveScalarRead { result, .. } =
                    &mut unit.functions[0].blocks[0].nodes[2].operation
                else {
                    panic!("read")
                };
                result.scalar_type = ScalarType::Boolean;
            }
        }
        refresh_function_derivatives(&mut unit, 0);
        assert!(validate_psi_optimization_unit(&unit).is_err());
    }
}

#[test]
fn incoming_primitive_reads_require_readable_access() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
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
            access != StructuralAccess::WriteOnlyBorrow
        );
    }
}

#[test]
fn local_claims_and_parameter_substitution_are_rejected() {
    let mut unit = primitive_local_unit();
    let O::EstablishPrimitiveLocal { result, .. } =
        &mut unit.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("local")
    };
    result
        .claims
        .push(terminal_psi::StructuralResultClaimBinding {
            claim: id(1, semantic_vocabulary::ClaimId::new),
            path: Vec::new(),
        });
    refresh_function_derivatives(&mut unit, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&unit),
        Err(OptimizationUnitValidationError::InvalidPrimitiveLocal { .. })
    ));

    let mut unit = write_only_store_unit();
    let O::WriteOnlyPrimitiveStore {
        destination, value, ..
    } = unit.functions[0].blocks[0].nodes[1].operation.clone()
    else {
        panic!("store")
    };
    unit.functions[0].blocks[0].nodes[1].operation = O::PrimitiveLocalStore {
        psi_operation: id(57, OperationId::new),
        destination: destination.place,
        value,
    };
    refresh_function_derivatives(&mut unit, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&unit),
        Err(OptimizationUnitValidationError::InvalidPrimitiveLocal { .. })
    ));
}
