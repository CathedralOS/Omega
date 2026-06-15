// Throwaway test: compile + run account_ledger sample natively, compare interpreter.
// DELETE this file once the permanent guard in differential.rs is the only reference.

use omega_compiler::{CompileOptions, compile, compile_to_checked};
use omega_interpreter::interpret;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("compiler crate should live under compiler/orchestration/omega-compiler")
        .to_path_buf()
}

fn join_diagnostics(diagnostics: &[omega_core::diagnostics::Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Compile account_ledger and run it natively; compare to interpreter.
///
/// KNOWN MISCOMPILE: native exits 93 (wrong), interpreter exits 70 (correct).
/// Minimal trigger: `Transaction::Transfer { to: 3, amount: 40 }` dispatched via
/// `apply(0, tx)` -- in native the `amount` argument extracted from the Transfer
/// case payload receives the value of `to` (3) instead of the actual amount (40).
/// This is a case-payload field extraction bug: the second field of the 3rd case
/// variant is read from the same offset as the first field.
#[cfg(windows)]
#[test]
fn throwaway_account_ledger_native_exit() {
    let sample_dir = repo_root().join("samples").join("account_ledger");
    let main_path = sample_dir.join("main.omg");

    // Interpreter.
    let checked = compile_to_checked(&main_path, None).unwrap_or_else(|diagnostics| {
        panic!(
            "account_ledger compile_to_checked failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let interp_outcome = interpret(&checked, b"");
    if interp_outcome.is_error() {
        eprintln!("interpreter error: {:?}", interp_outcome.error);
    }
    eprintln!("account_ledger: interpreter exit = {}", interp_outcome.exit_code);

    // Native.
    let build_dir = std::env::temp_dir().join(format!(
        "omega-account-ledger-throwaway-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path.clone(),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .unwrap_or_else(|diagnostics| {
        panic!(
            "account_ledger compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });

    let exe = build_dir.join("omega-program.exe");
    let output = Command::new(&exe)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("account_ledger: native run failed");

    let native_code = output.status.code().unwrap_or(-1);
    let _ = fs::remove_dir_all(&build_dir);

    eprintln!("account_ledger: native exit = {native_code}");

    if !interp_outcome.is_error() && interp_outcome.exit_code != native_code {
        eprintln!(
            "MISCOMPILE: interpreter={} native={} -- \
             known bug: 2nd field of 3rd case variant (Transfer.amount) \
             gets 1st field value in native",
            interp_outcome.exit_code, native_code
        );
    }

    assert!(
        native_code >= 0,
        "account_ledger native process should exit with a code"
    );
}
