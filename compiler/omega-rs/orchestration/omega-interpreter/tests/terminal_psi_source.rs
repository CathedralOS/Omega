//! Real-source proof that the transitional producer emits a self-contained
//! terminal-Psi module: frontend trees are dropped before verification and
//! execution.

use omega_checked_trees_to_terminal_psi::{LoweringError, lower_machine};
use omega_compiler::compile_to_checked;
use omega_interpreter::{TerminalScalarValue, interpret_terminal};
use psi_core::{IntegerSign, IntegerType, IntegerValue};
use psi_proof_kernel::AdmissionProfile;
use psi_terminal_verifier::verify_module;
use std::path::{Path, PathBuf};

fn source_canary() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("omega-interpreter lives under compiler/omega-rs/orchestration")
        .join("canaries/pass/terminal_psi/integer_control_contract/main.omg")
}

#[test]
fn checked_source_survives_frontend_drop_as_verified_terminal_psi() {
    let checked = compile_to_checked(&source_canary(), None).unwrap_or_else(|diagnostics| {
        panic!(
            "terminal-Psi source canary should compile:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let lowered = lower_machine(&checked, "terminal_constant")
        .expect("accepted source slice should lower to terminal Psi");

    drop(checked);

    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source-produced terminal Psi and its proof should verify");
    let result = interpret_terminal(&verified, &[])
        .expect("verified source-produced terminal Psi should execute");
    assert_eq!(
        result,
        TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Signed, 32).expect("i32"),
            value: IntegerValue::Signed(7),
        }
    );
}

#[test]
fn compatibility_lowering_rejects_source_outside_its_declared_slice() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi source canary should compile");
    assert_eq!(
        lower_machine(&checked, "Main::main").expect_err("attached main must fail closed"),
        LoweringError::Unsupported(
            "attached machines are not in the first terminal-Psi source slice"
        )
    );
}
