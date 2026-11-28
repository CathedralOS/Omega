//! A local assembly label receives the state left by the preceding instruction.
//! The same fact is required at the label on each engine.
#[path = "support/observations.rs"]
mod observations;
use observations::project;
use platform_custody::temporary_directory::TemporaryDirectory;

fn stage_source(source: &str) -> TemporaryDirectory {
    project::stage("checked-assembly", &[("main.omg", source)])
}

fn check_x86_source(source: &str) -> (std::process::Output, String) {
    observations::run(
        &stage_source(source),
        &["--check", "--target", "linux_x86_64"],
        &["main.omg"],
    )
}

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

#[test]
fn a_checked_assembly_return_reaches_its_caller_in_psi_and_native() {
    let result_register = if cfg!(target_arch = "aarch64") {
        "w0"
    } else {
        "eax"
    };
    let source = ANSWER.replace("eax", result_register);
    observations::assert_silent_return(|| stage_source(&source), &["main.omg"]);
}

#[test]
fn the_incoming_register_value_must_prove_the_local_entry_requirement() {
    let wrong = ANSWER.replace("mov eax, 42", "mov eax, 41");
    let (status, report) = check_x86_source(&wrong);
    assert!(!status.status.success(), "{report}");
    assert!(
        report.contains("entry requirement of assembly block") && report.contains("finish"),
        "{report}"
    );
}

#[test]
fn a_working_register_has_no_default_incoming_value() {
    let unreadable = ANSWER.replace("mov eax, 42", "mov eax, ecx");
    let (status, report) = check_x86_source(&unreadable);
    assert!(!status.status.success(), "{report}");
    assert!(report.contains("reads a register before"), "{report}");
}

#[test]
fn inline_moves_can_only_read_registers_initialized_in_the_same_region() {
    let source =
        "machine main() { let mutable result: u32 = 0; asm { mov eax, 37; mov result, eax } }";
    let (status, report) = check_x86_source(source);
    assert!(status.status.success(), "{report}");
    for moves in ["mov result, eax", "mov eax, 37 } asm { mov result, eax"] {
        let (status, report) =
            check_x86_source(&source.replace("mov eax, 37; mov result, eax", moves));
        assert!(!status.status.success(), "{report}");
        assert!(report.contains("reads a register before"), "{report}");
    }
}

#[test]
fn the_artifact_retains_the_local_edge_and_its_discharged_precondition() {
    use terminal_psi::AssemblyExitConstraint::{Adjacent, ScalarReturn};

    let scratch = stage_source(ANSWER);
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
    let Some(Adjacent { target, registers }) = &start.assembly_exit else {
        panic!("adjacent local transfer");
    };
    assert_eq!(*target, finish.id);
    assert_eq!(
        registers.iter().map(|view| view.name()).collect::<Vec<_>>(),
        ["eax"]
    );
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
    let scratch = stage_source(ANSWER);
    assert_immediate_return_image(scratch.path(), &["main.omg"], &[42]);
}

#[test]
fn selecting_call_inlining_preserves_the_checked_assembly_call_and_return() {
    let build = r#"
use main.omg as app;
machine build(plan: @mutable BuildPlan) {
    plan.product = BuildProduct::Application { name: "assembly_return" };
    Optimizations::enable(@mutable plan.optimizations, Optimization::SelectedAcyclicCallInliningV1);
    Build::bind(plan, linux_x86_64::ProgramEntry, |app|::main);
}
"#;
    let scratch = project::stage(
        "inlined-checked-assembly",
        &[("build.omg", build), ("main.omg", ANSWER)],
    );
    assert_immediate_return_image(
        scratch.path(),
        &[
            "--build-input",
            "build.omg",
            "--build-input",
            "main.omg",
            "build.omg",
        ],
        &[42],
    );
}

fn assert_immediate_return_image(directory: &std::path::Path, inputs: &[&str], expected: &[u32]) {
    let mut options = vec!["--target", "linux_x86_64", "--build-dir", "image"];
    options.extend_from_slice(inputs);
    let output = project::omega(directory, &options);
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
    let image = std::fs::read(directory.join(executable)).unwrap();
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
    assert!(
        called_bodies.into_iter().any(|callee| {
            expected
                .iter()
                .try_fold(callee, |remaining, &value| {
                    after_write_to_eax(remaining, value)
                })
                .is_some_and(|tail| tail.first() == Some(&0xc3))
        }),
        "no direct call reaches the required consecutive register writes and RET"
    );
}

// The emitter can use either the register-immediate or r/m-immediate MOV,
// including a full-width move with the same zero-extended result.
fn after_write_to_eax(bytes: &[u8], value: u32) -> Option<&[u8]> {
    for prefix in [&[0xb8][..], &[0xc7, 0xc0], &[0x48, 0xc7, 0xc0]] {
        if let Some(rest) = bytes
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_prefix(&value.to_le_bytes()[..]))
        {
            return Some(rest);
        }
    }
    bytes
        .strip_prefix(&[0x48, 0xb8][..])?
        .strip_prefix(&u64::from(value).to_le_bytes()[..])
}

#[cfg(target_arch = "x86_64")]
#[test]
fn a_register_overwritten_after_a_local_edge_retains_both_versions() {
    let source = ANSWER.replace("mov eax, 42;", "mov eax, 1;").replace(
        "requires eax == 42u32 { ret; }",
        "requires eax == 1u32 { mov eax, 42; ret; }",
    );
    let scratch = stage_source(&source);
    assert_immediate_return_image(scratch.path(), &["main.omg"], &[1, 42]);
    observations::assert_silent_return(|| stage_source(&source), &["main.omg"]);
    let missing_intermediate = source.replace("mov eax, 1;", "mov eax, 2;");
    let (output, report) = check_x86_source(&missing_intermediate);
    assert!(
        !output.status.success()
            && report.contains("entry requirement of assembly block")
            && report.contains("finish"),
        "{report}"
    );
}
