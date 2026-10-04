use super::*;
use layout_plans::{IntegerInterpretation, LayoutPlacementReport};
use semantic_vocabulary::IntegerSign;
use typed_trees::data::DataMember;

fn program_with_storage(source: &str, width: u16, sign: IntegerSign) -> TypedTrees {
    let mut program = crate::tests::front_end::typed_program(&format!(
        "{source}\ndata StoragePolicy {{}}\nmachine StoragePolicy::plan() {{}}"
    ));
    let policy_symbol = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "StoragePolicy")
        .unwrap()
        .symbol;
    let policy_plan_machine_symbol = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "StoragePolicy::plan")
        .unwrap()
        .symbol;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Cell")
        .unwrap();
    let symbol = data.symbol;
    let [DataMember::Field(field)] = program.data_members(data) else {
        panic!("one field");
    };
    let field = field.symbol;
    let interpretation = match sign {
        IntegerSign::Signed => IntegerInterpretation::Signed,
        IntegerSign::Unsigned => IntegerInterpretation::Unsigned,
    };
    // This query consumes the exact normalized geometry published by layout
    // evaluation. The public recast corpus fixture exercises that producer.
    program.plan_laid_layouts.push(typed_trees::PlanLaidLayout {
        data_name: "Cell".to_owned(),
        data_symbol: symbol,
        field_symbols: vec![field],
        schema_symbol: symbol,
        schema_field_symbols: vec![field],
        policy_symbol,
        policy_plan_machine_symbol,
        validated_layout: layout_plans::LayoutPlanReport {
            // Exact omega.schema.v2 report identity of the fixed schema
            // Cell { value: i64; }, checked by ordinary plan validation.
            schema_report_fingerprint: 0xe66b_91e5_b35b_8a82,
            entries: vec![layout_plans::LayoutFieldEntryReport {
                field: "value".to_owned(),
                member_identity: None,
                placement: LayoutPlacementReport::IntegerAt {
                    offset: 0,
                    stored_width: u64::from(width),
                    interpretation,
                },
            }],
            offsets: None,
            size: Some(u64::from(width / 8)),
            align: 1,
        },
        private_callback_demands: Vec::new(),
        offsets: vec![0],
        bit_fields: Vec::new(),
        integer_fields: vec![typed_trees::PlanLaidIntegerField {
            field_index: 0,
            stored_width_bits: width,
            interpretation,
            write_is_total: false,
        }],
        repeated_fields: Vec::new(),
        size: usize::from(width / 8),
        align: 1,
    });
    program
}

fn checked(source: &str) -> Result<checked_trees::CheckedTrees, Vec<Diagnostic>> {
    crate::lower_typed_trees(
        program_with_storage(source, 8, IntegerSign::Signed),
        &crate::CheckingRequest::settled(),
    )
}

#[test]
fn field_loan_calls_check_each_direct_and_forwarded_value() {
    for (value, accepted) in [(127, true), (128, false), (-128, true), (-129, false)] {
        for callee in ["store", "forward"] {
            let source = format!(
                "data Cell {{ value: i64; }}
                 machine store(destination: &mut i64, value: i64) {{ destination = value; }}
                 machine forward(destination: &mut i64, value: i64) {{ store(destination, value); }}
                 machine run(cell: &mut Cell) {{ {callee}(&mut cell.value, {value}); }}"
            );
            let result = checked(&source);
            assert_eq!(result.is_ok(), accepted, "{callee}({value}): {result:?}");
            if let Ok(checked) = result {
                assert!(
                    checked
                        .facts
                        .stored_integer_writes
                        .iter()
                        .any(|(_, row)| matches!(row.site, StoredIntegerWriteSite::Call { .. }))
                );
            } else if let Err(errors) = result {
                assert!(
                    errors
                        .iter()
                        .any(|error| error.message.contains("IntegerAt call")),
                    "{errors:?}"
                );
            }
        }
    }
}

#[test]
fn later_values_and_crashes_do_not_erase_earlier_invalid_stores() {
    for body in [
        "destination = 128; destination = 0;",
        "destination = 128; crash Trap;",
    ] {
        let source = format!(
            "data Cell {{ value: i64; }}
             machine store(destination: &mut i64) crashes Trap {{ {body} }}
             machine forward(destination: &mut i64) crashes Trap {{ store(destination); }}
             machine run(cell: &mut Cell) crashes Trap {{ forward(&mut cell.value); }}"
        );
        let errors = checked(&source).expect_err("every performed write must fit");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("IntegerAt call")),
            "{errors:?}"
        );
    }
}

#[test]
fn distinct_inner_invocations_do_not_share_a_leaf_write_decision() {
    let source = "data Cell { value: i64; }
        machine store(destination: &mut i64, value: i64) { destination = value; }
        machine twice(destination: &mut i64) { store(destination, 127); store(destination, 128); }
        machine run(cell: &mut Cell) { twice(&mut cell.value); }";
    let errors = checked(source).expect_err("the second invocation writes a different value");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("IntegerAt call")),
        "{errors:?}"
    );
}

#[test]
fn unrelated_encoded_types_do_not_constrain_ordinary_reference_calls() {
    let source = "data Cell { value: i64; }
        machine store(destination: &mut i64, value: i64) { destination = value; }
        machine forward(destination: &mut i64, value: i64) { store(destination, value); }
        machine run(destination: &mut i64) { forward(destination, 128); }";
    assert!(
        checked(source).is_ok(),
        "an ordinary scalar reference does not acquire another type's physical width"
    );
}
