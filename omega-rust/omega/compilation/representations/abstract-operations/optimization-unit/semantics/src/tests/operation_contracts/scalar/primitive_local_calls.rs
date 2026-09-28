//! Borrowed local calls require establishment and cannot transfer the local owner.
use crate::tests::fixtures::dominance::refresh_function_derivatives;
use crate::tests::fixtures::primitive_locals::primitive_local_call_plan;
use crate::tests::support::id;
use crate::validate_psi_optimization_unit;
use abstract_operations::{AbstractOperation as O, AbstractOperationPlan};
use optimization_unit::reconstruct_psi_optimization_unit_seed;
use semantic_vocabulary::EdgeId;
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
