//! Unit cleanup and structural-return producer tests.

use super::*;

#[test]
fn retains_exact_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 3];
            values[0] = Empty {};
            values[1] = Empty {};
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 2);
    for (index, local) in plan.trivial_affine_locals.iter().enumerate() {
        assert_eq!(usize::try_from(local.declaration_ordinal), Ok(index));
        assert_eq!(local.type_identity, "named(name(Empty))");
        let construction = local
            .construction
            .as_ref()
            .expect("each local represents one established array element");
        assert_eq!(
            construction.root_type_identity,
            "array(named(name(Empty)),literal(3))"
        );
        assert_eq!(usize::try_from(construction.index), Ok(index));
    }
    assert!(matches!(
        plan.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 1,
                declaration_ordinal: 0,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 2,
                declaration_ordinal: 1,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit {
                statement_index: 3,
                trivial_affine_local_discard_ordinals,
                trivial_affine_discards,
            },
        ] if trivial_affine_local_discard_ordinals == &[1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_three_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 4];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("three-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 3);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(4))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert!(matches!(
        plan.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 1,
                declaration_ordinal: 0,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 2,
                declaration_ordinal: 1,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 3,
                declaration_ordinal: 2,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit {
                statement_index: 4,
                trivial_affine_local_discard_ordinals,
                trivial_affine_discards,
            },
        ] if trivial_affine_local_discard_ordinals == &[2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_four_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 5];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("four-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 4);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(5))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert!(matches!(
        plan.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 1,
                declaration_ordinal: 0,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 2,
                declaration_ordinal: 1,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 3,
                declaration_ordinal: 2,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 4,
                declaration_ordinal: 3,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit {
                statement_index: 5,
                trivial_affine_local_discard_ordinals,
                trivial_affine_discards,
            },
        ] if trivial_affine_local_discard_ordinals == &[3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_five_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 6];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("five-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 5);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(6))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert!(matches!(
        plan.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 1,
                declaration_ordinal: 0,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 2,
                declaration_ordinal: 1,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 3,
                declaration_ordinal: 2,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 4,
                declaration_ordinal: 3,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 5,
                declaration_ordinal: 4,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit {
                statement_index: 6,
                trivial_affine_local_discard_ordinals,
                trivial_affine_discards,
            },
        ] if trivial_affine_local_discard_ordinals == &[4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_six_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 7];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("six-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 6);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(7))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert!(matches!(
        plan.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 1,
                declaration_ordinal: 0,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 2,
                declaration_ordinal: 1,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 3,
                declaration_ordinal: 2,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 4,
                declaration_ordinal: 3,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 5,
                declaration_ordinal: 4,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 6,
                declaration_ordinal: 5,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit {
                statement_index: 7,
                trivial_affine_local_discard_ordinals,
                trivial_affine_discards,
            },
        ] if trivial_affine_local_discard_ordinals == &[5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_seven_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 8];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("seven-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 7);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(8))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert!(matches!(
        plan.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 1,
                declaration_ordinal: 0,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 2,
                declaration_ordinal: 1,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 3,
                declaration_ordinal: 2,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 4,
                declaration_ordinal: 3,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 5,
                declaration_ordinal: 4,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 6,
                declaration_ordinal: 5,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 7,
                declaration_ordinal: 6,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit {
                statement_index: 8,
                trivial_affine_local_discard_ordinals,
                trivial_affine_discards,
            },
        ] if trivial_affine_local_discard_ordinals == &[6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_eight_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 9];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
            values[7] = Empty {};
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("eight-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 8);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(9))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert!(matches!(
        plan.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 1,
                declaration_ordinal: 0,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 2,
                declaration_ordinal: 1,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 3,
                declaration_ordinal: 2,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 4,
                declaration_ordinal: 3,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 5,
                declaration_ordinal: 4,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 6,
                declaration_ordinal: 5,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 7,
                declaration_ordinal: 6,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 8,
                declaration_ordinal: 7,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit {
                statement_index: 9,
                trivial_affine_local_discard_ordinals,
                trivial_affine_discards,
            },
        ] if trivial_affine_local_discard_ordinals == &[7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_nine_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 10];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
            values[7] = Empty {};
            values[8] = Empty {};
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("nine-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 9);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(10))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert!(matches!(
        plan.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 1,
                declaration_ordinal: 0,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 2,
                declaration_ordinal: 1,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 3,
                declaration_ordinal: 2,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 4,
                declaration_ordinal: 3,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 5,
                declaration_ordinal: 4,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 6,
                declaration_ordinal: 5,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 7,
                declaration_ordinal: 6,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 8,
                declaration_ordinal: 7,
                ..
            },
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: 9,
                declaration_ordinal: 8,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit {
                statement_index: 10,
                trivial_affine_local_discard_ordinals,
                trivial_affine_discards,
            },
        ] if trivial_affine_local_discard_ordinals == &[8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_ten_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 11];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
            values[7] = Empty {};
            values[8] = Empty {};
            values[9] = Empty {};
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("ten-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 10);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(11))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert_eq!(plan.operations.len(), 11);
    assert!(
        plan.operations[..10]
            .iter()
            .enumerate()
            .all(|(index, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                        statement_index,
                        declaration_ordinal,
                        ..
                    } if usize::try_from(*statement_index) == Ok(index + 1)
                        && usize::try_from(*declaration_ordinal) == Ok(index)
                )
            })
    );
    assert!(matches!(
        &plan.operations[10],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 11,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals == &[9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_eleven_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 12];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
            values[7] = Empty {};
            values[8] = Empty {};
            values[9] = Empty {};
            values[10] = Empty {};
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("eleven-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 11);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(12))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert_eq!(plan.operations.len(), 12);
    assert!(
        plan.operations[..11]
            .iter()
            .enumerate()
            .all(|(index, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                        statement_index,
                        declaration_ordinal,
                        ..
                    } if usize::try_from(*statement_index) == Ok(index + 1)
                        && usize::try_from(*declaration_ordinal) == Ok(index)
                )
            })
    );
    assert!(matches!(
        &plan.operations[11],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 12,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals == &[10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_twelve_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 13];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
            values[7] = Empty {};
            values[8] = Empty {};
            values[9] = Empty {};
            values[10] = Empty {};
            values[11] = Empty {};
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("twelve-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 12);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(13))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert_eq!(plan.operations.len(), 13);
    assert!(
        plan.operations[..12]
            .iter()
            .enumerate()
            .all(|(index, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                        statement_index,
                        declaration_ordinal,
                        ..
                    } if usize::try_from(*statement_index) == Ok(index + 1)
                        && usize::try_from(*declaration_ordinal) == Ok(index)
                )
            })
    );
    assert!(matches!(
        &plan.operations[12],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 13,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals == &[11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_thirteen_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 14];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
            values[7] = Empty {};
            values[8] = Empty {};
            values[9] = Empty {};
            values[10] = Empty {};
            values[11] = Empty {};
            values[12] = Empty {};
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("thirteen-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 13);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(14))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert_eq!(plan.operations.len(), 14);
    assert!(
        plan.operations[..13]
            .iter()
            .enumerate()
            .all(|(index, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                        statement_index,
                        declaration_ordinal,
                        ..
                    } if usize::try_from(*statement_index) == Ok(index + 1)
                        && usize::try_from(*declaration_ordinal) == Ok(index)
                )
            })
    );
    assert!(matches!(
        &plan.operations[13],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 14,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals
            == &[12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_fourteen_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 15];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
            values[7] = Empty {};
            values[8] = Empty {};
            values[9] = Empty {};
            values[10] = Empty {};
            values[11] = Empty {};
            values[12] = Empty {};
            values[13] = Empty {};
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("fourteen-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 14);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(15))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert_eq!(plan.operations.len(), 15);
    assert!(
        plan.operations[..14]
            .iter()
            .enumerate()
            .all(|(index, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                        statement_index,
                        declaration_ordinal,
                        ..
                    } if usize::try_from(*statement_index) == Ok(index + 1)
                        && usize::try_from(*declaration_ordinal) == Ok(index)
                )
            })
    );
    assert!(matches!(
        &plan.operations[14],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 15,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals
            == &[13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_fifteen_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 16];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
            values[7] = Empty {};
            values[8] = Empty {};
            values[9] = Empty {};
            values[10] = Empty {};
            values[11] = Empty {};
            values[12] = Empty {};
            values[13] = Empty {};
            values[14] = Empty {};
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("fifteen-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 15);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(16))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert_eq!(plan.operations.len(), 16);
    assert!(
        plan.operations[..15]
            .iter()
            .enumerate()
            .all(|(index, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                        statement_index,
                        declaration_ordinal,
                        ..
                    } if usize::try_from(*statement_index) == Ok(index + 1)
                        && usize::try_from(*declaration_ordinal) == Ok(index)
                )
            })
    );
    assert!(matches!(
        &plan.operations[15],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 16,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals
            == &[14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_sixteen_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 17];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
            values[7] = Empty {};
            values[8] = Empty {};
            values[9] = Empty {};
            values[10] = Empty {};
            values[11] = Empty {};
            values[12] = Empty {};
            values[13] = Empty {};
            values[14] = Empty {};
            values[15] = Empty {};
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("sixteen-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 16);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(17))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert_eq!(plan.operations.len(), 17);
    assert!(
        plan.operations[..16]
            .iter()
            .enumerate()
            .all(|(index, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                        statement_index,
                        declaration_ordinal,
                        ..
                    } if usize::try_from(*statement_index) == Ok(index + 1)
                        && usize::try_from(*declaration_ordinal) == Ok(index)
                )
            })
    );
    assert!(matches!(
        &plan.operations[16],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 17,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals
            == &[15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_seventeen_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 18];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
            values[7] = Empty {};
            values[8] = Empty {};
            values[9] = Empty {};
            values[10] = Empty {};
            values[11] = Empty {};
            values[12] = Empty {};
            values[13] = Empty {};
            values[14] = Empty {};
            values[15] = Empty {};
            values[16] = Empty {};
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("seventeen-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 17);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(18))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert_eq!(plan.operations.len(), 18);
    assert!(
        plan.operations[..17]
            .iter()
            .enumerate()
            .all(|(index, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                        statement_index,
                        declaration_ordinal,
                        ..
                    } if usize::try_from(*statement_index) == Ok(index + 1)
                        && usize::try_from(*declaration_ordinal) == Ok(index)
                )
            })
    );
    assert!(matches!(
        &plan.operations[17],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 18,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals
            == &[16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_eighteen_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 19];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
            values[7] = Empty {};
            values[8] = Empty {};
            values[9] = Empty {};
            values[10] = Empty {};
            values[11] = Empty {};
            values[12] = Empty {};
            values[13] = Empty {};
            values[14] = Empty {};
            values[15] = Empty {};
            values[16] = Empty {};
            values[17] = Empty {};
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("eighteen-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 18);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(19))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert_eq!(plan.operations.len(), 19);
    assert!(
        plan.operations[..18]
            .iter()
            .enumerate()
            .all(|(index, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                        statement_index,
                        declaration_ordinal,
                        ..
                    } if usize::try_from(*statement_index) == Ok(index + 1)
                        && usize::try_from(*declaration_ordinal) == Ok(index)
                )
            })
    );
    assert!(matches!(
        &plan.operations[18],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 19,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals
            == &[17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_nineteen_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 20];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
            values[7] = Empty {};
            values[8] = Empty {};
            values[9] = Empty {};
            values[10] = Empty {};
            values[11] = Empty {};
            values[12] = Empty {};
            values[13] = Empty {};
            values[14] = Empty {};
            values[15] = Empty {};
            values[16] = Empty {};
            values[17] = Empty {};
            values[18] = Empty {};
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("nineteen-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 19);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(20))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert_eq!(plan.operations.len(), 20);
    assert!(
        plan.operations[..19]
            .iter()
            .enumerate()
            .all(|(index, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                        statement_index,
                        declaration_ordinal,
                        ..
                    } if usize::try_from(*statement_index) == Ok(index + 1)
                        && usize::try_from(*declaration_ordinal) == Ok(index)
                )
            })
    );
    assert!(matches!(
        &plan.operations[19],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 20,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals
            == &[18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_twenty_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 21];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
            values[7] = Empty {};
            values[8] = Empty {};
            values[9] = Empty {};
            values[10] = Empty {};
            values[11] = Empty {};
            values[12] = Empty {};
            values[13] = Empty {};
            values[14] = Empty {};
            values[15] = Empty {};
            values[16] = Empty {};
            values[17] = Empty {};
            values[18] = Empty {};
            values[19] = Empty {};
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("twenty-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 20);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(21))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert_eq!(plan.operations.len(), 21);
    assert!(
        plan.operations[..20]
            .iter()
            .enumerate()
            .all(|(index, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                        statement_index,
                        declaration_ordinal,
                        ..
                    } if usize::try_from(*statement_index) == Ok(index + 1)
                        && usize::try_from(*declaration_ordinal) == Ok(index)
                )
            })
    );
    assert!(matches!(
        &plan.operations[20],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 21,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals
            == &[19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_twenty_one_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 22];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
            values[7] = Empty {};
            values[8] = Empty {};
            values[9] = Empty {};
            values[10] = Empty {};
            values[11] = Empty {};
            values[12] = Empty {};
            values[13] = Empty {};
            values[14] = Empty {};
            values[15] = Empty {};
            values[16] = Empty {};
            values[17] = Empty {};
            values[18] = Empty {};
            values[19] = Empty {};
            values[20] = Empty {};
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("twenty-one-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 21);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(22))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert_eq!(plan.operations.len(), 22);
    assert!(
        plan.operations[..21]
            .iter()
            .enumerate()
            .all(|(index, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                        statement_index,
                        declaration_ordinal,
                        ..
                    } if usize::try_from(*statement_index) == Ok(index + 1)
                        && usize::try_from(*declaration_ordinal) == Ok(index)
                )
            })
    );
    assert!(matches!(
        &plan.operations[21],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 22,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals
            == &[20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_twenty_two_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 23];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
            values[7] = Empty {};
            values[8] = Empty {};
            values[9] = Empty {};
            values[10] = Empty {};
            values[11] = Empty {};
            values[12] = Empty {};
            values[13] = Empty {};
            values[14] = Empty {};
            values[15] = Empty {};
            values[16] = Empty {};
            values[17] = Empty {};
            values[18] = Empty {};
            values[19] = Empty {};
            values[20] = Empty {};
            values[21] = Empty {};
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("twenty-two-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 22);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(23))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert_eq!(plan.operations.len(), 23);
    assert!(
        plan.operations[..22]
            .iter()
            .enumerate()
            .all(|(index, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                        statement_index,
                        declaration_ordinal,
                        ..
                    } if usize::try_from(*statement_index) == Ok(index + 1)
                        && usize::try_from(*declaration_ordinal) == Ok(index)
                )
            })
    );
    assert!(matches!(
        &plan.operations[22],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 23,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals
            == &[21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_twenty_three_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 24];
            values[0] = Empty {};
            values[1] = Empty {};
            values[2] = Empty {};
            values[3] = Empty {};
            values[4] = Empty {};
            values[5] = Empty {};
            values[6] = Empty {};
            values[7] = Empty {};
            values[8] = Empty {};
            values[9] = Empty {};
            values[10] = Empty {};
            values[11] = Empty {};
            values[12] = Empty {};
            values[13] = Empty {};
            values[14] = Empty {};
            values[15] = Empty {};
            values[16] = Empty {};
            values[17] = Empty {};
            values[18] = Empty {};
            values[19] = Empty {};
            values[20] = Empty {};
            values[21] = Empty {};
            values[22] = Empty {};
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("twenty-three-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 23);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(24))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert_eq!(plan.operations.len(), 24);
    assert!(
        plan.operations[..23]
            .iter()
            .enumerate()
            .all(|(index, operation)| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                        statement_index,
                        declaration_ordinal,
                        ..
                    } if usize::try_from(*statement_index) == Ok(index + 1)
                        && usize::try_from(*declaration_ordinal) == Ok(index)
                )
            })
    );
    assert!(matches!(
        &plan.operations[23],
        CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 24,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals
            == &[22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn wider_construction_prefix_rejects_missing_or_reordered_establishments() {
    for (name, body) in [
        (
            "missing",
            r#"
                let mut values: [Empty; 4];
                values[0] = Empty {};
                values[1] = Empty {};
            "#,
        ),
        (
            "reordered",
            r#"
                let mut values: [Empty; 4];
                values[0] = Empty {};
                values[2] = Empty {};
                values[1] = Empty {};
            "#,
        ),
        (
            "missing_five",
            r#"
                let mut values: [Empty; 5];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
            "#,
        ),
        (
            "reordered_five",
            r#"
                let mut values: [Empty; 5];
                values[0] = Empty {};
                values[1] = Empty {};
                values[3] = Empty {};
                values[2] = Empty {};
            "#,
        ),
        (
            "missing_six",
            r#"
                let mut values: [Empty; 6];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
            "#,
        ),
        (
            "reordered_six",
            r#"
                let mut values: [Empty; 6];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[4] = Empty {};
                values[3] = Empty {};
            "#,
        ),
        (
            "missing_seven",
            r#"
                let mut values: [Empty; 7];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
            "#,
        ),
        (
            "reordered_seven",
            r#"
                let mut values: [Empty; 7];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[5] = Empty {};
                values[4] = Empty {};
            "#,
        ),
        (
            "missing_eight",
            r#"
                let mut values: [Empty; 8];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
            "#,
        ),
        (
            "reordered_eight",
            r#"
                let mut values: [Empty; 8];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[6] = Empty {};
                values[5] = Empty {};
            "#,
        ),
        (
            "missing_nine",
            r#"
                let mut values: [Empty; 9];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
            "#,
        ),
        (
            "reordered_nine",
            r#"
                let mut values: [Empty; 9];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[7] = Empty {};
                values[6] = Empty {};
            "#,
        ),
        (
            "missing_ten",
            r#"
                let mut values: [Empty; 10];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
            "#,
        ),
        (
            "reordered_ten",
            r#"
                let mut values: [Empty; 10];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[8] = Empty {};
                values[7] = Empty {};
            "#,
        ),
        (
            "missing_eleven",
            r#"
                let mut values: [Empty; 11];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
            "#,
        ),
        (
            "reordered_eleven",
            r#"
                let mut values: [Empty; 11];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[9] = Empty {};
                values[8] = Empty {};
            "#,
        ),
        (
            "missing_twelve",
            r#"
                let mut values: [Empty; 12];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
            "#,
        ),
        (
            "reordered_twelve",
            r#"
                let mut values: [Empty; 12];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[10] = Empty {};
                values[9] = Empty {};
            "#,
        ),
        (
            "missing_thirteen",
            r#"
                let mut values: [Empty; 13];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
            "#,
        ),
        (
            "reordered_thirteen",
            r#"
                let mut values: [Empty; 13];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[11] = Empty {};
                values[10] = Empty {};
            "#,
        ),
        (
            "missing_fourteen",
            r#"
                let mut values: [Empty; 14];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
            "#,
        ),
        (
            "reordered_fourteen",
            r#"
                let mut values: [Empty; 14];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[12] = Empty {};
                values[11] = Empty {};
            "#,
        ),
        (
            "missing_fifteen",
            r#"
                let mut values: [Empty; 15];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
            "#,
        ),
        (
            "reordered_fifteen",
            r#"
                let mut values: [Empty; 15];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[13] = Empty {};
                values[12] = Empty {};
            "#,
        ),
        (
            "missing_sixteen",
            r#"
                let mut values: [Empty; 16];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
            "#,
        ),
        (
            "reordered_sixteen",
            r#"
                let mut values: [Empty; 16];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[14] = Empty {};
                values[13] = Empty {};
            "#,
        ),
        (
            "missing_seventeen",
            r#"
                let mut values: [Empty; 17];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[14] = Empty {};
            "#,
        ),
        (
            "reordered_seventeen",
            r#"
                let mut values: [Empty; 17];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[15] = Empty {};
                values[14] = Empty {};
            "#,
        ),
        (
            "missing_eighteen",
            r#"
                let mut values: [Empty; 18];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[14] = Empty {};
                values[15] = Empty {};
            "#,
        ),
        (
            "reordered_eighteen",
            r#"
                let mut values: [Empty; 18];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[14] = Empty {};
                values[16] = Empty {};
                values[15] = Empty {};
            "#,
        ),
        (
            "missing_nineteen",
            r#"
                let mut values: [Empty; 19];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[14] = Empty {};
                values[15] = Empty {};
                values[16] = Empty {};
            "#,
        ),
        (
            "reordered_nineteen",
            r#"
                let mut values: [Empty; 19];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[14] = Empty {};
                values[15] = Empty {};
                values[17] = Empty {};
                values[16] = Empty {};
            "#,
        ),
        (
            "missing_twenty",
            r#"
                let mut values: [Empty; 20];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[14] = Empty {};
                values[15] = Empty {};
                values[16] = Empty {};
                values[17] = Empty {};
            "#,
        ),
        (
            "reordered_twenty",
            r#"
                let mut values: [Empty; 20];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[14] = Empty {};
                values[15] = Empty {};
                values[16] = Empty {};
                values[18] = Empty {};
                values[17] = Empty {};
            "#,
        ),
        (
            "missing_twenty_one",
            r#"
                let mut values: [Empty; 21];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[14] = Empty {};
                values[15] = Empty {};
                values[16] = Empty {};
                values[17] = Empty {};
                values[18] = Empty {};
            "#,
        ),
        (
            "reordered_twenty_one",
            r#"
                let mut values: [Empty; 21];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[14] = Empty {};
                values[15] = Empty {};
                values[16] = Empty {};
                values[17] = Empty {};
                values[19] = Empty {};
                values[18] = Empty {};
            "#,
        ),
        (
            "missing_twenty_two",
            r#"
                let mut values: [Empty; 22];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[14] = Empty {};
                values[15] = Empty {};
                values[16] = Empty {};
                values[17] = Empty {};
                values[18] = Empty {};
                values[19] = Empty {};
            "#,
        ),
        (
            "reordered_twenty_two",
            r#"
                let mut values: [Empty; 22];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[14] = Empty {};
                values[15] = Empty {};
                values[16] = Empty {};
                values[17] = Empty {};
                values[18] = Empty {};
                values[20] = Empty {};
                values[19] = Empty {};
            "#,
        ),
        (
            "missing_twenty_three",
            r#"
                let mut values: [Empty; 23];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[14] = Empty {};
                values[15] = Empty {};
                values[16] = Empty {};
                values[17] = Empty {};
                values[18] = Empty {};
                values[19] = Empty {};
                values[20] = Empty {};
            "#,
        ),
        (
            "reordered_twenty_three",
            r#"
                let mut values: [Empty; 23];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[14] = Empty {};
                values[15] = Empty {};
                values[16] = Empty {};
                values[17] = Empty {};
                values[18] = Empty {};
                values[19] = Empty {};
                values[21] = Empty {};
                values[20] = Empty {};
            "#,
        ),
        (
            "missing_twenty_four",
            r#"
                let mut values: [Empty; 24];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[14] = Empty {};
                values[15] = Empty {};
                values[16] = Empty {};
                values[17] = Empty {};
                values[18] = Empty {};
                values[19] = Empty {};
                values[20] = Empty {};
                values[21] = Empty {};
            "#,
        ),
        (
            "reordered_twenty_four",
            r#"
                let mut values: [Empty; 24];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[14] = Empty {};
                values[15] = Empty {};
                values[16] = Empty {};
                values[17] = Empty {};
                values[18] = Empty {};
                values[19] = Empty {};
                values[20] = Empty {};
                values[22] = Empty {};
                values[21] = Empty {};
            "#,
        ),
        (
            "length_twenty_five",
            r#"
                let mut values: [Empty; 25];
                values[0] = Empty {};
                values[1] = Empty {};
                values[2] = Empty {};
                values[3] = Empty {};
                values[4] = Empty {};
                values[5] = Empty {};
                values[6] = Empty {};
                values[7] = Empty {};
                values[8] = Empty {};
                values[9] = Empty {};
                values[10] = Empty {};
                values[11] = Empty {};
                values[12] = Empty {};
                values[13] = Empty {};
                values[14] = Empty {};
                values[15] = Empty {};
                values[16] = Empty {};
                values[17] = Empty {};
                values[18] = Empty {};
                values[19] = Empty {};
                values[20] = Empty {};
                values[21] = Empty {};
                values[22] = Empty {};
                values[23] = Empty {};
            "#,
        ),
    ] {
        let checked = checked(&format!(
            "data Empty {{}} data Root {{}} machine Root::{name}() {{ {body} }}"
        ));
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine_named(&checked, name))
                .is_none(),
            "{name} is outside the exact construction-prefix carrier"
        );
    }
}

#[test]
fn retains_source_ordered_direct_field_transfers_with_exact_residual_affine_cleanup() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Quartet { first: Token; second: Token; third: Token; fourth: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::enter(value: Quartet) {
            Sink::take(value.third);
            Sink::take(value.first);
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .is_none(),
        "path-sensitive cleanup must not leak through the root-only terminal lane"
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine)
        .expect("direct-field transfers with exact affine sibling cleanup");
    let moved_paths = plan.machine.operations[..2]
        .iter()
        .map(|operation| match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                claim_transfers,
                ..
            } if structural_arguments.len() == 1 && claim_transfers.is_empty() => {
                assert_eq!(structural_arguments[0].source_parameter_index(), Some(0));
                structural_arguments[0].path.clone()
            }
            _ => panic!("partial cleanup requires source-ordered direct Unit calls"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        moved_paths,
        vec![
            vec![CheckedUnitStructuralPathSegment::Field("third".to_owned())],
            vec![CheckedUnitStructuralPathSegment::Field("first".to_owned())],
        ]
    );
    assert!(matches!(
        plan.machine.operations.last(),
        Some(CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 2,
            trivial_affine_discards,
            ..
        }) if trivial_affine_discards.is_empty()
    ));
    assert_eq!(plan.residual_affine_discards.len(), 2);
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| {
                assert_eq!(discard.source_parameter_index, 0);
                assert!(discard.type_identity.contains("Token"));
                discard.path.clone()
            })
            .collect::<Vec<_>>(),
        vec![
            vec![CheckedUnitStructuralPathSegment::Field("fourth".to_owned())],
            vec![CheckedUnitStructuralPathSegment::Field("second".to_owned())],
        ]
    );
}

#[test]
fn retains_mixed_prefix_disjoint_field_transfers_with_maximal_residual_cleanup() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Deep { low: Token; middle: Token; high: Token; }
        data Branch { head: Token; deep: Deep; tail: Token; }
        data Outer { first: Token; left: Branch; right: Branch; last: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::enter(value: Outer) {
            Sink::take(value.left.deep.middle);
            Sink::take(value.right.tail);
            Sink::take(value.first);
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("mixed disjoint field moves have an exact maximal residual plan");
    assert_eq!(
        plan.machine.operations[..3]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => structural_arguments[0].path.clone(),
                _ => panic!("partial cleanup begins with source-ordered Unit calls"),
            })
            .collect::<Vec<_>>(),
        vec![
            vec![
                CheckedUnitStructuralPathSegment::Field("left".to_owned()),
                CheckedUnitStructuralPathSegment::Field("deep".to_owned()),
                CheckedUnitStructuralPathSegment::Field("middle".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("right".to_owned()),
                CheckedUnitStructuralPathSegment::Field("tail".to_owned()),
            ],
            vec![CheckedUnitStructuralPathSegment::Field("first".to_owned())],
        ]
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| discard.path.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![CheckedUnitStructuralPathSegment::Field("last".to_owned())],
            vec![
                CheckedUnitStructuralPathSegment::Field("right".to_owned()),
                CheckedUnitStructuralPathSegment::Field("deep".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("right".to_owned()),
                CheckedUnitStructuralPathSegment::Field("head".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("left".to_owned()),
                CheckedUnitStructuralPathSegment::Field("tail".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("left".to_owned()),
                CheckedUnitStructuralPathSegment::Field("deep".to_owned()),
                CheckedUnitStructuralPathSegment::Field("high".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("left".to_owned()),
                CheckedUnitStructuralPathSegment::Field("deep".to_owned()),
                CheckedUnitStructuralPathSegment::Field("low".to_owned()),
            ],
            vec![
                CheckedUnitStructuralPathSegment::Field("left".to_owned()),
                CheckedUnitStructuralPathSegment::Field("head".to_owned()),
            ],
        ]
    );
}

#[test]
fn partial_cleanup_fails_closed_outside_finite_structural_record_paths() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data One { right: Token; }
        data Inner { right: Token; }
        data Outer { left: Token; inner: Inner; }
        data Pair { left: Token; right: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::missing(value: One) {
            Sink::take(value.right);
        }
        machine Root::complete(value: Pair) {
            Sink::take(value.right);
            Sink::take(value.left);
        }
        "#,
    );

    for machine in ["missing", "complete"] {
        assert!(
            checked
                .facts
                .flow
                .terminal_partial_affine_unit_cleanups
                .for_machine(machine_named(&checked, machine))
                .is_none(),
            "`{machine}` must remain outside the exact partial-cleanup slice"
        );
    }
}

#[test]
fn mixed_scalar_and_affine_record_retains_only_structural_residual_cleanup() {
    let checked = checked(
        r#"
        domain [u8; 3]::Utf8
        requires
            valid_utf8(self);
        domain [u8; 8]::Utf8
        requires
            valid_utf8(self);
        data Token { value: u64; }
        data Mixed {
            before: u8;
            before_bytes: [u8; 3] in Utf8;
            before_float: f32;
            left: Token;
            between: bool;
            between_bytes: [u8; 8] in Utf8;
            between_float: f64;
            right: Token;
            after: u64;
        }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::enter(value: Mixed) {
            Sink::take(value.right);
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("scalar fields participate in shape identity without acquiring cleanup");
    let root_identity = &plan.machine.structural_parameters[0].type_identity;
    let root = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .structural_types
        .iter()
        .find(|shape| &shape.identity == root_identity)
        .expect("mixed root shape");
    let CheckedUnitStructuralTypeShape::Record { fields } = &root.shape else {
        panic!("mixed root remains a record")
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| {
                (
                    field.identity.as_str(),
                    matches!(field.field_type, CheckedUnitStructuralFieldType::Scalar(_)),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            ("before", true),
            ("before_bytes", false),
            ("before_float", true),
            ("left", false),
            ("between", true),
            ("between_bytes", false),
            ("between_float", true),
            ("right", false),
            ("after", true),
        ]
    );
    assert_eq!(
        fields
            .iter()
            .filter_map(|field| match field.field_type {
                CheckedUnitStructuralFieldType::Scalar(PrimitiveType::F32) => {
                    Some((field.identity.as_str(), PrimitiveType::F32))
                }
                CheckedUnitStructuralFieldType::Scalar(PrimitiveType::F64) => {
                    Some((field.identity.as_str(), PrimitiveType::F64))
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![
            ("before_float", PrimitiveType::F32),
            ("between_float", PrimitiveType::F64),
        ],
        "both exact IEEE source formats remain ordered checked shape identity"
    );
    assert_eq!(
        fields
            .iter()
            .filter_map(|field| match field.field_type {
                CheckedUnitStructuralFieldType::ByteSequence(carrier) => {
                    Some((field.identity.as_str(), carrier))
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![
            (
                "before_bytes",
                psi_checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity: 3 },
            ),
            (
                "between_bytes",
                psi_checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity: 8 },
            ),
        ],
        "bounded byte carriers retain exact source capacities and declaration order"
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| discard.path.clone())
            .collect::<Vec<_>>(),
        vec![vec![CheckedUnitStructuralPathSegment::Field(
            "left".to_owned()
        )]],
        "scalar, float, and bounded-byte fields are cleanup-free even before, between, and after affine fields"
    );
}

#[test]
fn partial_cleanup_keeps_borrowed_byte_views_fenced() {
    let checked = checked(
        r#"
        domain [u8]::Utf8
        requires
            valid_utf8(self);
        data Token { value: u64; }
        data Mixed { view: &[u8] in Utf8; left: Token; right: Token; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::enter(value: Mixed) {
            Sink::take(value.right);
        }
        "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .for_machine(machine_named(&checked, "enter"))
            .is_none(),
        "a borrowed byte view needs explicit loan retirement and cannot enter no-code cleanup"
    );
}

#[test]
fn two_element_affine_array_moves_one_literal_index_and_discards_its_sibling() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::first(values: [Token; 2]) {
            Sink::take(values[0]);
        }
        machine Root::second(values: [Token; 2]) {
            Sink::take(values[1]);
        }
        "#,
    );
    for (machine, moved, residual) in [("first", 0, 1), ("second", 1, 0)] {
        let plan = checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .for_machine(machine_named(&checked, machine))
            .expect("one literal array move leaves one exact affine sibling");
        assert_eq!(plan.machine.operations.len(), 2);
        let CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        } = &plan.machine.operations[0]
        else {
            panic!("array cleanup starts with one ordinary Unit call")
        };
        assert!(claim_transfers.is_empty());
        assert_eq!(
            structural_arguments[0].path,
            [CheckedUnitStructuralPathSegment::FixedIndex(moved)]
        );
        assert_eq!(
            plan.residual_affine_discards[0].path,
            [CheckedUnitStructuralPathSegment::FixedIndex(residual)]
        );
        assert_eq!(plan.residual_affine_discards.len(), 1);
        assert_eq!(
            plan.residual_affine_discards[0].type_identity,
            structural_arguments[0].type_identity
        );
    }
}

#[test]
fn two_element_affine_array_may_move_both_elements_without_residual_cleanup() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::forward(values: [Token; 2]) {
            Sink::take(values[0]);
            Sink::take(values[1]);
        }
        machine Root::reverse(values: [Token; 2]) {
            Sink::take(values[1]);
            Sink::take(values[0]);
        }
        "#,
    );
    for (machine, expected_paths) in [("forward", [0, 1]), ("reverse", [1, 0])] {
        let plan = checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .for_machine(machine_named(&checked, machine))
            .expect("both exact array elements should transfer in authored order");
        assert!(plan.residual_affine_discards.is_empty());
        assert_eq!(plan.machine.operations.len(), 3);
        assert_eq!(
            plan.machine.operations[..2]
                .iter()
                .map(|operation| {
                    let CheckedUnitEffectOperationPlan::CallUnit {
                        structural_arguments,
                        claim_transfers,
                        ..
                    } = operation
                    else {
                        panic!("full array consumption contains only Unit calls before return")
                    };
                    assert!(claim_transfers.is_empty());
                    let [argument] = structural_arguments.as_slice() else {
                        panic!("each array move supplies one argument")
                    };
                    let [CheckedUnitStructuralPathSegment::FixedIndex(index)] =
                        argument.path.as_slice()
                    else {
                        panic!("each move retains one literal array index")
                    };
                    *index
                })
                .collect::<Vec<_>>(),
            expected_paths
        );
        assert!(matches!(
            plan.machine.operations[2],
            CheckedUnitEffectOperationPlan::ReturnUnit {
                ref trivial_affine_local_discard_ordinals,
                ref trivial_affine_discards,
                ..
            } if trivial_affine_local_discard_ordinals.is_empty()
                && trivial_affine_discards.is_empty()
        ));
    }
}

#[test]
fn three_element_affine_array_moves_two_indices_and_discards_the_sole_residual() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::middle(values: [Token; 3]) {
            Sink::take(values[2]);
            Sink::take(values[0]);
        }
        machine Root::last(values: [Token; 3]) {
            Sink::take(values[0]);
            Sink::take(values[1]);
        }
        machine Root::first(values: [Token; 3]) {
            Sink::take(values[1]);
            Sink::take(values[2]);
        }
        machine Root::one_move(values: [Token; 3]) {
            Sink::take(values[0]);
        }
        machine Root::all(values: [Token; 3]) {
            Sink::take(values[0]);
            Sink::take(values[1]);
            Sink::take(values[2]);
        }
        "#,
    );
    for (machine, expected_paths, residual) in [
        ("middle", [2, 0], 1),
        ("last", [0, 1], 2),
        ("first", [1, 2], 0),
    ] {
        let plan = checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .for_machine(machine_named(&checked, machine))
            .expect("two distinct moves from an affine triple leave one exact residual");
        assert_eq!(plan.machine.operations.len(), 3);
        assert_eq!(
            plan.machine.operations[..2]
                .iter()
                .map(|operation| {
                    let CheckedUnitEffectOperationPlan::CallUnit {
                        structural_arguments,
                        claim_transfers,
                        ..
                    } = operation
                    else {
                        panic!("triple cleanup contains Unit calls before return")
                    };
                    assert!(claim_transfers.is_empty());
                    let [CheckedUnitStructuralPathSegment::FixedIndex(index)] =
                        structural_arguments[0].path.as_slice()
                    else {
                        panic!("triple move retains one literal index")
                    };
                    *index
                })
                .collect::<Vec<_>>(),
            expected_paths
        );
        assert_eq!(plan.residual_affine_discards.len(), 1);
        assert_eq!(
            plan.residual_affine_discards[0].path,
            [CheckedUnitStructuralPathSegment::FixedIndex(residual)]
        );
    }
    let one_move = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "one_move"))
        .expect("one triple move leaves two statically ordered residuals");
    assert_eq!(one_move.machine.operations.len(), 2);
    assert_eq!(
        one_move
            .residual_affine_discards
            .iter()
            .map(|discard| match discard.path.as_slice() {
                [CheckedUnitStructuralPathSegment::FixedIndex(index)] => *index,
                _ => panic!("array residual is one literal index"),
            })
            .collect::<Vec<_>>(),
        vec![2, 1],
        "live array siblings clean in decreasing index order",
    );
    for machine in ["all"] {
        assert!(
            checked
                .facts
                .flow
                .terminal_partial_affine_unit_cleanups
                .for_machine(machine_named(&checked, machine))
                .is_none(),
            "three moves belong to a separate no-residual rung"
        );
    }
}

#[test]
fn affine_array_partial_cleanup_fences_other_lengths() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::one(values: [Token; 1]) {
            Sink::take(values[0]);
        }
        machine Root::five(values: [Token; 5]) {
            Sink::take(values[0]);
            Sink::take(values[1]);
        }
        "#,
    );
    for machine in ["one", "five"] {
        assert!(
            checked
                .facts
                .flow
                .terminal_partial_affine_unit_cleanups
                .for_machine(machine_named(&checked, machine))
                .is_none(),
            "`{machine}` remains outside the exact bounded array slice"
        );
    }
}

#[test]
fn four_element_affine_array_moves_two_indices_and_discards_the_complement_decreasing() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::outer(values: [Token; 4]) {
            Sink::take(values[1]);
            Sink::take(values[3]);
        }
        machine Root::inner(values: [Token; 4]) {
            Sink::take(values[2]);
            Sink::take(values[1]);
        }
        machine Root::one(values: [Token; 4]) {
            Sink::take(values[0]);
        }
        machine Root::three(values: [Token; 4]) {
            Sink::take(values[0]);
            Sink::take(values[1]);
            Sink::take(values[2]);
        }
        "#,
    );
    for (machine, moves, residuals) in [("outer", [1, 3], [2, 0]), ("inner", [2, 1], [3, 0])] {
        let plan = checked
            .facts
            .flow
            .terminal_partial_affine_unit_cleanups
            .for_machine(machine_named(&checked, machine))
            .expect("two quartet moves leave the exact decreasing complement");
        assert_eq!(
            plan.machine.operations[..2]
                .iter()
                .map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::CallUnit {
                        structural_arguments,
                        ..
                    } => match structural_arguments[0].path.as_slice() {
                        [CheckedUnitStructuralPathSegment::FixedIndex(index)] => *index,
                        _ => panic!("quartet move is one literal index"),
                    },
                    _ => panic!("quartet cleanup contains calls before return"),
                })
                .collect::<Vec<_>>(),
            moves,
            "authored move order is retained",
        );
        assert_eq!(
            plan.residual_affine_discards
                .iter()
                .map(|discard| match discard.path.as_slice() {
                    [CheckedUnitStructuralPathSegment::FixedIndex(index)] => *index,
                    _ => panic!("quartet residual is one literal index"),
                })
                .collect::<Vec<_>>(),
            residuals,
            "the compiler emits the live complement in decreasing index order",
        );
    }
    for machine in ["one", "three"] {
        assert!(
            checked
                .facts
                .flow
                .terminal_partial_affine_unit_cleanups
                .for_machine(machine_named(&checked, machine))
                .is_none(),
            "the quartet rung admits exactly two moves",
        );
    }
}

#[test]
fn nested_affine_arrays_discard_each_live_complement_in_decreasing_index_order() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Sink {}
        machine Sink::take(token: Token) {}
        data Root {}
        machine Root::nested(values: [[Token; 3]; 2]) {
            Sink::take(values[1][0]);
            Sink::take(values[0][1]);
        }
        machine Root::nested_four(values: [[Token; 4]; 2]) {
            Sink::take(values[1][3]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer(values: [[Token; 3]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][1]);
        }
        machine Root::one(values: [[Token; 3]; 2]) {
            Sink::take(values[1][2]);
        }
        machine Root::same_outer_four(values: [[Token; 4]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][3]);
        }
        machine Root::one_four(values: [[Token; 4]; 2]) {
            Sink::take(values[1][3]);
        }
        machine Root::nested_five(values: [[Token; 5]; 2]) {
            Sink::take(values[1][4]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer_five(values: [[Token; 5]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][4]);
        }
        machine Root::one_five(values: [[Token; 5]; 2]) {
            Sink::take(values[1][4]);
        }
        machine Root::nested_six(values: [[Token; 6]; 2]) {
            Sink::take(values[1][5]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer_six(values: [[Token; 6]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][5]);
        }
        machine Root::one_six(values: [[Token; 6]; 2]) {
            Sink::take(values[1][5]);
        }
        machine Root::nested_seven(values: [[Token; 7]; 2]) {
            Sink::take(values[1][6]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer_seven(values: [[Token; 7]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][6]);
        }
        machine Root::one_seven(values: [[Token; 7]; 2]) {
            Sink::take(values[1][6]);
        }
        machine Root::nested_eight(values: [[Token; 8]; 2]) {
            Sink::take(values[1][7]);
            Sink::take(values[0][1]);
        }
        machine Root::same_outer_eight(values: [[Token; 8]; 2]) {
            Sink::take(values[0][0]);
            Sink::take(values[0][7]);
        }
        machine Root::one_eight(values: [[Token; 8]; 2]) {
            Sink::take(values[1][7]);
        }
        machine Root::too_wide(values: [[Token; 9]; 2]) {
            Sink::take(values[1][8]);
            Sink::take(values[0][1]);
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested"))
        .expect("one leaf move per outer array leaves four exact residual leaves");
    let path = |path: &[CheckedUnitStructuralPathSegment]| match path {
        [
            CheckedUnitStructuralPathSegment::FixedIndex(outer),
            CheckedUnitStructuralPathSegment::FixedIndex(inner),
        ] => (*outer, *inner),
        _ => panic!("nested array leaf has exactly two literal indices"),
    };
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 0), (0, 1)],
        "authored move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![(1, 2), (1, 1), (0, 2), (0, 0)],
        "outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_four"))
        .expect("one leaf move per outer quartet leaves six exact residual leaves");
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested quartet cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 3), (0, 1)],
        "authored nested-quartet move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![(1, 2), (1, 1), (1, 0), (0, 3), (0, 2), (0, 0)],
        "quartet outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_five"))
        .expect("one leaf move per outer quintet leaves eight exact residual leaves");
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested quintet cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 4), (0, 1)],
        "authored nested-quintet move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
        "quintet outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_six"))
        .expect("one leaf move per outer sextet leaves ten exact residual leaves");
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested sextet cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 5), (0, 1)],
        "authored nested-sextet move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![
            (1, 4),
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 5),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
        "sextet outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_seven"))
        .expect("one leaf move per outer septet leaves twelve exact residual leaves");
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested septet cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 6), (0, 1)],
        "authored nested-septet move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![
            (1, 5),
            (1, 4),
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 6),
            (0, 5),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
        "septet outer and inner live complements both descend",
    );
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(&checked, "nested_eight"))
        .expect("one leaf move per outer octet leaves fourteen exact residual leaves");
    assert_eq!(
        plan.machine.operations[..2]
            .iter()
            .map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested octet cleanup contains calls before return"),
            })
            .collect::<Vec<_>>(),
        vec![(1, 7), (0, 1)],
        "authored nested-octet move order is retained",
    );
    assert_eq!(
        plan.residual_affine_discards
            .iter()
            .map(|discard| path(&discard.path))
            .collect::<Vec<_>>(),
        vec![
            (1, 6),
            (1, 5),
            (1, 4),
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 7),
            (0, 6),
            (0, 5),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
        "octet outer and inner live complements both descend",
    );
    for machine in [
        "same_outer",
        "one",
        "same_outer_four",
        "one_four",
        "same_outer_five",
        "one_five",
        "same_outer_six",
        "one_six",
        "same_outer_seven",
        "one_seven",
        "same_outer_eight",
        "one_eight",
        "too_wide",
    ] {
        assert!(
            checked
                .facts
                .flow
                .terminal_partial_affine_unit_cleanups
                .for_machine(machine_named(&checked, machine))
                .is_none(),
            "`{machine}` remains outside the exact nested-array rung",
        );
    }
}

#[test]
fn affine_triple_partial_cleanup_rejects_nominal_elements_and_qualification() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        data Sink {}
        machine Sink::take(token: Token) {}

        data NominalToken { value: u64; }
        machine NominalToken::drop(&mut self) {}
        machine Sink::take_nominal(token: NominalToken) {}

        domain [Token; 3]::Ready
        requires
            true;

        data Root {}
        machine Root::nominal(values: [NominalToken; 3]) {
            Sink::take_nominal(values[0]);
            Sink::take_nominal(values[1]);
        }
        machine Root::qualified(values: [Token; 3] in Ready) {
            Sink::take(values[0]);
            Sink::take(values[1]);
        }
        "#,
    );
    for machine in ["nominal", "qualified"] {
        assert!(
            checked
                .facts
                .flow
                .terminal_partial_affine_unit_cleanups
                .for_machine(machine_named(&checked, machine))
                .is_none(),
            "`{machine}` is outside the exact claim-free unqualified structural-affine carrier"
        );
    }
}

#[test]
fn unit_body_retains_empty_affine_local_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Token { value: u64; }
        data Root {}

        machine Root::cleanup(first: Token, second: Token) {
            let one: Empty = Empty {};
            let two: Empty = Empty {};
        }
        "#,
    );
    let machine = machine_named(&checked, "cleanup");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("bounded Unit local cleanup plan");
    assert_eq!(plan.trivial_affine_locals.len(), 2);
    assert_eq!(plan.trivial_affine_locals[0].declaration_ordinal, 0);
    assert_eq!(plan.trivial_affine_locals[1].declaration_ordinal, 1);
    assert!(matches!(
        plan.operations.as_slice(),
        [
            psi_checked_trees::CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                declaration_ordinal: 0,
                ..
            },
            psi_checked_trees::CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                declaration_ordinal: 1,
                ..
            },
            psi_checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit {
                trivial_affine_local_discard_ordinals,
                trivial_affine_discards,
                ..
            }
        ] if trivial_affine_local_discard_ordinals == &[1, 0]
            && trivial_affine_discards == &[1, 0]
    ));
}

#[test]
fn unit_body_affine_local_slice_fences_every_wider_local_shape() {
    let checked = checked(
        r#"
        data Empty {}
        data Nonempty { value: u64; }
        data Qualified {}
        domain Qualified::Owned;
        data Nominal {}
        machine Nominal::drop(&mut self) {}
        data Root {}

        machine Root::mutable_local() {
            let mut local: Empty = Empty {};
        }
        machine Root::nonempty_local() {
            let local: Nonempty = Nonempty { value: 1 };
        }
        machine Root::qualified_local(value: Qualified in Owned) {
            let local: Qualified in Owned = value;
        }
        machine Root::nominal_cleanup_local() {
            let local: Nominal = Nominal {};
        }
        machine Root::local_after_effect()
        reaches PortIo
        {
            asm { out 32, 7 }
            let local: Empty = Empty {};
        }
        "#,
    );

    for machine in [
        "mutable_local",
        "nonempty_local",
        "qualified_local",
        "nominal_cleanup_local",
        "local_after_effect",
    ] {
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine_named(&checked, machine))
                .is_none(),
            "`{machine}` must remain outside the bounded Unit affine-local slice"
        );
    }
}

#[test]
fn no_code_unit_and_scalar_returns_reject_reachable_nominal_cleanup() {
    let checked = checked(
        r#"
        data Nominal {}
        machine Nominal::drop(&mut self) {}
        data Wrapper<T> { value: T; }
        data Plain { value: u64; }
        data Root {}

        machine Root::plain_unit(value: Plain) {}
        machine Root::nested_unit(value: Wrapper<Nominal>) {}
        machine Root::nested_scalar(value: Wrapper<Nominal>) -> u64 { 7 }
        "#,
    );

    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "plain_unit"))
            .is_some(),
        "ordinary affine records remain eligible for checked no-code disposal"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "nested_unit"))
            .is_none(),
        "Unit return must not erase nested generic nominal cleanup"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, "nested_scalar"))
            .is_none(),
        "scalar return must not erase nested generic nominal cleanup"
    );
}

#[test]
fn scalar_return_retains_one_exact_nominal_cleanup_after_result_materialization() {
    let checked = checked(
        r#"
        data Helper {}
        machine Helper::touch() {}
        data Token { value: u64; }
        machine Token::drop(&mut self) { Helper::touch(); }
        data Root {}
        machine Root::measure(token: Token) -> u64 { 7u64 }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "measure"))
        .expect("scalar return retains its nominal cleanup");
    let [psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup)] =
        plan.cleanup_actions.as_slice()
    else {
        panic!("scalar return cleanup is exactly one nominal action")
    };
    assert_eq!(cleanup.source_parameter_index, 0);
    assert!(cleanup.requirements.is_empty());
    let target = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(cleanup.cleanup_machine)
        .expect("scalar nominal cleanup target remains executable");
    assert!(matches!(
        target.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::CallUnit { .. },
            CheckedUnitEffectOperationPlan::ReturnUnit { .. }
        ]
    ));
}

#[test]
fn scalar_return_retains_finite_all_nominal_cleanups_in_reverse_parameter_order() {
    let checked = checked(
        r#"
        data First { value: u64; }
        machine First::drop(&mut self) {}
        data Helper {}
        machine Helper::touch() {}
        data Second { value: u64; }
        machine Second::drop(&mut self) { Helper::touch(); }
        data Root {}
        machine Root::measure(first: First, second: Second) -> u64 { 7u64 }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "measure"))
        .expect("scalar return retains its complete nominal cleanup frontier");
    assert_eq!(
        plan.cleanup_actions
            .iter()
            .map(|action| match action {
                psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(
                    cleanup,
                ) => cleanup.source_parameter_index,
                psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(_) => {
                    panic!("the all-nominal case must not publish a trivial discard")
                }
            })
            .collect::<Vec<_>>(),
        vec![1, 0],
        "nominal scalar-return cleanup order is reverse authored order"
    );
    assert!(plan.cleanup_actions.iter().all(|action| matches!(
        action,
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(
            cleanup
        ) if cleanup.requirements.is_empty()
    )));
    let target_operation_lengths = plan
        .cleanup_actions
        .iter()
        .map(|action| {
            let psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(
                cleanup,
            ) = action
            else {
                unreachable!("all-nominal action list")
            };
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(cleanup.cleanup_machine)
                .expect("each nominal cleanup target remains executable")
                .operations
                .len()
        })
        .collect::<Vec<_>>();
    assert_eq!(target_operation_lengths, vec![2, 1]);
}

#[test]
fn scalar_return_retains_mixed_cleanup_actions_in_reverse_parameter_order() {
    let checked = checked(
        r#"
        data First { value: u64; }
        machine First::drop(&mut self) {}
        data Plain { value: u64; }
        data Second { value: u64; }
        machine Second::drop(&mut self) {}
        data Root {}
        machine Root::measure(first: First, plain: Plain, second: Second) -> u64 { 7u64 }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "measure"))
        .expect("the complete mixed scalar cleanup frontier is retained");
    let [
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(second),
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(1),
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(first),
    ] = plan.cleanup_actions.as_slice()
    else {
        panic!("mixed cleanup actions preserve one reverse-authored stream")
    };
    assert_eq!(second.source_parameter_index, 2);
    assert_eq!(first.source_parameter_index, 0);
}

#[test]
fn scalar_return_retains_contextual_requirements_for_finite_all_nominal_roots() {
    let checked = checked(
        r#"
        data Token { ready: bool; enabled: bool; observed: bool; }
        machine Token::drop(&mut self)
        requires
            self.ready;
            !self.enabled
        {}

        data Root {}
        machine Root::measure(first: Token, second: Token) -> u64
        requires
            first.observed;
            first.ready;
            !first.enabled;
            second.ready;
            !second.enabled
        { 7u64 }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "measure"))
        .expect("closed scalar return retains its contextual nominal cleanups");
    assert_eq!(
        plan.caller_requirements
            .iter()
            .map(|requirement| {
                (
                    requirement.source_parameter_index,
                    requirement.field_identity.as_str(),
                    requirement.expected,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (0, "enabled", false),
            (0, "observed", true),
            (0, "ready", true),
            (1, "enabled", false),
            (1, "ready", true),
        ],
        "caller facts remain canonical and retain an unrelated supported premise",
    );
    let [
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(second),
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(first),
    ] = plan.cleanup_actions.as_slice()
    else {
        panic!("contextual scalar cleanups remain in reverse authored root order")
    };
    assert_eq!(second.source_parameter_index, 1);
    assert_eq!(first.source_parameter_index, 0);
    for cleanup in [second, first] {
        assert_eq!(
            cleanup
                .requirements
                .iter()
                .map(|requirement| { (requirement.field_identity.as_str(), requirement.expected) })
                .collect::<Vec<_>>(),
            vec![("enabled", false), ("ready", true)],
        );
    }
}

#[test]
fn scalar_return_rejects_the_exact_nominal_root_missing_a_cleanup_premise() {
    let diagnostics = contextual_cleanup_diagnostics(
        r#"
        data Token { ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}

        data Plain { observed: bool; }
        data Root {}
        machine Root::measure(first: Token, plain: Plain, second: Token) -> u64
        requires first.ready, plain.observed
        { 7u64 }
        "#,
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove automatic cleanup requires at scalar return edge")
            && diagnostic.message.contains("missing second.ready == true")
            && diagnostic.message.contains("Token::drop")
    }));
}

#[test]
fn scalar_return_retains_mixed_contextual_facts_and_cleanup_order() {
    let mixed = checked(
        r#"
        data Token { ready: bool; enabled: bool; }
        machine Token::drop(&mut self)
        requires self.ready, !self.enabled
        {}
        data Plain { observed: bool; }
        data Root {}
        machine Root::measure(first: Token, plain: Plain, second: Token) -> u64
        requires
            first.ready;
            !first.enabled;
            plain.observed;
            second.ready;
            !second.enabled
        { 7u64 }
        "#,
    );
    let plan = mixed
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&mixed, "measure"))
        .expect("mixed contextual roots retain one complete checked cleanup stream");
    assert_eq!(
        plan.caller_requirements
            .iter()
            .map(|requirement| {
                (
                    requirement.source_parameter_index,
                    requirement.field_identity.as_str(),
                    requirement.expected,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (0, "enabled", false),
            (0, "ready", true),
            (1, "observed", true),
            (2, "enabled", false),
            (2, "ready", true),
        ],
        "supported trivial-root facts remain caller assumptions",
    );
    let [
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(second),
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(1),
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(first),
    ] = plan.cleanup_actions.as_slice()
    else {
        panic!("mixed contextual actions preserve reverse authored root order")
    };
    assert_eq!(second.source_parameter_index, 2);
    assert_eq!(first.source_parameter_index, 0);
    for cleanup in [second, first] {
        assert_eq!(
            cleanup
                .requirements
                .iter()
                .map(|requirement| { (requirement.field_identity.as_str(), requirement.expected) })
                .collect::<Vec<_>>(),
            vec![("enabled", false), ("ready", true)],
        );
    }
}

#[test]
fn contextual_scalar_cleanup_keeps_all_trivial_roots_fenced() {
    let all_trivial = checked(
        r#"
        data Plain { observed: bool; }
        data Root {}
        machine Root::measure(plain: Plain) -> u64
        requires plain.observed
        { 7u64 }
        "#,
    );
    assert!(
        all_trivial
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&all_trivial, "measure"))
            .is_none(),
        "contextual scalar cleanup remains tied to at least one nominal action",
    );
}

#[test]
fn nominal_scalar_cleanup_retains_finite_branch_free_primitive_locals() {
    let checked = checked(
        r#"
        data Token { ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}
        data Plain { observed: bool; }
        data Root {}
        machine Root::measure(token: Token, plain: Plain) -> u64
        requires token.ready, plain.observed
        {
            let base: u64 = 3u64 + 4u64;
            let doubled: u64 = base * 2u64;
            doubled
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "measure"))
        .expect("finite dependency-ordered scalar locals compose with mixed contextual cleanup");
    assert_eq!(
        plan.bindings
            .iter()
            .map(|binding| (binding.statement_ordinal, binding.primitive_type))
            .collect::<Vec<_>>(),
        vec![(0, PrimitiveType::U64), (1, PrimitiveType::U64)],
    );
    assert_eq!(plan.return_statement_ordinal, 2);
    assert_eq!(
        plan.caller_requirements
            .iter()
            .map(|requirement| {
                (
                    requirement.source_parameter_index,
                    requirement.field_identity.as_str(),
                    requirement.expected,
                )
            })
            .collect::<Vec<_>>(),
        vec![(0, "ready", true), (1, "observed", true)],
    );
    let [
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(1),
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup),
    ] = plan.cleanup_actions.as_slice()
    else {
        panic!("mixed cleanup remains reverse-authored after the scalar binding prefix")
    };
    assert_eq!(cleanup.source_parameter_index, 0);
    assert_eq!(
        cleanup
            .requirements
            .iter()
            .map(|requirement| (requirement.field_identity.as_str(), requirement.expected))
            .collect::<Vec<_>>(),
        vec![("ready", true)],
    );
}

#[test]
fn nominal_scalar_cleanup_retains_interleaved_scalar_inputs_before_locals() {
    let checked = checked(
        r#"
        data Token { ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}
        data Plain { observed: bool; }
        data Root {}
        machine Root::measure(
            first: Token,
            offset: u64,
            plain: Plain,
            scale: u64,
            second: Token
        ) -> u64
        requires first.ready, plain.observed, second.ready
        {
            let shifted: u64 = offset ^ 1u64;
            let scaled: u64 = shifted | scale;
            scaled
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "measure"))
        .expect("direct scalar inputs compose with branch-free mixed contextual cleanup");
    assert_eq!(
        plan.structural_parameters
            .iter()
            .map(|parameter| parameter.position)
            .collect::<Vec<_>>(),
        vec![0, 2, 4],
    );
    assert_eq!(
        plan.scalar_parameters
            .iter()
            .map(|parameter| (parameter.source_position, parameter.primitive_type))
            .collect::<Vec<_>>(),
        vec![(1, PrimitiveType::U64), (3, PrimitiveType::U64)],
        "scalar inputs retain authored positions in dense scalar order",
    );
    let mut complete_partition = plan
        .structural_parameters
        .iter()
        .map(|parameter| parameter.position)
        .chain(
            plan.scalar_parameters
                .iter()
                .map(|parameter| parameter.source_position),
        )
        .collect::<Vec<_>>();
    complete_partition.sort_unstable();
    assert_eq!(complete_partition, vec![0, 1, 2, 3, 4]);
    assert_eq!(
        plan.bindings
            .iter()
            .map(|binding| binding.statement_ordinal)
            .collect::<Vec<_>>(),
        vec![0, 1],
    );
    assert_eq!(plan.return_statement_ordinal, 2);

    let shifted = checked
        .facts
        .values
        .scalar_expressions
        .expression_at(
            plan.state,
            0,
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
        )
        .expect("first local expression");
    assert!(matches!(
        shifted,
        CheckedScalarExpression::IntegerBinary { left, .. }
            if matches!(left.as_ref(), CheckedScalarExpression::Parameter { position: 0, .. })
    ));
    let scaled = checked
        .facts
        .values
        .scalar_expressions
        .expression_at(
            plan.state,
            1,
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 1 },
        )
        .expect("second local expression");
    assert!(matches!(
        scaled,
        CheckedScalarExpression::IntegerBinary { left, right, .. }
            if matches!(left.as_ref(), CheckedScalarExpression::Local { position: 2, .. })
                && matches!(right.as_ref(), CheckedScalarExpression::Parameter { position: 1, .. })
    ));
    let returned = checked
        .facts
        .values
        .scalar_expressions
        .expression_at(plan.state, 2, CheckedScalarExpressionRole::Return)
        .expect("return expression");
    assert!(matches!(
        returned,
        CheckedScalarExpression::Local { position: 3, .. }
    ));
    assert_eq!(
        plan.caller_requirements
            .iter()
            .map(|requirement| {
                (
                    requirement.source_parameter_index,
                    requirement.field_identity.as_str(),
                    requirement.expected,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (0, "ready", true),
            (2, "observed", true),
            (4, "ready", true)
        ],
    );
    let [
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(second),
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(2),
        psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(first),
    ] = plan.cleanup_actions.as_slice()
    else {
        panic!("cleanup retains reverse authored structural-root order")
    };
    assert_eq!(second.source_parameter_index, 4);
    assert_eq!(first.source_parameter_index, 0);
}

#[test]
fn nominal_scalar_cleanup_accepts_one_final_short_circuit_boolean_decision() {
    let checked = checked(
        r#"
        data Token {}
        machine Token::drop(&mut self) {}
        data Root {}

        machine Root::and_return(token: Token, left: bool, right: bool) -> bool {
            let inverted: bool = !right;
            left && inverted
        }
        machine Root::or_return(token: Token, left: bool, right: bool) -> bool {
            let inverted: bool = !right;
            left || inverted
        }
        "#,
    );

    for (machine, expected_or) in [("and_return", false), ("or_return", true)] {
        let plan = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, machine))
            .unwrap_or_else(|| panic!("`{machine}` should retain one final Boolean decision"));
        assert_eq!(plan.bindings.len(), 1);
        assert_eq!(plan.return_statement_ordinal, 1);
        assert_eq!(
            plan.scalar_parameters
                .iter()
                .map(|parameter| parameter.source_position)
                .collect::<Vec<_>>(),
            vec![1, 2],
        );
        assert!(matches!(
            plan.cleanup_actions.as_slice(),
            [psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(
                cleanup
            )] if cleanup.source_parameter_index == 0
        ));
        let returned = checked
            .facts
            .values
            .scalar_expressions
            .expression_at(plan.state, 1, CheckedScalarExpressionRole::Return)
            .expect("checked short-circuit return expression");
        assert!(match returned {
            CheckedScalarExpression::Boolean(expression) if expected_or => {
                matches!(expression.as_ref(), CheckedBooleanExpression::Or { .. })
            }
            CheckedScalarExpression::Boolean(expression) => {
                matches!(expression.as_ref(), CheckedBooleanExpression::And { .. })
            }
            _ => false,
        });
    }
}

#[test]
fn nominal_scalar_cleanup_retains_contextual_short_circuit_return() {
    let checked = checked(
        r#"
        data Token { ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}
        data Root {}

        machine Root::measure(token: Token, left: bool, right: bool) -> bool
        requires token.ready
        {
            left && right
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(machine_named(&checked, "measure"))
        .expect("contextual short-circuit cleanup retains its checked scalar-return plan");
    assert_eq!(plan.caller_requirements.len(), 1);
    assert!(matches!(
        plan.cleanup_actions.as_slice(),
        [psi_checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup)]
            if cleanup.requirements.len() == 1
    ));
}

#[test]
fn retains_exact_empty_whole_root_nominal_cleanup_separately_from_trivial_discard() {
    let checked = checked(
        r#"
        data Token {}
        machine Token::drop(&mut self) {}

        data Root {}
        machine Root::enter(token: Token) {}
        "#,
    );
    let enter = machine_named(&checked, "enter");
    let drop = machine_named(&checked, "drop");

    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(enter)
            .is_none(),
        "nominal cleanup must not leak through the trivial-discard lane"
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(enter)
        .expect("exact empty nominal-cleanup plan");
    assert_eq!(plan.machine.structural_parameters.len(), 1);
    assert_eq!(
        plan.machine.structural_parameters[0].multiplicity,
        Multiplicity::Affine
    );
    assert!(
        plan.machine.structural_parameters[0]
            .qualifications
            .is_empty()
    );
    assert!(plan.machine.entry_claims.is_empty());
    assert!(matches!(
        plan.machine.operations.as_slice(),
        [CheckedUnitEffectOperationPlan::ReturnUnit {
            statement_index: 0,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        }] if trivial_affine_local_discard_ordinals.is_empty()
            && trivial_affine_discards.is_empty()
    ));
    assert_eq!(plan.cleanups[0].source_parameter_index, 0);
    assert_eq!(
        plan.cleanups[0].type_identity,
        plan.machine.structural_parameters[0].type_identity
    );
    assert_eq!(plan.cleanups[0].cleanup_machine, drop);
    assert_eq!(
        plan.cleanups[0].cleanup_state,
        checked.machine_states(
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == drop)
                .expect("drop machine"),
        )[0]
        .symbol
    );
    let token_shape = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .structural_types
        .iter()
        .find(|shape| shape.identity == plan.cleanups[0].type_identity)
        .expect("cleanup type shape");
    assert!(record_fields(token_shape).is_empty());
}

#[test]
fn nominal_cleanup_uses_exact_attached_symbol_when_spelling_is_spoofed() {
    let source = r#"
        boundary trait PortIo {}
        data First {}
        machine First::drop(&mut self) {}

        data Second {}
        machine Second::drop(&mut self) {}

        data Root {}
        machine Root::enter(value: First) {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let mut typed = lower_symbol_resolved_trees(&resolved).expect("type");

    let first_drop = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "First::drop")
        .expect("First cleanup")
        .symbol;
    let second_drop = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Second::drop")
        .expect("Second cleanup")
        .symbol;
    typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.symbol == second_drop)
        .expect("mutable Second cleanup")
        .attached_data = Some(psi_typed_trees::name::Identifier::generated("First"));

    let checked = lower_typed_trees(typed).expect("exact identity survives diagnostic spoofing");
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("First receives its exact cleanup");

    assert_eq!(plan.cleanups[0].cleanup_machine, first_drop);
    assert_ne!(plan.cleanups[0].cleanup_machine, second_drop);
}

#[test]
fn retains_exactly_one_executable_drop_in_a_two_root_nominal_cleanup_list() {
    let checked = checked(
        r#"
        data Helper {}
        machine Helper::touch() {}

        data First {}
        machine First::drop(&mut self) { Helper::touch(); }
        data Second {}
        machine Second::drop(&mut self) {}

        data Root {}
        machine Root::enter(first: First, second: Second) {}
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("one executable and one empty cleanup are retained");
    assert_eq!(
        plan.cleanups
            .iter()
            .map(|cleanup| cleanup.source_parameter_index)
            .collect::<Vec<_>>(),
        vec![1, 0]
    );
    let operation_counts = plan
        .cleanups
        .iter()
        .map(|cleanup| {
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(cleanup.cleanup_machine)
                .expect("cleanup has an exact Unit plan")
                .operations
                .len()
                - 1
        })
        .collect::<Vec<_>>();
    assert_eq!(operation_counts, vec![0, 1]);
}

#[test]
fn retains_two_executable_drop_bodies_with_distinct_helpers() {
    let checked = checked(
        r#"
        data FirstHelper {}
        machine FirstHelper::touch() {}
        data SecondHelper {}
        machine SecondHelper::touch() {}

        data First {}
        machine First::drop(&mut self) { FirstHelper::touch(); }
        data Second {}
        machine Second::drop(&mut self) { SecondHelper::touch(); }

        data Root {}
        machine Root::enter(first: First, second: Second) {}
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("both bounded executable cleanup actions are retained");
    assert_eq!(
        plan.cleanups
            .iter()
            .map(|cleanup| cleanup.source_parameter_index)
            .collect::<Vec<_>>(),
        vec![1, 0]
    );
    let cleanup_targets = plan
        .cleanups
        .iter()
        .map(|cleanup| {
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(cleanup.cleanup_machine)
                .expect("cleanup target")
        })
        .collect::<Vec<_>>();
    assert_ne!(cleanup_targets[0].machine, cleanup_targets[1].machine);
    assert!(
        cleanup_targets
            .iter()
            .all(|target| target.operations.len() == 2)
    );
    let helper_targets = cleanup_targets
        .iter()
        .map(|target| match &target.operations[0] {
            CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. } => *target_machine,
            _ => panic!("executable cleanup starts with its helper call"),
        })
        .collect::<Vec<_>>();
    assert_ne!(helper_targets[0], helper_targets[1]);
}

#[test]
fn retains_five_call_executable_drop_body_in_source_order() {
    let checked = checked(
        r#"
        data FirstHelper {}
        machine FirstHelper::touch() {}
        data SecondHelper {}
        machine SecondHelper::touch() {}
        data ThirdHelper {}
        machine ThirdHelper::touch() {}
        data FourthHelper {}
        machine FourthHelper::touch() {}
        data FifthHelper {}
        machine FifthHelper::touch() {}

        data Token { value: u64; }
        machine Token::drop(&mut self) {
            FirstHelper::touch();
            SecondHelper::touch();
            ThirdHelper::touch();
            FourthHelper::touch();
            FifthHelper::touch();
        }

        data Root {}
        machine Root::enter(token: Token) {}
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("five-call executable cleanup is retained");
    let [cleanup] = plan.cleanups.as_slice() else {
        panic!("entry retains one nominal cleanup")
    };
    let target = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(cleanup.cleanup_machine)
        .expect("cleanup has an exact Unit plan");
    assert_eq!(target.operations.len(), 6);
    let helper_targets = target.operations[..5]
        .iter()
        .map(|operation| match operation {
            CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. } => *target_machine,
            _ => panic!("cleanup prefix remains a helper call"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        helper_targets,
        [
            "FirstHelper::touch",
            "SecondHelper::touch",
            "ThirdHelper::touch",
            "FourthHelper::touch",
            "FifthHelper::touch"
        ]
        .map(|name| machine_named(&checked, name))
    );
    assert!(matches!(
        target.operations[5],
        CheckedUnitEffectOperationPlan::ReturnUnit { .. }
    ));
}

#[test]
fn retains_one_relevant_primitive_scalar_whole_root_nominal_cleanup() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        machine Token::drop(&mut self) {}

        data Root {}
        machine Root::enter(token: Token) {}
        "#,
    );
    let enter = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(enter)
        .expect("one-scalar-field nominal-cleanup plan");
    let token_shape = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .structural_types
        .iter()
        .find(|shape| shape.identity == plan.cleanups[0].type_identity)
        .expect("cleanup type shape");
    let [field] = record_fields(token_shape) else {
        panic!("bounded nominal cleanup retains exactly one field")
    };
    assert_eq!(field.identity, "value");
    assert_eq!(field.relevance, BindingRelevance::Relevant);
    assert!(matches!(
        field.field_type,
        CheckedUnitStructuralFieldType::Scalar(PrimitiveType::U64)
    ));
    assert!(plan.machine.entry_claims.is_empty());
    assert!(matches!(
        plan.machine.operations.as_slice(),
        [CheckedUnitEffectOperationPlan::ReturnUnit {
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
            ..
        }] if trivial_affine_local_discard_ordinals.is_empty()
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_contextual_nominal_cleanup_boolean_requirement_at_the_return_edge() {
    let checked = checked(
        r#"
        data Token { ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}

        data Root {}
        machine Root::enter(token: Token)
        requires token.ready
        {}
        "#,
    );
    let enter = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(enter)
        .expect("contextually proved nominal cleanup plan");
    let [cleanup] = plan.cleanups.as_slice() else {
        panic!("one cleanup action")
    };
    let [requirement] = cleanup.requirements.as_slice() else {
        panic!("one contextual cleanup requirement")
    };
    assert_eq!(requirement.field_identity, "ready");
    assert!(requirement.expected);
}

#[test]
fn canonicalizes_multiple_contextual_cleanup_requirements_independent_of_caller_order() {
    let checked = checked(
        r#"
        data Token { armed: bool; extra: bool; ready: bool; }
        machine Token::drop(&mut self)
        requires
            self.ready;
            self.armed == true
        {}

        data Root {}
        machine Root::enter(token: Token)
        requires
            token.armed;
            token.ready == true;
            token.extra
        {}
        "#,
    );
    let enter = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(enter)
        .expect("order-independent contextual nominal cleanup plan");
    let [cleanup] = plan.cleanups.as_slice() else {
        panic!("one cleanup action")
    };
    assert_eq!(
        cleanup
            .requirements
            .iter()
            .map(|requirement| (requirement.field_identity.as_str(), requirement.expected))
            .collect::<Vec<_>>(),
        vec![("armed", true), ("ready", true)],
        "checked cleanup requirements use canonical declaration-identity order"
    );
    assert_eq!(
        plan.caller_requirements
            .iter()
            .map(|requirement| {
                (
                    requirement.source_parameter_index,
                    requirement.field_identity.as_str(),
                    requirement.expected,
                )
            })
            .collect::<Vec<_>>(),
        vec![(0, "armed", true), (0, "extra", true), (0, "ready", true)],
        "the machine plan retains the full canonical supported caller superset"
    );
}

#[test]
fn retains_contextual_multi_root_cleanups_with_distinct_targets() {
    let checked = checked(
        r#"
        data First { armed: bool; }
        machine First::drop(&mut self)
        requires self.armed
        {}

        data Second { ready: bool; }
        machine Second::drop(&mut self)
        requires self.ready
        {}

        data Root {}
        machine Root::enter(first: First, second: Second)
        requires
            second.ready;
            first.armed
        {}
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("distinct contextual cleanup targets are retained");
    assert_eq!(
        plan.cleanups
            .iter()
            .map(|cleanup| cleanup.source_parameter_index)
            .collect::<Vec<_>>(),
        vec![1, 0],
        "contextual roots retain reverse declaration cleanup order"
    );
    assert_ne!(
        plan.cleanups[0].cleanup_machine, plan.cleanups[1].cleanup_machine,
        "distinct nominal types retain distinct cleanup targets"
    );
    assert_eq!(
        plan.cleanups
            .iter()
            .map(|cleanup| {
                cleanup
                    .requirements
                    .iter()
                    .map(|requirement| (requirement.field_identity.as_str(), requirement.expected))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
        vec![vec![("ready", true)], vec![("armed", true)]],
        "each reverse-ordered action retains its target-local requirement"
    );
    assert_eq!(
        plan.caller_requirements
            .iter()
            .map(|requirement| {
                (
                    requirement.source_parameter_index,
                    requirement.field_identity.as_str(),
                    requirement.expected,
                )
            })
            .collect::<Vec<_>>(),
        vec![(0, "armed", true), (1, "ready", true)],
        "caller requirements remain canonical in source-root order"
    );
}

#[test]
fn retains_shared_contextual_target_for_each_reverse_ordered_root() {
    let checked = checked(
        r#"
        data Token { first_only: bool; ready: bool; second_only: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}

        data Root {}
        machine Root::enter(first: Token, second: Token)
        requires
            second.second_only;
            first.ready;
            second.ready;
            first.first_only
        {}
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("shared contextual cleanup target is retained for both roots");
    assert_eq!(
        plan.cleanups
            .iter()
            .map(|cleanup| cleanup.source_parameter_index)
            .collect::<Vec<_>>(),
        vec![1, 0],
        "shared-target actions retain reverse declaration order"
    );
    assert_eq!(
        plan.cleanups[0].cleanup_machine, plan.cleanups[1].cleanup_machine,
        "same-type roots share the exact contextual cleanup target"
    );
    assert!(plan.cleanups.iter().all(|cleanup| {
        matches!(
            cleanup.requirements.as_slice(),
            [requirement] if requirement.field_identity == "ready" && requirement.expected
        )
    }));
    assert_eq!(
        plan.caller_requirements
            .iter()
            .map(|requirement| {
                (
                    requirement.source_parameter_index,
                    requirement.field_identity.as_str(),
                    requirement.expected,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (0, "first_only", true),
            (0, "ready", true),
            (1, "ready", true),
            (1, "second_only", true),
        ],
        "root-specific caller facts remain attached to their source parameter"
    );
}

#[test]
fn retains_contextual_requirements_with_an_executable_cleanup_body() {
    let checked = checked(
        r#"
        data Helper {}
        machine Helper::touch() {}

        data Token { ready: bool; padding: u8; }
        machine Token::drop(&mut self)
        requires self.ready
        { Helper::touch(); }

        data Root {}
        machine Root::enter(first: Token, second: Token)
        requires second.ready, first.ready
        {}
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("contextual executable cleanup plan");
    assert_eq!(
        plan.cleanups
            .iter()
            .map(|cleanup| cleanup.source_parameter_index)
            .collect::<Vec<_>>(),
        vec![1, 0]
    );
    assert!(plan.cleanups.iter().all(|cleanup| {
        matches!(
            cleanup.requirements.as_slice(),
            [requirement] if requirement.field_identity == "ready" && requirement.expected
        )
    }));
    let target = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(plan.cleanups[0].cleanup_machine)
        .expect("contextual executable cleanup target");
    assert!(matches!(
        target.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::CallUnit { .. },
            CheckedUnitEffectOperationPlan::ReturnUnit { .. }
        ]
    ));
}

#[test]
fn canonicalizes_shallow_boolean_cleanup_requirement_spellings() {
    let checked = checked(
        r#"
        data Token { a: bool; b: bool; c: bool; d: bool; e: bool; f: bool; }
        machine Token::drop(&mut self)
        requires
            self.a;
            !self.b;
            self.c == true;
            true == self.d;
            self.e != true;
            false != self.f
        {}

        data Root {}
        machine Root::enter(token: Token)
        requires
            token.a == true;
            token.b == false;
            true == token.c;
            token.d != false;
            false == token.e;
            token.f
        {}
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(machine_named(&checked, "enter"))
        .expect("both Boolean polarities form one contextual cleanup plan");
    assert_eq!(
        plan.cleanups[0]
            .requirements
            .iter()
            .map(|requirement| (requirement.field_identity.as_str(), requirement.expected))
            .collect::<Vec<_>>(),
        vec![
            ("a", true),
            ("b", false),
            ("c", true),
            ("d", true),
            ("e", false),
            ("f", true),
        ]
    );
    assert_eq!(
        plan.caller_requirements
            .iter()
            .map(|requirement| (requirement.field_identity.as_str(), requirement.expected))
            .collect::<Vec<_>>(),
        vec![
            ("a", true),
            ("b", false),
            ("c", true),
            ("d", true),
            ("e", false),
            ("f", true),
        ]
    );
}

#[test]
fn rejects_shared_contextual_target_when_one_root_lacks_its_premise() {
    let diagnostics = contextual_cleanup_diagnostics(
        r#"
        data Token { ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}

        data Root {}
        machine Root::enter(first: Token, second: Token)
        requires first.ready
        {}
        "#,
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove automatic cleanup requires at Unit return edge")
            && diagnostic.message.contains("missing second.ready == true")
            && diagnostic.message.contains("Token::drop")
    }));
}

#[test]
fn executable_cleanup_still_rejects_a_missing_root_premise() {
    let diagnostics = contextual_cleanup_diagnostics(
        r#"
        data Helper {}
        machine Helper::touch() {}
        data Token { ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        { Helper::touch(); }

        data Root {}
        machine Root::enter(first: Token, second: Token)
        requires first.ready
        {}
        "#,
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("missing second.ready == true required by Token::drop")
    }));
}

#[test]
fn rejects_multiple_contextual_cleanup_requirements_when_one_is_missing() {
    let source = r#"
        data Token { armed: bool; ready: bool; }
        machine Token::drop(&mut self)
        requires
            self.ready;
            self.armed
        {}

        data Root {}
        machine Root::enter(token: Token)
        requires token.armed
        {}
    "#;
    let diagnostics = contextual_cleanup_diagnostics(source);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove automatic cleanup requires at Unit return edge")
            && diagnostic.message.contains("missing token.ready == true")
            && diagnostic.message.contains("Token::drop")
    }));
}

#[test]
fn rejects_contextual_cleanup_requirement_set_with_a_mismatched_caller_clause() {
    let source = r#"
        data Token { armed: bool; ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}

        data Root {}
        machine Root::enter(token: Token)
        requires token.armed
        {}
    "#;
    let diagnostics = contextual_cleanup_diagnostics(source);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove automatic cleanup requires at Unit return edge")
            && diagnostic.message.contains("missing token.ready == true")
            && diagnostic.message.contains("Token::drop")
    }));
}

#[test]
fn fences_non_boolean_caller_clauses_out_of_the_bounded_contextual_cleanup_lane() {
    let checked = checked(
        r#"
        data Token { count: u64; ready: bool; }
        machine Token::drop(&mut self)
        requires self.ready
        {}

        data Root {}
        machine Root::enter(token: Token)
        requires
            token.ready;
            token.count == 1
        {}
        "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .for_machine(machine_named(&checked, "enter"))
            .is_none(),
        "a non-Boolean-field caller clause must fail closed out of this bounded lane"
    );
}

#[test]
fn retains_wide_flat_mixed_primitive_record_for_whole_root_nominal_cleanup() {
    let checked = checked(
        r#"
        data Token { flag: bool; tag: u8; delta: i16; payload: u64; address: addr; }
        machine Token::drop(&mut self) {}

        data Root {}
        machine Root::enter(token: Token) {}
        "#,
    );
    let enter = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .for_machine(enter)
        .expect("wide flat scalar nominal-cleanup plan");
    let token_shape = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .structural_types
        .iter()
        .find(|shape| shape.identity == plan.cleanups[0].type_identity)
        .expect("cleanup type shape");
    let [flag, tag, delta, payload, address] = record_fields(token_shape) else {
        panic!("bounded nominal cleanup retains every flat primitive field")
    };
    for (field, identity, primitive) in [
        (flag, "flag", PrimitiveType::Bool),
        (tag, "tag", PrimitiveType::U8),
        (delta, "delta", PrimitiveType::I16),
        (payload, "payload", PrimitiveType::U64),
        (address, "address", PrimitiveType::Addr),
    ] {
        assert_eq!(field.identity, identity);
        assert_eq!(field.relevance, BindingRelevance::Relevant);
        assert!(matches!(
            field.field_type,
            CheckedUnitStructuralFieldType::Scalar(actual) if actual == primitive
        ));
    }
    assert!(plan.machine.entry_claims.is_empty());
    assert!(matches!(
        plan.machine.operations.as_slice(),
        [CheckedUnitEffectOperationPlan::ReturnUnit {
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
            ..
        }] if trivial_affine_local_discard_ordinals.is_empty()
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn bounded_whole_root_nominal_cleanup_plan_accepts_finite_lists_and_fails_closed_for_unsupported_shapes()
 {
    let checked = checked(
        r#"
        data Empty {}
        data Token {}
        machine Token::drop(&mut self) {}
        machine Token::self_cleanup(self) {}
        data Leaf {}
        data Structural { value: Leaf; }
        machine Structural::drop(&mut self) {}
        data Fixed { values: [Leaf; 2]; }
        machine Fixed::drop(&mut self) {}
        data ErasedOnly { proof [erased]: u64; }
        machine ErasedOnly::drop(&mut self) {}
        data ScalarAndErased { value: u64; proof [erased]: u64; }
        machine ScalarAndErased::drop(&mut self) {}
        data Float { value: f64; }
        machine Float::drop(&mut self) {}
        data Qualified { value: u64; }
        domain Qualified::Owned;
        machine Qualified::drop(&mut self) {}
        data Generic<T> {}
        machine Generic::drop(&mut self) {}
        data Wrapper { token: Token; }
        data Sink { marker: u64; }
        machine Sink::take(token: Token) {}

        data Root {}
        machine Root::exact(token: Token) {}
        machine Root::two(first: Token, second: Token) {}
        machine Root::three(first: Token, second: Token, third: Token) {}
        machine Root::five(first: Token, second: Token, third: Token, fourth: Token, fifth: Token) {}
        machine Root::with_local(token: Token) {
            let local: Empty = Empty {};
        }
        machine Root::with_call(token: Token) {
            Sink::take(token);
        }
        machine Root::with_contract(token: Token)
        ensures true
        {}
        machine Root::structural(value: Structural) {}
        machine Root::fixed(value: Fixed) {}
        machine Root::erased(value: ErasedOnly) {}
        machine Root::scalar_and_erased(value: ScalarAndErased) {}
        machine Root::floating(value: Float) {}
        machine Root::qualified(value: Qualified in Owned) {}
        machine Root::generic(value: Generic<u64>) {}
        machine Root::nested(value: Wrapper) {}

        data NonemptyRoot { marker: u64; }
        machine NonemptyRoot::attached_nonempty(token: Token) {}
        "#,
    );

    let plans = &checked.facts.flow.terminal_nominal_affine_unit_cleanups;
    assert!(
        plans
            .for_machine(machine_named(&checked, "exact"))
            .is_some()
    );
    let ordered = plans
        .for_machine(machine_named(&checked, "two"))
        .expect("two whole affine roots have an ordered cleanup plan");
    assert_eq!(
        ordered
            .cleanups
            .iter()
            .map(|cleanup| cleanup.source_parameter_index)
            .collect::<Vec<_>>(),
        [1, 0],
        "independent roots clean in reverse declaration order"
    );
    assert_eq!(
        ordered.cleanups[0].cleanup_machine, ordered.cleanups[1].cleanup_machine,
        "same-type roots may share their exact cleanup target"
    );
    let three = plans
        .for_machine(machine_named(&checked, "three"))
        .expect("three whole affine roots have an ordered cleanup plan");
    assert_eq!(
        three
            .cleanups
            .iter()
            .map(|cleanup| cleanup.source_parameter_index)
            .collect::<Vec<_>>(),
        [2, 1, 0],
        "three independent roots clean in reverse declaration order"
    );
    assert!(
        three
            .cleanups
            .iter()
            .all(|cleanup| cleanup.cleanup_machine == three.cleanups[0].cleanup_machine),
        "same-type roots may share one exact cleanup target"
    );
    let five = plans
        .for_machine(machine_named(&checked, "five"))
        .expect("five whole affine roots have an ordered cleanup plan");
    assert_eq!(
        five.cleanups
            .iter()
            .map(|cleanup| cleanup.source_parameter_index)
            .collect::<Vec<_>>(),
        [4, 3, 2, 1, 0],
        "five independent roots clean in reverse declaration order"
    );
    assert!(
        five.cleanups
            .iter()
            .all(|cleanup| cleanup.cleanup_machine == five.cleanups[0].cleanup_machine),
        "same-type roots may share one exact cleanup target"
    );
    for machine in [
        "with_local",
        "with_call",
        "with_contract",
        "self_cleanup",
        "structural",
        "fixed",
        "erased",
        "scalar_and_erased",
        "floating",
        "qualified",
        "generic",
        "nested",
        "attached_nonempty",
    ] {
        assert!(
            plans
                .for_machine(machine_named(&checked, machine))
                .is_none(),
            "`{machine}` must remain outside the exact nominal-cleanup slice"
        );
    }
    assert_eq!(
        plans.machines.len(),
        4,
        "rejected candidates must not leave partial cleanup plans"
    );
}
