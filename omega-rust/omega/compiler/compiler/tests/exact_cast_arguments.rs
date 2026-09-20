//! Exact conversion obligations apply inside ordinary call arguments, before
//! the checked Unit planner decides whether it can realize the call.

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

fn check_call(parameter: &str, body: &str) -> Result<(), String> {
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "omega-exact-cast-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
    )));
    fs::create_dir_all(&fixture.0).unwrap();
    let root = fixture.0.join("main.omg");
    fs::write(
        &root,
        format!(
            r#"
pub data Sink {{ last: u8; }}
machine Sink::store(&mut self, byte: u8) {{ self.last = byte; }}
machine Sink::store_after(&mut self, ignored: u8, byte: u8) {{ self.last = byte; }}
machine overwrite(value: &mut i32) {{ value = 256; }}
machine overwrite_result(value: &mut i32) -> u8 {{ value = 256; 0u8 }}
pub machine Sink::accept(&mut self, value: {parameter}) {{ {body} }}
"#
        ),
    )
    .unwrap();
    compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: root,
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

#[test]
fn exact_call_argument_without_fit_evidence_rejects_during_checking() {
    for parameter in ["i32", "i32 [-1..=255]", "i32 [0..=256]"] {
        let diagnostics = check_call(parameter, "self.store(value as u8);")
            .expect_err("an unproven exact conversion must fail source checking");
        assert!(
            diagnostics.contains("Exact integer cast")
                && diagnostics.contains("not provably representable"),
            "{parameter}: {diagnostics}"
        );
    }
}

#[test]
fn exact_call_argument_uses_declared_and_live_guard_bounds() {
    check_call("i32 [0..=255]", "self.store(value as u8);")
        .expect("declared bounds establish the exact conversion");
    check_call(
        "i32",
        r#"
        transition value >= 0 && value <= 255 {
            true -> store_value(value)
            _ -> done()
        }
        state store_value(&mut self, value: i32) { self.store(value as u8); }
        state done(&mut self) {}
    "#,
    )
    .expect("live guard bounds establish the exact conversion at its call");
    check_call("u8", "self.store((value as u16) as u8);")
        .expect("widening retains its source bound for the later exact narrowing");
}

#[test]
fn exact_call_argument_requires_both_live_bounds() {
    for guard in ["value >= 0", "value <= 255"] {
        let diagnostics = check_call(
            "i32",
            &format!(
                r#"
            transition {guard} {{ true -> store_value(value) _ -> done() }}
            state store_value(&mut self, value: i32) {{ self.store(value as u8); }}
            state done(&mut self) {{}}
        "#
            ),
        )
        .expect_err("one bound alone cannot establish an exact signed-to-byte cast");
        assert!(
            diagnostics.contains("Exact integer cast"),
            "{guard}: {diagnostics}"
        );
    }
}

#[test]
fn exact_call_argument_cannot_reuse_bounds_invalidated_by_a_call() {
    let diagnostics = check_call(
        "i32",
        r#"
        let candidate: i32 = 1;
        overwrite(&mut candidate);
        self.store(candidate as u8);
    "#,
    )
    .expect_err("an earlier in-range value does not prove the mutated argument fits");
    assert!(diagnostics.contains("Exact integer cast"), "{diagnostics}");
}

#[test]
fn exact_call_argument_cannot_reuse_bounds_invalidated_by_an_earlier_argument() {
    let diagnostics = check_call(
        "i32",
        r#"
        let candidate: i32 = 1;
        self.store_after(overwrite_result(&mut candidate), candidate as u8);
    "#,
    )
    .expect_err("an earlier argument may invalidate a later argument's conversion bound");
    assert!(diagnostics.contains("Exact integer cast"), "{diagnostics}");
}
