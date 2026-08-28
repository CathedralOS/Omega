//! Focused carry-vs-suspension canaries.
//!
//! These fixtures deliberately end at checked trees: they exercise semantic
//! liveness around a `Scheduler::park` boundary, not a concrete host scheduler
//! provider. Keeping them outside the native mega-roster prevents missing host
//! lowering from obscuring the carry result they actually specify.

use omega_compiler::compile_to_checked;
use std::path::{Path, PathBuf};

#[test]
fn suspension_carry_canaries_pin_statement_bound_liveness() {
    for name in [
        "concurrency/suspend_after_last_use_compile",
        "concurrency/suspend_after_earlier_operand_compile",
        "concurrency/suspend_after_self_field_last_use_compile",
    ] {
        let pass = pass_canary(name);
        compile_to_checked(&pass.join("main.omg"), None).unwrap_or_else(|diagnostics| {
            panic!(
                "{} should compile after the restrictive value's last use:\n{}",
                pass.display(),
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    }

    for (name, expected) in [
        (
            "concurrency/suspend_live_value_rejected",
            "may suspend while `message` remains live",
        ),
        (
            "concurrency/suspend_self_field_reachable_state_rejected",
            "may suspend while `self.message` remains live",
        ),
        (
            "concurrency/suspend_call_argument_rejected",
            "may suspend while `message` remains live",
        ),
        (
            "concurrency/suspend_later_operand_rejected",
            "nested inside a partially evaluated expression",
        ),
    ] {
        let fail = fail_canary(name);
        let diagnostics = compile_to_checked(&fail.join("main.omg"), None)
            .expect_err("a restrictive live value must reject possible suspension");
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            combined.contains(expected),
            "{} emitted the wrong diagnostic:\n{}",
            fail.display(),
            combined
        );
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect(
            "compiler crate should live under source/omega-rust/omega/orchestration/omega-compiler",
        )
        .to_path_buf()
}

fn pass_canary(path: &str) -> PathBuf {
    repo_root().join("tests/canaries/pass").join(path)
}

fn fail_canary(path: &str) -> PathBuf {
    repo_root().join("tests/canaries/fail").join(path)
}
