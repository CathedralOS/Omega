//! Missing-operator diagnostics use the same complete tuple as operator selection.

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
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn check(source: &str) -> Result<(), String> {
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "omega-operator-operands-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
    )));
    fs::create_dir(&fixture.0).unwrap();
    fs::write(fixture.0.join("main.omg"), source).unwrap();
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

const PAIR: &str = r#"
    data Wrapped { value: u8; }
    machine + Wrapped::add(left: Wrapped, right: Wrapped) -> u64 {
        7u64
    }
"#;

#[test]
fn unrelated_tuple_cannot_supply_either_operand_order() {
    for (parameters, expression) in [
        ("left: Wrapped, right: u64", "left + right"),
        ("left: u64, right: Wrapped", "left + right"),
    ] {
        let diagnostics = check(&format!(
            "{PAIR} machine combine({parameters}) -> u64 {{ {expression} }}"
        ))
        .expect_err("a same-token declaration for another tuple cannot supply this use");
        assert!(
            diagnostics.contains("no such operator is declared for these operand types"),
            "{diagnostics}"
        );
    }
}

#[test]
fn matching_tuple_keeps_its_declared_meaning() {
    check(&format!(
        "{PAIR} machine combine(left: Wrapped, right: Wrapped) -> u64 {{ left + right }}"
    ))
    .expect("matching both operands retains the ordinary declared call");
}

#[test]
fn mixed_tuple_can_be_owned_by_either_operand() {
    for parameters in ["left: &Wrapped, right: u64", "left: u64, right: &Wrapped"] {
        check(&format!(
            r#"
            data Wrapped {{ value: u8; }}
            machine + Wrapped::combine({parameters}) -> u64 {{ 7u64 }}
            machine invoke({parameters}) -> u64 {{ left + right }}
        "#
        ))
        .expect("a matching heterogeneous tuple need not put its data operand first");
    }
}

#[test]
fn literal_operands_still_require_the_selected_parameter_type() {
    // The early binder may retain an unknown literal type as a candidate;
    // ordinary call validation must still reject the incompatible argument.
    for (parameters, expression) in [
        ("left: Wrapped", "left + 3u64"),
        ("right: Wrapped", "3u64 + right"),
    ] {
        let diagnostics = check(&format!(
            "{PAIR} machine combine({parameters}) -> u64 {{ {expression} }}"
        ))
        .expect_err("an unresolved literal cannot bypass nominal parameter checking");
        assert!(
            diagnostics.contains("a scalar value cannot fill a struct or enum slot"),
            "{diagnostics}"
        );
    }
}
