//! Specification applications retain selected callees and substituted operands.

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
