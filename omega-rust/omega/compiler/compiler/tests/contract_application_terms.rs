//! Specification applications retain selected callees and substituted operands.

use checked_interpreter::BuildMachineEntry;
use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn check_files(sources: &[(&str, &str)]) -> Result<(), String> {
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "omega-contract-application-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
    )));
    fs::create_dir_all(&fixture.0).unwrap();
    for (name, source) in sources {
        fs::write(fixture.0.join(name), source).unwrap();
    }
    compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: fixture.0.join("main.omg"),
            build_dir: Some(fixture.0.join("build")),
            target_name: None,
        })
        .with_requested_product(RequestedCompileProduct::Check),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .map(|_| ())
    .map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

const DECLARATIONS: &str = r#"
    pub machine observe(value: bool) -> bool
    terminates;
    { transition { _ -> (value == true) } }
    pub machine restricted(left: bool, right: bool) -> bool
    requires observe(left) == observe(right);
    terminates;
    { left }
"#;

#[test]
fn runtime_body_calls_establish_equal_observations() {
    for body in [
        "restricted(false, false)",
        "let answer: bool = restricted(false, false); answer",
        "_ = restricted(false, false); true",
    ] {
        let source = format!("{DECLARATIONS} machine caller() -> bool {{ {body} }}");
        check_files(&[("main.omg", &source)])
            .unwrap_or_else(|diagnostics| panic!("{body}: {diagnostics}"));
    }
}

#[test]
fn runtime_body_calls_reject_unequal_observations() {
    for body in [
        "restricted(false, true)",
        "let answer: bool = restricted(false, true); answer",
        "_ = restricted(false, true); true",
    ] {
        let source = format!("{DECLARATIONS} machine caller() -> bool {{ {body} }}");
        let diagnostics = check_files(&[("main.omg", &source)])
            .expect_err("runtime result use cannot waive the call precondition");
        assert!(diagnostics.contains("requires"), "{body}: {diagnostics}");
    }
}

#[test]
fn runtime_body_calls_preserve_saved_value_identity() {
    for initializer in ["sample(value)", "sample(value) == true"] {
        for operands in ["first, first", "first, alias"] {
            let source = format!(
                "{DECLARATIONS}
                machine sample(value: bool) -> bool {{ value }}
                machine caller(value: bool) -> bool {{
                    let first: bool = {initializer};
                    let alias: bool = first;
                    restricted({operands})
                }}"
            );
            check_files(&[("main.omg", &source)])
                .unwrap_or_else(|diagnostics| panic!("{initializer}, {operands}: {diagnostics}"));
        }
    }
}

#[test]
fn runtime_body_calls_do_not_identify_independent_results() {
    for (left, right) in [
        ("sample(left)", "sample(right)"),
        ("sample(left) == true", "sample(right) == true"),
    ] {
        let source = format!(
            "{DECLARATIONS}
            machine sample(value: bool) -> bool {{ value }}
            machine caller(left: bool, right: bool) -> bool {{
                let first: bool = {left};
                let second: bool = {right};
                restricted(first, second)
            }}"
        );
        let diagnostics = check_files(&[("main.omg", &source)])
            .expect_err("independent observations need equality evidence");
        assert!(diagnostics.contains("requires"), "{diagnostics}");
    }
}

#[test]
fn runtime_body_calls_do_not_assume_float_reflexivity() {
    let diagnostics = check_files(&[(
        "main.omg",
        r#"
        machine restricted(left: f64, right: f64) -> bool
        requires left == right;
        { true }
        machine caller(value: f64) -> bool { restricted(value, value) }
    "#,
    )])
    .expect_err("the input may be NaN");
    assert!(diagnostics.contains("requires"), "{diagnostics}");
}

#[test]
fn runtime_body_calls_transport_preserved_entry_equalities() {
    let source = format!(
        r#"
        {DECLARATIONS}
        machine caller(left: bool, right: bool) -> bool
        requires observe(left) == observe(right);
        {{ restricted(left, right) }}
    "#
    );
    check_files(&[("main.omg", &source)]).expect("preserved entry evidence establishes the call");
}

#[test]
fn runtime_body_calls_do_not_treat_nan_premises_as_contradictions() {
    let source = format!(
        r#"
        {DECLARATIONS}
        machine caller(value: f64) -> bool
        requires value != value;
        {{ restricted(false, true) }}
    "#
    );
    let diagnostics = check_files(&[("main.omg", &source)])
        .expect_err("NaN satisfies the caller premise but not the callee premise");
    assert!(diagnostics.contains("requires"), "{diagnostics}");
}

#[test]
fn runtime_body_calls_execute_with_checked_premises() {
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "omega-contract-runtime-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
    )));
    fs::create_dir_all(&fixture.0).unwrap();
    let root = fixture.0.join("main.omg");
    fs::write(
        &root,
        format!(
            "{DECLARATIONS}
        machine caller(value: bool) -> bool {{
            let saved: bool = value;
            restricted(saved, value)
        }}
        machine run_false() -> i32 {{
            transition caller(false) {{ true -> 99 false -> 7 }}
        }}
        machine run_true() -> i32 {{
            transition caller(true) {{ true -> 11 false -> 99 }}
        }}"
        ),
    )
    .unwrap();
    let checked = compiler::compile_to_checked(compiler::CheckedCompileRequest::new(&root, None))
        .expect("runtime call premise");
    drop(fixture);
    assert!(!root.exists(), "checked execution cannot reread source");
    for (entry, expected) in [("run_false", 7), ("run_true", 11)] {
        let outcome = checked_interpreter::interpret_entry(
            &checked,
            BuildMachineEntry::Name(entry),
            &[],
            checked_interpreter::InterpretOptions::default(),
        );
        assert_eq!(outcome.error, None);
        assert_eq!(outcome.exit_code, expected);
    }
}

#[test]
fn runtime_body_calls_substitute_nested_projection_roots() {
    for (operand, accepted) in [("first.inner.flag", false), ("zero.inner.flag", true)] {
        let source = format!(
            r#"
            data Inner {{ flag: bool; }}
            data Outer {{ inner: Inner; }}
            machine projected(first: Outer, second: bool) -> bool
            requires first.inner.flag == second;
            {{ second }}
            machine caller(first: Outer) -> bool {{
                let zero: Outer = Outer {{ inner: Inner {{ flag: false }} }};
                projected(zero, {operand})
            }}
        "#
        );
        let outcome = check_files(&[("main.omg", &source)]);
        if accepted {
            outcome.expect("the actual nested constructor field agrees");
        } else {
            let diagnostics = outcome.expect_err("callee-root spelling is not caller identity");
            assert!(diagnostics.contains("requires"), "{diagnostics}");
        }
    }
}

#[test]
fn runtime_body_calls_keep_case_payloads_distinct_from_authored_names() {
    for (operand, accepted) in [("__ih_match_payload_0_flag", false), ("flag", true)] {
        let source = format!(
            r#"
            {DECLARATIONS}
            data Flag {{ case Present(flag: bool); }}
            machine caller(value: Flag, __ih_match_payload_0_flag: bool) -> bool {{
                transition {{ _ -> match_payload(value, __ih_match_payload_0_flag) }}
                state match_payload(value: Flag, __ih_match_payload_0_flag: bool) -> bool {{
                    transition value {{
                        Flag::Present {{ flag }} -> (restricted(flag, {operand}))
                    }}
                }}
            }}
        "#
        );
        let outcome = check_files(&[("main.omg", &source)]);
        if accepted {
            outcome.expect("the selected payload retains its own identity");
        } else {
            let diagnostics =
                outcome.expect_err("an authored name cannot name an internal payload");
            assert!(diagnostics.contains("requires"), "{diagnostics}");
        }
    }
}

#[test]
fn runtime_body_calls_do_not_reuse_mutated_entry_equalities() {
    let source = format!(
        r#"
        {DECLARATIONS}
        machine caller(mut left: bool, right: bool) -> bool
        requires observe(left) == observe(right);
        {{
            left = false;
            restricted(left, right)
        }}
    "#
    );
    let diagnostics = check_files(&[("main.omg", &source)])
        .expect_err("entry observations do not describe the mutated argument");
    assert!(diagnostics.contains("requires"), "{diagnostics}");
}

#[test]
fn equal_substituted_arguments_form_a_specification_application() {
    let source = format!(
        "{DECLARATIONS}
        machine caller()
        requires restricted(false, false) == restricted(false, false);
        {{}}"
    );
    check_files(&[("main.omg", &source)])
        .expect("equal fully substituted observations establish the selected precondition");
}

#[test]
fn unequal_substituted_arguments_cannot_hide_in_a_reflexive_application() {
    let source = format!(
        "{DECLARATIONS}
        machine caller()
        requires restricted(false, true) == restricted(false, true);
        {{}}"
    );
    let diagnostics = check_files(&[("main.omg", &source)])
        .expect_err("outer reflexivity cannot establish the inner call's false precondition");
    assert!(
        diagnostics.contains("specification call") && diagnostics.contains("restricted"),
        "{diagnostics}"
    );
}

#[test]
fn imported_application_callees_keep_their_exact_owner_in_both_orders() {
    let selected = format!("module selected; {DECLARATIONS}");
    let decoy = r#"
        module decoy;
        pub machine observe(value: bool) -> bool terminates; { true }
        pub machine restricted(left: bool, right: bool) -> bool
        requires left != right;
        terminates;
        { left }
    "#;
    for imports in ["use decoy; use selected;", "use selected; use decoy;"] {
        for (right, accepted) in [("false", true), ("true", false)] {
            let root = format!(
                "{imports}
                machine caller()
                requires selected::restricted(false, {right})
                    == selected::restricted(false, {right});
                {{}}"
            );
            let result = check_files(&[
                ("main.omg", &root),
                ("selected.omg", &selected),
                ("decoy.omg", decoy),
            ]);
            if accepted {
                result.expect(
                    "the selected declaration's equal operands satisfy its own requirement",
                );
            } else {
                let diagnostics = result
                    .expect_err("neither a foreign observer nor callee may supply the requirement");
                assert!(
                    diagnostics.contains("specification call")
                        && diagnostics.contains("restricted"),
                    "{imports}: {diagnostics}"
                );
            }
        }
    }
}

#[test]
fn proof_body_calls_cannot_erase_unequal_substituted_operands() {
    for (right, accepted) in [("false", true), ("true", false)] {
        for body in [
            format!("restricted(false, {right})"),
            format!("let answer: Nat = restricted(false, {right}); answer"),
            format!("restricted(false, {right}); Nat::Zero"),
        ] {
            let source = format!(
                "data Nat {{ case Zero; case Succ(previous: Nat); }}
                machine observe(value: bool) -> bool
                terminates;
                {{ transition {{ _ -> (value == true) }} }}
                machine restricted(left: bool, right: bool) -> Nat
                requires observe(left) == observe(right);
                terminates;
                {{ Nat::Zero }}
                machine caller() -> Nat
                ensures true == true;
                terminates;
                {{ {body} }}"
            );
            let result = check_files(&[("main.omg", &source)]);
            if accepted {
                result.expect("equal observations establish a proof call's precondition");
            } else {
                let diagnostics = result
                    .expect_err("proof classification cannot waive a call's false precondition");
                assert!(diagnostics.contains("requires"), "{body}: {diagnostics}");
            }
        }
    }
}

#[test]
fn concrete_result_fields_discharge_only_the_selected_precondition() {
    for body in [
        "Observation { flag: value, other: true }",
        "let observed: bool = value; Observation { flag: observed, other: true }",
        "transition { _ -> Observation { flag: value, other: true } }",
    ] {
        for (left, right, accepted) in [
            ("false", "false", true),
            ("true", "true", true),
            ("false", "true", false),
            ("true", "false", false),
        ] {
            let source = format!(
                "data Observation {{ flag: bool; other: bool; }}
                machine observe(value: bool) -> Observation
                terminates;
                {{ {body} }}
                machine restricted(left: bool, right: bool) -> bool
                requires observe(left).flag == right;
                terminates;
                {{ left }}
                machine caller()
                requires restricted({left}, {right}) == restricted({left}, {right});
                {{}}"
            );
            let result = check_files(&[("main.omg", &source)]);
            if accepted {
                result.unwrap_or_else(|diagnostics| panic!("body {body}: {diagnostics}"));
            } else {
                let diagnostics = result
                    .expect_err("outer reflexivity cannot hide a false result-field precondition");
                assert!(
                    diagnostics.contains("specification call")
                        && diagnostics.contains("restricted"),
                    "{diagnostics}"
                );
            }
        }
    }
}

#[test]
fn imported_result_fields_keep_the_selected_callee_and_field() {
    let selected = r#"
        module selected;
        pub data Observation { other: bool; flag: bool; }
        pub machine observe(value: bool) -> Observation
        terminates;
        { transition { _ -> Observation { other: true, flag: value } } }
    "#;
    let decoy = r#"
        module decoy;
        pub data Observation { flag: bool; other: bool; }
        pub machine observe(value: bool) -> Observation
        terminates;
        { transition { _ -> Observation { flag: true, other: value } } }
    "#;
    for imports in ["use decoy; use selected;", "use selected; use decoy;"] {
        for (field, right, accepted) in [
            ("flag", "false", true),
            ("flag", "true", false),
            ("other", "true", true),
            ("other", "false", false),
        ] {
            let root = format!(
                "{imports}
                machine restricted(right: bool) -> bool
                requires selected::observe(false).{field} == right;
                terminates;
                {{ right }}
                machine caller()
                requires restricted({right}) == restricted({right});
                {{}}"
            );
            let result = check_files(&[
                ("main.omg", &root),
                ("selected.omg", selected),
                ("decoy.omg", decoy),
            ]);
            if accepted {
                result.expect("the selected record field, not a foreign namesake or neighboring field, supplies the value");
            } else {
                let diagnostics = result.expect_err(
                    "a foreign declaration or neighboring field cannot establish the precondition",
                );
                assert!(
                    diagnostics.contains("specification call")
                        && diagnostics.contains("restricted"),
                    "{imports}: {diagnostics}"
                );
            }
        }
    }
}

#[test]
fn result_field_proofs_do_not_require_normalizing_unobserved_fields() {
    let source = r#"
        data Observation { flag: bool; unobserved: f64; }
        machine observe(value: bool) -> Observation terminates;
        { transition { _ -> Observation { flag: value, unobserved: 0.0 } } }
        machine restricted(value: bool) -> bool
        requires observe(value).flag == value;
        terminates;
        { value }
        machine caller()
        requires restricted(false) == restricted(false);
        {}
    "#;
    check_files(&[("main.omg", source)])
        .expect("an unrelated field need not have a structural normal form");
}

#[test]
fn a_false_result_guarantee_cannot_supply_a_field_precondition() {
    let source = r#"
        trait Equatable { machine equals(&self, other: &Self) -> bool; }
        data Observation { flag: bool; }
        ObservationEquatable: Observation satisfies Equatable;
        machine observe(value: bool) -> Observation
        ensures result == (Observation { flag: true });
        terminates;
        { Observation { flag: value } }
        machine restricted() -> bool
        requires observe(false).flag == true;
        terminates;
        { true }
        machine caller()
        requires restricted() == restricted();
        {}
    "#;
    let diagnostics = check_files(&[("main.omg", source)])
        .expect_err("a selected callee's false guarantee cannot supply the field premise");
    assert!(
        diagnostics.contains("specification call") && diagnostics.contains("restricted"),
        "{diagnostics}"
    );
}

#[test]
fn result_field_premises_must_precede_the_application_they_license() {
    for (premises, accepted) in [
        (
            "requires observe(value).flag == expected; requires restricted(value, expected) == restricted(value, expected);",
            true,
        ),
        (
            "requires restricted(value, expected) == restricted(value, expected); requires observe(value).flag == expected;",
            false,
        ),
    ] {
        let source = format!(
            "data Observation {{ flag: bool; }}
            machine observe(value: bool) -> Observation terminates;
            {{ Observation {{ flag: value }} }}
            machine restricted(value: bool, expected: bool) -> bool
            requires observe(value).flag == expected;
            terminates;
            {{ value }}
            machine caller(value: bool, expected: bool)
            {premises}
            {{}}"
        );
        let result = check_files(&[("main.omg", &source)]);
        if accepted {
            result.expect("the independently formed earlier field premise licenses the call");
        } else {
            let diagnostics =
                result.expect_err("a later field premise cannot license an earlier application");
            assert!(
                diagnostics.contains("specification call") && diagnostics.contains("restricted"),
                "{diagnostics}"
            );
        }
    }
}
