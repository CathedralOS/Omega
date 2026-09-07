//! Plain Unit catalog production and independent replay.

use crate::tests::fixtures::plain_unit::plain_unit_fixture;
use crate::{legalize_target_operations, validate_legalized_operations};
use semantic_vocabulary::{EdgeId, MachineId};

#[test]
fn plain_unit_catalog_form_is_produced_and_independently_replayed() {
    let (abstract_plan, target, unit) = plain_unit_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit)
        .expect("plain Unit return legalizes through its catalog row");
    assert!(legalized.plan().functions.is_empty());
    assert!(legalized.plan().structural_unit_functions.is_empty());
    assert_eq!(legalized.plan().scalar_functions.len(), 1);
    assert!(matches!(
        legalized.plan().scalar_functions[0].blocks[0].terminator,
        legalized_operations::LegalizedScalarTerminator::Return(
            legalized_operations::LegalizedScalarReturn {
                value: legalized_operations::LegalizedScalarReturnValue::Unit,
                ..
            }
        )
    ));
    assert_eq!(legalized.receipt().function_count(), 1);

    let mut wrong_edge = legalized.plan().clone();
    let legalized_operations::LegalizedScalarTerminator::Return(returned) =
        &mut wrong_edge.scalar_functions[0].blocks[0].terminator
    else {
        panic!("Unit return");
    };
    returned.edge = EdgeId::new(2).unwrap();
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, wrong_edge).is_err());

    let mut wrong_machine = legalized.plan().clone();
    wrong_machine.scalar_functions[0].machine = MachineId::new(2).unwrap();
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, wrong_machine).is_err());

    let mut duplicate = legalized.plan().clone();
    duplicate
        .scalar_functions
        .push(duplicate.scalar_functions[0].clone());
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, duplicate).is_err());

    let mut erased = legalized.plan().clone();
    erased.scalar_functions.clear();
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, erased).is_err());
}

#[test]
fn publication_classification_uses_the_existing_unit_grammar() {
    let (abstracted, targeted, unit) = plain_unit_fixture();
    assert!(crate::legalization::accepts_fragment_publication_input(
        &targeted,
        &abstracted,
        &unit
    ));
    let mut changed = abstracted.clone();
    changed.functions[0].operations.pop();
    assert!(!crate::legalization::accepts_fragment_publication_input(
        &targeted, &changed, &unit
    ));
}
