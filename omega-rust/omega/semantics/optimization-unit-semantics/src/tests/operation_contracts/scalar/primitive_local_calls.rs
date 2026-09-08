//! Borrowed local calls require establishment and cannot transfer the local owner.

use crate::tests::*;
use crate::validate_psi_optimization_unit;
use abstract_operations::AbstractOperation as O;
use terminal_psi::StructuralAccess;

#[test]
fn local_borrow_call_preserves_read_observations_and_rejects_owned_escape() {
    let mut plan = primitive_local_call_plan();
    let construct = |plan: &AbstractOperationPlan| {
        reconstruct_psi_optimization_unit_seed(
            plan,
            terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        )
        .unwrap()
    };
    let unit = construct(&plan);
    assert_eq!(validate_psi_optimization_unit(&unit), Ok(()));
    let observations = &unit.functions[0].blocks[0].nodes;
    assert_ne!(
        observations[2].definitions[0].value,
        observations[5].definitions[0].value
    );

    let mut before_establishment = unit.clone();
    before_establishment.functions[0].blocks[0].nodes.swap(1, 4);
    refresh_function_derivatives(&mut before_establishment, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&before_establishment),
        Err(crate::OptimizationUnitValidationError::StructuralPlaceNotAvailable { .. })
    ));

    let O::CallUnit {
        structural_arguments,
        ..
    } = &mut plan.functions[0].operations[4]
    else {
        panic!("call")
    };
    structural_arguments[0].access = StructuralAccess::Owned;
    plan.functions[1].structural_parameters[0].access = StructuralAccess::Owned;
    plan.functions[1].operations = vec![O::ReturnUnit {
        psi_edge: id(76, EdgeId::new),
        cleanup_actions: Vec::new(),
    }];
    assert!(validate_psi_optimization_unit(&construct(&plan)).is_err());
}

#[test]
fn local_reentry_preserves_storage_operations_and_requires_explicit_cycle_admission() {
    let mut plan = primitive_local_call_plan();
    let function = &mut plan.functions[0];
    *function.operations.last_mut().unwrap() = O::Jump {
        psi_edge: id(58, EdgeId::new),
        target: function.entry,
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    let unit = reconstruct_psi_optimization_unit_seed(
        &plan,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap();
    assert!(validate_psi_optimization_unit(&unit).is_err());
    assert_eq!(
        crate::validate_psi_optimization_unit_with_admitted_cycle_machines(&unit, &[unit.entry]),
        Ok(())
    );
}

#[test]
fn activation_local_cannot_escape_as_structural_return() {
    let mut plan = primitive_local_plan();
    let function = &mut plan.functions[0];
    function.result =
        AbstractFunctionResult::Structural(terminal_psi::StructuralResultDeclaration {
            place: id(80, PlaceId::new),
            structural_type: id(55, StructuralTypeId::new),
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    *function.operations.last_mut().unwrap() = O::ReturnStructural {
        psi_edge: id(58, EdgeId::new),
        source: id(54, PlaceId::new),
        returned_claims: Vec::new(),
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let unit = reconstruct_psi_optimization_unit_seed(
        &plan,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap();
    assert!(matches!(
        validate_psi_optimization_unit(&unit),
        Err(crate::OptimizationUnitValidationError::StructuralReturnSourceContractMismatch { .. })
    ));
}
