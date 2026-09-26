use crate::tests::flow::terminal_unit::{CheckedUnitEffectOperationPlan, checked, machine_named};

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
