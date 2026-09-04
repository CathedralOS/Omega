//! Attached-Unit scalar-call production and independent corruption replay.

use crate::tests::fixtures::scalar_call_unit::scalar_call_unit_fixture;
use crate::{legalize_target_operations, validate_legalized_operations};

#[test]
fn exact_u64_equality_three_call_chain_is_produced_and_replayed() {
    let (abstract_plan, target, unit) = scalar_call_unit_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit)
        .expect("exact attached-Unit scalar call chain legalizes");
    assert_eq!(legalized.plan().scalar_call_unit_functions.len(), 1);
    assert_eq!(legalized.plan().functions.len(), 1);
    assert_eq!(legalized.receipt().function_count(), 2);
    let function = &legalized.plan().scalar_call_unit_functions[0];
    assert_eq!(function.constants.len(), 2);
    assert_eq!(function.calls.len(), 3);
    assert_eq!(function.calls[0].arguments, function.calls[1].arguments);
    assert!(
        matches!(function.calls[2].arguments[0].source, omega_target_operations::TargetUnitScalarArgumentSource::Home(home) if home == function.calls[0].result_home)
    );
    assert!(
        matches!(function.calls[2].arguments[1].source, omega_target_operations::TargetUnitScalarArgumentSource::Home(home) if home == function.calls[1].result_home)
    );

    let mut corruptions = Vec::new();
    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_call_unit_functions[0].constants.swap(0, 1);
    corruptions.push(corrupted);
    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_call_unit_functions[0].calls[0]
        .arguments
        .swap(0, 1);
    corruptions.push(corrupted);
    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_call_unit_functions[0].calls[2].result_home =
        corrupted.scalar_call_unit_functions[0].calls[1].result_home;
    corruptions.push(corrupted);
    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_call_unit_functions[0].calls[1].fuel[0].units += 1;
    corruptions.push(corrupted);
    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_call_unit_functions.clear();
    corruptions.push(corrupted);
    for corrupted in corruptions {
        assert!(validate_legalized_operations(&target, &abstract_plan, &unit, corrupted).is_err());
    }
}
