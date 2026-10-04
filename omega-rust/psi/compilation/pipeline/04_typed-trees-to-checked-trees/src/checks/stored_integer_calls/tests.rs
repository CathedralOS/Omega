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
            assert_eq!(
                result.is_ok(),
                accepted,
                "{callee}({value}): {:?}",
                result.as_ref().err()
            );
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

#[test]
fn named_state_storage_origins_cover_calls_and_direct_writes() {
    for (value, accepted) in [(127, true), (128, false)] {
        for statement in [
            format!("output = {value};"),
            format!("store(output, {value});"),
        ] {
            let source = format!(
                "data Cell {{ value: i64; }}
                 machine store(destination: &mut i64, value: i64) {{ destination = value; }}
                 machine run(cell: &mut Cell) {{
                     transition {{ _ -> write(&mut cell.value) }}
                     state write(output: &mut i64) {{ {statement} }}
                 }}"
            );
            let result = checked(&source);
            assert_eq!(
                result.is_ok(),
                accepted,
                "{source}\n{:?}",
                result.as_ref().err()
            );
        }
    }
}

#[test]
fn recursive_calls_cannot_hide_an_invalid_store() {
    for (value, accepted) in [(127, true), (128, false)] {
        let source = format!(
            "data Cell {{ value: i64; }}
             machine repeat(destination: &mut i64) {{
                 destination = {value}; repeat(destination);
             }}
             machine run(cell: &mut Cell) {{ repeat(&mut cell.value); }}"
        );
        let result = checked(&source);
        assert_eq!(
            result.is_ok(),
            accepted,
            "{source}\n{:?}",
            result.as_ref().err()
        );
    }
}

#[test]
fn ordinary_named_state_reference_writes_stay_unencoded() {
    let source = "data Cell { value: i64; }
        machine run(input: &mut i64) {
            transition { _ -> write(input) }
            state write(output: &mut i64) { output = 128; }
        }";
    let result = checked(source);
    assert!(result.is_ok(), "{:?}", result.as_ref().err());
}

#[test]
fn direct_guarded_assignment_reuses_its_established_landing() {
    let source = "data Cell { value: i64; }
        machine run(cell: &mut Cell, input: i64) {
            transition input >= -128 && input <= 127 {
                true -> write(cell, input)
                _ -> done()
            }
            state write(cell: &mut Cell, value: i64) { cell.value = value; }
            state done() {}
        }";
    let result = checked(source);
    assert!(result.is_ok(), "{:?}", result.as_ref().err());
    let checked = result.unwrap();
    let rows = checked
        .facts
        .stored_integer_writes
        .iter()
        .filter(|(_, row)| matches!(row.site, StoredIntegerWriteSite::Assignment { .. }))
        .map(|(_, row)| row)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1, "one source assignment, one landing decision");
}

#[test]
fn owned_builder_array_writes_do_not_need_reference_origins() {
    let source = "data Cell { value: i64; }
        data Entry { offset: i64; }
        machine build() -> i64 {
            let mut entries: [Entry; 2];
            entries[0] = Entry { offset: 128 };
            entries[1] = Entry { offset: 256 };
            entries[0].offset = 512;
            entries[1].offset
        }";
    let result = checked(source);
    assert!(result.is_ok(), "{:?}", result.as_ref().err());
}

#[test]
fn spelled_operator_effects_are_checked_at_their_actual_storage() {
    // A nominal operand selects the authored operator. An i64 reference
    // expression can instead select builtin integer arithmetic, which would
    // not exercise a mutating operator invocation at all.
    for (carrier, accepted) in [("Cell", false), ("Ordinary", true)] {
        let source = format!(
            "data Cell {{ value: i64; }}
             data Ordinary {{ value: i64; }}
             boundary operator + Store::write(destination: &mut {carrier}, value: i64) -> i64;
             machine run(destination: &mut {carrier}) {{
                 let observed: i64 = destination + 128;
             }}"
        );
        let result = checked(&source);
        assert_eq!(
            result.is_ok(),
            accepted,
            "{source}\n{:?}",
            result.as_ref().err()
        );
        match result {
            Ok(checked) => {
                let selected = checked
                    .typed
                    .operators()
                    .iter()
                    .find(|operator| checked.symbols.name(operator.symbol) == "Store::write")
                    .expect("authored operator")
                    .symbol;
                assert!(
                    checked.facts.operators.uses.iter().any(|(_, usage)| {
                        usage.selected_operator_symbol == selected
                            && usage.status
                                == checked_trees::CheckedOperatorResolutionStatus::Resolved
                    }),
                    "the positive control must execute the selected declaration"
                );
            }
            Err(errors) => assert!(
                errors.iter().any(|error| {
                    error.message.contains("IntegerAt call") && error.message.contains("operator")
                }),
                "{errors:?}"
            ),
        }
    }
}

#[test]
fn token_machine_bodies_share_ordinary_write_admission() {
    for (body, accepted) in [
        ("destination.value = 127;", true),
        ("destination.value = 128; destination.value = 0;", false),
    ] {
        let source = format!(
            "data Cell {{ value: i64; }}
             machine + Cell::write(destination: &mut Cell, ignored: i64) -> i64 {{
                 {body}
                 0
             }}
             machine run(destination: &mut Cell) {{
                 let observed: i64 = destination + 1;
             }}"
        );
        let result = checked(&source);
        assert_eq!(
            result.is_ok(),
            accepted,
            "{source}\n{:?}",
            result.as_ref().err()
        );
        match result {
            Ok(checked) => {
                let machine = checked
                    .typed
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == "Cell::write")
                    .expect("selected token machine");
                let entry = checked
                    .typed
                    .machine_states(machine)
                    .first()
                    .expect("ordinary executable entry");
                assert!(
                    checked
                        .facts
                        .flow
                        .control
                        .calls
                        .iter()
                        .any(|(_, call)| { call.target_symbol == entry.symbol }),
                    "the token expression must become an ordinary call to its body"
                );
                assert_eq!(
                    checked
                        .facts
                        .mutation
                        .integer_writes
                        .iter()
                        .filter(|(_, write)| write.machine == machine.symbol)
                        .count(),
                    1,
                    "ordinary body inventory retains the performed store"
                );
            }
            Err(errors) => assert!(
                errors.iter().any(|error| {
                    error.message.contains("IntegerAt") && error.message.contains("fit")
                }),
                "{errors:?}"
            ),
        }
    }
}
