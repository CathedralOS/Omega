//! A local assembly label receives the state left by the preceding instruction.
//! The same fact is required at the label on each engine.
#[path = "support/program.rs"]
mod program;

const ANSWER: &str = r#"
asm machine answer() -> u32 {
    mov eax, 42;
    transition -> finish;

    asm machine finish requires eax == 42u32 { ret; }
}

public machine main() crashes Trap {
    transition answer() == 42u32 { true -> done(), false -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
"#;

#[cfg(target_arch = "x86_64")]
#[test]
fn a_checked_assembly_return_reaches_its_caller_in_psi_and_native() {
    program::assert_silent_return(ANSWER);
}

#[test]
fn the_incoming_register_value_must_prove_the_local_entry_requirement() {
    let wrong = ANSWER.replace("mov eax, 42", "mov eax, 41");
    let (status, report) = program::invoke(&wrong, &["--check", "--target", "linux_x86_64"]);
    assert!(!status.status.success(), "{report}");
    assert!(
        report.contains("entry requirement of assembly block") && report.contains("finish"),
        "{report}"
    );
}

#[test]
fn a_working_register_has_no_default_incoming_value() {
    let unreadable = ANSWER.replace("mov eax, 42", "mov eax, ecx");
    let (status, report) = program::invoke(&unreadable, &["--check", "--target", "linux_x86_64"]);
    assert!(!status.status.success(), "{report}");
    assert!(report.contains("reads a register before"), "{report}");
}
