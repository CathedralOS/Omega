//! Differential oracle test: for a curated set of SIMPLE canaries that use only the
//! interpreter's supported subset, compile + run the NATIVE binary (windows_x64) AND run
//! the reference interpreter, then assert their exit code and stdout MATCH.
//!
//! This proves the oracle works: on supported programs, `interpret() == native`. When the
//! interpreter returns an error (unsupported construct), the canary is SKIPPED rather than
//! failed -- a missing feature is not a backend mismatch. As interpreter coverage grows,
//! a genuine `interpret() != native` divergence localizes a native backend bug.

use omega_compiler::{compile, compile_to_checked, CompileOptions};
use omega_interpreter::interpret;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Canaries known to use only the supported subset (scalar fields/locals, arithmetic,
/// comparison, guarded multi-arm transitions, value-calls returning a scalar, enum match
/// desugaring, `&mut`-argument aliasing, the Console boundary `exit_process`). Each exits
/// 70 on success with no stdout.
const DIFFERENTIAL_CANARIES: &[&str] = &[
    "expressions/runtime_field_default_exit",
    "expressions/runtime_match_value_exit",
    "expressions/runtime_call_result_binary_operand_exit",
    "arithmetic/runtime_chained_field_mutation_exit",
    "arithmetic/runtime_comparison_guard_signedness_exit",
    "arithmetic/runtime_comparison_value_signedness_exit",
    "arithmetic/runtime_min_max_signedness_exit",
    "arithmetic/runtime_copy_then_read_exit",
    "operators/compound_assignment_exit",
    "operators/unary_negation_exit",
];

#[test]
fn interpreter_matches_native_on_supported_canaries() {
    let mut matched = Vec::new();
    let mut skipped = Vec::new();

    for canary_name in DIFFERENTIAL_CANARIES {
        let canary = pass_canary(canary_name);
        let main_path = canary.join("main.omg");

        let checked = match compile_to_checked(&main_path, None) {
            Ok(checked) => checked,
            Err(diagnostics) => panic!(
                "{canary_name}: frontend compile failed:\n{}",
                join_diagnostics(&diagnostics)
            ),
        };

        let outcome = interpret(&checked, b"");
        if outcome.is_error() {
            // Unsupported construct -> skip (not a mismatch).
            skipped.push((canary_name, outcome.error.clone().unwrap_or_default()));
            continue;
        }

        // Native: compile to a windows_x64 PE and run it.
        let (native_code, native_stdout) = compile_and_run_native(canary_name, &main_path);

        assert_eq!(
            outcome.exit_code, native_code,
            "{canary_name}: interpreter exit {} != native exit {}",
            outcome.exit_code, native_code
        );
        assert_eq!(
            outcome.stdout, native_stdout,
            "{canary_name}: interpreter stdout {:?} != native stdout {:?}",
            String::from_utf8_lossy(&outcome.stdout),
            String::from_utf8_lossy(&native_stdout)
        );

        matched.push(canary_name);
    }

    eprintln!(
        "differential oracle: {} matched (interp==native), {} skipped (unsupported)",
        matched.len(),
        skipped.len()
    );
    for (name, reason) in &skipped {
        eprintln!("  skipped {name}: {reason}");
    }
    for name in &matched {
        eprintln!("  matched {name}");
    }

    // The oracle is only meaningful if at least one canary actually matched.
    assert!(
        !matched.is_empty(),
        "expected at least one canary where interpreter == native, but all were skipped:\n{}",
        skipped
            .iter()
            .map(|(name, reason)| format!("  {name}: {reason}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn compile_and_run_native(canary_name: &str, main_path: &Path) -> (i32, Vec<u8>) {
    let build_dir = std::env::temp_dir().join(format!(
        "omega-interp-diff-{}-{}",
        canary_name.replace(['/', '\\'], "_"),
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path.to_path_buf(),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|diagnostics| {
        panic!(
            "{canary_name}: native compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .unwrap_or_else(|error| panic!("{canary_name}: native run failed: {error}"));

    let code = output.status.code().unwrap_or(-1);
    let stdout = output.stdout.clone();
    let _ = fs::remove_dir_all(&build_dir);
    (code, stdout)
}

fn join_diagnostics(diagnostics: &[omega_core::diagnostics::Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("interpreter crate should live under compiler/orchestration/omega-interpreter")
        .to_path_buf()
}

fn pass_canary(path: &str) -> PathBuf {
    repo_root().join("canaries/pass").join(path)
}

#[cfg(windows)]
fn executable_name() -> &'static str {
    "omega-program.exe"
}

#[cfg(not(windows))]
fn executable_name() -> &'static str {
    "omega-program"
}
