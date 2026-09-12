use super::*;

#[test]
fn operator_entry_guard_does_not_survive_entry_state_rearrival() {
    for transfer in [
        "transition { _ -> compare(-1, right) }",
        "transition { _ -> again(right) } state again(right: i32) -> bool { transition { _ -> compare(-1, right) } }",
        "transition { _ -> self }",
    ] {
        let source = |ceiling: &str| {
            format!(
            "boundary operator == Comparison::equal(left: i32, right: i32) -> bool crashes Trap left < 0;
             pub machine compare(left: i32, right: i32) -> bool {ceiling} {{
                 let answer: bool = left == right;
                 {transfer}
             }}"
        )
        };
        check(&source("crashes Trap")).expect("unconditional route covers conservative re-entry");
        let diagnostics = match check(&source("crashes Trap left < 0")) {
            Ok(_) => {
                panic!("a state-arrival parameter is not the initial invocation value: {transfer}")
            }
            Err(diagnostics) => diagnostics,
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("crash")),
            "{diagnostics:#?}"
        );
    }
}

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize operator route fixture");
    let syntax = parse_syntax_trees(&tokens).expect("parse operator route fixture");
    let resolved = lower_syntax_trees(&syntax).expect("resolve operator route fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type operator route fixture");
    lower_typed_trees(typed)
}

#[test]
fn operator_field_routes_preserve_immutable_owned_and_shared_entry_values() {
    for carrier in ["Flag", "&Flag"] {
        for operand in ["flag.enabled", "snapshot"] {
            let source = format!(
                "boundary operator == Comparison::equal(left: bool, right: bool) -> bool crashes Trap left;
                 pub data Flag {{ enabled: bool; }}
                 pub machine compare(flag: {carrier}, right: bool) -> bool crashes Trap flag.enabled {{
                     let snapshot: bool = flag.enabled;
                     {operand} == right
                 }}"
            );
            check(&source).unwrap_or_else(|errors| panic!("{source}: {errors:?}"));
        }
    }
}

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
        check(&source("")).expect("the unconditional ceiling covers the actual mutation");
        let errors = check(&source("flag.enabled"))
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
fn operator_field_entry_guard_does_not_survive_state_rearrival() {
    let source = |guard: &str| {
        format!(
        "boundary operator == Comparison::equal(left: bool, right: bool) -> bool crashes Trap left;
         pub data Flag {{ enabled: bool; }}
         pub machine compare(flag: Flag, next: Flag, right: bool) -> bool crashes Trap {guard} {{
             let observed: bool = flag.enabled == right;
             transition {{ _ -> compare(next, flag, right) }}
         }}"
    )
    };
    check(&source("")).expect("unconditional crash contract covers every arrival");
    let errors = check(&source("flag.enabled"))
        .expect_err("reordered state arrivals are not invocation-entry fields");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("uncovered Trap")),
        "{errors:?}"
    );
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
            check(&source(&format!("crashes {cause}")))
                .expect("same cause covers the selected invocation");
            for ceiling in [String::new(), format!("crashes {wrong}")] {
                let diagnostics = check(&source(&ceiling))
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
fn false_operator_route_retains_examined_empty_invocation() {
    for body in ["left == right", "match left { right -> true, _ -> false }"] {
        let checked = check(&format!(
            "boundary operator == Comparison::equal(left: f32, right: f32) -> bool crashes Trap false;
             pub machine compare(left: f32, right: f32) -> bool {{ {body} }}"
        )).expect("a false published route needs no caller crash ceiling");
        let sites = checked
            .facts
            .contract_plans
            .machines
            .iter()
            .flat_map(|machine| machine.crash.checked_operators())
            .collect::<Vec<_>>();
        assert_eq!(sites.len(), 1);
        assert!(sites[0].surviving.is_empty());
        assert!(!sites[0].published.is_empty());
    }
}

#[test]
fn operator_crash_survives_private_helper_summary() {
    let source = |ceiling: &str| {
        format!(
            "boundary operator == Comparison::equal(left: f32, right: f32) -> bool crashes Trap;
         machine helper(left: f32, right: f32) -> bool {{ left == right }}
         pub machine compare(left: f32, right: f32) -> bool {ceiling} {{ helper(left, right) }}"
        )
    };
    check(&source("crashes Trap")).expect("wrapper covers the private operator invocation");
    let diagnostics =
        check(&source("")).expect_err("private summaries must retain operator crash causes");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("crash")),
        "{diagnostics:#?}"
    );
}

#[test]
fn operator_crash_guard_cannot_relabel_mutable_storage_as_entry() {
    let source = |mutable: &str, assignment: &str| {
        format!(
        "boundary operator == Comparison::equal(left: i32, right: i32) -> bool crashes Trap left < 0;
         machine compare({mutable}left: i32, right: i32) -> bool crashes Trap left < 0 {{ {assignment} left == right }}"
    )
    };
    check(&source("", "")).expect("immutable entry operand retains exact published guard identity");
    let diagnostics = check(&source("mut ", "left = -1;"))
        .expect_err("current storage cannot impersonate entry crash guard");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("crash")),
        "{diagnostics:#?}"
    );
}

#[test]
fn private_operator_summary_cannot_relabel_mutated_call_actual_as_entry() {
    for actual in ["left", "saved"] {
        let checked = check(&format!(
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
fn operator_crash_discharge_uses_captured_values_not_later_storage() {
    for (premise, right, accepted) in [
        ("requires value >= 0", "reset(&mut value)", true),
        ("", "prepare(&mut value)", false),
    ] {
        let source = format!(
            "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
             crashes Trap !(left >= 0);
             machine reset(value: &mut i32) -> i32 {{ value = -1; 0 }}
             machine prepare(value: &mut i32) -> i32 ensures value >= 0 {{ value = 1; 0 }}
             pub machine compare(mut value: i32) -> bool {premise} {{ value == {right} }}"
        );
        let checked = check(&source);
        assert_eq!(checked.is_ok(), accepted, "{source}\n{:?}", checked.err());
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
        let checked = check(&source);
        assert_eq!(checked.is_ok(), accepted, "{source}\n{:?}", checked.err());
    }
}

#[test]
fn operator_crash_entry_guard_substitution_preserves_argument_order() {
    for (guard, accepted) in [("right < 0", true), ("left < 0", false)] {
        let checked = check(&format!(
            "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
             crashes Trap left < 0;
             pub machine compare(left: i32, right: i32) -> bool crashes Trap {guard} {{
                 right == left
             }}"
        ));
        assert_eq!(checked.is_ok(), accepted, "{guard}: {:?}", checked.err());
    }
}

#[test]
fn operator_crash_records_reject_missing_or_changed_surviving_routes() {
    let checked = check(
        "boundary operator == Comparison::equal(left: i32, right: i32) -> bool crashes Trap;
         pub machine compare(left: i32, right: i32) -> bool crashes Trap { left == right }",
    )
    .unwrap();
    let owner = checked
        .facts
        .contract_plans
        .machines
        .iter()
        .position(|machine| !machine.crash.checked_operators().is_empty())
        .unwrap();
    for change in ["missing", "surviving", "published", "selected", "capture"] {
        let mut facts = checked.facts.clone();
        let plan = &mut facts.contract_plans.machines[owner];
        let mut rows = plan.crash.checked_operators().to_vec();
        match change {
            "missing" => rows.clear(),
            "surviving" => rows[0].surviving.clear(),
            "published" => rows[0].published.clear(),
            "selected" => rows[0].selected_operator = plan.machine,
            "capture" => rows[0].invocation = arena::Handle::from_arena_index(9999),
            _ => unreachable!(),
        }
        plan.crash = plan.crash.clone().with_checked_operators(rows).unwrap();
        let diagnostics =
            crate::checks::check_checked_facts(&checked.typed, &facts).expect_err(change);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("crash invocation evidence")),
            "{change}: {diagnostics:#?}"
        );
    }
}

#[test]
fn each_match_comparison_retains_its_own_operator_crash_record() {
    let checked = check(
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
