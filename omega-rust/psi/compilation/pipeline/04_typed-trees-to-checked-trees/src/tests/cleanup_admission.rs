use crate::tests::front_end::checked_program_result;

fn program(inner_body: &str, nested: bool) -> String {
    let owner = if nested {
        "data Outer { value: i32; }
         machine Outer::drop(value: &mut Outer) {
             let inner: Inner = Inner { value: 1 };
         }
         machine consume(value: Outer) {}"
    } else {
        "machine consume(value: Inner) {}"
    };
    format!(
        "machine spin() {{ transition {{ _ -> repeat() }}
             state repeat() {{ transition {{ _ -> repeat() }} }}
         }}
         data Inner {{ value: i32; }}
         machine Inner::drop(value: &mut Inner) {{ {inner_body} }}
         {owner}"
    )
}

#[test]
fn selected_cleanup_rejects_inferred_crash_in_direct_and_nested_hooks() {
    for nested in [false, true] {
        let errors = checked_program_result(&program("crash Abort;", nested))
            .map(|_| ())
            .expect_err("a private inferred crash cannot become an eligible cleanup");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("cleanup") && error.message.contains("crash")),
            "nested={nested}: {errors:?}"
        );
    }
}

#[test]
fn selected_cleanup_rejects_unranked_direct_and_nested_hooks() {
    let body = "spin();";
    for nested in [false, true] {
        let errors = checked_program_result(&program(body, nested))
            .map(|_| ())
            .expect_err("a hook without finite progress cannot become an eligible cleanup");
        assert!(
            errors.iter().any(
                |error| error.message.contains("cleanup") && error.message.contains("terminat")
            ),
            "nested={nested}: {errors:?}"
        );
    }
}

#[test]
fn selected_cleanup_accepts_finite_direct_and_nested_hooks() {
    for nested in [false, true] {
        checked_program_result(&program("value.value = 0;", nested))
            .unwrap_or_else(|errors| panic!("nested={nested}: {errors:?}"));
    }
}

#[test]
fn selected_cleanup_rejects_boundary_without_progress_contract() {
    let source = "pub boundary trait Output { machine write(value: i32) reaches Output; }
                  data Inner { value: i32; }
                  machine Inner::drop(value: &mut Inner) reaches Output {
                      Output::write(value.value);
                  }
                  machine consume(value: Inner) {}";
    let errors = checked_program_result(source)
        .map(|_| ())
        .expect_err("normal boundary completion does not supply a termination guarantee");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("cleanup") && error.message.contains("terminat")),
        "{errors:?}"
    );
}

#[test]
fn selected_cleanup_uses_declared_boundary_progress_without_granting_it() {
    checked_program_result(
        "pub boundary trait Output {
             machine write(value: i32) reaches Output terminates;
         }
         data Inner { value: i32; }
         machine Inner::drop(value: &mut Inner) reaches Output {
             Output::write(value.value);
         }
         machine consume(value: Inner) {}",
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
}

#[test]
fn selected_cleanup_retains_exact_requirement_discharge_of_helper_crash() {
    let declarations = "
        data Inner { ready: bool; other: bool; }
        machine may_fail(ready: bool) crashes Abort !ready {
            transition ready { true -> {} false -> fail() }
            state fail() { crash Abort; }
        }
        machine consume(value: Inner) requires value.ready {}";
    let positive = format!(
        "{declarations}
         machine Inner::drop(value: &mut Inner) requires value.ready {{
             may_fail(value.ready);
         }}"
    );
    checked_program_result(&positive).unwrap_or_else(|errors| panic!("{errors:?}"));
    let sibling_write = positive.replace(
        "may_fail(value.ready);",
        "value.other = false; may_fail(value.ready);",
    );
    checked_program_result(&sibling_write).unwrap_or_else(|errors| panic!("{errors:?}"));
    for replacement in [
        "may_fail(false);",
        "may_fail(value.other);",
        "value.ready = false; may_fail(value.ready); value.ready = true;",
    ] {
        let wrong_actual = positive.replace("may_fail(value.ready);", replacement);
        let errors = checked_program_result(&wrong_actual)
            .map(|_| ())
            .expect_err("a different or overwritten actual cannot reuse the entry requirement");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("cleanup") && error.message.contains("crash")),
            "{replacement}: {errors:?}"
        );
    }
}

#[test]
fn selected_cleanup_rejects_missing_invocation_crash_evidence() {
    let source = "data Inner { value: i32; }
                  machine finish() {}
                  machine Inner::drop(value: &mut Inner) { finish(); }
                  machine consume(value: Inner) {}";
    let mut checked = checked_program_result(source).unwrap_or_else(|errors| panic!("{errors:?}"));
    let hook = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Inner::drop")
        .unwrap()
        .symbol;
    let contract = checked
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|contract| contract.machine == hook)
        .unwrap();
    assert_eq!(contract.crash.checked_calls().len(), 1);
    contract.crash = contract
        .crash
        .clone()
        .with_checked_calls(Vec::new())
        .unwrap();
    let errors = crate::checks::validate_selected_cleanup_guarantees(&checked, &checked.facts)
        .expect_err("missing evidence differs from a checked empty crash refinement");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("unexamined call")),
        "{errors:?}"
    );
}
