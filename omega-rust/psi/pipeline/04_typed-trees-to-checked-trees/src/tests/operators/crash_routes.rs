use crate::tests::front_end::checked_program_result;

#[test]
fn operator_field_routes_cannot_relabel_mutable_contents_as_entry() {
    for parameter in ["mut flag: Flag", "flag: &mut Flag"] {
        let source = |guard: &str| {
            format!(
            "boundary operator == Comparison::equal(left: bool, right: bool) -> bool crashes Trap left;
             pub data Flag {{ enabled: bool; }}
             pub machine compare({parameter}, right: bool) -> bool crashes Trap {guard} {{
                 flag.enabled = true;
                 flag.enabled == right
             }}"
        )
        };
        checked_program_result(&source(""))
            .expect("the unconditional ceiling covers the actual mutation");
        let errors = checked_program_result(&source("flag.enabled"))
            .expect_err("current fields cannot inherit invocation-entry identity");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("uncovered Trap")),
            "{errors:?}"
        );
    }
}

#[test]
fn selected_operator_crash_routes_require_same_cause_coverage() {
    for (cause, wrong) in [("Trap", "Abort"), ("Abort", "Trap")] {
        for body in ["left == right", "match left { right -> true, _ -> false }"] {
            let source = |ceiling: &str| {
                format!(
                "boundary operator == Comparison::equal(left: f32, right: f32) -> bool crashes {cause};
                 pub machine compare(left: f32, right: f32) -> bool {ceiling} {{ {body} }}"
            )
            };
            checked_program_result(&source(&format!("crashes {cause}")))
                .expect("same cause covers the selected invocation");
            for ceiling in [String::new(), format!("crashes {wrong}")] {
                let diagnostics = checked_program_result(&source(&ceiling))
                    .expect_err("selected crash cannot disappear or change cause");
                assert!(
                    diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains("crash")),
                    "{diagnostics:#?}"
                );
            }
        }
    }
}

#[test]
fn private_operator_summary_cannot_relabel_mutated_call_actual_as_entry() {
    for actual in ["left", "saved"] {
        let checked = checked_program_result(&format!(
        "boundary operator == Comparison::equal(left: i32, right: i32) -> bool crashes Trap left < 0;
         machine helper(left: i32, right: i32) -> bool {{ left == right }}
         pub machine compare(mut left: i32, right: i32) -> bool crashes Trap left < 0 {{
             left = -1;
             let saved: i32 = left;
             helper({actual}, right)
         }}",
    ));
        let Err(diagnostics) = checked else {
            panic!("a private helper cannot restore a mutable caller actual's entry identity");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("crash")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn operator_crash_discharge_observes_earlier_operand_effects() {
    for (operation, accepted) in [("reset", false), ("prepare", true)] {
        let source = format!(
            "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
             crashes Trap !(right >= 0);
             machine reset(value: &mut i32) -> i32 {{ value = -1; 0 }}
             machine prepare(value: &mut i32) -> i32 ensures value >= 0 {{ value = 1; 0 }}
             pub machine compare(mut value: i32) -> bool requires value >= 0 {{
                 {operation}(&mut value) == value
             }}"
        );
        let checked = checked_program_result(&source);
        assert_eq!(checked.is_ok(), accepted, "{source}\n{:?}", checked.err());
    }
}

#[test]
fn each_match_comparison_retains_its_own_operator_crash_record() {
    let checked = checked_program_result(
        "boundary operator == Float::equal(left: f32, right: f32) -> bool crashes Trap false;
         pub machine compare(value: f32, first: f32, second: f32) -> bool {
             match value { first -> true, second -> true, _ -> false }
         }",
    )
    .unwrap();
    let rows = checked
        .facts
        .contract_plans
        .machines
        .iter()
        .flat_map(|machine| machine.crash.checked_operators())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].operator_use, rows[1].operator_use);
    assert_ne!(rows[0].invocation, rows[1].invocation);
    assert!(rows.iter().all(|row| row.surviving.is_empty()));
}
