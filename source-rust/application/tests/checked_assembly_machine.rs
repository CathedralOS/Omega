//! A local assembly label receives the state left by the preceding instruction.
//! The same fact is required at the label on each engine.
#[path = "support/program.rs"]
mod program;
#[path = "support/project.rs"]
mod project;

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

#[test]
fn the_artifact_retains_the_local_edge_and_its_discharged_precondition() {
    use terminal_psi::AssemblyExitConstraint::{Adjacent, ScalarReturn};

    let scratch = project::stage("checked-assembly-block", &[("main.omg", ANSWER)]);
    let output = project::omega(
        scratch.path(),
        &[
            "--emit-psi",
            "answer.psi",
            "--target",
            "linux_x86_64",
            "main.omg",
        ],
    );
    assert!(output.status.success(), "{output:?}");
    let bytes = std::fs::read(scratch.path().join("answer.psi")).unwrap();
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
    let module = artifact.view().module();
    let answer = module
        .machines
        .iter()
        .find(|machine| {
            machine
                .blocks
                .iter()
                .any(|block| matches!(block.assembly_exit, Some(ScalarReturn(_))))
        })
        .expect("checked assembly return in the published program");
    let [start, finish] = answer.blocks.as_slice() else {
        panic!("two local blocks");
    };
    assert_eq!(start.assembly_exit, Some(Adjacent(finish.id)));
    let Some(ScalarReturn(view)) = finish.assembly_exit else {
        panic!("checked scalar return");
    };
    assert_eq!(view.name(), "eax");
    assert!(
        module
            .proofs
            .scalar_block_invariants
            .iter()
            .any(|invariant| invariant.machine == answer.id
                && invariant.header == finish.id
                && invariant.arrivals.len() == 1)
    );
}

#[test]
fn an_adjacent_edge_emits_no_jump_or_register_setup() {
    let scratch = project::stage("checked-assembly-layout", &[("main.omg", ANSWER)]);
    let output = project::omega(
        scratch.path(),
        &[
            "--target",
            "linux_x86_64",
            "--build-dir",
            "image",
            "main.omg",
        ],
    );
    assert!(output.status.success(), "{output:?}");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let executable = report
        .lines()
        .find_map(|line| line.strip_prefix("published native output to "))
        .expect("published native executable");
    let image = std::fs::read(scratch.path().join(executable)).unwrap();
    // Follow a direct CALL in the image. Its destination begins with the
    // authored immediate write and the very next byte starts RET. A branch or
    // register copy at `finish` breaks this assertion even if main still exits 0.
    let called_bodies = image.windows(5).enumerate().filter_map(|(offset, call)| {
        if call[0] != 0xe8 {
            return None;
        }
        let displacement = i32::from_le_bytes(call[1..].try_into().unwrap());
        let start = (offset + 5).checked_add_signed(displacement as isize)?;
        image.get(start..)
    });
    let immediate_writes: &[&[u8]] = &[
        &[0xb8, 42, 0, 0, 0],
        &[0xc7, 0xc0, 42, 0, 0, 0],
        &[0x48, 0xc7, 0xc0, 42, 0, 0, 0],
        &[0x48, 0xb8, 42, 0, 0, 0, 0, 0, 0, 0],
    ];
    assert!(
        called_bodies
            .into_iter()
            .any(|callee| immediate_writes.iter().any(|opcode| callee
                .strip_prefix(*opcode)
                .is_some_and(|tail| tail.first() == Some(&0xc3)))),
        "no call reaches the adjacent immediate-write and return instructions"
    );
}
