use crate::tests::flow::terminal_unit::{CheckedUnitEffectOperationPlan, checked, machine_named};

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
            CheckedUnitEffectOperationPlan::Complete {
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
            CheckedUnitEffectOperationPlan::Complete {
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
            CheckedUnitEffectOperationPlan::Complete {
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
            CheckedUnitEffectOperationPlan::Complete {
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
            CheckedUnitEffectOperationPlan::Complete {
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
            CheckedUnitEffectOperationPlan::Complete {
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
            CheckedUnitEffectOperationPlan::Complete {
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
            CheckedUnitEffectOperationPlan::Complete {
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
        CheckedUnitEffectOperationPlan::Complete {
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
        CheckedUnitEffectOperationPlan::Complete {
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
        CheckedUnitEffectOperationPlan::Complete {
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
        CheckedUnitEffectOperationPlan::Complete {
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
        CheckedUnitEffectOperationPlan::Complete {
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
        CheckedUnitEffectOperationPlan::Complete {
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
        CheckedUnitEffectOperationPlan::Complete {
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
        CheckedUnitEffectOperationPlan::Complete {
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
        CheckedUnitEffectOperationPlan::Complete {
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
        CheckedUnitEffectOperationPlan::Complete {
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
        CheckedUnitEffectOperationPlan::Complete {
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
        CheckedUnitEffectOperationPlan::Complete {
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
        CheckedUnitEffectOperationPlan::Complete {
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
        CheckedUnitEffectOperationPlan::Complete {
            statement_index: 24,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals
            == &[22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_twenty_four_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
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
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("twenty-four-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 24);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(25))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert_eq!(plan.operations.len(), 25);
    assert!(
        plan.operations[..24]
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
        &plan.operations[24],
        CheckedUnitEffectOperationPlan::Complete {
            statement_index: 25,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals
            == &[23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
            && trivial_affine_discards.is_empty()
    ));
}

#[test]
fn retains_twenty_five_element_fixed_array_construction_prefix_and_reverse_cleanup() {
    let checked = checked(
        r#"
        data Empty {}
        data Root {}
        machine Root::enter() {
            let mut values: [Empty; 26];
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
            values[24] = Empty {};
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "enter"))
        .expect("twenty-five-element construction prefix should have a Unit plan");
    assert_eq!(plan.trivial_affine_locals.len(), 25);
    assert!(
        plan.trivial_affine_locals
            .iter()
            .enumerate()
            .all(|(index, local)| {
                usize::try_from(local.declaration_ordinal) == Ok(index)
                    && local.type_identity == "named(name(Empty))"
                    && local.construction.as_ref().is_some_and(|construction| {
                        construction.root_type_identity == "array(named(name(Empty)),literal(26))"
                            && usize::try_from(construction.index) == Ok(index)
                    })
            })
    );
    assert_eq!(plan.operations.len(), 26);
    assert!(
        plan.operations[..25]
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
        &plan.operations[25],
        CheckedUnitEffectOperationPlan::Complete {
            statement_index: 26,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } if trivial_affine_local_discard_ordinals
            == &[24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
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
            "missing_twenty_five",
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
            "#,
        ),
        (
            "reordered_twenty_five",
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
                values[23] = Empty {};
                values[22] = Empty {};
            "#,
        ),
        (
            "missing_twenty_six",
            r#"
                let mut values: [Empty; 26];
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
        (
            "reordered_twenty_six",
            r#"
                let mut values: [Empty; 26];
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
                values[24] = Empty {};
                values[23] = Empty {};
            "#,
        ),
        (
            "length_twenty_seven",
            r#"
                let mut values: [Empty; 27];
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
                values[24] = Empty {};
                values[25] = Empty {};
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
