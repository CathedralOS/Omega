use omega_compiler::{CompileOptions, CompileReport, compile};
use omega_core::diagnostics::Diagnostic;
use std::fs;
#[cfg(not(windows))]
use std::io::Write;
#[cfg(windows)]
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(not(windows))]
use std::process::Command;
#[cfg(windows)]
use std::process::Command;
#[cfg(not(windows))]
use std::process::Stdio;
#[cfg(windows)]
use std::process::Stdio;

#[cfg(windows)]
#[test]
fn windows_x64_cli_mvp_emits_runnable_pe() {
    let sample = repo_root().join("samples").join("cli_mvp");
    let main_path = sample.join("main.omg");
    // Build into the in-repo `build/` so the committed/runnable artifact always
    // matches HEAD: a passing suite leaves a fresh exe at samples/cli_mvp/build/.
    // (Regenerated clean each run; NOT deleted afterward, unlike the temp-dir
    // canaries.) Prevents the "run the exe in the folder and see stale garbage"
    // trap.
    let build_dir = sample.join("build");
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".to_owned()),
        write_output: true,
    })
    .expect("Windows x64 CLI MVP should compile to a PE executable");

    let output = Command::new(build_dir.join("omega-program.exe"))
        .output()
        .expect("Windows x64 PE executable should run");

    assert!(
        output.status.success(),
        "generated PE exited with {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Hello, Omega.\n");
}

// Cross-compile cli_mvp to a linux_x64 ELF and verify its structure + syscall
// sequences. No execution (the suite host is Windows): the x86_64 Linux System V
// syscall host-call path + ELF emission are validated by the emitted bytes. Guards
// the genericized host-call pipeline (x86_64 now has both win32-import and
// linux-syscall host calls).
#[test]
fn linux_x64_cli_mvp_emits_elf_with_syscalls() {
    let sample = repo_root().join("samples").join("cli_mvp");
    let main_path = sample.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-linux-x64-cli-mvp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".to_owned()),
        write_output: true,
    })
    .expect("linux_x64 cli_mvp should compile to an ELF executable");

    let elf = fs::read(build_dir.join("omega-program")).expect("linux_x64 ELF should be emitted");

    // ELF64 magic + e_machine == EM_X86_64 (62) at offset 18.
    assert_eq!(&elf[0..4], b"\x7fELF", "ELF magic");
    assert_eq!(
        u16::from_le_bytes([elf[18], elf[19]]),
        62,
        "e_machine should be EM_X86_64"
    );
    // The `syscall` instruction (0F 05) and the exit_group syscall number setup
    // (`mov rax, 231`) must be present -- the System V syscall sequence.
    assert!(
        elf.windows(2).any(|w| w == [0x0f, 0x05]),
        "ELF should contain a `syscall` (0F 05) instruction"
    );
    assert!(
        elf.windows(10)
            .any(|w| w == [0x48, 0xb8, 0xe7, 0, 0, 0, 0, 0, 0, 0]),
        "ELF should set rax = 231 (exit_group) for the exit syscall"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Cross-compile the dungeon crawler (the richest non-visual sample) to a linux_x64
// ELF. Unlike cli_mvp this drives runtime-storage syscall arguments: the room
// descriptions are String descriptors in statically-allocated state regions, so the
// write(2) syscall marshals a runtime pointer/length into rsi/rdx via the r15/rax
// staging path. No execution (Windows host); validated by the emitted ELF + the
// presence of the runtime-storage load sequence (`mov r15, imm64` then a load).
#[test]
fn linux_x64_dungeon_crawler_emits_elf_with_runtime_storage_syscalls() {
    let sample = repo_root().join("samples").join("dungeon_crawler_cli");
    let main_path = sample.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-linux-x64-dungeon-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: Some("linux_x64".to_owned()),
        write_output: true,
    })
    .expect("linux_x64 dungeon crawler should compile to an ELF executable");

    let elf = fs::read(build_dir.join("omega-program")).expect("linux_x64 ELF should be emitted");

    assert_eq!(&elf[0..4], b"\x7fELF", "ELF magic");
    assert_eq!(
        u16::from_le_bytes([elf[18], elf[19]]),
        62,
        "e_machine should be EM_X86_64"
    );
    assert!(
        elf.windows(2).any(|w| w == [0x0f, 0x05]),
        "ELF should contain a `syscall` (0F 05) instruction"
    );
    // Runtime-storage syscall marshalling: `mov r15, imm64` (49 BF, the relocated
    // region base) followed somewhere by `mov rax, [r15 + disp32]` (49 8B 87) and the
    // staging `mov rsi, rax` (48 89 C6). Their presence proves a runtime String was
    // marshalled into a syscall argument rather than rejected by the encoder.
    assert!(
        elf.windows(2).any(|w| w == [0x49, 0xbf]),
        "ELF should load a relocated region base into r15 for a runtime-storage syscall arg"
    );
    assert!(
        elf.windows(3).any(|w| w == [0x49, 0x8b, 0x87]),
        "ELF should read a String descriptor field into rax (mov rax, [r15+disp32])"
    );
    // The line read (read_line) lowers to a byte-at-a-time read(2) loop, NOT a win32
    // ReadFile import: each iteration reads one byte -- `mov edx,1` (count) + `mov eax,0`
    // (read syscall number) + syscall. Its presence proves stdin input works via the
    // syscall path (a win32-import read would emit a `call rel32`, no read syscall).
    assert!(
        elf.windows(12).any(|w| w
            == [
                0xba, 0x01, 0x00, 0x00, 0x00, 0xb8, 0x00, 0x00, 0x00, 0x00, 0x0f, 0x05
            ]),
        "ELF should set up a read(2) line-read loop (mov edx,1; mov eax,0; syscall)"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[cfg(windows)]
#[test]
fn windows_x64_dungeon_crawler_emits_runnable_pe() {
    let sample = repo_root().join("samples").join("dungeon_crawler_cli");
    let main_path = sample.join("main.omg");
    // Build into the in-repo `build/` so the runnable artifact always matches HEAD
    // (regenerated clean each run, NOT deleted afterward). This is the durable fix
    // for the stale-artifact trap: `samples/dungeon_crawler_cli/build/omega-program.exe`
    // is rewritten by every green suite run.
    let build_dir = sample.join("build");
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x64".to_owned()),
        write_output: true,
    })
    .expect("Windows x64 dungeon crawler should compile to a PE executable");

    let mut child = Command::new(build_dir.join("omega-program.exe"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Windows x64 dungeon PE executable should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        // Drive the full non-visual game loop: move into deeper rooms, claim the
        // treasure (inventory mutation), and resolve combat. This exercises
        // movement-at-depth, room events, the item/use path, and the enemy/fight
        // path -- the systems that make the dungeon a real integration marker.
        .write_all(b"north\r\nnorth\r\nuse\r\nnorth\r\nfight\r\nquit\r\n")
        .expect("scripted dungeon input should be written");
    let output = child
        .wait_with_output()
        .expect("Windows x64 dungeon PE executable should finish");

    assert!(
        output.status.success(),
        "generated dungeon PE exited with {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Dungeon Crawler"));
    assert!(stdout.contains("A bottomless dark room near the dungeon heart."));
    // The room event and paths lines are produced by inline-branching calls that
    // run in render's CONTINUATION after the dispatched (looping) find_room call.
    // They must render their own text, not echo the description (regression guard
    // for the leaf-binding + cross-segment frame-slot resolution fix).
    assert!(
        stdout.contains("The room is quiet."),
        "room event line should render, not echo the description\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("[Paths] north"),
        "room paths line should render, not echo the description\nstdout:\n{stdout}"
    );
    // Look-AT-DEPTH: after moving north (R00 -> R01) the deeper room must render its
    // OWN generated data, not blank/stale. find_room is a VALUE-position call to a
    // looping machine; before the value-return-in-dispatch keystone it returned a
    // too-small room_count at the real program's scale so deeper rooms rendered
    // empty. Guards that the keystone fixed the dungeon's "trash on move" bug.
    assert!(
        stdout.contains("== Branch Room =="),
        "deeper room (R01) name should render after moving north\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("A shallow limestone room with fresh claw marks."),
        "deeper room (R01) description should render after moving north\nstdout:\n{stdout}"
    );
    // The item/use path: claiming the treasure cache (deeper room) mutates state
    // and reports it -- a cross-machine inventory write driven from a command.
    assert!(
        stdout.contains("You take the treasure."),
        "the `use` command should claim the treasure in the treasure room\nstdout:\n{stdout}"
    );
    // The combat path: an enemy room resolves a fight to a win + reward.
    assert!(
        stdout.contains("The enemy collapses."),
        "the `fight` command should resolve combat in an enemy room\nstdout:\n{stdout}"
    );
    // Intentionally NOT removing build_dir: leave the fresh, verified artifact in
    // samples/dungeon_crawler_cli/build/ so running the in-repo exe matches HEAD.
}

#[test]
fn contract_canary_visualizes_flow_contract_summaries() {
    let canary = pass_canary("domains/contracts_domain_membership_surface");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-contract-canary-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("contract canary should compile with visual artifacts");

    let state_graph = fs::read_to_string(build_dir.join("06_state_graph.html"))
        .expect("state graph visualization should be written");
    let control_flow = fs::read_to_string(build_dir.join("07_control_flow.html"))
        .expect("control flow visualization should be written");
    let checked_trees = fs::read_to_string(build_dir.join("05_checked_trees.html"))
        .expect("checked tree visualization should be written");
    let abstract_operations = fs::read_to_string(build_dir.join("08_abstract_operations.html"))
        .expect("abstract operations visualization should be written");
    let machine_instructions = fs::read_to_string(build_dir.join("11_machine_instructions.html"))
        .expect("machine instructions visualization should be written");

    assert!(
        state_graph.contains("contract call #1.0 requires 1 ensures 1"),
        "state graph should show propagated contract call summaries"
    );
    assert!(
        control_flow.contains("contract call #1.0 requires 1 ensures 1"),
        "control flow should show propagated contract call summaries"
    );
    assert!(
        checked_trees.contains("call Main::heal::heal"),
        "checked tree visualization should expose checked call nodes"
    );
    assert!(
        checked_trees.contains("contracts: requires 1 ensures 1"),
        "checked call nodes should summarize propagated contract counts"
    );
    assert!(
        checked_trees.contains("borrow: access player: mutable invalidations 1"),
        "checked call nodes should surface borrow access and invalidation detail"
    );
    assert!(
        checked_trees.contains("Main::main::main [checked]"),
        "checked tree visualization should now be a scoped graph view instead of a text report"
    );
    assert!(
        abstract_operations.contains("Main::main::main [0]")
            && abstract_operations.contains("00 EnterFunction @ statement 0"),
        "abstract operations should render backend state blocks with ordered instruction lines"
    );
    assert!(
        machine_instructions.contains("Machine Instructions")
            && machine_instructions.contains("DispatchLoopEnter")
            && machine_instructions.contains("Main::main::main [0]")
            && machine_instructions.contains("control:")
            && machine_instructions.contains("terminator:"),
        "machine instruction stage should render block-local instruction listings"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn boundary_trait_canary_reports_capability_use() {
    let canary = pass_canary("traits/boundary_trait_effects_host_call");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-capability-manifest-canary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("boundary trait canary should compile with capability artifacts");

    let manifest = fs::read_to_string(build_dir.join("05_capability_manifest.json"))
        .expect("capability manifest should be written");
    assert!(
        manifest.contains("\"capability_flows\": {\"uses\": 2"),
        "capability manifest should report both boundary capability uses\n{}",
        manifest
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Frozen decision 10 (wire eras): cross-era type changes are legal evolution
// surfaced as "requires migration" verdicts in the wire protocol compatibility
// report, and the report compares ADJACENT eras along the version chain
// (v1 -> v2, newest era -> current), never every era against current.
#[test]
fn wire_cross_era_type_change_reports_requires_migration_verdict() {
    let canary = pass_canary("wire/wire_cross_era_type_change_migration");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-wire-migration-verdict-canary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("cross-era type change canary should compile with a migration verdict, not an error");

    let report = fs::read_to_string(build_dir.join("04_wire_protocols.txt"))
        .expect("wire protocol compatibility report should be written");
    assert!(
        report.contains("### compatibility v1 -> v2")
            && report.contains("### compatibility v2 -> current"),
        "wire report should compare adjacent eras along the version chain\n{}",
        report
    );
    assert!(
        report.contains(
            "field 0 changes type i32 -> i64; decode via the old era's table and migrate up the chain"
        ),
        "wire report should record the cross-era type change as a requires-migration verdict\n{}",
        report
    );
    assert!(
        !report.contains("### compatibility v1 -> current"),
        "wire report should not compare a non-newest era against the current body\n{}",
        report
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The lowered ownership summary must stay visible per event in the backend
// report's Artifact Semantic Spine: each move/drop names its place, its
// machine/state, and its source point after surviving the full spine (checked
// trees -> state graph -> control flow -> abstract -> target -> assigned ->
// machine instructions -> encoded machine). The canary moves `self.seed` into
// an owned local and moves the local out through a transition `Value` target,
// so the spine must show both moves plus the local's exit-edge drop
// obligation. Drops here are obligations, not emitted cleanup code: no type
// carries a cleanup machine yet.
#[test]
fn backend_report_renders_ownership_summary_events() {
    let canary = pass_canary("ownership/transition_value_owned_move");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-ownership-spine-canary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("transition value owned move canary should compile");

    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("backend report should be written");
    assert!(
        report.contains("- move `self.seed` in machine `Main::main` state `main` at statement 0"),
        "spine should record the `self.seed` initializer move\n{}",
        report
    );
    assert!(
        report.contains("- move `produced` in machine `Main::main` state `main` at statement 1"),
        "spine should record the transition value target move of the owned local\n{}",
        report
    );
    assert!(
        report.contains("- drop `produced` in machine `Main::main` state `main` at state exit"),
        "spine should record the owned local's exit-edge drop obligation\n{}",
        report
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn capability_pass_canaries_compile_in_isolation() {
    // A focused guard for the capability canaries, independent of the batched
    // `pass_canaries_compile` sweep (which also covers them).
    for canary_name in [
        "capabilities/uses_caller_folder",
        "capabilities/acquires_filesystem_authority",
        "capabilities/stores_capability",
    ] {
        let canary = pass_canary(canary_name);
        if let Err(diagnostics) = compile_canary_without_output(&canary) {
            panic!(
                "expected capability canary {} to compile, but got diagnostics:\n{}",
                canary.display(),
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
    }
}

#[test]
fn capability_manifest_reports_authority_flow_verbs() {
    for (canary_name, verb) in [
        ("capabilities/acquires_filesystem_authority", "acquires"),
        ("capabilities/stores_capability", "stores"),
    ] {
        let canary = pass_canary(canary_name);
        let build_dir = std::env::temp_dir().join(format!(
            "omega-capability-verb-canary-{}-{}",
            verb,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);

        compile(CompileOptions {
            root_path: canary.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: None,
            write_output: true,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "{canary_name} should compile, got:\n{}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });

        let manifest = fs::read_to_string(build_dir.join("05_capability_manifest.json"))
            .expect("capability manifest should be written");
        assert!(
            !manifest.contains(&format!("\"{verb}\": 0"))
                && manifest.contains(&format!("\"{verb}\":")),
            "manifest for {canary_name} should report a non-zero {verb} verb\n{manifest}"
        );

        let boundary = fs::read_to_string(build_dir.join("10_boundary.html"))
            .expect("boundary report should be written");
        assert!(
            boundary.contains("Capability Blast Radius") && boundary.contains("approved provider"),
            "boundary report for {canary_name} should surface the capability blast radius\n{boundary}"
        );
        assert!(
            boundary.contains("Boundary Providers"),
            "boundary report for {canary_name} should surface the provider registry\n{boundary}"
        );

        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn capability_flows_propagate_through_nested_helpers() {
    // Capability facts must follow returns/derives/acquires across nested calls,
    // not just direct boundary calls: a helper that mints or derives authority
    // and returns it flows the same verb up to its caller, and the boundary
    // report records the helper as provenance.
    for (canary_name, propagated_lines) in [
        (
            "capabilities/acquires_through_helper_return",
            // The second line shows the verb traveling a further call level: the
            // entry machine acquires through the mid-level helper, which acquired
            // through the boundary-touching helper.
            &[
                "Backup::stage acquires via Vault::pick",
                "Main::main acquires via Backup::stage",
            ][..],
        ),
        (
            "capabilities/derives_through_helper",
            &["Worker::open_main_log derives via Worker::open_log"][..],
        ),
    ] {
        let canary = pass_canary(canary_name);
        let build_dir = std::env::temp_dir().join(format!(
            "omega-capability-nested-canary-{}-{}",
            canary_name.rsplit('/').next().unwrap_or("canary"),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);

        compile(CompileOptions {
            root_path: canary.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: None,
            write_output: true,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "{canary_name} should compile, got:\n{}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });

        let boundary = fs::read_to_string(build_dir.join("10_boundary.html"))
            .expect("boundary report should be written");
        for propagated_line in propagated_lines {
            assert!(
                boundary.contains(propagated_line),
                "boundary report for {canary_name} should record the nested-helper provenance \
                 line `{propagated_line}`\n{boundary}"
            );
        }

        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn unapproved_host_call_canary_is_rejected() {
    let canary = fail_canary("capabilities/unapproved_host_call");
    let diagnostics = match compile_canary_without_output(&canary) {
        Ok(report) => panic!(
            "expected unapproved host call canary to reject, but it compiled: {}",
            report.summary()
        ),
        Err(diagnostics) => diagnostics,
    };
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("unapproved host call"),
        "expected unapproved host call diagnostic, got:\n{combined}"
    );
}

#[test]
fn unary_negation_exit_canary_runs() {
    let canary = pass_canary("operators/unary_negation_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-unary-negation-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("unary negation canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("unary negation canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected negative literals and unary negation to evaluate correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_shift_operators_exit_canary_runs() {
    let canary = pass_canary("operators/runtime_shift_operators_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-shift-operators-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("shift operators canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("shift operators canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `<<` and arithmetic `>>` (incl. negative) to evaluate correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn integer_literal_suffix_exit_canary_runs() {
    let canary = pass_canary("operators/integer_literal_suffix_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-integer-literal-suffix-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("integer literal suffix canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("integer literal suffix canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected suffixed integer literals (i64/u32/usize) to round-trip (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_position_branching_call_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_value_position_branching_call_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-position-branching-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("value-position branching call canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("value-position branching call canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `let r = self.classify(x)` to bind the selected arm value (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_free_machine_value_call_exit_canary_runs() {
    // A value-position call to a FREE stateful machine (top-level `machine pick`,
    // no attached data, 2-arm guarded value transition) must deliver the selected
    // arm's value. The backend state-call collector resolved only local states and
    // attached (method) machines, so `pick(self.v)` was never collected: `let n =
    // pick(self.v)` silently left n at 0 and a field target failed loudly. Covers
    // both a `let` local and a field target, both arms; exits 70 only when correct.
    let canary = pass_canary("calls/runtime_free_machine_value_call_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-free-machine-value-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("free-machine value call canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("free-machine value call canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `let n = pick(self.v)` / `self.low = pick(self.v)` on a free machine to bind the selected arm value (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_free_machine_value_call_mut_arg_exit_canary_runs() {
    // A free-machine value call carrying a `&mut` tally argument alongside the
    // returned value: the callee increments the caller's field through the
    // reference AND returns the selected arm value. The tally is a counting probe
    // pinning call-count semantics (exactly one call). Exits 70 only when both
    // the returned value and the tally are correct.
    let canary = pass_canary("calls/runtime_free_machine_value_call_mut_arg_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-free-machine-mut-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("free-machine mut-arg value call canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("free-machine mut-arg value call canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a free-machine value call with a &mut tally arg to return 100 and bump the tally once (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_free_machine_looping_value_call_exit_canary_runs() {
    // A value-position call to a LOOPING free machine (`count` walks a slice via
    // the self-recursive transition `count(s[1..], acc + 1)`). The recursive
    // target names the MACHINE, whose implicit body state is the generated
    // `entry` (attached machines name it after the method), so the transition
    // planner rejected it ("unknown state transition target"); it now resolves to
    // the entry segment as a real back-edge, and the looped accumulator is
    // delivered to the caller's `let n` slot. Exits 70 only when n == 5.
    let canary = pass_canary("calls/runtime_free_machine_looping_value_call_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-free-machine-looping-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("looping free-machine value call canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("looping free-machine value call canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a looping free-machine value call to deliver the looped accumulator (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_numeric_cast_exit_canary_runs() {
    let canary = pass_canary("expressions/runtime_numeric_cast_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-numeric-cast-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("numeric cast canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("numeric cast canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected numeric casts (float->int, int->float, signed widen) to evaluate correctly and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_float_place_comparison_exit_canary_runs() {
    let canary = pass_canary("expressions/runtime_float_place_comparison_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-float-place-compare-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("float place comparison canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float place comparison canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected field-to-field float comparisons (<, >=, negative) to evaluate correctly and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_float_comparison_exit_canary_runs() {
    let canary = pass_canary("expressions/runtime_float_comparison_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-float-compare-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("float comparison canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float comparison canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected float comparison guards (==, <, negative operand) to evaluate correctly and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_float_arithmetic_exit_canary_runs() {
    let canary = pass_canary("expressions/runtime_float_arithmetic_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-float-arith-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("float arithmetic canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected float arithmetic (add/sub/mul/div + field operand) to execute and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_field_default_exit_canary_runs() {
    // A `data` field with a default initializer (`x: i32 = 5`) must hold that value at
    // runtime; the default was captured front-end but never emitted (fields read 0).
    // Entry-machine constant field defaults are now initialized before the dispatch
    // loop, recursing into nested `data` members. Covers int/f64/bool defaults +
    // explicit overwrite + nested-data defaults; exits 70 when correct.
    let canary = pass_canary("expressions/runtime_field_default_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-field-default-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("field-default canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("field-default canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `data` field defaults (int/f64/bool) to initialize and overwrite (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_version_migration_exit_canary_runs() {
    // Chapter 21 (Versioned Data), first runtime migration: a historical-shape
    // value is CONSTRUCTED (`Counter::v1 { counter: 3 }` -- the brace literal
    // resolves to the data's `version v1` shape, not a case), the migration
    // machine `Counter::from_v1(old, &mut current)` is called through the data
    // type name, and the migrated current-shape fields drive the exit (the
    // guard `count * 10 + timestamp == 77` holds only when both migrated
    // writes landed; exits 70 when correct).
    let canary = pass_canary("versioning/runtime_version_migration_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-version-migration-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("version migration canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("version migration canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Counter::v1 construction + Counter::from_v1 migration to land both current-shape writes (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_versioned_era_query_exit_canary_runs() {
    // Chapter 21 stage 3 (frozen decision 14): `Versioned<T>` is the builtin
    // era-tagged container and `era` is read-only source-queryable. The only
    // reachable container today is the zero-initialized data field (no source
    // constructor -- boundaries mint it), so `self.raw.era` must read 0 (the
    // oldest declared era under decision-10 numbering) and exit 70.
    let canary = pass_canary("versioning/runtime_versioned_era_query_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-versioned-era-query-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("versioned era query canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("versioned era query canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the ZII `Versioned<Counter>` field's `era` to read 0 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_versioned_match_zii_exit_canary_runs() {
    // Chapter 21 stage 3 (frozen decision 14): version match arms are legal on
    // `Versioned<T>` subjects; the paren form binds the WHOLE historical
    // value. The ZII container carries era 0 = v1, and the CURRENT arm is
    // written FIRST, so selecting the v1 arm pins real era-tag dispatch (not
    // first-arm fallthrough); the bound `old.counter` payload reads 0 -> 70.
    let canary = pass_canary("versioning/runtime_versioned_match_zii_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-versioned-match-zii-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("versioned match canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("versioned match canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the era-0 (v1) arm to be selected with a zero payload (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_versioned_era_guard_exit_canary_runs() {
    // Chapter 21 stage 3 (frozen decision 14): `era` is read-only
    // source-queryable DIRECTLY as a transition guard subject (the era-query
    // canary reads it through a local first; this pins the guard-position
    // read). ZII container carries era 0 -> the `== 0` arm exits 70.
    let canary = pass_canary("versioning/runtime_versioned_era_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-versioned-era-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("versioned era guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("versioned era guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the ZII `Versioned<Counter>` era to read 0 in guard position (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_versioned_three_era_match_zii_exit_canary_runs() {
    // Chapter 21 stage 3: version match arms over a THREE era chain (v1 = era
    // 0, v2 = era 1, current = era 2). The ZII container is era 0, and both
    // newer arms are written first, so selecting the v1 arm pins that era-tag
    // dispatch scales past two arms (no first-arm fallthrough, no
    // tag/boolean confusion). The bound v1 payload reads 0 -> 70.
    let canary = pass_canary("versioning/runtime_versioned_three_era_match_zii_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-versioned-three-era-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("three-era versioned match canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("three-era versioned match canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the era-0 (v1) arm among three eras to be selected (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_equatable_scalar_not_equals_guard_exit_canary_runs() {
    // Equatable `!=` negation + `==` of a scalar record DIRECTLY in guard
    // position (the other equatable canaries route compares through a `let`).
    // String-bearing variants are pending: the negated String term miscompiles
    // (pending/traits/equatable_string_not_equals_value) and guard-position
    // String equality rejects (pending/traits/
    // equatable_string_equality_guard_unlowered).
    let canary = pass_canary("traits/runtime_equatable_scalar_not_equals_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-equatable-scalar-neq-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("equatable scalar != guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("equatable scalar != guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected !=/== in guard position over a scalar Equatable record to drive all three rungs (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_case_membership_mixed_shape_exit_canary_runs() {
    // Decision 11 `in` membership over a MIXED shape (decision 7): common
    // fields live between the tag and the payload overlay, so the membership
    // test must stay tag-only -- and survive a common-field write.
    let canary = pass_canary("data/runtime_case_membership_mixed_shape_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-membership-mixed-shape-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("mixed-shape membership canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("mixed-shape membership canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected tag-only membership over a mixed shape across all three rungs (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_roundtrip_repeated_max_one_exit_canary_runs() {
    // Wire repeated field with the DEGENERATE maximum `[u32; 1]`: the
    // unrolled guarded element ops collapse to one, the count companion
    // still rules, and the packed framing round-trips (written = read = 6).
    let canary = pass_canary("wire/runtime_wire_roundtrip_repeated_max_one_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-wire-repeated-max-one-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("repeated max-one wire canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("repeated max-one wire canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the max=1 repeated field to round-trip (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_encode_repeated_then_string_exit_canary_runs() {
    // Wire repeated + String-last in one message: two runtime-sized appends
    // in sequence -- the String's cursor must start where the packed payload
    // actually ended. Encode-only (String decode has not landed); the exact
    // 10 bytes are asserted in-program.
    let canary = pass_canary("wire/runtime_wire_encode_repeated_then_string_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-wire-repeated-string-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("repeated-then-string wire canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("repeated-then-string wire canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the packed repeated payload then the String field to encode the asserted 10 bytes (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_roundtrip_nested_and_repeated_exit_canary_runs() {
    // Wire nested message + repeated field in ONE message: both stage
    // runtime-sized payloads, so the composition pins cursor handoff between
    // them on both the encode and decode sides (written = read = 13).
    let canary = pass_canary("wire/runtime_wire_roundtrip_nested_and_repeated_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-wire-nested-repeated-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("nested-and-repeated wire canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("nested-and-repeated wire canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the nested sub-message and the packed repeated field to round-trip together (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_array_length_transitive_exit_canary_runs() {
    // Comptime stage 1, transitive purity: the const-position callee CALLS
    // another effect-free machine (base() * 3 + 1 = 16), pinning that const
    // evaluation runs the call machinery, not just expression folding.
    let canary = pass_canary("comptime/runtime_const_array_length_transitive_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-const-length-transitive-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("transitive const length canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("transitive const length canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the transitively-evaluated 16-slot array to hold both end values (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_method_view_write_after_last_use_exit_canary_runs() {
    // Lifetimes stage 1, NLL complement of
    // fail/borrow/method_view_receiver_unrelated_field_write: the
    // receiver-wide loan of a method-returned view ends at the view's LAST
    // USE, so a later write to another field of the same receiver compiles
    // and both writes land (7 + 63 = 70).
    let canary = pass_canary("borrow/runtime_method_view_write_after_last_use_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-method-view-after-last-use-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("method-view write-after-last-use canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("method-view write-after-last-use canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the view write then the post-loan field write to both land (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_view_of_view_chain_exit_canary_runs() {
    // Lifetimes stage 1: CHAINED view-of-view through two free machines
    // (pick -> &mut Cell, narrow -> &mut i32). The elision linkage composes
    // and the two-hop write lands in the root machine-owned storage.
    let canary = pass_canary("borrow/runtime_view_of_view_chain_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-view-of-view-chain-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("view-of-view chain canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("view-of-view chain canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the write through the chained leaf view to reach the root array element (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Chapter 21 stage 3b (frozen decision 14): migration-chain completeness
// along the declared version chain is a REPORT VERDICT in the wire protocol
// artifact, never a compile error -- an arm may handle an old era manually.
// `Counter` declares v1 + v2 with only `Counter::from_v1` written, so the
// report must show the v1 migration as present and a MISSING verdict for v2.
#[test]
fn version_chain_report_renders_missing_migration_verdict() {
    let canary = pass_canary("versioning/version_chain_report");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-version-chain-report-canary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("incomplete migration chain must stay a report verdict, not a compile error");

    let report = fs::read_to_string(build_dir.join("04_wire_protocols.txt"))
        .expect("wire protocol report should be written");
    assert!(
        report.contains("## data Counter") && report.contains("versions: v1, v2, current"),
        "report should list the declared data version chain\n{}",
        report
    );
    assert!(
        report.contains("v1 -> current via `Counter::from_v1`"),
        "report should record the present migration machine\n{}",
        report
    );
    assert!(
        report.contains(
            "era v2 declared but no `Counter::from_v2` exists; `Versioned<Counter>` consumers must handle `Counter::v2` arms manually"
        ),
        "report should record the missing migration as a chain-completeness verdict\n{}",
        report
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The COMPLETE side of the chain-completeness verdict: both `Counter::from_v1`
// and `Counter::from_v2` exist, so the report must list both hops and emit NO
// missing-migration verdict ("missing:\n  none") -- no off-by-one flagging the
// newest era despite its migration being written.
#[test]
fn version_chain_report_renders_complete_chain() {
    let canary = pass_canary("versioning/version_chain_report_complete");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-version-chain-report-complete-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("complete migration chain canary should compile");

    let report = fs::read_to_string(build_dir.join("04_wire_protocols.txt"))
        .expect("wire protocol report should be written");
    assert!(
        report.contains("## data Counter") && report.contains("versions: v1, v2, current"),
        "report should list the declared data version chain\n{}",
        report
    );
    assert!(
        report.contains("v1 -> current via `Counter::from_v1`")
            && report.contains("v2 -> current via `Counter::from_v2`"),
        "report should record both present migration machines\n{}",
        report
    );
    assert!(
        report.contains("missing:\n  none"),
        "a complete chain should report no missing migrations\n{}",
        report
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_shrinking_slice_recursion_exit_canary_runs() {
    // Self-recursive dispatch with threaded scalar arguments over a shrinking
    // slice: `self.accumulate(items[1..], items[0].value)` retargets the SAME
    // frame slots it reads (a self-recursive machine shares one call context),
    // so the transition must stage the subslice descriptor AND read the head
    // element THROUGH the old descriptor before committing either. A past bug
    // resolved `items[0].value` as a plain place over the descriptor slot,
    // handing `take` the data pointer's low bytes instead of the element.
    // 10+20+15+25 threaded one step behind sums to 70 in machine state.
    let canary = pass_canary("termination/runtime_shrinking_slice_recursion_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-shrinking-slice-recursion-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("shrinking slice recursion canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("shrinking slice recursion canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the threaded scalar accumulation over the shrinking slice to total 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_encode_primitive_exit_canary_runs() {
    // Wire stage 2a: `CounterMessage::encode_wire(&msg, &mut self.buffer,
    // &mut self.written)` frames the schema's CURRENT era in compact_binary
    // v0 -- era varint, then per field in field-number order a tag varint and
    // a value varint (LEB128; signed values zigzag; bool 0/1). The canary
    // checks the eight expected bytes (hand-computed in its header comment)
    // and the written count in-language; exits 70 when byte-exact.
    let canary = pass_canary("wire/runtime_wire_encode_primitive_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-wire-encode-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("wire encode canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("wire encode canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the compact_binary v0 encoder to produce the hand-computed bytes (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_encode_era_discriminator_exit_canary_runs() {
    // Wire stage 2a + frozen decision 10: a schema with one declared version
    // block snapshots it as era 0, so the CURRENT body encodes era 1 -- the
    // first byte of every encoded message. The canary asserts the era byte,
    // the recycled field's tag/value bytes, and the written count; exits 70
    // when byte-exact.
    let canary = pass_canary("wire/runtime_wire_encode_era_discriminator_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-wire-era-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("wire era discriminator canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("wire era discriminator canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the current body to encode era 1 after one version block (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_roundtrip_primitive_exit_canary_runs() {
    // Wire stage 2b: encode { counter: 300, delta: -2, flag: true } into
    // [0x00, 0x00, 0xAC, 0x02, 0x01, 0x03, 0x02, 0x01] (hand-computed in the
    // canary header), then `decode_wire(&mut decoded, &buffer, &mut read,
    // &mut ok)` reads the same 8 bytes back: ok = true, read = 8, and every
    // decoded field equals the original (zigzag round-trips -2). Exits 70 on
    // a full match.
    let canary = pass_canary("wire/runtime_wire_roundtrip_primitive_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-wire-roundtrip-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("wire roundtrip canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("wire roundtrip canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the compact_binary v0 decoder to round-trip the encoded message (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_roundtrip_nested_exit_canary_runs() {
    // Wire nested message fields: encode { header: { room_id: 300, kind: -2 },
    // depth: -64 } into [0x00, 0x00, 0x05, 0x00, 0xAC, 0x02, 0x01, 0x03,
    // 0x01, 0x7F] (hand-computed in the canary header -- the nested field is
    // tag + LENGTH varint + the child's fields with NO era discriminator),
    // then decode back into a fresh value: ok = true, read = 10, and every
    // field including the nested ones equals the original. Exits 70 on a
    // full match.
    let canary = pass_canary("wire/runtime_wire_roundtrip_nested_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-wire-nested-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("wire nested roundtrip canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("wire nested roundtrip canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the compact_binary v0 round trip to preserve the nested message (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_decode_rejects_bad_nested_length_exit_canary_runs() {
    // Wire nested message fields, failure path: a hand-built buffer whose
    // nested LENGTH byte says 6 where the child's fields occupy 5 must fail
    // the decode -- the nested CLOSE check clears the sticky ok because the
    // cursor lands one byte before the declared end bound (walk in the
    // canary header). Exits 70 on the failure path.
    let canary = pass_canary("wire/runtime_wire_decode_rejects_bad_nested_length_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-wire-nested-length-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("wire bad-nested-length canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("wire bad-nested-length canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the decoder to reject a nested length that disagrees with the content (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_roundtrip_repeated_exit_canary_runs() {
    // Wire repeated fields: encode { sensor_id: 7, samples: [150, -2, 0, 0],
    // samples_count: 2, flag: true } into [0x00, 0x00, 0x07, 0x01, 0x03,
    // 0xAC, 0x02, 0x03, 0x02, 0x01] (hand-computed in the canary header --
    // the repeated field packs LENGTH-delimited: tag + byte-length varint +
    // the live element varints, no per-element tags), then decode back into
    // a fresh value: ok = true, read = 10, both live elements and the count
    // companion round-trip. Exits 70 on a full match.
    let canary = pass_canary("wire/runtime_wire_roundtrip_repeated_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-wire-repeated-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("wire repeated roundtrip canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("wire repeated roundtrip canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the compact_binary v0 repeated field to round-trip (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_decode_rejects_repeated_overflow_exit_canary_runs() {
    // Wire repeated fields, failure paths: a packed payload carrying MORE
    // elements than the declared maximum must fail the decode (the unrolled
    // guarded reads stop at the maximum, so the cursor lands short of the
    // declared end bound and the CLOSE check clears ok -- the count
    // companion reports the capped element count), and a hostile byte-length
    // claiming more than the buffer holds must fail at the OPEN check
    // without reading out of bounds. Exits 70 when both decodes report
    // failure (walk in the canary header).
    let canary = pass_canary("wire/runtime_wire_decode_rejects_repeated_overflow_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-wire-repeated-overflow-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("wire repeated overflow canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("wire repeated overflow canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the decoder to reject repeated payloads past the declared maximum or the buffer (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_decode_rejects_wrong_era_exit_canary_runs() {
    // Wire stage 2b: a hand-built buffer carrying era byte 5 (the schema's
    // current era is 0) must fail to decode -- the era discriminator is the
    // first expected byte and the failure flag is sticky. The canary exits 70
    // on the failure path (`ok` = false).
    let canary = pass_canary("wire/runtime_wire_decode_rejects_wrong_era_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-wire-wrong-era-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("wire wrong-era canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("wire wrong-era canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected decode_wire to reject a non-current era discriminator (exit 70 on the failure path), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_encode_string_exit_canary_runs() {
    // Wire stage 2a, String fields: a String field rides as tag varint +
    // LENGTH varint (byte count) + raw UTF-8 bytes (no NUL, no padding), and
    // must encode LAST. The canary checks the seven expected bytes for
    // { count: 7, label: "hi" } (hand-computed in its header comment) and the
    // written count in-language; exits 70 when byte-exact.
    let canary = pass_canary("wire/runtime_wire_encode_string_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-wire-string-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("wire encode string canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("wire encode string canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the compact_binary v0 encoder to frame the String field as len varint + raw bytes (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_call_result_binary_operand_exit_canary_runs() {
    // A state-call result used as an operand of a larger value (`x = f() + 1`,
    // `x = max(y, f()+1)`) must apply the operator, not collapse to just the call's
    // result. The dispatch-body mutation path had a statement-level "copy call result
    // to target" shortcut that fired even when the call was a sub-expression, dropping
    // the `+1`/`max`. It now fires only for a bare, non-builtin call value.
    let canary = pass_canary("expressions/runtime_call_result_binary_operand_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-call-result-binary-operand-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("call-result-binary-operand canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("call-result-binary-operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a call result used as a binary/max operand to apply the operator (exit 70), \
         got {:?} (71 = the surrounding operator was dropped and only the call result written)\
         \nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_cast_operand_exit_canary_runs() {
    // A numeric `as` cast used as a binary operand (`self.a + (self.b as f64)`) must
    // convert the source in place via a Convert value operand, not be dropped. Covers
    // int->float, float->int, and integer widening; exits 70 only when all convert.
    let canary = pass_canary("expressions/runtime_cast_operand_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-cast-operand-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("cast-operand canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("cast-operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `a + (b as T)` casts in operand position to convert correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_f32_arithmetic_exit_canary_runs() {
    // Single-precision f32 store/copy/compare and field arithmetic must use the
    // single-precision SSE forms (movd + ucomiss/addss/...) keyed on byte_size 4,
    // with is_float recognizing F32 -- previously f32 was compared/operated as an
    // integer or as double precision. Exits 70 only when every f32 op is correct.
    let canary = pass_canary("expressions/runtime_f32_arithmetic_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-f32-arith-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("f32 arithmetic canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("f32 arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32 store/copy/compare + add/sub/mul/div to evaluate correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_f32_local_arithmetic_exit_canary_runs() {
    // Single-precision f32 arithmetic/comparison into LOCAL variables (frame slots),
    // the companion to the field-based runtime_f32_arithmetic_exit. A local f32 binary
    // write reaches the pre-resolved-place selection path, which previously did no f32
    // narrowing -- so `let c: f32 = a + b` ran addss over an f64 bit pattern (garbage).
    // Also covers a cast of a folded f32 arithmetic expression into a local int
    // (`let n: i32 = c as i32`). Exits 70 only when add/sub/mul/div, an f32 `<`
    // compare, and the f32->i32 cast all evaluate correctly.
    let canary = pass_canary("expressions/runtime_f32_local_arithmetic_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-f32-local-arith-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("f32 local arithmetic canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("f32 local arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32 local add/sub/mul/div + compare + cast to evaluate correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_multi_arm_value_transition_exit_canary_runs() {
    // A value-returning machine whose body is a 3-arm guarded transition must select
    // the MIDDLE arm, not fall through to the default. The guard-failure jump used to
    // land on the matched arm's own body copy (before its forward skip), so a failed
    // first-arm guard skipped the middle arm. Exits 70 only when all three arms
    // (first/middle/default) select correctly.
    let canary = pass_canary("calls/runtime_multi_arm_value_transition_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-multi-arm-value-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("multi-arm value transition canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("multi-arm value transition canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a 3-arm value transition to select first/middle/default correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_spawn_join_moved_arg_exit_canary_runs() {
    // CONCURRENCY STAGE 1: `spawn { Worker::run(move x) }` lowers to a BLOCKING
    // call (the parser's synchronous-spawn desugar -- no scheduler/atomics exist,
    // so nothing can observe interleaving). `Join<i32>` erases to `i32` and
    // `handle.join()` is the identity on the completed handle. Two independent
    // spawn+join pairs must each deliver their own moved-arg computation
    // (exit 71 = first joined result wrong, 72 = second). Exits 70 when both
    // joined results are correct.
    let canary = pass_canary("concurrency/runtime_spawn_join_moved_arg_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-spawn-join-moved-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("spawn join moved-arg canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("spawn join moved-arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both spawn+join pairs to deliver their moved-arg results (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_spawn_interleaved_join_exit_canary_runs() {
    // CONCURRENCY STAGE 1: spawn EARLY, interleave field/local work, join LATER.
    // Under the synchronous-spawn desugar the spawned call completes at the spawn
    // site (one legal schedule of the concurrent program); the later join must
    // still deliver the spawned result and the interleaved statements must be
    // unaffected (exit 71 = joined result wrong, 72 = interleaved field wrong,
    // 73 = interleaved local wrong). Exits 70 when all three are correct.
    let canary = pass_canary("concurrency/runtime_spawn_interleaved_join_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-spawn-interleaved-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("spawn interleaved join canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("spawn interleaved join canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the late join to deliver the spawned result with interleaved work intact (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_transition_unsigned_guard_exit_canary_runs() {
    // A value-transition arm guard on an UNSIGNED (u32) operand must branch with
    // unsigned comparison conditions. The leaf value-transition guard path picked
    // the SIGNED jcc regardless of operand signedness (only the dispatch-edge path
    // post-processed the operator with the operand's unsignedness). For a u32 with
    // its top bit set (4000000000 > INT_MAX), `x <= 2` is FALSE unsigned (correct)
    // but TRUE signed (wrong). A signed mis-compare selects the first arm and exits
    // 71; a correct unsigned compare selects the default arm and exits 70.
    let canary = pass_canary("calls/runtime_value_transition_unsigned_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-transition-unsigned-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("unsigned value-transition guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("unsigned value-transition guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a u32 value-transition guard to branch unsigned (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_array_length_exit_canary_runs() {
    // COMPTIME STAGE 1: `slots: [i64; table_size()]` sizes a data field by an
    // effect-free machine call, const-evaluated by the reference interpreter
    // before checking/layout (the callee computes 12 + 4, pinning evaluation,
    // not literal forwarding). Indexing slots[15] only type-checks if the
    // substituted Literal(16) reached the range checker, and the values only
    // read back if layout sized the field as 16 elements -- identically to a
    // written `[i64; 16]`. Exits 70 only when both ends hold their values.
    let canary = pass_canary("comptime/runtime_const_array_length_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-const-array-length-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("const array length canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("const array length canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `[i64; table_size()]` to const-evaluate to 16 and behave exactly like a literal length (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_fixed_vec_round_trip_exit_canary_runs() {
    // ALLOCATOR STORY STAGE 1: the fixed-capacity vec pattern (core
    // FixedVec<T, const N: usize>, hand-instantiated at i32/N=4 pending
    // generic machine instantiation) round-trips at runtime with every
    // bounds obligation PROOF-discharged through contract chaining: clear
    // establishes room, push consumes it and guarantees non-emptiness plus
    // the popped slot's bound, pop/get consume push's guarantees. The guard
    // ladder checks the actual data flow (pushed value lands, pop returns it
    // and shrinks, a second clear/push cycle overwrites slot 0, final length
    // is 1) and exits 70 only when all hold.
    let canary = pass_canary("collections/runtime_fixed_vec_round_trip_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-fixed-vec-round-trip-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("fixed vec round trip canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("fixed vec round trip canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the proof-discharged push/pop/get round trip to hold its values (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_fixed_array_field_guard_exit_canary_runs() {
    // Reading `self.cells[i].value` (fixed-array element field, constant index) in a
    // GUARD must apply the index: the guard-operand layout consumed the root field
    // without folding its out-of-band constant index, so `cells[1].value` read
    // element 0. The canary writes two distinct elements and guards each; a dropped
    // index exits 71 instead of 70.
    let canary = pass_canary("expressions/runtime_fixed_array_field_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-fixed-array-field-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("fixed-array field guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("fixed-array field guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.cells[i].value` guards to apply the constant index (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_fixed_array_field_value_exit_canary_runs() {
    // Reading `self.cells[2].value` (fixed-array element field, NON-ZERO constant
    // index) as a VALUE must apply the index. The GUARD path was fixed in 8e775fbd,
    // but the non-guard place resolvers used for value reads dropped the constant
    // index, so every `arr[const].field` value read aliased element 0. The canary
    // writes three distinct elements, reads the middle-high one into a field, and
    // guards it; a dropped index exits 71 instead of 70.
    let canary = pass_canary("expressions/runtime_fixed_array_field_value_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-fixed-array-field-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("fixed-array field value canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("fixed-array field value canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `let d = self.cells[i].value` to apply the constant index (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn fixed_array_element_guard_canary_runs() {
    // A guard comparing a fixed-array element to a constant (`self.cells[2] == 7.0`,
    // cells `[f64; 4]`) must resolve one 8-byte element, not the whole 32-byte array
    // (which the encoder rejected). Promoted from pending once the guard-operand
    // layout applied the constant index; exits 0 when the guard reads cells[2].
    let canary = pass_canary("control_flow/fixed_array_element_guard");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-fixed-array-elem-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("fixed-array element guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("fixed-array element guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected `self.cells[2] == 7.0` to resolve one element and match (exit 0), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_float_local_arithmetic_exit_canary_runs() {
    // Float arithmetic whose result is a `let`-bound LOCAL must lower to an SSE
    // op (addsd/...), not an integer add over the IEEE bits. The local-target
    // binary write used to emit an integer op; the canary guards the exact result
    // (6.5) and exits 70 only when correct (71 otherwise).
    let canary = pass_canary("expressions/runtime_float_local_arithmetic_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-float-local-arith-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("float local arithmetic canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float local arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected float arithmetic into locals to use SSE ops and yield 6.5 (exit 70), got {:?} (71 = integer op over float bits)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_literal_source_cast_exit_canary_runs() {
    // A numeric `as` cast whose source folds to a literal (`10.0 as i32`) must
    // still emit a convert. The selector used to bail (no place type for a
    // literal source) and emit nothing, leaving the destination 0. Guards both
    // float->int and int->float results, exits 70 only when both are correct.
    let canary = pass_canary("expressions/runtime_literal_source_cast_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-literal-source-cast-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("literal source cast canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("literal source cast canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected literal-source casts (10.0 as i32, 7 as f64) to emit converts and exit 70, got {:?} (71 = wrong/missing convert)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_float_constant_store_exit_canary_runs() {
    let canary = pass_canary("expressions/runtime_float_constant_store_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-float-store-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("float constant store canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float constant store canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected float constant stores (f64 + f32 + 0.0) to execute and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_match_value_exit_canary_runs() {
    let canary = pass_canary("expressions/runtime_match_value_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-match-value-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("match value canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("match value canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected value-position match (enum + integer + wildcard) to select the right arm (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_conformance_item_exit_canary_runs() {
    let canary = pass_canary("traits/runtime_conformance_item_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-conformance-item-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("conformance item canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("conformance item canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `Circle satisfies Shape;` to validate against the written member and run unchanged (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn equatable_record_equality_exit_canary_runs() {
    // Equatable synthesis (decisions 8 + 11): `Point satisfies Equatable;`
    // makes `==`/`!=` on the record structural -- equal values match, one
    // differing middle field misses.
    let canary = pass_canary("traits/equatable_record_equality_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-equatable-record-equality-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("equatable record equality canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("equatable record equality canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `==`/`!=` on `Point` to compare field by field (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn equatable_sum_payload_equality_exit_canary_runs() {
    // Equatable synthesis on a payload-bearing sum: tag equality AND the
    // matching case's payload fields. Same-case-equal matches; same-case-
    // different-payload and different-case miss; the constructed-literal
    // compare pins the single-arm form.
    let canary = pass_canary("traits/equatable_sum_payload_equality_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-equatable-sum-payload-equality-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("equatable sum payload equality canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("equatable sum payload equality canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `==` on `Command` to compare tag AND payload (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn equatable_mixed_shape_equality_exit_canary_runs() {
    // Equatable synthesis on a MIXED shape: common fields AND tag AND the
    // matching case's payload. The second compare differs ONLY in a common
    // field (the reconstruction zero-initialized it), so equality that skips
    // common fields exits 71. Also regression net for the boolean-folding
    // factor/distribute mutual recursion this expansion first exposed.
    let canary = pass_canary("traits/equatable_mixed_shape_equality_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-equatable-mixed-shape-equality-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("equatable mixed shape equality canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("equatable mixed shape equality canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `==` on `RoomEvent` to compare common fields AND tag AND payload (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn equatable_string_field_equality_exit_canary_runs() {
    // Equatable synthesis over a String field: `==` compares text CONTENT
    // (length AND bytes) through the value-position text-equals operand --
    // equal contents match; same-length-different-bytes, different-length,
    // and equal-text-different-scalar-sibling all miss.
    let canary = pass_canary("traits/equatable_string_field_equality_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-equatable-string-field-equality-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("equatable string field equality canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("equatable string field equality canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `==` on `Tag` to compare String content (length AND bytes) plus the scalar sibling (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_data_properties_exit_canary_runs() {
    let canary = pass_canary("data/runtime_data_properties_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-data-properties-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("data properties canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("data properties canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `[copy, zero_init]` declarations to verify and run identically to property-free data (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn compound_assignment_exit_canary_runs() {
    let canary = pass_canary("operators/compound_assignment_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-compound-assignment-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("compound assignment canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("compound assignment canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `+= -= *= /= %=` to chain correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_chained_field_mutation_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_chained_field_mutation_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-chained-field-mutation-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("chained field mutation canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("chained field mutation canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected chained read-modify-write to observe prior writes (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_comparison_guard_signedness_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_comparison_guard_signedness_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-comparison-guard-signedness-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("comparison guard signedness canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("comparison guard signedness canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected guard comparisons to respect operand signedness (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_comparison_value_signedness_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_comparison_value_signedness_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-comparison-value-signedness-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("comparison value signedness canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("comparison value signedness canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected value-position comparisons to respect operand signedness (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_min_max_signedness_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_min_max_signedness_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-min-max-signedness-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("min/max signedness canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("min/max signedness canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected min/max to respect operand signedness (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_unsigned_division_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_unsigned_division_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-unsigned-division-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("unsigned division canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("unsigned division canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected unsigned division/remainder/logical-shift on high-bit u32 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_unsigned_modulo_call_argument_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_unsigned_modulo_call_argument_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-unsigned-modulo-call-argument-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("unsigned modulo call-argument canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("unsigned modulo call-argument canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the inline `raw % 100` call argument to use UNSIGNED modulo \
         so the dispatch ladder selects the satisfied arm (exit 70 = 3 RNG \
         draws, interpreter semantics; exit 71 = the signed-remainder misfire \
         routed the second event into the enemy arm and drew once extra -- the \
         dungeon seed-7 14-vs-15 residual), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_unsigned_modulo_cast_operand_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_unsigned_modulo_cast_operand_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-unsigned-modulo-cast-operand-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("unsigned modulo cast-operand canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("unsigned modulo cast-operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `((random.seed >> 32) as u32) % 199` to use UNSIGNED modulo \
         (the cast's TARGET type decides operand signedness; exit 70 = roll 158, \
         interpreter semantics; exit 71 = the signed-remainder misfire stored \
         -87 in the u32 slot), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_signed_division_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_signed_division_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-signed-division-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("signed division canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("signed division canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed division/remainder of a negative dividend (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_copy_then_read_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_copy_then_read_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-copy-then-read-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("copy-then-read canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("copy-then-read canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a read after a copy to observe the copied value (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_i64_full_width_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_i64_full_width_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-i64-full-width-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("i64 full-width canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("i64 full-width canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i64 store/add/compare to keep full 64-bit precision (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_chained_string_append_exit_canary_runs() {
    let canary = pass_canary("text/runtime_chained_string_append_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-chained-string-append-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("chained string append canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("chained string append canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected chained in-place appends to be visible to a later guard (exit 70), got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_string_concat_two_fields_exit_canary_runs() {
    let canary = pass_canary("text/runtime_string_concat_two_fields_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-string-concat-two-fields-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("two-field string concat canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("two-field string concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected concat of two runtime String fields (no literal anchor) to produce the joined text (exit 70), got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_string_append_in_place_exit_canary_runs() {
    let canary = pass_canary("text/runtime_machine_string_append_in_place_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-string-append-in-place-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("string append-in-place canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("string append-in-place canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected in-place machine String append to preserve the prefix (exit 70), got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_string_field_copy_through_mut_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_local_string_field_copy_through_mut_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-local-string-field-copy-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("local string field copy canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("local string field copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a local struct String field copied through a &mut String param to reach the caller (exit 70), got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_direct_boolean_conjunction_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_direct_boolean_conjunction_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-direct-bool-conjunction-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime direct boolean conjunction canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime direct boolean conjunction canary should run");

    assert_eq!(
        output.status.code(),
        Some(21),
        "expected runtime direct boolean conjunction canary to route to ambush exit code 21, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn executable_domain_membership_expression_exit_canary_runs() {
    let canary = pass_canary("domains/executable_domain_membership_expression_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-domain-membership-expression-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("executable domain membership expression canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("executable domain membership expression canary should run");

    assert_eq!(
        output.status.code(),
        Some(81),
        "expected executable domain membership expression canary to route to exit code 81, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn executable_imported_domain_membership_exit_canary_runs() {
    let canary = pass_canary("domains/executable_imported_domain_membership_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-imported-domain-membership-expression-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("executable imported domain membership canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("executable imported domain membership canary should run");

    assert_eq!(
        output.status.code(),
        Some(91),
        "expected executable imported domain membership canary to route to exit code 91, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn executable_imported_domain_membership_guard_exit_canary_runs() {
    let canary = pass_canary("domains/executable_imported_domain_membership_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-imported-domain-membership-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("executable imported domain membership guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("executable imported domain membership guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(81),
        "expected executable imported domain membership guard canary to route to exit code 81, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn executable_imported_domain_membership_intersection_guard_exit_canary_runs() {
    let canary =
        pass_canary("domains/executable_imported_domain_membership_intersection_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-imported-domain-membership-intersection-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("executable imported domain membership intersection guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("executable imported domain membership intersection guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(219),
        "expected executable imported domain membership intersection guard canary to route to exit code 219, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn executable_imported_domain_membership_union_guard_exit_canary_runs() {
    let canary = pass_canary("domains/executable_imported_domain_membership_union_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-imported-domain-membership-union-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("executable imported domain membership union guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("executable imported domain membership union guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(217),
        "expected executable imported domain membership union guard canary to route to exit code 217, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn executable_domain_membership_intersection_guard_exit_canary_runs() {
    let canary = pass_canary("domains/executable_domain_membership_intersection_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-domain-membership-intersection-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("executable domain membership intersection canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("executable domain membership intersection canary should run");

    assert_eq!(
        output.status.code(),
        Some(231),
        "expected executable domain membership intersection canary to route to exit code 231, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn executable_domain_membership_union_guard_exit_canary_runs() {
    let canary = pass_canary("domains/executable_domain_membership_union_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-domain-membership-union-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("executable domain membership union canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("executable domain membership union canary should run");

    assert_eq!(
        output.status.code(),
        Some(241),
        "expected executable domain membership union canary to route to exit code 241, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn executable_domain_membership_union_value_exit_canary_runs() {
    let canary = pass_canary("domains/executable_domain_membership_union_value_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-domain-membership-union-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("executable domain membership union value canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("executable domain membership union value canary should run");

    assert_eq!(
        output.status.code(),
        Some(205),
        "expected executable domain membership union value canary to route to exit code 205, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn executable_domain_membership_intersection_value_exit_canary_runs() {
    let canary = pass_canary("domains/executable_domain_membership_intersection_value_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-domain-membership-intersection-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("executable domain membership intersection value canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("executable domain membership intersection value canary should run");

    assert_eq!(
        output.status.code(),
        Some(233),
        "expected executable domain membership intersection value canary to route to exit code 233, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn executable_imported_domain_membership_union_value_exit_canary_runs() {
    let canary = pass_canary("domains/executable_imported_domain_membership_union_value_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-imported-domain-membership-union-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("executable imported domain membership union value canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("executable imported domain membership union value canary should run");

    assert_eq!(
        output.status.code(),
        Some(215),
        "expected executable imported domain membership union value canary to route to exit code 215, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn executable_imported_domain_membership_intersection_value_exit_canary_runs() {
    let canary =
        pass_canary("domains/executable_imported_domain_membership_intersection_value_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-imported-domain-membership-intersection-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("executable imported domain membership intersection value canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("executable imported domain membership intersection value canary should run");

    assert_eq!(
        output.status.code(),
        Some(217),
        "expected executable imported domain membership intersection value canary to route to exit code 217, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_boolean_or_value_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_local_boolean_or_value_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-boolean-or-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime local boolean or value canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime local boolean or value canary should run");

    assert_eq!(
        output.status.code(),
        Some(251),
        "expected runtime local boolean or value canary to route to exit code 251, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Straight-line `main` with NO transitions whose terminal expression is a
// LOCAL read. Pre-fix, only a bare literal terminal delivered as the exit
// code; a local terminal silently fell through to the default exit path
// (exit 1). Guards the terminal-value constant fold through local
// initializers.
#[test]
fn runtime_straight_line_terminal_local_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_straight_line_terminal_local_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-straight-line-terminal-local-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("straight-line terminal local canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("straight-line terminal local canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the terminal local read to deliver as the exit code 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The runtime half of the straight-line terminal shape: a field WRITE followed
// by a terminal field READ-BACK. Unlike the local variant this cannot constant
// fold — it exercises the CopyRuntimeStorageToReturnRegister load.
#[test]
fn runtime_straight_line_terminal_field_readback_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_straight_line_terminal_field_readback_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-straight-line-terminal-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("straight-line terminal field read-back canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("straight-line terminal field read-back canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the terminal field read-back to deliver as the exit code 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_negated_boolean_place_guard_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_negated_boolean_place_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-negated-bool-place-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime negated boolean place guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime negated boolean place guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(73),
        "expected runtime negated boolean place guard canary to route to exit code 73, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_boolean_conjunction_value_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_local_boolean_conjunction_value_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-bool-conjunction-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime local boolean conjunction value canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime local boolean conjunction value canary should run");

    assert_eq!(
        output.status.code(),
        Some(74),
        "expected runtime local boolean conjunction value canary to route to exit code 74, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_scalar_comparison_value_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_local_scalar_comparison_value_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-scalar-comparison-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime local scalar comparison value canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime local scalar comparison value canary should run");

    assert_eq!(
        output.status.code(),
        Some(76),
        "expected runtime local scalar comparison value canary to route to exit code 76, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_string_comparison_value_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_local_string_comparison_value_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-string-comparison-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime local string comparison value canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime local string comparison value canary should run");

    assert_eq!(
        output.status.code(),
        Some(78),
        "expected runtime local string comparison value canary to route to exit code 78, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_boolean_or_guard_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_boolean_or_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-bool-or-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime boolean or guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime boolean or guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(71),
        "expected runtime boolean or guard canary to route to exit code 71, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_direct_boolean_transition_argument_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_direct_boolean_transition_argument_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-direct-bool-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime direct boolean transition argument canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime direct boolean transition argument canary should run");

    assert_eq!(
        output.status.code(),
        Some(211),
        "expected runtime direct boolean transition argument canary to route to exit code 211, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_boolean_transition_argument_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_local_boolean_transition_argument_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-bool-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime local boolean transition argument canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime local boolean transition argument canary should run");

    assert_eq!(
        output.status.code(),
        Some(201),
        "expected runtime local boolean transition argument canary to route to exit code 201, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_boolean_transition_argument_after_string_guard_exit_canary_runs() {
    let canary =
        pass_canary("control_flow/runtime_boolean_transition_argument_after_string_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-bool-transition-after-string-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime boolean transition argument after string guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime boolean transition argument after string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(247),
        "expected runtime boolean transition argument after string guard canary to route to exit code 247, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_indexed_nested_room_copy_exit_canary_runs() {
    let canary = pass_canary("storage/runtime_machine_owned_indexed_nested_room_copy_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-indexed-nested-room-copy-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime machine-owned indexed nested room copy canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime machine-owned indexed nested room copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(87),
        "expected runtime machine-owned indexed nested room copy canary to route to exit code 87, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_negated_comparison_guard_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_negated_comparison_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-negated-comparison-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime negated comparison guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime negated comparison guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(75),
        "expected runtime negated comparison guard canary to route to exit code 75, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_case_member_dispatch_exit_canary_runs() {
    // Payload-less `case` members (the spelling that replaces `enum`) must
    // dispatch in a transition exactly like the retired keyword did.
    let canary = pass_canary("control_flow/runtime_case_member_dispatch_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-case-member-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime case member dispatch canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime case member dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected case-member transition dispatch to select Direction::South (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn case_payload_native_construction_canary_runs() {
    // Case payload construction (`Command::Move { steps: 70 }`) lowers natively:
    // the i32 case tag writes at offset 0, the payload field at its packed
    // offset, the transition arm compares only the 4-byte tag, and the
    // destructured `steps` binding reads the payload member into the target
    // state's argument. Promoted from pending/ when payload codegen landed.
    let canary = pass_canary("data/case_payload_native_construction");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-case-payload-construction-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("case payload construction canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("case payload construction canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected case payload construction + tag dispatch + payload read (exit 70), got {:?} (71 = wrong arm)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_case_payload_guard_read_exit_canary_runs() {
    // A multi-field case payload read in a destructure `if` guard: the guard
    // must read the SECOND payload field (`bonus`, packed after `power`) from
    // the enum value, not match on tag alone -- a decoy same-case arm with a
    // wrong bonus sits first and catches a dropped `if` clause.
    let canary = pass_canary("data/runtime_case_payload_guard_read_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-case-payload-guard-read-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("case payload guard read canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("case payload guard read canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the `if bonus == 10` payload guard to select the second Strike arm (exit 70), got {:?} (71 = decoy/default arm)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn case_membership_value_exit_canary_runs() {
    // Decision 11: `cmd in Command::Move` in VALUE position lowers to a
    // tag-only compare. The constructed payload (`dx: 3`) exits 71 if the
    // compare reads payload bytes instead of clamping to the tag.
    let canary = pass_canary("data/case_membership_value_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-case-membership-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("case membership value canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("case membership value canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.cmd in Command::Move` to be a true tag test (exit 70), got {:?} (71 = membership missed, e.g. payload bytes leaked into the compare)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn match_exhaustive_by_cases_canary_runs() {
    // Exhaustiveness over implicit case-domains: one arm per case counts as
    // a complete tag set (no `_`), and the counted dispatch still selects
    // the right arm at runtime.
    let canary = pass_canary("data/match_exhaustive_by_cases");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-match-exhaustive-by-cases-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("exhaustive-by-cases canary should compile without a `_` arm");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("exhaustive-by-cases canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the held case's arm (exit 70), got {:?} (71 = dispatch missed)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn match_exhaustive_by_case_union_domain_canary_runs() {
    // A PURE case-union domain arm (`when self in Command::Move |
    // Command::Say`, nothing else) contributes its tag set to exhaustiveness
    // -- no `_` needed -- and classifies at runtime: the held value is the
    // SECOND union member, so a lowering that drops union arms exits 71.
    let canary = pass_canary("data/match_exhaustive_by_case_union_domain");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-match-exhaustive-union-domain-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("case-union-domain canary should compile without a `_` arm");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("case-union-domain canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the union-domain arm to classify `Command::Say` (exit 70), got {:?} (71 = union membership missed)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn case_membership_union_guard_exit_canary_runs() {
    // Decision 11: a union of implicit case domains as a transition guard
    // subject; the held value matches the SECOND (payload-bearing) arm.
    let canary = pass_canary("data/case_membership_union_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-case-membership-union-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("case membership union guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("case membership union guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.cmd in Command::Quit | Command::Move` to take the matched arm (exit 70), got {:?} (71 = union membership missed)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_case_reassignment_exit_canary_runs() {
    // Overwriting one payload-carrying case with another (`Walk { pace: 9 }`
    // then `Run { speed: 70 }`) must rewrite both the tag and the overlaying
    // payload bytes: a stale tag selects the first (Walk) arm and exits 9, a
    // stale payload exits with the wrong code.
    let canary = pass_canary("data/runtime_case_reassignment_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-case-reassignment-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("case reassignment canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("case reassignment canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the second case construction to fully replace the first (exit 70), got {:?} (9 = stale tag took the Walk arm, 72 = no arm matched)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mixed_shape_exit_canary_runs() {
    // MIXED data shape (frozen decision 7): common fields + cases in one
    // declaration. Construction names a common field alongside the payload,
    // a case change zero-initializes the unnamed common field, a common
    // field is read AND written without case knowledge, and tag dispatch
    // binds the payload. Layout: tag at 0, common fields after the tag,
    // payload overlay after the common fields.
    let canary = pass_canary("data/runtime_mixed_shape_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!("omega-mixed-shape-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("mixed shape canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("mixed shape canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected common-field reads/writes and payload binding to agree (exit 70), got {:?} (71 = a dispatch step observed the wrong value)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_array_literal_string_field_exit_canary_runs() {
    // ARRAY-literal struct-element initialization (`let rooms: [Room; 2] =
    // [Room { number: 11, label: "expected" }, ..]`) must write every element
    // field natively. The local-initializer mutation path had a StructLiteral
    // arm but no ArrayLiteral arm, so the whole initializer fell through to
    // the scalar path and selected NOTHING -- scalar element fields read 0 and
    // String descriptors read empty while the interpreter initialized them.
    // Guards read each element's scalar sibling and String field through a
    // runtime index (frame reads, no static folds) plus a cross check that
    // element 0 does not equal element 1's literal.
    let canary = pass_canary("data/runtime_array_literal_string_field_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-array-literal-string-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("array literal string field canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("array literal string field canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected array-literal element init to write scalar and String fields natively (exit 70), got {:?} (71 = a guard observed a zeroed/incorrect element field)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_struct_literal_string_field_exit_canary_runs() {
    // Struct-LITERAL String field initialization (`let msg: T = T { label:
    // "hi" }`) must emit the native descriptor write, same as the assignment
    // form. Data planning previously collected string literals only from
    // assignments / state values / branch targets -- never from `let` local
    // initializers -- so the descriptor-write selection found no data object
    // and silently skipped the write (descriptor stayed zeroed natively).
    // Observed through the wire encoder's bytes plus a case-literal String
    // payload (`Command::Say { text: "ok" }`) destructured and compared.
    let canary = pass_canary("data/runtime_struct_literal_string_field_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-struct-literal-string-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("struct literal string field canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("struct literal string field canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected literal-form String field init to write the descriptor natively (exit 70), got {:?} (71 = empty/incorrect descriptor observed)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_tuple_transition_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_tuple_transition_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-tuple-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime tuple transition canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime tuple transition canary should run");

    assert_eq!(
        output.status.code(),
        Some(22),
        "expected runtime tuple transition canary to route to tuple arm exit code 22, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_room_use_reentry_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_room_use_reentry_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-room-reentry-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime room use reentry canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime room use reentry canary should run");

    assert_eq!(
        output.status.code(),
        Some(41),
        "expected runtime room use reentry canary to route to spent-fountain exit code 41, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_enemy_clear_reentry_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_enemy_clear_reentry_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-enemy-reentry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime enemy clear reentry canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime enemy clear reentry canary should run");

    assert_eq!(
        output.status.code(),
        Some(51),
        "expected runtime enemy clear reentry canary to route to cleared-hall exit code 51, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_clear_carve_render_string_fields_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_clear_carve_render_string_fields_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-clear-carve-render-string-fields-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime clear/carve/render string fields canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime clear/carve/render string fields canary should run");

    assert_eq!(
        output.status.code(),
        Some(198),
        "expected cleared then carved room label to render through lookup and exit 198, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_full_level_wrapper_lookup_string_field_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_full_level_wrapper_lookup_string_field_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-full-level-wrapper-lookup-string-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime full-level wrapper lookup string field canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime full-level wrapper lookup string field canary should run");

    assert_eq!(
        output.status.code(),
        Some(202),
        "expected runtime full-level wrapper lookup string field canary to preserve the room label through wrapper lookup, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_multi_room_reentry_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_multi_room_reentry_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-multi-room-reentry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime multi-room reentry canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime multi-room reentry canary should run");

    assert_eq!(
        output.status.code(),
        Some(63),
        "expected runtime multi-room reentry canary to preserve all three room flags and exit 63, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_slice_element_write_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_mutable_slice_element_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-slice-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime mutable slice write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime mutable slice write canary should run");

    assert_eq!(
        output.status.code(),
        Some(21),
        "expected runtime mutable slice write canary to preserve alias mutation and exit 21, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The promoted straight-line sibling: same mutable-slice-view write, but the
// machine body has NO transitions and delivers a field READ-BACK as its
// terminal value. Guards the straight-line terminal-value path (the
// CopyRuntimeStorageToReturnRegister load) end to end through a slice write.
#[test]
fn runtime_mutable_slice_element_write_straight_line_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_mutable_slice_element_write_straight_line_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-slice-write-straight-line-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime mutable slice write straight-line canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime mutable slice write straight-line canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the slice-view write to land and the terminal field read-back to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dispatch_mutable_slice_element_write_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_dispatch_mutable_slice_element_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dispatch-mutable-slice-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime dispatch mutable slice write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime dispatch mutable slice write canary should run");

    assert_eq!(
        output.status.code(),
        Some(31),
        "expected runtime dispatch mutable slice write canary to preserve alias mutation and exit 31, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_of_slice_param_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_subslice_of_slice_param_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-param-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime subslice of slice param canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime subslice of slice param canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected subslicing a runtime slice param to shrink the length (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_index_read_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_slice_index_read_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-slice-read-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime slice index read canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime slice index read canary should run");

    assert_eq!(
        output.status.code(),
        Some(41),
        "expected runtime slice index read canary to preserve dynamic slice reads and exit 41, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_index_read_dispatch_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_slice_index_read_dispatch_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-read-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime dispatch slice index read canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime dispatch slice index read canary should run");

    assert_eq!(
        output.status.code(),
        Some(43),
        "expected runtime dispatch slice index read canary to preserve dynamic slice reads and exit 43, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_index_copy_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_slice_index_copy_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-slice-copy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime slice index copy canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime slice index copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(51),
        "expected runtime slice index copy canary to preserve dynamic element copies and exit 51, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_index_copy_dispatch_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_slice_index_copy_dispatch_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-copy-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime dispatch slice index copy canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime dispatch slice index copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(61),
        "expected runtime dispatch slice index copy canary to preserve dynamic element copies and exit 61, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_frame_array_slice_parameter_alias_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_frame_array_slice_parameter_alias_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-frame-array-slice-parameter-alias-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime frame array slice parameter alias canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime frame array slice parameter alias canary should run");

    assert_eq!(
        output.status.code(),
        Some(72),
        "expected a slice made from a by-value frame parameter's inline array to \
         preserve its backing storage across the transition into a slice-parameter \
         state, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_len_transition_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_slice_len_transition_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-len-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime slice len transition canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime slice len transition canary should run");

    assert_eq!(
        output.status.code(),
        Some(101),
        "expected runtime slice len transition canary to preserve slice descriptors across transitions and exit 101, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_param_bounded_range_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_subslice_param_bounded_range_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-param-bounded-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime subslice param bounded range canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime subslice param bounded range canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a bounded literal subslice of a runtime slice param to materialize length 3 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_param_end_only_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_subslice_param_end_only_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-param-end-only-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime subslice param end-only canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime subslice param end-only canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an end-only subslice of a runtime slice param to materialize length 2 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_param_local_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_subslice_param_local_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-param-local-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime subslice param local canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime subslice param local canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a subslice of a slice param assigned to a local to shrink the descriptor and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_runtime_start_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_subslice_runtime_start_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-runtime-start-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime subslice runtime start canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime subslice runtime start canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a runtime-start subslice (sub[start..]) to offset the descriptor pointer and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_runtime_end_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_subslice_runtime_end_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-runtime-end-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime subslice runtime end canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime subslice runtime end canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a runtime-end subslice (sub[..end]) to take the runtime length and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_nested_of_param_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_subslice_nested_of_param_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-nested-param-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime nested subslice of param canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime nested subslice of param canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a nested subslice (sub[1..][1..]) over a runtime slice param to compose biases and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_runtime_start_over_local_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_subslice_runtime_start_over_local_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-start-over-local-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime subslice runtime start over local canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime subslice runtime start over local canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a runtime-start subslice over a subslice local (tail[start..]) to compose and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_param_inclusive_end_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_subslice_param_inclusive_end_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-inclusive-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime subslice param inclusive end canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime subslice param inclusive end canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an inclusive-end subslice (sub[1..=3]) over a runtime slice param to fold to end + 1 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_range_len_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_subslice_range_len_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-subslice-len-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime subslice range len canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime subslice range len canary should run");

    assert_eq!(
        output.status.code(),
        Some(203),
        "expected runtime subslice range len canary to materialize the shortened descriptor length and exit 203, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_bounded_range_len_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_subslice_bounded_range_len_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-bounded-len-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime bounded subslice range len canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime bounded subslice range len canary should run");

    assert_eq!(
        output.status.code(),
        Some(215),
        "expected runtime bounded subslice range len canary to materialize the two-sided descriptor length and exit 215, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_range_pointer_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_subslice_range_pointer_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-pointer-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime subslice range pointer canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime subslice range pointer canary should run");

    assert_eq!(
        output.status.code(),
        Some(205),
        "expected runtime subslice range pointer canary to offset the descriptor pointer and exit 205, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_dynamic_index_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_subslice_dynamic_index_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-dynamic-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime subslice dynamic index canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime subslice dynamic index canary should run");

    assert_eq!(
        output.status.code(),
        Some(207),
        "expected runtime subslice dynamic index canary to read through the adjusted descriptor pointer and exit 207, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_bounded_dynamic_index_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_subslice_bounded_dynamic_index_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-bounded-dynamic-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime bounded subslice dynamic index canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime bounded subslice dynamic index canary should run");

    assert_eq!(
        output.status.code(),
        Some(209),
        "expected runtime bounded subslice dynamic index canary to read through the adjusted descriptor pointer and exit 209, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_end_dynamic_index_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_subslice_end_dynamic_index_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-subslice-end-dynamic-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime end subslice dynamic index canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime end subslice dynamic index canary should run");

    assert_eq!(
        output.status.code(),
        Some(211),
        "expected runtime end subslice dynamic index canary to read through the descriptor and exit 211, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_subslice_dynamic_index_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_nested_subslice_dynamic_index_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-nested-subslice-dynamic-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime nested subslice dynamic index canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime nested subslice dynamic index canary should run");

    assert_eq!(
        output.status.code(),
        Some(213),
        "expected runtime nested subslice dynamic index canary to compose descriptor windows and exit 213, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_subslice_fixed_index_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_nested_subslice_fixed_index_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-nested-subslice-fixed-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime nested subslice fixed index canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime nested subslice fixed index canary should run");

    assert_eq!(
        output.status.code(),
        Some(215),
        "expected runtime nested subslice fixed index canary to copy from the composed window and exit 215, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_fixed_index_guard_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_slice_fixed_index_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-fixed-index-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime slice fixed index guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime slice fixed index guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(121),
        "expected runtime slice fixed index guard canary to preserve transitioned fixed-index reads and exit 121, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_slice_len_comparison_value_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_local_slice_len_comparison_value_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-slice-len-comparison-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime local slice len comparison canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime local slice len comparison canary should run");

    assert_eq!(
        output.status.code(),
        Some(191),
        "expected runtime local slice len comparison canary to preserve slice len comparisons in local bool values and exit 191, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_index_transition_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_slice_index_transition_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-index-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime slice index transition canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime slice index transition canary should run");

    assert_eq!(
        output.status.code(),
        Some(111),
        "expected runtime slice index transition canary to preserve whole-element copies across transitioned slice parameters and exit 111, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_iteration_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_slice_iteration_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-iteration-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime slice iteration canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime slice iteration canary should run");

    assert_eq!(
        output.status.code(),
        Some(91),
        "expected runtime slice iteration canary to preserve iterative transitioned indexed reads and exit 91, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_string_concat_membership_exit_canary_runs() {
    let canary = pass_canary("text/runtime_string_concat_membership_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-string-concat-membership-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime string concat membership canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime string concat membership canary should run");

    assert_eq!(
        output.status.code(),
        Some(71),
        "expected runtime string concat membership canary to preserve concat result and exit 71, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_string_field_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_string_field_concat_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-string-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime string field concat canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime string field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(73),
        "expected runtime string field concat canary to preserve nested string writes and exit 73, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_indexed_string_field_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_machine_owned_indexed_string_field_concat_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-indexed-string-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime machine-owned indexed string field concat canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime machine-owned indexed string field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(81),
        "expected runtime machine-owned indexed string field concat canary to preserve direct machine-owned indexed string writes and exit 81, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_machine_owned_parameter_write_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_mutable_machine_owned_parameter_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-machine-owned-parameter-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime mutable machine-owned parameter write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime mutable machine-owned parameter write canary should run");

    assert_eq!(
        output.status.code(),
        Some(141),
        "expected runtime mutable machine-owned parameter write canary to preserve writes through mutable machine-owned call parameters and exit 141, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_local_parameter_write_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_mutable_local_parameter_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-local-parameter-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime mutable local parameter write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime mutable local parameter write canary should run");

    assert_eq!(
        output.status.code(),
        Some(171),
        "expected runtime mutable local parameter write canary to preserve writes through local mutable call parameters and exit 171, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_parameter_read_modify_write_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_mutable_parameter_read_modify_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-parameter-read-modify-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime mutable parameter read/modify/write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime mutable parameter read/modify/write canary should run");

    assert_eq!(
        output.status.code(),
        Some(191),
        "expected runtime mutable parameter read/modify/write canary to preserve aliased binary writes and exit 191, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_referenced_local_outlives_sibling_guard_call_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_referenced_local_outlives_sibling_guard_call_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-referenced-local-outlives-sibling-guard-call-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("referenced-local-outlives-sibling-guard-call canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("referenced-local-outlives-sibling-guard-call canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a `&mut local` pointee to survive a sibling state's value-call guard chain \
         (hall_two must run and write room_count = 8 -> exit 70; exit 2 means the dispatch \
         silently fell through after the guard), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_view_linked_input_unrelated_ref_write_exit_canary_runs() {
    let canary = pass_canary("borrow/runtime_view_linked_input_unrelated_ref_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-view-linked-input-unrelated-ref-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("view-linked-input-unrelated-ref-write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("view-linked-input-unrelated-ref-write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the elision-linked view (borrowing `a` only) to coexist with a write to the \
         unlinked ref input `b` (lifetimes stage 1 win), and both writes to land \
         (first.cells[2]=7 + second.cells[0]=1 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_call_single_execution_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_value_call_single_execution_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-value-call-single-execution-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("value-call single-execution canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("value-call single-execution canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected each written call to execute its callee exactly once (two calls -> \
         two increments -> exit 70; exit 2/3 means the splice and branch prelude both \
         ran the callee body), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_explicit_discard_executes_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_explicit_discard_executes_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-explicit-discard-executes-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("explicit-discard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("explicit-discard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `_ = self.roll(&mut self.tally);` to EXECUTE the callee exactly once          (tally 40 -> 41 -> exit 70; exit 10 means the discard dropped the call, exit 3          means it ran twice), got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_transition_subject_call_single_evaluation_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_transition_subject_call_single_evaluation_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-transition-subject-call-single-evaluation-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("transition-subject single-evaluation canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("transition-subject single-evaluation canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the transition guard subject call to run its callee exactly ONCE \
         (one increment -> exit 70; exit 2 means the callee ran per arm or its body \
         was emitted twice), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_effectful_subject_single_evaluation_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_effectful_subject_single_evaluation_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-effectful-subject-single-evaluation-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("effectful-subject single-evaluation canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("effectful-subject single-evaluation canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the diverging-arm transition's effectful subject (nested call \
         chain) to run exactly ONCE (one increment -> exit 70; exit 2/3/77 means \
         the subject re-ran per arm or per nesting level -- the dungeon's \
         32-RNG-draws eager-guard divergence), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_statement_call_single_execution_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_statement_call_single_execution_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-statement-call-single-execution-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("statement-call single-execution canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("statement-call single-execution canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the STATEMENT-position call chain's leaf side effect to run \
         exactly ONCE (one increment -> exit 70; exit 2/3 reports the executor \
         count -- the dungeon's non-guard RNG over-draw where the splice, the \
         prelude's StateCall arm, and the nested-walk straight-line expansion \
         all emitted the leaf), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_assignment_call_post_mutation_value_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_assignment_call_post_mutation_value_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-assignment-call-post-mutation-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("assignment-call post-mutation value canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("assignment-call post-mutation value canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `let v = self.next(&mut state)` to deliver the POST-mutation \
         value (exit 70 = interpreter semantics; exit 2 = the call-result value \
         selection emitted before the splice's mutation writes and read the \
         stale pre-mutation state), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_call_result_after_splice_mutation_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_call_result_after_splice_mutation_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-call-result-after-splice-mutation-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("call-result-after-splice-mutation canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("call-result-after-splice-mutation canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `let seed = self.next_seed(&mut rng)` followed by a consumer \
         `let` to deliver the POST-mutation value (exit 70 = interpreter \
         semantics; exit 71 = the consumer made the storage plan elide seed's \
         LocalStorage slot, the deferral had no landing op, and the call-result \
         copy emitted before the splice's mutation writes), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_called_machine_loop_search_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_called_machine_loop_search_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-called-machine-loop-search-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime called machine loop search canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime called machine loop search canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a called machine whose state loops over a slice (a cyclic state call \
         with arguments) to lower as a dispatch back-edge -- not inline-unroll -- and exit 70, \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_trailing_local_return_exit_canary_runs() {
    // A machine whose trailing terminal expression is a BARE LOCAL NAME must
    // return that local's value, captured at its declaration. The storage
    // planner did not count a trailing `expression` statement as a reference
    // that requires storage, so the local had no frame slot, the bare name
    // could not resolve as a place at selection, and the call-result write
    // silently dropped (`let r = f()` left r at 0). The canary pins three
    // shapes: capture-before-field-mutation (must deliver the CAPTURED value,
    // not the post-mutation re-read), computed-from-param, and a free machine
    // returning a literal-folded local.
    let canary = pass_canary("calls/runtime_trailing_local_return_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-trailing-local-return-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime trailing local return canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime trailing local return canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected every trailing-bare-local return to deliver its declaration-time value \
         (exit 70); 71 = capture-before-mutation returned wrong/zero, 72 = param-computed \
         local wrong, 73 = free-machine literal local wrong. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_recursive_value_return_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_recursive_value_return_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-recursive-value-return-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime recursive value return canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime recursive value return canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a VALUE-position call to a looping machine (`let n = count(s, 0)`) to \
         dispatch the loop AND write the callee's terminal value back into n's call-result \
         slot, yielding n == 5 (exit 70); a 0 return (the pre-keystone bug) exits 71. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_call_slice_len_guard_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_value_call_slice_len_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-value-call-slice-len-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime value call slice len guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime value call slice len guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an inline-leaf VALUE call (`let n = self.m.classify(s)`) whose arm guard \
         reads the slice param's length (`s.len > 0`, `s` bound through the call to an \
         ELIDED caller local aliasing a fixed [i32; 5]) to fold the length to the static \
         element count and take the matching arm (n == 99, exit 70); a dropped arm leaves \
         n == 0 and exits 71. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_exit_code_exit_canary_runs() {
    // `exit_process(self.v)` with a RUNTIME (non-constant) i32 must exit with the
    // computed value. Regression guard for the documented footgun where a runtime
    // exit-code operand was ignored and the process silently exited 0. The canary
    // computes 5 + 65 = 70 and exits with `self.v`.
    let canary = pass_canary("calls/runtime_exit_code_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-exit-code-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime exit code canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime exit code canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `exit_process(self.v)` with a runtime i32 (5 + 65) to exit with the \
         computed value 70; a 0 exit is the pre-fix bug where the runtime exit-code \
         operand was ignored. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_u8_field_arith_exit_canary_runs() {
    let canary = pass_canary("types/runtime_u8_field_arith_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-u8-field-arith-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("u8 field arithmetic canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("u8 field arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected u8 fields to store/add/compare as 1-byte values (100+50==150, exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_i8_signed_arith_exit_canary_runs() {
    let canary = pass_canary("types/runtime_i8_signed_arith_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-i8-signed-arith-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("i8 signed arithmetic canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("i8 signed arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i8 fields to be SIGNED 1-byte values (-10+4==-6, exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_i16_signed_arith_exit_canary_runs() {
    let canary = pass_canary("types/runtime_i16_signed_arith_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-i16-signed-arith-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("i16 signed arithmetic canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("i16 signed arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i16 fields to be SIGNED 2-byte values (-1000+400==-600, then -600<0 \
         via a signed 16-bit guard compare; an unsigned or 1-byte treatment exits 71), \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_u16_field_arith_exit_canary_runs() {
    let canary = pass_canary("types/runtime_u16_field_arith_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-u16-field-arith-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("u16 field arithmetic canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("u16 field arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected u16 fields to store/add/compare as 2-byte UNSIGNED values \
         (40000+30000 wraps to 4464; 40000>30000 needs an unsigned 16-bit compare), \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_isize_signed_arith_exit_canary_runs() {
    let canary = pass_canary("types/runtime_isize_signed_arith_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-isize-signed-arith-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("isize signed arithmetic canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("isize signed arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected isize to be a SIGNED pointer-width integer (-42-8==-50, exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_ref_param_method_dispatch_exit_canary_runs() {
    let canary = pass_canary("traits/runtime_ref_param_method_dispatch_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ref-param-method-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("ref-param method dispatch canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("ref-param method dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a method call on a `&mut Data` reference param to resolve to the data's \
         attached machine (Circle::code() == 99 -> exit 70); an unresolved call returns 0 \
         (exit 71). got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_typed_two_method_receivers_exit_canary_runs() {
    // Two data types implement a SAME-NAMED method (`Circle::code` == 9,
    // `Square::code` == 4), each called through a typed `&mut` reference param.
    // The inline value fold matched callee leafs by state NAME, so the
    // lexically-first impl won at every call site (both calls 9 -> n == 99).
    // Receiver-type discrimination keeps them apart: n == 9*10+4 == 94 -> 70.
    let canary = pass_canary("traits/runtime_typed_two_method_receivers_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-typed-two-method-receivers-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("typed two-method receivers canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("typed two-method receivers canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected same-named methods on two data types to dispatch by the \
         receiver's static type (9*10+4 == 94 -> exit 70); the name-keyed fold \
         picked the first impl for both calls (99 -> exit 71). got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dyn_single_impl_dispatch_exit_canary_runs() {
    let canary = pass_canary("traits/runtime_dyn_single_impl_dispatch_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dyn-single-impl-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("dyn single-impl dispatch canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("dyn single-impl dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `&mut dyn Shape` to devirtualize to the single impl Circle and dispatch \
         Circle::code() == 99 -> exit 70; pre-devirtualization dyn dispatch returned 0 \
         (exit 71). got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dyn_two_impl_dispatch_exit_canary_runs() {
    // TWO data types satisfy Shape, so the `&mut dyn Shape` receiver cannot
    // devirtualize; the call is monomorphized over the trait's closed world and
    // each call site's receiver type picks the impl: Circle::code() == 9 then
    // Square::code() == 4 -> n == 94 -> exit 70. Mirrors the interpreter
    // coverage test dyn_two_impl_dispatch_selects_impl_by_runtime_type.
    let canary = pass_canary("traits/runtime_dyn_two_impl_dispatch_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dyn-two-impl-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("dyn two-impl dispatch canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("dyn two-impl dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `&mut dyn Shape` with TWO impls to dispatch by the call site's \
         receiver type (Circle 9, Square 4 -> n == 94 -> exit 70); an unresolved \
         dyn call returns 0 for both (exit 71). got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dyn_two_impl_dispatch_swapped_exit_canary_runs() {
    // Same two impls, call order swapped: Square (4) first, then Circle (9)
    // -> n == 49 -> exit 70. A dispatcher that always picks the lexically-first
    // impl cannot pass both this and the unswapped canary (it scores 99 twice).
    let canary = pass_canary("traits/runtime_dyn_two_impl_dispatch_swapped_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dyn-two-impl-dispatch-swapped-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("dyn two-impl swapped dispatch canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("dyn two-impl swapped dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the swapped call order to dispatch Square (4) then Circle (9) \
         -> n == 49 -> exit 70. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_alias_write_through_guarded_transition_exit_canary_runs() {
    // A `&mut` param forwarded through a GUARDED transition into a sub-state that
    // writes through it must reach the caller's object. When the callee inlines as a
    // branching leaf, by-value args bind as `mut <literal>` (e.g. `mut 2`), so the
    // leaf guard `key < 4` carried a `Mutable(Integer)` operand the value resolvers
    // didn't see through -- the arm (guard + its alias write) was dropped entirely.
    let canary = pass_canary("calls/runtime_alias_write_through_guarded_transition_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-alias-write-guarded-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("alias-write-through-guarded-transition canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("alias-write-through-guarded-transition canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a &mut alias written in a sub-state reached via a guarded transition \
         to reach the caller (exit 70), got {:?} (71 = the guarded arm's alias write was \
         dropped)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_reference_param_forwarded_through_loop_exit_canary_runs() {
    // Forwarding a `&mut` param to another `&mut` param through a (self-looping)
    // dispatch transition must copy the POINTER VALUE, not the pointee. The materializer
    // dereferenced whenever the referent size equalled the target slot size; for a
    // pointer-sized referent it wrote room data into the pointer slot, so the next write
    // through it faulted. The deref branch now fires only for VALUE targets.
    let canary = pass_canary("calls/runtime_reference_param_forwarded_through_loop_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-reference-param-forwarded-loop-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("reference-param-forwarded-through-loop canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("reference-param-forwarded-through-loop canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a &mut param forwarded to another &mut param through a loop to copy the \
         pointer value (exit 70), got {:?} (139/segfault = pointee written into pointer slot)\
         \nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_call_through_alias_in_dispatch_exit_canary_runs() {
    // A value-returning inline-branching call written through a `&mut` alias inside a
    // DISPATCHED callee must yield the matched arm's value. The guard's forward-skip
    // distance must cover the per-arm pointee copy; `is_guarded_effect` was missing
    // RuntimeStorageCopyToRuntimePointee / RuntimePointee{Integer,Binary}Write, so a
    // skipped arm ran the pointee copy unconditionally and stranded the matched arm.
    let canary = pass_canary("calls/runtime_value_call_through_alias_in_dispatch_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-call-alias-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("value-call-through-alias-in-dispatch canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("value-call-through-alias-in-dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a branching-call result written through a &mut alias in a dispatched callee \
         to yield the matched arm (exit 70), got {:?} (71 = a skipped arm's pointee copy ran \
         unconditionally and stranded the match)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_value_call_in_substate_exit_canary_runs() {
    // Two-level nested call from a sub-state: hall1 -> carve (statement) -> room_mut
    // (in `let x = self.f()` position). carve was misclassified as a leaf (leaf check
    // only scanned `OperationKind::Call` ops, missing the call in a `let` initializer),
    // so its nested room_mut was dropped -> null `&mut Room` -> fault. The classifier
    // now treats a state that sources any non-host call as non-leaf (InlineExpansion).
    let canary = pass_canary("calls/runtime_nested_value_call_in_substate_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nested-value-call-substate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("nested-value-call-in-substate canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("nested-value-call-in-substate canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a 2-level nested call (sub-state -> helper -> value-position call) to be \
         expanded (exit 70), got {:?} (139/71 = the helper was treated as a leaf and its \
         nested call dropped)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_call_in_inlined_substate_exit_canary_runs() {
    // A transition target (sub-state) that calls in `let x = self.f()` position must
    // lower as a straight-line branch so the nested call is expanded. It was
    // misclassified as a leaf (leaf check looked only for Statement-role calls), and
    // leaf expansion can't carry a nested call -> the call was dropped, leaving its
    // `&mut`/value result null -> the next use faulted. Dungeon generation shape.
    let canary = pass_canary("calls/runtime_call_in_inlined_substate_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-call-in-inlined-substate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("call-in-inlined-substate canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("call-in-inlined-substate canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a `let x = self.f()` call in a transition sub-state to be expanded (exit 70), \
         got {:?} (139/71 = the sub-state was treated as a leaf and the call dropped)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_alias_indexed_read_through_transition_exit_canary_runs() {
    // An inlined leaf reading `items[key].field` (constant-index element of a
    // forwarded slice) through a forwarded `&mut` alias: the inlined by-value `key`
    // binds as `mut 2`, so `items[key]` became `items[mut 2]` and the index-path
    // resolvers rejected the `Mutable`-wrapped index, dropping the copy. The `mut`
    // is now stripped on a resolved leaf index. Mirrors the dungeon find_room shape.
    let canary = pass_canary("calls/runtime_alias_indexed_read_through_transition_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-alias-indexed-read-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("alias-indexed-read-through-transition canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("alias-indexed-read-through-transition canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `items[key].field` (constant index) copied through a &mut alias in a \
         guarded sub-state to resolve (exit 70), got {:?} (71 = `mut`-wrapped index rejected)\
         \nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dispatch_binary_call_argument_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_dispatch_binary_call_argument_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dispatch-binary-call-arg-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime dispatch binary call argument canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime dispatch binary call argument canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a binary expression passed as a call argument (`carve(level, 100 + index)`) \
         into a dispatched callee to lower and carry the correct value, exiting 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_called_machine_loop_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_nested_called_machine_loop_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-nested-called-machine-loop-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime nested called machine loop canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime nested called machine loop canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a loop nested two calls deep (Main -> Helper::run -> Lookup::search -> \
         find_at) to specialize the whole call chain and thread main's continuation down \
         through the tail calls, exiting 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_state_loop_indexed_search_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_state_loop_indexed_search_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-state-loop-indexed-search-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime state loop indexed search canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime state loop indexed search canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a self-looping state (dispatch back-edge) that searches a slice by a \
         loop-carried index and passes the found element's field to a successor state to \
         exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_call_result_through_reference_field_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_call_result_through_reference_field_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-call-result-through-reference-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime call result through reference field canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime call result through reference field canary should run");

    assert_eq!(
        output.status.code(),
        Some(183),
        "expected a machine-call result assigned through a reference field \
         (`ref.field = self.call()`) to write through the pointer once, not also \
         clobber the reference slot, and exit 183, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_string_call_result_through_reference_field_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_string_call_result_through_reference_field_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-string-call-result-through-reference-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime string call result through reference field canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime string call result through reference field canary should run");

    assert_eq!(
        output.status.code(),
        Some(186),
        "expected a string machine-call result assigned through a reference field \
         (`ref.label = self.call()`) to copy the returned string descriptor through \
         the pointer and exit 186, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_two_string_call_results_through_reference_fields_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_two_string_call_results_through_reference_fields_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-two-string-call-results-through-reference-fields-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime two string call results through reference fields canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime two string call results through reference fields canary should run");

    assert_eq!(
        output.status.code(),
        Some(194),
        "expected two string call results assigned through reference fields to preserve both descriptors and exit 194, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_offset_string_call_results_through_reference_fields_exit_canary_runs() {
    let canary =
        pass_canary("calls/runtime_offset_string_call_results_through_reference_fields_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-offset-string-call-results-through-reference-fields-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime offset string call results through reference fields canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime offset string call results through reference fields canary should run");

    assert_eq!(
        output.status.code(),
        Some(196),
        "expected string call results assigned through +16/+32 reference fields to preserve both descriptors and exit 196, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_reference_returned_slice_element_write_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_reference_returned_slice_element_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-reference-returned-slice-element-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime reference returned slice element write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime reference returned slice element write canary should run");

    assert_eq!(
        output.status.code(),
        Some(181),
        "expected a machine returning `&mut slice[index]` to bind the element address \
         (not copy the referent) so writes through the reference land, and exit 181, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_reference_returned_slice_element_through_param_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_reference_returned_slice_element_through_param_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-reference-returned-slice-element-through-param-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("reference-returning called machine canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("reference-returning called machine canary should run");

    // The called machine `pick` returns `&mut cells[2]`; its `let cells = ...
    // as_mut_slice()` descriptor init must be materialised, otherwise the returned
    // address is computed from an uninitialized descriptor and the write segfaults.
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a called machine returning `&mut slice[index]` to materialise its \
         slice-descriptor local and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_guarded_reference_returned_slice_element_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_nested_guarded_reference_returned_slice_element_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-nested-guarded-reference-returned-slice-element-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("nested guarded reference-returning called machine canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("nested guarded reference-returning called machine canary should run");

    assert_eq!(
        output.status.code(),
        Some(184),
        "expected a nested guarded call returning `&mut slice[index]` to materialise \
         the returned reference slot before the caller writes through it, and exit 184, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_local_indexed_parameter_write_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_mutable_local_indexed_parameter_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-local-indexed-parameter-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime mutable local indexed parameter write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime mutable local indexed parameter write canary should run");

    assert_eq!(
        output.status.code(),
        Some(171),
        "expected runtime mutable local indexed parameter write canary to preserve writes through local fixed-array indexed mutable call parameters and exit 171, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_machine_owned_local_indexed_parameter_write_exit_canary_runs() {
    let canary =
        pass_canary("calls/runtime_mutable_machine_owned_local_indexed_parameter_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-machine-owned-local-indexed-parameter-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime mutable machine-owned local indexed parameter write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime mutable machine-owned local indexed parameter write canary should run");

    assert_eq!(
        output.status.code(),
        Some(173),
        "expected runtime mutable machine-owned local indexed parameter write canary to preserve writes through machine-owned collection + local indexed mutable call parameters and exit 173, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_dynamic_indexed_machine_owned_parameter_write_exit_canary_runs() {
    let canary =
        pass_canary("calls/runtime_mutable_dynamic_indexed_machine_owned_parameter_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-dynamic-indexed-machine-owned-parameter-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime mutable dynamic indexed machine-owned parameter write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime mutable dynamic indexed machine-owned parameter write canary should run");

    assert_eq!(
        output.status.code(),
        Some(175),
        "expected runtime mutable dynamic indexed machine-owned parameter write canary to preserve writes through machine-owned collection + dynamic indexed mutable call parameters and exit 175, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dispatch_local_index_binary_write_exit_canary_runs() {
    let canary = pass_canary("storage/runtime_dispatch_local_index_binary_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-index-binary-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime local index binary write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime local index binary write canary should run");

    assert_eq!(
        output.status.code(),
        Some(191),
        "expected runtime local index binary write canary to preserve direct caller-local indexed binary writes and exit 191, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dispatch_helper_local_alias_add_exit_canary_runs() {
    let canary = pass_canary("storage/runtime_dispatch_helper_local_alias_add_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dispatch-helper-local-alias-add-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime dispatch helper local alias add canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime dispatch helper local alias add canary should run");

    assert_eq!(
        output.status.code(),
        Some(181),
        "expected runtime dispatch helper local alias add canary to preserve append_exit mutation through local slice alias and exit 181, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_alias_indexed_field_write_exit_canary_runs() {
    let canary = pass_canary("storage/runtime_slice_alias_indexed_field_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-alias-indexed-field-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime slice alias indexed field write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime slice alias indexed field write canary should run");

    assert_eq!(
        output.status.code(),
        Some(201),
        "expected runtime slice alias indexed field write canary to write through a local slice alias and exit 201, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_stdin_command_branch_exit_canary_runs() {
    let canary = pass_canary("text/runtime_stdin_command_branch_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-stdin-command-branch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime stdin command branch canary should compile");

    let mut child = Command::new(build_dir.join(executable_name()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("runtime stdin command branch canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"look\n")
        .expect("stdin command branch input should be written");
    let output = child
        .wait_with_output()
        .expect("runtime stdin command branch canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected runtime stdin command branch canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "look\n",
        "expected runtime stdin command branch canary to echo the resolved command output"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_stderr_write_exit_canary_runs() {
    // The `write_error` host capability mirrors `write` but targets the stderr
    // handle (GetStdHandle(-12)) instead of stdout (-11). The program must emit
    // its text on stderr only, leaving stdout empty, and exit with the requested
    // code.
    let canary = pass_canary("text/runtime_stderr_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-stderr-write-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime stderr write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("runtime stderr write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected runtime stderr write canary to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "hello-stderr\n",
        "expected runtime stderr write canary to emit its text on stderr"
    );
    assert!(
        output.stdout.is_empty(),
        "expected runtime stderr write canary to leave stdout empty, got {:?}",
        String::from_utf8_lossy(&output.stdout)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_stdin_line_buffering_exit_canary_runs() {
    let canary = pass_canary("text/runtime_stdin_line_buffering_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-stdin-line-buffering-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime stdin line buffering canary should compile");

    let mut child = Command::new(build_dir.join(executable_name()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("runtime stdin line buffering canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"hello\nworld\n")
        .expect("stdin line buffering input should be written");
    let output = child
        .wait_with_output()
        .expect("runtime stdin line buffering canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected runtime stdin line buffering canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "hello\nworld\n",
        "expected runtime stdin line buffering canary to preserve one logical line per read"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_stdin_crlf_line_read_canary_runs() {
    // Windows terminals (and piped CRLF input) terminate each line with "\r\n".
    // The line reader must treat that as ONE terminator: a '\r'-ended line must
    // not leave the trailing '\n' to surface as a phantom empty line on the next
    // read_line. Reuses the two-read echo sample; with the bug the second read
    // returns "" and the output is "hello\n\n".
    let canary = pass_canary("text/runtime_stdin_line_buffering_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-stdin-crlf-line-read-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime stdin crlf line read canary should compile");

    let mut child = Command::new(build_dir.join(executable_name()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("runtime stdin crlf line read canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"hello\r\nworld\r\n")
        .expect("stdin crlf line read input should be written");
    let output = child
        .wait_with_output()
        .expect("runtime stdin crlf line read canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected runtime stdin crlf line read canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "hello\nworld\n",
        "expected CRLF input to read two clean lines (no phantom empty line from the trailing \\n)"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_alias_indexed_string_field_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_slice_alias_indexed_string_field_concat_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-alias-indexed-string-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime slice alias indexed string field concat canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime slice alias indexed string field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime slice alias indexed string field concat canary to preserve alias-indexed string writes and exit 77, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_indexed_string_guard_exit_canary_runs() {
    // A slice-indexed String field compared against a literal in guard
    // position: an EMPTY (default-zeroed) field takes the false arm, the
    // matching field takes the true arm, and a same-length differing field
    // takes the false arm. Exit 70 only when all three behave (the lying-guard
    // regression took the true arm unconditionally, exiting 71).
    let canary = pass_canary("text/runtime_slice_indexed_string_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-indexed-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime slice indexed string guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime slice indexed string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected slice-indexed String guard compares to be content compares (empty != literal, match == literal, same-length differ != literal) and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_string_field_literal_guard_exit_canary_runs() {
    // The storage-place sibling of the slice-indexed shape: a machine-owned
    // String field guard-compared against a literal (empty field takes the
    // false arm; written field takes the true arm).
    let canary = pass_canary("text/runtime_string_field_literal_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-string-field-literal-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime string field literal guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime string field literal guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected machine String field guard compares against literals to be content compares and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_array_indexed_string_guard_exit_canary_runs() {
    // The frame-BASE-indexed sibling of the slice-indexed shape: a LOCAL
    // inline fixed array's element String field guard-compared against a
    // literal at a runtime index (empty field takes the false arm, matching
    // takes true, same-length-differing takes false; the lying-guard
    // regression selected no compare and took the true arm unconditionally).
    let canary = pass_canary("text/runtime_local_array_indexed_string_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-array-indexed-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime local array indexed string guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime local array indexed string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected local-array-indexed String guard compares to be content compares (empty != literal, match == literal, same-length differ != literal) and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_fixed_indexed_string_guard_exit_canary_runs() {
    // The CONSTANT-index sibling of the slice-indexed shape: a slice
    // element's String field guard-compared against a literal at a literal
    // index (`room_slice[0]`), lowering through the fixed-indexed place
    // (descriptor deref + folded constant offset). Same three regimes.
    let canary = pass_canary("text/runtime_slice_fixed_indexed_string_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-slice-fixed-indexed-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime slice fixed indexed string guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime slice fixed indexed string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected fixed-indexed String guard compares to be content compares (empty != literal, match == literal, same-length differ != literal) and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_pointee_string_guard_exit_canary_runs() {
    // The POINTEE sibling: a String field read through a `&mut Room` pointer
    // slot (local alias AND called-machine parameter), guard-compared against
    // a literal. The pre-fix regression here was an always-unequal compare:
    // the place resolved to the pointer slot's raw bytes rather than the
    // pointee's descriptor, so the MATCH regime took the false arm.
    let canary = pass_canary("text/runtime_pointee_string_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-pointee-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime pointee string guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime pointee string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected pointee String guard compares to be content compares (empty != literal, match == literal, same-length differ != literal, parameter shape included) and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_string_parameter_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_mutable_string_parameter_concat_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-string-parameter-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime mutable string parameter concat canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime mutable string parameter concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime mutable string parameter concat canary to preserve pointee string writes and exit 77, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_string_parameter_concat_write_line_canary_runs() {
    let canary = pass_canary("text/runtime_mutable_string_parameter_concat_write_line");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-string-parameter-concat-write-line-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime mutable string parameter concat write_line canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime mutable string parameter concat write_line canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime mutable string parameter concat write_line canary to print a generated pointee string and exit 77, got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("prefix omega"),
        "expected generated line to be printed; stdout was:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_string_parameter_wrapped_concat_write_line_canary_runs() {
    let canary = pass_canary("text/runtime_mutable_string_parameter_wrapped_concat_write_line");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-string-parameter-wrapped-concat-write-line-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime mutable string parameter wrapped concat write_line canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime mutable string parameter wrapped concat write_line canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime mutable string parameter wrapped concat write_line canary to print a generated pointee string and exit 77, got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("prefix omega done"),
        "expected generated wrapped line to be printed; stdout was:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_struct_string_field_copy_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_mutable_struct_string_field_copy_concat_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-struct-string-field-copy-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime mutable struct string field copy concat canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime mutable struct string field copy concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime mutable struct string field copy concat canary to preserve copied string fields and exit 77, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_struct_string_field_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_local_struct_string_field_concat_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-struct-string-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime local struct string field concat canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime local struct string field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(188),
        "expected generated string concat to append a copied local struct string field and exit 188, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_lookup_struct_field_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_lookup_struct_field_concat_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-lookup-struct-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime lookup struct field concat canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime lookup struct field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(190),
        "expected lookup-filled local struct field to feed generated string concat and exit 190, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_large_lookup_struct_field_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_large_lookup_struct_field_concat_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-large-lookup-struct-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime large lookup struct field concat canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime large lookup struct field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(192),
        "expected large-frame lookup-filled local struct field concat to exit 192, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_large_room_lookup_struct_field_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_large_room_lookup_struct_field_concat_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-large-room-lookup-struct-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime large room lookup struct field concat canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime large room lookup struct field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(200),
        "expected large indexed room copy to preserve label for generated concat and exit 200, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_call_argument_struct_string_field_slice_alias_exit_canary_runs() {
    let canary = pass_canary("text/runtime_call_argument_struct_string_field_slice_alias_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-call-argument-struct-string-slice-alias-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime call argument struct string slice alias canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime call argument struct string slice alias canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime call argument string copied through slice-alias struct field to exit 77, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_struct_string_field_copy_concat_write_line_canary_runs() {
    let canary = pass_canary("text/runtime_mutable_struct_string_field_copy_concat_write_line");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-struct-string-field-copy-concat-write-line-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime mutable struct string field copy concat write_line canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime mutable struct string field copy concat write_line canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime mutable struct string field copy concat write_line canary to print copied-field generated text and exit 77, got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("prefix omega done"),
        "expected copied-field generated line to be printed; stdout was:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_indexed_integer_write_exit_canary_runs() {
    let canary = pass_canary("storage/runtime_machine_owned_indexed_integer_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-indexed-integer-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime machine-owned indexed integer write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime machine-owned indexed integer write canary should run");

    assert_eq!(
        output.status.code(),
        Some(79),
        "expected runtime machine-owned indexed integer write canary to preserve direct machine-owned indexed writes and exit 79, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_fixed_indexed_struct_copy_exit_canary_runs() {
    let canary = pass_canary("storage/runtime_machine_owned_fixed_indexed_struct_copy_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-fixed-indexed-struct-copy-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime machine-owned fixed indexed struct copy canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime machine-owned fixed indexed struct copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(83),
        "expected runtime machine-owned fixed indexed struct copy canary to preserve direct fixed-index machine-owned copies and exit 83, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_indexed_struct_copy_exit_canary_runs() {
    let canary = pass_canary("storage/runtime_machine_owned_indexed_struct_copy_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-indexed-struct-copy-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime machine-owned indexed struct copy canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime machine-owned indexed struct copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(85),
        "expected runtime machine-owned indexed struct copy canary to preserve direct indexed machine-owned copies and exit 85, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_indexed_nested_exit_write_exit_canary_runs() {
    let canary = pass_canary("storage/runtime_machine_owned_indexed_nested_exit_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-indexed-nested-exit-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime machine-owned indexed nested exit write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime machine-owned indexed nested exit write canary should run");

    assert_eq!(
        output.status.code(),
        Some(89),
        "expected runtime machine-owned indexed nested exit write canary to preserve nested fixed-array writes and exit 89, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_ordered_room_dispatch_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_ordered_room_dispatch_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime ordered room dispatch canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime ordered room dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(73),
        "expected runtime ordered room dispatch canary to route to ambush_clear exit code 73, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_ordered_room_dispatch_after_call_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_ordered_room_dispatch_after_call_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-after-call-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime ordered room dispatch after call canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime ordered room dispatch after call canary should run");

    assert_eq!(
        output.status.code(),
        Some(83),
        "expected runtime ordered room dispatch after call canary to route to ambush_clear exit code 83, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_ordered_room_dispatch_game_shape_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_ordered_room_dispatch_game_shape_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-game-shape-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime ordered room dispatch game-shape canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime ordered room dispatch game-shape canary should run");

    assert_eq!(
        output.status.code(),
        Some(93),
        "expected runtime ordered room dispatch game-shape canary to route to show_ambush_room exit code 93, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_ordered_room_dispatch_large_machine_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_ordered_room_dispatch_large_machine_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-large-machine-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime ordered room dispatch large-machine canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime ordered room dispatch large-machine canary should run");

    assert_eq!(
        output.status.code(),
        Some(103),
        "expected runtime ordered room dispatch large-machine canary to route to show_ambush_room exit code 103, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_ordered_room_dispatch_loop_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_ordered_room_dispatch_loop_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-loop-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime ordered room dispatch loop canary should compile");

    let mut child = Command::new(build_dir.join(executable_name()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("runtime ordered room dispatch loop canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"east\n")
        .expect("loop canary input should be written");
    let output = child
        .wait_with_output()
        .expect("runtime ordered room dispatch loop canary should finish");

    assert_eq!(
        output.status.code(),
        Some(135),
        "expected runtime ordered room dispatch loop canary to route to show_ambush_encounter exit code 135, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_guarded_inline_leaf_arm_skip_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_guarded_inline_leaf_arm_skip_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-guarded-inline-leaf-arm-skip-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime guarded inline leaf arm skip canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime guarded inline leaf arm skip canary should run");

    // The matched value-switch arm (`1 -> store(20)`) must skip its sibling arms;
    // exit 71 would mean the `_ -> store(30)` fallback clobbered the result to 30.
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected guarded inline leaf arm to survive sibling clobber (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_ordered_room_dispatch_real_show_states_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_ordered_room_dispatch_real_show_states_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-real-show-states-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime ordered room dispatch real-show-states canary should compile");

    let mut child = Command::new(build_dir.join(executable_name()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("runtime ordered room dispatch real-show-states canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"east\n")
        .expect("real show states canary input should be written");
    let output = child
        .wait_with_output()
        .expect("runtime ordered room dispatch real-show-states canary should finish");

    assert_eq!(
        output.status.code(),
        Some(145),
        "expected runtime ordered room dispatch real-show-states canary to route to show_ambush_encounter exit code 145, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_threaded_mut_arg_interrupt_soak_exit_canary_runs() {
    // Soak net for interrupt-clobbered scratch registers in frame-slot copies:
    // the encoder once parked slot copies in x18, which the Darwin arm64 kernel
    // zeroes on every kernel->user return, so threaded `&mut` args corrupted
    // whenever a timer tick landed inside a copy pair (the dungeon hot-potato
    // segfault). Fifty million dispatched pointer-threaded increments span many
    // ticks; a lost copy shows up as exit 71 (dropped count) or a crash.
    let canary = pass_canary("dungeon/runtime_threaded_mut_arg_interrupt_soak_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-threaded-mut-arg-interrupt-soak-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("threaded mut-arg interrupt soak canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("threaded mut-arg interrupt soak canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected all fifty million pointer-threaded increments to land (exit 70), got {:?} (71 = increments lost to a clobbered scratch register)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_value_call_caller_local_guard_exit_canary_runs() {
    // A guarded transition on a value call whose NESTED inline value call
    // returns a comparison against the CALLER's fold-only local (`chance`'s
    // `roll < numerator` with `numerator` bound to should_carve's slot-less
    // local `chance`). The leaf context could not resolve the name as a place,
    // so the call-result write was silently dropped: the guard byte stayed 0
    // and the TRUE arm never dispatched -- the dungeon's side rooms R05/R06
    // were never carved, rendering empty description lines.
    let canary = pass_canary("dungeon/runtime_nested_value_call_caller_local_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nested-value-call-caller-local-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("nested value-call caller-local guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("nested value-call caller-local guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the nested chance comparison to reach its call-result slot so \
         the TRUE transition arm dispatches (exit 70, interpreter semantics; \
         exit 71 = the result write was dropped and the guard byte read 0), \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[cfg(not(windows))]
#[test]
fn native_dungeon_crawler_runs_stable_scripted_loop() {
    let sample = repo_root().join("samples").join("dungeon_crawler_cli");
    let main_path = sample.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-native-dungeon-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("native dungeon crawler should compile to a runnable executable");

    let mut child = Command::new(build_dir.join(executable_name()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("native dungeon crawler executable should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(
            // The detour at R02 (`west` into R06, `east` back) visits the second
            // side room so its data-driven description line is exercised too.
            b"look\nnorth\nnorth\nuse\nlook\nnorth\nfight\nlook\nnorth\nuse\nlook\nsouth\nsouth\nwest\neast\nsouth\neast\nuse\nlook\ninv\nwest\nsouth\nhelp\nexit\n",
        )
        .expect("scripted dungeon input should be written");
    let output = child
        .wait_with_output()
        .expect("native dungeon crawler executable should finish");

    assert!(
        output.status.success(),
        "generated native dungeon executable exited with {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Dungeon Crawler"));
    assert!(stdout.contains("== Gate =="));
    // Canonical strings come from the sample's data-driven room view (commit
    // 3971b22f replaced the hardcoded "A stone gate opens..." text with the maze
    // builder's depth-derived descriptions); every string below is produced
    // identically by the interpreter oracle on this exact script.
    assert!(stdout.contains("A bottomless dark room near the dungeon heart."));
    assert!(stdout.contains("== Branch Room =="));
    assert!(stdout.contains("A winding branch room where the walls sweat mineral dust."));
    assert!(stdout.contains("[Paths] north"));
    assert!(stdout.contains("[Paths] south | north | east"));
    assert!(stdout.contains("You take the treasure."));
    assert!(stdout.contains("The enemy collapses. You find a little gold."));
    assert!(stdout.contains("The fountain heals your wounds."));
    assert!(stdout.contains("You collect the loose gold."));
    // The side rooms' data-driven DESCRIPTIONS are asserted with their adjacent
    // unique lines so the match pins the right room view. These were the last
    // native/interpreter divergence: the side rooms were never CARVED natively
    // because `should_carve`'s nested `chance` value (`roll < numerator`, with
    // `numerator` bound to the caller's slot-less local `chance`) lost its
    // call-result write, so the carve transition's TRUE arm never fired --
    // fixed by resolving caller-local initializer names in leaf terminal value
    // writes, pinned by dungeon/runtime_nested_value_call_caller_local_guard_exit.
    // With this the scripted tour is byte-for-byte the interpreter's output.
    assert!(stdout.contains(
        "A shallow limestone room with fresh claw marks.\nLoose gold glitters in the dust."
    ));
    assert!(stdout.contains("Loose gold glitters in the dust."));
    assert!(stdout.contains("[Paths] west"));
    // R06 (the west side chamber off R02): depth-3 description, quiet event,
    // and its unique single east exit.
    assert!(stdout.contains(
        "A winding branch room where the walls sweat mineral dust.\nThe room is quiet.\n[Paths] east"
    ));
    assert!(stdout.contains("Inv: 30 gold. Purse heavy, charm secured."));

    let _ = fs::remove_dir_all(&build_dir);
}

#[cfg(not(windows))]
#[test]
fn native_dungeon_direct_movement_dispatch_runs() {
    let source = repo_root().join("samples").join("dungeon_crawler_cli");
    let package_dir = std::env::temp_dir().join(format!(
        "omega-dungeon-direct-movement-{}",
        std::process::id()
    ));
    let build_dir = package_dir.join("build");
    let _ = fs::remove_dir_all(&package_dir);
    copy_dir_recursive(&source, &package_dir).expect("sample package should copy into temp repro");

    compile(CompileOptions {
        root_path: package_dir.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("patched dungeon direct-dispatch repro should compile");

    let mut child = Command::new(build_dir.join(executable_name()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("patched dungeon direct-dispatch repro should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"north\neast\nexit\n")
        .expect("repro input should be written");
    let output = child
        .wait_with_output()
        .expect("patched dungeon direct-dispatch repro should finish");

    assert!(
        output.status.success(),
        "patched dungeon direct-dispatch repro exited with {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // R05 (the east side chamber) is identified by its data-driven description
    // followed by its gold-cache event line and its unique "[Paths] west" exit
    // list -- all rendered identically by the interpreter oracle on this script.
    assert!(
        stdout.contains(
            "A shallow limestone room with fresh claw marks.\nLoose gold glitters in the dust."
        ),
        "expected the side chamber's depth-derived description right before its gold-cache event line; stdout was:\n{}",
        stdout
    );
    assert!(
        stdout.contains("Loose gold glitters in the dust."),
        "expected direct movement dispatch sample to reach the side chamber after 'north' then 'east'; stdout was:\n{}",
        stdout
    );
    assert!(
        stdout.contains("[Paths] west"),
        "expected the side chamber's exit list after 'north' then 'east'; stdout was:\n{}",
        stdout
    );
    assert!(
        !stdout.contains("[Input] That action is not available right now."),
        "expected direct movement dispatch sample not to reject 'north' then 'east'; stdout was:\n{}",
        stdout
    );

    let _ = fs::remove_dir_all(&package_dir);
}

// Positive proof-context operator selection (chapter 8): the proven caller
// `requires` fact admits the domain-owned `+` meaning, and the checked
// evidence must record THAT meaning as the selected one — not merely compile.
#[test]
fn domain_operator_selection_records_proven_domain_meaning_as_evidence() {
    let canary = pass_canary("domains/domain_operator_proven_fact_selects_meaning");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("proven domain fact canary should compile to checked trees");

    let selected_domain_uses = checked
        .facts
        .operators
        .resolved_uses()
        .filter(|operator_use| {
            operator_use.spelling == omega_core::operator_spelling::OperatorSpelling::Add
        })
        .filter_map(|operator_use| checked.facts.operators.selected_candidate(operator_use))
        .filter(|candidate| candidate.is_domain_owned())
        .count();
    assert!(
        selected_domain_uses > 0,
        "expected the proven `Quantity::Additive` fact to select the domain-owned `+` meaning \
         and record it in the operator evidence"
    );
}

// The other half of the selection ruling: without a proven fact the domain
// meaning is inadmissible and the ordinary builtin operation stays selected.
// The evidence must say so explicitly (builtin fallback), not pretend the
// domain meaning won.
#[test]
fn domain_operator_selection_records_builtin_fallback_when_fact_unproven() {
    let canary = pass_canary("domains/domain_operator_unproven_keeps_builtin_meaning");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("unproven builtin fallback canary should compile to checked trees");

    let fallback_uses = checked
        .facts
        .operators
        .uses_with_status(omega_checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback)
        .count();
    assert!(
        fallback_uses > 0,
        "expected the unproven `i32::Degrees` membership to de-admit the domain `+` meaning \
         and record the use as a builtin fallback"
    );
    assert_eq!(
        checked
            .facts
            .operators
            .resolved_uses()
            .filter_map(|operator_use| checked.facts.operators.selected_candidate(operator_use))
            .filter(|candidate| candidate.is_domain_owned())
            .count(),
        0,
        "no domain-owned meaning may be selected without a proven domain fact"
    );
}

#[test]
fn pass_canaries_compile() {
    for canary_name in ACTIVE_PASS_CANARIES {
        let canary = pass_canary(canary_name);

        if let Err(diagnostics) = compile_canary_without_output(&canary) {
            panic!(
                "expected pass canary {} to compile, but got diagnostics:\n{}",
                canary.display(),
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
    }
}

#[test]
fn fail_canaries_reject_with_expected_diagnostic_fragment() {
    for canary_name in ACTIVE_FAIL_CANARIES {
        let canary = fail_canary(canary_name);
        let expected_path = canary.join("expected.txt");
        let expected_fragment = fs::read_to_string(&expected_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", expected_path.display()))
            .trim()
            .to_owned();

        let diagnostics = match compile_canary_without_output(&canary) {
            Ok(report) => {
                panic!(
                    "expected fail canary {} to reject, but it compiled successfully: {}",
                    canary.display(),
                    report.summary()
                )
            }
            Err(diagnostics) => diagnostics,
        };
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            combined.contains(&expected_fragment),
            "fail canary {} did not contain expected fragment {:?}\nactual diagnostics:\n{}",
            canary.display(),
            expected_fragment,
            combined
        );
    }
}

#[test]
fn pending_canaries_reproduce_known_gaps() {
    for canary in ACTIVE_PENDING_CANARIES {
        let canary_dir = pending_canary(canary.path);
        let result = compile_canary_without_output(&canary_dir);
        match canary.expectation {
            PendingCanaryExpectation::CurrentlyAccepts => {
                if let Err(diagnostics) = result {
                    let combined = diagnostics
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n");
                    panic!(
                        "pending canary {} now rejects. Promote it to fail and update the suite.\nactual diagnostics:\n{}",
                        canary_dir.display(),
                        combined
                    );
                }
            }
            PendingCanaryExpectation::CurrentlyRejects { fragment } => {
                let diagnostics = match result {
                    Ok(report) => {
                        panic!(
                            "pending canary {} no longer rejects. Promote it to pass/fail and update the suite: {}",
                            canary_dir.display(),
                            report.summary()
                        )
                    }
                    Err(diagnostics) => diagnostics,
                };
                let combined = diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n");

                assert!(
                    combined.contains(fragment),
                    "pending canary {} rejected differently than expected. expected fragment {:?}\nactual diagnostics:\n{}",
                    canary_dir.display(),
                    fragment,
                    combined
                );
            }
        }
    }
}

fn compile_canary_without_output(canary_dir: &Path) -> Result<CompileReport, Vec<Diagnostic>> {
    // `compile` writes pipeline phase artifacts into `build_dir()` even when
    // `write_output` is false, and a `None` build dir defaults to `<canary>/build`
    // -- a path SHARED by every test that compiles the same canary. Under parallel
    // test threads two such compiles race on the artifact files (delete-while-write
    // / file-in-use on Windows), which is exactly the intermittent
    // `pass_canaries_compile` vs `capability_pass_canaries_compile_in_isolation`
    // full-suite flake. Give every no-output compile its own temp dir instead.
    let build_dir = unique_no_output_build_dir();
    let result = compile(CompileOptions {
        root_path: canary_dir.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: false,
    });
    let _ = fs::remove_dir_all(&build_dir);
    result
}

/// A build dir no other concurrent compile can collide with: process id plus a
/// process-wide counter (parallel test threads share the process, so the id alone
/// is not unique).
fn unique_no_output_build_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_BUILD_DIR: AtomicU64 = AtomicU64::new(0);
    let unique = NEXT_BUILD_DIR.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "omega-canary-no-output-{}-{unique}",
        std::process::id()
    ))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("compiler crate should live under compiler/orchestration/omega-compiler")
        .to_path_buf()
}

fn pass_canary(path: &str) -> PathBuf {
    repo_root().join("canaries/pass").join(path)
}

fn fail_canary(path: &str) -> PathBuf {
    repo_root().join("canaries/fail").join(path)
}

fn pending_canary(path: &str) -> PathBuf {
    repo_root().join("canaries/pending").join(path)
}

fn copy_dir_recursive(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let destination = to.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &destination)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn executable_name() -> &'static str {
    "omega-program.exe"
}

#[cfg(not(windows))]
fn executable_name() -> &'static str {
    "omega-program"
}

const ACTIVE_PASS_CANARIES: &[&str] = &[
    "traits/boundary_trait_effects_host_call",
    "traits/dyn_trait_object_dispatch",
    "capabilities/uses_caller_folder",
    "capabilities/acquires_filesystem_authority",
    "capabilities/stores_capability",
    "comptime/runtime_const_array_length_exit",
    "concurrency/runtime_spawn_interleaved_join_exit",
    "concurrency/runtime_spawn_join_moved_arg_exit",
    "concurrency/spawn_fire_and_forget",
    "concurrency/spawn_join_handle",
    "data/case_payload_declaration",
    "data/case_payload_native_construction",
    "data/match_default_satisfies_exhaustiveness",
    "data/match_exhaustive_by_case_union_domain",
    "data/match_exhaustive_by_cases",
    "data/payload_less_case_equality",
    "data/runtime_array_literal_string_field_exit",
    "data/runtime_case_payload_guard_read_exit",
    "data/runtime_case_reassignment_exit",
    "data/runtime_mixed_shape_exit",
    "data/runtime_struct_literal_string_field_exit",
    "domains/call_requires_preserved_across_imported_disjoint_mutation",
    "domains/call_requires_preserved_across_disjoint_mutation",
    "domains/call_requires_satisfied_by_caller_requires",
    "domains/call_requires_free_machine_satisfied_by_caller_requires",
    "domains/call_requires_boundary_trait_satisfied_by_caller_requires",
    "domains/call_requires_platform_satisfied_by_caller_requires",
    "domains/call_requires_boolean_expression_from_domain_fact",
    "domains/call_requires_boolean_expression_preserved_across_disjoint_mutating_call",
    "domains/call_requires_dynamic_indexed_scalar_member_expression_from_domain_fact",
    "domains/call_requires_fixed_indexed_boolean_expression_preserved_across_disjoint_mutating_call",
    "domains/call_requires_fixed_indexed_scalar_member_expression_from_domain_fact",
    "domains/call_requires_scalar_member_expression_from_domain_fact",
    "domains/call_requires_boolean_union_expression_from_domain_fact",
    "domains/call_requires_domain_intersection_preserved",
    "domains/call_requires_domain_union_left_branch_preserved",
    "domains/call_requires_domain_union_right_branch_preserved",
    "domains/call_requires_domain_membership_preserved_across_disjoint_dynamic_field_mutation",
    "domains/call_requires_domain_membership_preserved_across_disjoint_literal_element_mutation",
    "control_flow/composite_field_guard_dispatch",
    "control_flow/composite_range_guard_dispatch",
    "control_flow/runtime_case_member_dispatch_exit",
    "control_flow/runtime_local_boolean_or_value_exit",
    "control_flow/runtime_straight_line_terminal_local_exit",
    "control_flow/runtime_straight_line_terminal_field_readback_exit",
    "control_flow/termination_countdown_compile",
    "control_flow/termination_index_distance_compile",
    "termination/custom_ranking_field_countdown_compile",
    "termination/custom_ranking_order_compile",
    "termination/custom_ranking_struct_view",
    "domains/contracts_domain_membership_surface",
    "domains/domain_operator_spelling_selected",
    "domains/domain_operator_proven_fact_selects_meaning",
    "domains/domain_operator_unproven_keeps_builtin_meaning",
    "domains/domain_operator_requires_discharged",
    "domains/string_non_empty_classifier",
    "domains/when_classifier_clause",
    "domains/executable_domain_membership_expression_exit",
    "domains/executable_domain_membership_intersection_guard_exit",
    "domains/executable_imported_domain_membership_exit",
    "domains/executable_imported_domain_membership_guard_exit",
    "domains/executable_domain_membership_union_guard_exit",
    "domains/executable_domain_membership_union_value_exit",
    "domains/if_domain_membership_check",
    "domains/domain_intersection_contract_surface",
    "domains/domain_import_valid",
    "domains/exit_ensures_domain_union_left_branch_preserved",
    "domains/exit_ensures_domain_union_right_branch_preserved",
    "domains/exit_ensures_boolean_expression_from_domain_fact",
    "domains/exit_ensures_boolean_expression_preserved_across_disjoint_mutating_call",
    "domains/exit_ensures_boolean_union_expression_from_domain_fact",
    "domains/exit_ensures_fixed_indexed_boolean_expression_preserved_across_disjoint_mutating_call",
    "domains/exit_ensures_dynamic_indexed_scalar_member_expression_from_domain_fact",
    "domains/exit_ensures_fixed_indexed_scalar_member_expression_from_domain_fact",
    "domains/indexed_domain_requires_preserved_across_disjoint_field_mutation",
    "borrows/borrow_disjoint_fixed_index_call_mut",
    "control_flow/entry_surface_receiver_paths",
    "domains/exit_ensures_preserved_from_entry",
    "borrows/borrow_disjoint_fixed_index_mut",
    "borrows/local_alias_boolean_transfer",
    "borrow/disjoint_subslice_owner_write_compile",
    "borrow/disjoint_mutable_slice_element_reborrow_compile",
    "borrow/disjoint_field_owner_call_compile",
    "borrow/runtime_view_linked_input_unrelated_ref_write_exit",
    "domains/local_alias_domain_transfer",
    "calls/effectless_mut_out_param_discard_compile",
    "calls/mutable_output_host_call",
    "calls/nested_machine_continuation",
    "calls/runtime_explicit_discard_executes_exit",
    "storage/runtime_alias_integer_write",
    "storage/runtime_alias_field_integer",
    "storage/runtime_alias_field_binary",
    "storage/runtime_machine_owned_fixed_indexed_struct_copy_exit",
    "storage/runtime_machine_owned_indexed_integer_write_exit",
    "storage/runtime_machine_owned_indexed_nested_exit_write_exit",
    "storage/runtime_machine_owned_indexed_struct_copy_exit",
    "storage/runtime_dispatch_helper_local_alias_add_exit",
    "storage/requires_slice_indexed_alias_field_binary_compile",
    "text/runtime_alias_string_write",
    "text/runtime_alias_text_builder_write",
    "text/runtime_string_concat_membership_exit",
    "text/runtime_string_field_concat_exit",
    "text/runtime_machine_owned_indexed_string_field_concat_exit",
    "text/runtime_slice_alias_indexed_string_field_concat_exit",
    "text/runtime_slice_indexed_string_guard_exit",
    "text/runtime_local_array_indexed_string_guard_exit",
    "text/runtime_slice_fixed_indexed_string_guard_exit",
    "text/runtime_pointee_string_guard_exit",
    "text/runtime_string_field_literal_guard_exit",
    "text/runtime_mutable_string_parameter_concat_exit",
    "text/runtime_mutable_string_parameter_concat_write_line",
    "text/runtime_mutable_string_parameter_wrapped_concat_write_line",
    "text/runtime_mutable_struct_string_field_copy_concat_exit",
    "text/runtime_local_struct_string_field_concat_exit",
    "text/runtime_lookup_struct_field_concat_exit",
    "text/runtime_large_lookup_struct_field_concat_exit",
    "text/runtime_large_room_lookup_struct_field_concat_exit",
    "text/runtime_call_argument_struct_string_field_slice_alias_exit",
    "text/runtime_mutable_struct_string_field_copy_concat_write_line",
    "arithmetic/runtime_arithmetic_guard",
    "arithmetic/runtime_arithmetic_value",
    "calls/runtime_call_guard",
    "control_flow/runtime_branching_helper_guard",
    "control_flow/runtime_branching_helper_local_guard_value",
    "control_flow/runtime_branching_helper_string",
    "control_flow/runtime_branching_helper_struct",
    "control_flow/runtime_branching_helper_value",
    "rewards/runtime_branch_enemy_reward_shape",
    "calls/runtime_call_value",
    "calls/runtime_call_enum_field_value",
    "calls/runtime_call_enum_field_with_args",
    "calls/runtime_call_enum_field_with_mut_arg",
    "calls/runtime_call_enum_sequence",
    "calls/runtime_call_enum_value",
    "calls/runtime_call_result_after_splice_mutation_exit",
    "calls/runtime_string_call_result_through_reference_field_exit",
    "calls/runtime_two_string_call_results_through_reference_fields_exit",
    "calls/runtime_offset_string_call_results_through_reference_fields_exit",
    "calls/runtime_reference_returned_slice_element_through_param_exit",
    "calls/runtime_nested_guarded_reference_returned_slice_element_exit",
    "calls/runtime_mutable_machine_owned_parameter_write_exit",
    "calls/runtime_mutable_local_indexed_parameter_write_exit",
    "calls/runtime_mutable_machine_owned_local_indexed_parameter_write_exit",
    "calls/runtime_mutable_dynamic_indexed_machine_owned_parameter_write_exit",
    "dungeon/runtime_boolean_helper_guard_dispatch",
    "dungeon/runtime_direct_boolean_conjunction_dispatch",
    "dungeon/runtime_direct_boolean_conjunction_exit",
    "dungeon/runtime_enemy_clear_reentry_exit",
    "dungeon/runtime_clear_carve_render_string_fields_exit",
    "dungeon/runtime_full_level_wrapper_lookup_string_field_exit",
    "dungeon/runtime_enemy_clear_reentry_guard",
    "control_flow/runtime_guarded_leaf_ordering_call",
    "dungeon/runtime_ordered_room_dispatch_after_call_exit",
    "dungeon/runtime_ordered_room_dispatch_exit",
    "dungeon/runtime_ordered_room_dispatch_game_shape_exit",
    "dungeon/runtime_ordered_room_dispatch_large_machine_exit",
    "dungeon/runtime_ordered_room_dispatch_loop_exit",
    "dungeon/runtime_ordered_room_dispatch_real_show_states_exit",
    "dungeon/runtime_guarded_inline_leaf_arm_skip_exit",
    "dungeon/runtime_nested_value_call_caller_local_guard_exit",
    "dungeon/runtime_threaded_mut_arg_interrupt_soak_exit",
    "calls/runtime_contained_call_value",
    "rewards/runtime_contained_reward_table_roll_item",
    "control_flow/runtime_nested_branch_assignment_prelude_value",
    "control_flow/runtime_nested_branch_prelude_value",
    "control_flow/runtime_nested_branch_value",
    "slices/runtime_dispatch_mutable_slice_element_write_compile",
    "slices/runtime_dispatch_mutable_slice_element_write_exit",
    "arithmetic/runtime_modulo_value",
    "control_flow/runtime_multi_assignment_value_calls",
    "control_flow/runtime_boolean_or_guard_exit",
    "control_flow/runtime_negated_boolean_place_guard_exit",
    "control_flow/runtime_negated_comparison_guard_exit",
    "dungeon/runtime_multi_room_reentry_exit",
    "slices/runtime_mutable_slice_element_write_straight_line_exit",
    "slices/runtime_mutable_slice_element_write_exit",
    "slices/guarded_slice_parameter_empty_false_index_compile",
    "slices/guarded_slice_parameter_empty_false_tail_compile",
    "slices/guarded_slice_parameter_bounded_subslice_compile",
    "slices/guarded_slice_parameter_end_subslice_compile",
    "slices/guarded_slice_parameter_end_equals_len_subslice_compile",
    "slices/guarded_slice_parameter_index_compile",
    "slices/guarded_slice_parameter_min_length_index_compile",
    "slices/guarded_slice_parameter_min_length_tail_compile",
    "slices/guarded_slice_parameter_nonempty_index_compile",
    "slices/guarded_slice_parameter_nonempty_tail_compile",
    "slices/guarded_slice_parameter_nonzero_index_compile",
    "slices/guarded_slice_parameter_nonzero_tail_compile",
    "slices/guarded_slice_parameter_start_equals_len_subslice_compile",
    "slices/guarded_slice_parameter_subslice_compile",
    "slices/guarded_slice_parameter_symmetric_false_guard_compile",
    "slices/guarded_slice_parameter_symmetric_true_guard_compile",
    "slices/guarded_slice_parameter_successor_index_compile",
    "slices/guarded_slice_parameter_successor_tail_compile",
    "slices/machine_field_index_initializer_compile",
    "slices/requires_field_count_alias_index_compile",
    "slices/requires_slice_parameter_bounded_subslice_compile",
    "slices/requires_slice_parameter_index_compile",
    "slices/requires_slice_parameter_successor_index_compile",
    "slices/slice_local_index_fact_compile",
    "slices/subslice_folded_bound_facts_compile",
    "slices/subslice_literal_bounds_compile",
    "slices/inclusive_subslice_literal_bounds_compile",
    "slices/inclusive_subslice_end_equals_len_minus_one_compile",
    "slices/full_range_subslice_compile",
    "slices/subslice_local_bound_facts_compile",
    "slices/subslice_range_surface_compile",
    "slices/window_shrink_exact_length_index_compile",
    "slices/window_shrink_unknown_base_index_compile",
    "slices/window_shrink_min_length_tail_index_compile",
    "slices/window_literal_bounds_min_length_parent_index_compile",
    "slices/window_subslice_within_exact_length_compile",
    "slices/disjoint_mut_subslice_windows_compile",
    "slices/runtime_subslice_range_len_exit",
    "slices/runtime_subslice_bounded_range_len_exit",
    "slices/runtime_subslice_bounded_dynamic_index_exit",
    "slices/runtime_subslice_dynamic_index_exit",
    "slices/runtime_subslice_end_dynamic_index_exit",
    "slices/runtime_nested_subslice_dynamic_index_exit",
    "slices/runtime_nested_subslice_fixed_index_exit",
    "slices/runtime_subslice_range_pointer_exit",
    "slices/termination_slice_length_compile",
    "slices/termination_slice_len_distance_compile",
    "slices/runtime_frame_array_slice_parameter_alias_exit",
    "slices/runtime_slice_index_copy_dispatch_exit",
    "slices/runtime_slice_index_copy_exit",
    "slices/runtime_slice_index_read_dispatch_exit",
    "slices/runtime_slice_index_read_exit",
    "slices/runtime_subslice_of_slice_param_exit",
    "slices/runtime_subslice_param_bounded_range_exit",
    "slices/runtime_subslice_param_end_only_exit",
    "slices/runtime_subslice_param_local_exit",
    "slices/runtime_subslice_runtime_start_exit",
    "slices/runtime_subslice_runtime_end_exit",
    "slices/runtime_subslice_nested_of_param_exit",
    "slices/runtime_subslice_runtime_start_over_local_exit",
    "slices/runtime_subslice_param_inclusive_end_exit",
    "rewards/runtime_reward_table_roll_item_shape",
    "dungeon/runtime_room_use_reentry_guard",
    "dungeon/runtime_room_use_reentry_exit",
    "text/runtime_text_storage",
    "text/runtime_stderr_write_exit",
    "text/runtime_stdin_command_branch_exit",
    "text/runtime_stdin_line_buffering_exit",
    "calls/runtime_trailing_local_return_exit",
    "calls/runtime_transition_subject_call_guard",
    "calls/runtime_transition_argument_call_value",
    "collections/std_option_storage_write",
    "collections/std_option_surface",
    "collections/runtime_fixed_vec_round_trip_exit",
    "core/array_core_surface",
    "core/fixed_vec_core_surface",
    "core/region_core_surface",
    "core/collections_text_core_surface",
    "core/nat_core_surface",
    "core/ptr_core_surface",
    "core/slice_core_surface",
    "core/str_core_surface",
    "core/vec_core_surface",
    "operators/core_operator_declaration_surface",
    "operators/core_boundary_operator_surface",
    "operators/domain_operator_declaration_surface",
    "operators/domain_operator_overload_signature_compile",
    "operators/root_operator_overload_signature_compile",
    "operators/core_operator_spelling_surface",
    "operators/slice_index_via_spelling_compile",
    "operators/accepted_core_provider_binding",
    "operators/unary_logical_not",
    "modules/module_declaration",
    "modules/package_declaration",
    "modules/pub_visibility_modifier",
    "ownership/move_keyword_field_assignment",
    "traits/trait_composition_satisfies",
    "traits/trait_declaration_bundle",
    "traits/trait_satisfies_machine_signature",
    "traits/equatable_record_equality_exit",
    "traits/equatable_mixed_shape_equality_exit",
    "traits/equatable_string_field_equality_exit",
    "traits/equatable_sum_payload_equality_exit",
    "termination/default_order_nat_countdown_compile",
    "termination/default_order_slice_length_compile",
    "termination/default_order_bounded_distance_compile",
    "termination/bounded_distance_named_view",
    "termination/default_order_unsigned_width_countdown_compile",
    "termination/runtime_shrinking_slice_recursion_exit",
    // --- Language-guide chapter coverage (Ch1-22) ---
    "calls/runtime_local_string_field_copy_through_mut_exit",
    "text/runtime_machine_string_append_in_place_exit",
    "text/runtime_string_concat_two_fields_exit",
    "text/runtime_chained_string_append_exit",
    "arithmetic/runtime_i64_full_width_exit",
    "arithmetic/runtime_chained_field_mutation_exit",
    "arithmetic/runtime_copy_then_read_exit",
    "arithmetic/runtime_signed_division_exit",
    "arithmetic/runtime_unsigned_division_exit",
    "arithmetic/runtime_min_max_signedness_exit",
    "arithmetic/runtime_comparison_value_signedness_exit",
    "arithmetic/runtime_comparison_guard_signedness_exit",
    "operators/unary_negation_exit",
    "operators/compound_assignment_exit",
    "expressions/runtime_match_value_exit",
    "expressions/runtime_float_constant_store_exit",
    "expressions/runtime_float_arithmetic_exit",
    "expressions/runtime_float_comparison_exit",
    "expressions/runtime_float_place_comparison_exit",
    "expressions/runtime_numeric_cast_exit",
    "calls/runtime_value_position_branching_call_exit",
    "calls/runtime_value_transition_unsigned_guard_exit",
    "calls/runtime_exit_code_exit",
    "operators/integer_literal_suffix_exit",
    "operators/runtime_shift_operators_exit",
    "calls/free_standing_machine_helper_compile",
    "calls/typed_return_from_local_call_compile",
    "capabilities/boundary_trait_multiple_effects",
    "capabilities/derives_authority_via_boundary",
    "capabilities/acquires_through_helper_return",
    "capabilities/derives_through_helper",
    "capabilities/provider_categories_all",
    "capabilities/invariant_parameterized_slice",
    "capabilities/string_domain_boundary_requirement",
    "capabilities/transitive_effect_inference",
    "capabilities/uses_caller_capability_requires",
    "constraints/multi_fact_contract_without_separators",
    "constraints/proof_machine_order_fact",
    "constraints/nat_proof_literal_suffix",
    "constraints/contract_range_membership_unimplemented",
    "constraints/scalar_ensures_field_contract_surface",
    "constraints/scalar_requires_satisfied_by_literal",
    "proofs/proof_constant_arithmetic_identity",
    "proofs/proof_order_transitivity",
    "proofs/proof_linear_range_sum",
    "proofs/proof_congruence_add_constant",
    "proofs/proof_addition_commutativity",
    "proofs/proof_nonlinear_square_range",
    "proofs/proof_order_antisymmetry",
    "proofs/proof_multiplication_distributivity",
    "proofs/proof_remainder_range",
    "proofs/proof_bag_view_reflexivity",
    "proofs/proof_inductive_gauss_sum",
    "proofs/proof_inductive_climbing_sum",
    "proofs/recursive_machine_with_requires_compiles",
    "control_flow/runtime_integer_literal_dispatch_exit",
    "control_flow/runtime_string_literal_dispatch_exit",
    "core/local_value_intro_compile",
    "core/self_read_only_receiver_compile",
    "domains/match_domain_patterns",
    "domains/match_interleaved_domain_data_guard",
    "drops/cleanup_machine_drop_shape",
    "drops/drop_ensures_domain_membership",
    "drops/drop_ensures_unlocked_predicate",
    "drops/machine_effects_annotation",
    "drops/transfer_cleanup_into_state",
    "errors/fallible_result_data_shape",
    "errors/host_failure_boundary_machine",
    "errors/trap_unrecoverable_statement",
    "expressions/float_literal_suffix",
    "expressions/integer_literal_suffix",
    "generics/const_data_param",
    "generics/const_machine_value_params",
    "generics/generic_data_instantiation",
    "generics/generic_data_type_param",
    "generics/generic_machine_call_monomorphization",
    "generics/generic_machine_multiple_type_params",
    "generics/generic_machine_type_param_signature",
    "generics/generic_machine_where_machine_requirement",
    "generics/generic_machine_where_trait_bound",
    "generics/generic_trait_type_param",
    "generics/generic_type_param_in_state",
    "generics/machine_bound_satisfied_at_call",
    "generics/property_bound_type_parameter",
    "inline_asm/asm_block_jmp_state",
    "memory/abi_calling_convention_machine",
    "memory/repr_native_stable_layout",
    "modules/use_imports_sibling_data",
    "modules/use_imports_sibling_trait",
    "operators/parenthesized_precedence_value",
    "operators/runtime_integer_division_value",
    "ownership/compound_assign_add_field",
    "ownership/copy_value_field_read_compile",
    "parameters/shared_and_mut_borrow_params_compile",
    "traits/generic_trait_parameter",
    "traits/trait_generic_bound_static_dispatch",
    "traits/trait_inferred_satisfaction",
    "traits/trait_invariant_clause",
    "traits/trait_method_ensures_clause",
    "traits/trait_oneoff_machine_requirement",
    "versioning/migration_generic_trait",
    "versioning/version_scoped_machine",
    "versioning/data_version_block",
    "versioning/migration_machine_from_v1",
    "versioning/runtime_version_migration_exit",
    "versioning/runtime_versioned_era_query_exit",
    "versioning/runtime_versioned_era_guard_exit",
    "versioning/runtime_versioned_match_zii_exit",
    "versioning/runtime_versioned_three_era_match_zii_exit",
    "versioning/version_chain_report",
    "versioning/version_chain_report_complete",
    "versioning/versioned_match_all_eras_exhaustive",
    "versioning/versioned_match_default_arm",
    "wire/wire_generic_trait",
    "wire/runtime_transform_machine_from_wire",
    "wire/runtime_transform_machine_to_wire",
    "wire/wire_data_field_numbers",
    "wire/wire_data_reserved_field",
    "wire/wire_data_version_block",
    "wire/wire_data_encoding_family",
    "wire/wire_multi_version_evolution",
    "wire/wire_field_references_program_types",
    "wire/wire_cross_era_type_change_migration",
    "wire/wire_cross_era_number_recycling",
    "wire/runtime_wire_encode_primitive_exit",
    "wire/runtime_wire_encode_era_discriminator_exit",
    "wire/runtime_wire_roundtrip_primitive_exit",
    "wire/runtime_wire_decode_rejects_wrong_era_exit",
    "wire/runtime_wire_encode_string_exit",
    "wire/runtime_wire_roundtrip_repeated_exit",
    "wire/runtime_wire_decode_rejects_repeated_overflow_exit",
    // --- 2026-06-12 canary coverage sweep (feature-edge additions) ---
    "wire/runtime_wire_roundtrip_repeated_max_one_exit",
    "wire/runtime_wire_encode_repeated_then_string_exit",
    "wire/runtime_wire_roundtrip_nested_and_repeated_exit",
    "comptime/runtime_const_array_length_transitive_exit",
    "data/property_send_declared",
    "data/property_zero_init_nested_array",
    "data/runtime_case_membership_mixed_shape_exit",
    "traits/runtime_equatable_scalar_not_equals_guard_exit",
    "borrow/runtime_view_of_view_chain_exit",
    "borrow/runtime_method_view_write_after_last_use_exit",
];

const ACTIVE_FAIL_CANARIES: &[&str] = &[
    "collections/fixed_vec_push_without_room",
    "collections/fixed_vec_get_past_length",
    "wire/duplicate_field_number",
    "wire/field_reuses_reserved_number",
    "wire/duplicate_version_declaration",
    "wire/unknown_field_type",
    "wire/version_field_retired_without_reserved",
    "wire/version_chain_retired_without_reserved",
    "wire/encode_unsupported_field_type",
    "wire/encode_string_field_not_last",
    "wire/decode_unsupported_field_type",
    "wire/encode_case_bearing_value",
    "wire/nested_schema_cycle",
    "wire/encode_nested_in_nested",
    "wire/repeated_string_element",
    "wire/repeated_nested_element",
    "wire/repeated_without_max",
    "wire/repeated_value_missing_count",
    "capabilities/unapproved_host_call",
    "comptime/effectful_const_array_length",
    "comptime/negative_const_array_length",
    "comptime/parameterized_const_array_length",
    "comptime/unknown_const_array_length",
    "data/bare_payload_case_equality_guard",
    "data/bare_payload_case_equality_suggests_in",
    "data/case_payload_equality_interim",
    "data/case_payload_malformed",
    "data/case_zero_payload",
    "data/enum_keyword_retired",
    "data/match_nonexhaustive_cases",
    "data/match_predicate_domain_needs_default",
    "data/mixed_common_field_default",
    "data/mixed_common_field_nonscalar",
    "data/mixed_payload_field_shadows_common",
    "data/mixed_record_literal",
    "data/property_copy_string_field",
    "data/property_copy_violation",
    "data/property_sized_declared",
    "data/property_unknown",
    "data/property_zero_init_nonzero_default",
    "generics/colon_bound_rejected",
    "generics/machine_bound_violated_at_call",
    "generics/property_bound_missing_on_field",
    "generics/property_bound_violated_at_instantiation",
    "domains/call_requires_invalidated_by_mutation",
    "domains/call_requires_domain_intersection_invalidated_by_mutation",
    "domains/call_requires_boolean_expression_invalidated_by_mutating_call",
    "domains/call_requires_dynamic_indexed_boolean_expression_invalidated_by_mutating_call",
    "domains/call_requires_domain_union_unproven",
    "domains/call_requires_fixed_indexed_boolean_expression_invalidated_by_mutating_call",
    "domains/call_requires_scalar_member_expression_invalidated_by_same_index_mutation",
    "domains/exit_ensures_boolean_expression_invalidated_by_mutating_call",
    "domains/exit_ensures_dynamic_indexed_boolean_expression_invalidated_by_mutating_call",
    "domains/exit_ensures_fixed_indexed_boolean_expression_invalidated_by_mutating_call",
    "domains/call_requires_unproven",
    "domains/call_requires_free_machine_value_unproven",
    "domains/call_requires_free_machine_statement_unproven",
    "domains/call_requires_boundary_trait_unproven",
    "domains/call_requires_platform_unproven",
    "domains/call_requires_domain_membership_invalidated_by_same_literal_element_call",
    "domains/exit_ensures_domain_union_unproven",
    "domains/exit_ensures_unproven",
    "ownership/assign_immutable_parameter",
    "borrows/borrow_duplicate_mut",
    "calls/discarded_call_result",
    "calls/discarded_trait_call_result",
    "calls/pure_discard_dead_code",
    "borrows/borrow_helper_alias_active",
    "borrows/borrow_local_alias_active",
    "borrows/borrow_local_alias_reborrow_active",
    "borrows/borrow_mut_and_read",
    "borrows/borrow_mut_literal",
    "borrows/borrow_same_fixed_index_call_mut",
    "borrows/borrow_same_fixed_index_mut",
    "borrows/borrow_same_fixed_index_slice_alias_mut",
    "borrows/borrow_unknown_index_pair_mut",
    "borrow/slice_view_invalidated_by_owner_write",
    "borrow/subslice_view_invalidated_by_owner_write",
    "borrow/string_view_invalidated_by_owner_write",
    "borrow/slice_view_invalidated_by_owner_call",
    "borrow/vec_view_invalidated_by_push",
    "borrow/free_machine_view_invalidated_by_linked_input_write",
    "borrow/view_return_ambiguous_ref_inputs",
    "concurrency/barrier_wait_contract",
    "concurrency/mutex_lock_guard",
    "concurrency/spawn_borrow_capture",
    "concurrency/spawn_self_capture",
    "concurrency/spawn_statement_block",
    "control_flow/bare_machine_arrow_transition",
    "control_flow/bare_state_arrow_transition",
    "inline_asm/asm_label_loop",
    "inline_asm/asm_structured_ldr_str",
    "inline_asm/asm_where_contract",
    "control_flow/termination_countdown_stalled_decrease",
    "control_flow/termination_cycle_missing_decreases",
    "termination/custom_ranking_field_stalled_decrease",
    "termination/custom_ranking_order_non_numeric",
    "termination/custom_ranking_order_parameter_mismatch",
    "termination/custom_ranking_order_unknown",
    "termination/custom_ranking_order_wrong_arity",
    "termination/mutual_recursion_no_decrease",
    "slices/dynamic_subslice_bounded_unproven",
    "slices/dynamic_subslice_end_unproven",
    "slices/dynamic_subslice_start_unproven",
    "slices/invalid_fixed_array_literal_index_unchecked",
    "slices/known_length_dynamic_index_unproven",
    "slices/machine_field_index_reassigned_unproven",
    "slices/invalid_slice_folded_index_unchecked",
    "slices/invalid_slice_local_index_unchecked",
    "slices/invalid_subslice_folded_bounds_unchecked",
    "slices/invalid_subslice_bounded_end_unchecked",
    "slices/invalid_subslice_bounded_order_unchecked",
    "slices/invalid_subslice_bounds_unchecked",
    "slices/invalid_subslice_end_bounds_unchecked",
    "slices/invalid_inclusive_subslice_end_at_len_unchecked",
    "slices/invalid_slice_literal_index_unchecked",
    "slices/invalid_slice_reassigned_local_index_unchecked",
    "slices/slice_parameter_index_unproven",
    "slices/slice_parameter_literal_index_unproven",
    "slices/slice_parameter_literal_subslice_unproven",
    "slices/slice_parameter_subslice_unproven",
    "slices/guarded_slice_parameter_bounded_subslice_order_unproven",
    "slices/guarded_slice_parameter_index_equals_len_compile",
    "slices/unguarded_slice_parameter_end_subslice_compile",
    "slices/unguarded_slice_parameter_index_compile",
    "slices/unguarded_slice_parameter_subslice_compile",
    "slices/window_shrink_index_out_of_length",
    "slices/window_subslice_end_over_exact_length",
    "slices/window_shrink_min_length_tail_index_unproven",
    "slices/window_literal_bounds_min_length_parent_index_unproven",
    "slices/window_reassigned_shrunk_floor_unproven",
    "slices/subslice_range_operator_contract_unproven",
    "slices/index_operator_contract_unproven",
    "slices/overlapping_mut_subslice_windows_rejected",
    "slices/unknown_bounds_mut_subslice_windows_rejected",
    "slices/termination_slice_length_order_unimplemented",
    "domains/domain_import_cycle",
    "domains/domain_import_unknown",
    "domains/domain_import_wrong_target",
    "domains/domain_non_boolean_fact",
    "domains/domain_operator_competing_spelling_meanings",
    "domains/domain_operator_meaning_unproven",
    "domains/domain_operator_meaning_invalidated_by_mutation",
    "domains/domain_operator_requires_unproven",
    "domains/indexed_domain_requires_invalidated_by_same_index_mutation",
    "domains/indexed_domain_requires_invalidated_by_unknown_index_mutation",
    "operators/domain_operator_alpha_equivalent_generic_duplicate",
    "operators/domain_operator_duplicate",
    "operators/domain_operator_reordered_generic_duplicate",
    "operators/domain_operator_return_only_overload",
    "operators/root_operator_alpha_equivalent_generic_duplicate",
    "operators/root_operator_reordered_generic_duplicate",
    "operators/root_operator_return_only_overload",
    "operators/root_operator_duplicate",
    "operators/duplicate_spelling_binding",
    "operators/app_package_provider_rejected",
    "operators/unregistered_provider_binding",
    "calls/runtime_helper_ordering_return",
    "traits/equatable_missing_conformance_suggested",
    "traits/equatable_field_not_equatable",
    "traits/equatable_recursive_type",
    "traits/equatable_string_field_literal_compare",
    "traits/trait_composition_missing_requirement",
    "traits/trait_requirement_cycle",
    "traits/trait_requires_unknown",
    "traits/trait_satisfies_missing_machine",
    "traits/trait_satisfies_parameter_mismatch",
    "traits/trait_satisfies_unknown",
    "traits/trait_unknown_signature_type",
    "termination/default_order_ambiguous",
    "termination/default_order_declared_measure_not_inferred",
    "termination/bounded_distance_inverted",
    "termination/subtraction_spelling_retired",
    // --- Language-guide chapter coverage (Ch1-22) ---
    "calls/terminal_return_type_mismatch_rejected",
    "capabilities/duplicate_provider_declaration",
    "capabilities/effect_ceiling_exceeded",
    "capabilities/effect_outside_trait_requirement",
    "capabilities/unknown_effect_name",
    "capabilities/unknown_provider_category",
    "constraints/scalar_requires_unproven_literal",
    "proofs/constant_equation_refuted",
    "proofs/order_asymmetry_refuted",
    // The eight false twins promoted from pending/proofs/ when the entailment
    // engine landed (they were moved on disk but never registered here).
    "proofs/order_transitivity_false_twin",
    "proofs/linear_range_sum_false_twin",
    "proofs/congruence_false_twin",
    "proofs/addition_commutativity_false_twin",
    "proofs/nonlinear_square_range_false_twin",
    "proofs/order_antisymmetry_false_twin",
    "proofs/remainder_range_false_twin",
    "proofs/bag_view_false_twin",
    "proofs/inductive_gauss_sum_false_twin",
    "proofs/inductive_gauss_sum_step_false_twin",
    "proofs/inductive_climbing_sum_step_false_twin",
    "drops/drop_nonblocking_effect_unknown",
    "modules/ambiguous_imported_data",
    "modules/use_unresolved_path",
    "traits/trait_satisfies_arity_mismatch",
    "versioning/match_on_version",
    "versioning/duplicate_version_declaration",
    "versioning/version_field_unknown_type",
    "versioning/version_scoped_machine_undeclared_version",
    "versioning/nested_version_block",
    "versioning/non_canonical_version_name",
    "versioning/cross_version_field_access",
    "versioning/versioned_match_unknown_era",
    "versioning/versioned_era_write",
    "versioning/versioned_container_unversioned_payload",
    "versioning/versioned_redeclared",
    // --- 2026-06-12 canary coverage sweep (feature-edge additions) ---
    "versioning/versioned_match_wrong_type_arm",
    "versioning/versioned_match_missing_current_arm",
    "data/property_send_case_payload_string",
    "data/property_zero_init_array_element_violation",
    "comptime/const_array_length_index_out_of_bounds",
    "comptime/fuel_exhausted_const_array_length",
    "borrow/method_view_receiver_unrelated_field_write",
    "concurrency/spawn_join_result_discarded",
];

#[derive(Clone, Copy)]
#[allow(dead_code)]
enum PendingCanaryExpectation {
    CurrentlyAccepts,
    CurrentlyRejects { fragment: &'static str },
}

#[allow(dead_code)]
struct PendingCanary {
    path: &'static str,
    expectation: PendingCanaryExpectation,
}

// fixed_array_element_guard was promoted to pass/ once the guard-operand layout
// applied the constant array index (see fixed_array_element_guard_canary_runs and
// runtime_fixed_array_field_guard_exit_canary_runs).
//
// The proofs false twins were promoted to fail/proofs/ when the contract
// entailment engine (omega-validation/src/contract_entailment.rs) landed:
// empty-body proof machines whose contracts lie inside the engine's language
// are now PROVED or REJECTED, never silently accepted. The pass/proofs/
// ladder pins the proving side; the rungs map to engine increments in
// wiki/proof_engine_roadmap.md.
//
// case_payload_native_construction was promoted to pass/data/ when native case
// payload codegen landed (tag-prefix write + payload field writes + tag-only
// guard compares + payload member reads); the compiler-side lowering gate was
// removed with it.
// machine_bound_value_call_unchecked: decision 13 residue frontier. The
// machine-call type-parameter bound check lives in `validate_call_node`
// (omega-validation/src/calls.rs), which only sees STATEMENT-position calls;
// a VALUE-position call (`let r = self.pick(&self.h)`) bypasses argument
// validation entirely, so its `[copy]` bound is not enforced yet. Promote to
// fail/generics/ when value-position calls gain argument validation.
// versioned_match_missing_current_arm was promoted to fail/versioning/ when
// version-match exhaustiveness counting landed (the decidable arm set of a
// `Versioned<T>` subject is {each declared era vN} + {current};
// crate::exhaustiveness in omega-symbol-resolved-trees-to-typed-trees).
// 2026-06-12 canary coverage sweep additions:
// - traits/equatable_string_not_equals_value: NATIVE MISCOMPILE -- `!=` over
//   an Equatable record with a String field drops the String term (native
//   exits 71 where the interpreter exits 70). `==` is right; scalar `!=` is
//   right.
// - traits/equatable_string_equality_guard_unlowered: guard-position
//   structural `==` with a String field rejects (16-byte runtime operand)
//   instead of reusing the value-position text-equals lowering.
// - comptime/const_array_length_bare_call_arm: a parenthesized BARE call as
//   the const callee's value arm fails const-eval resolution ("transition
//   target not found"); the same call wrapped in arithmetic evaluates fine.
// - concurrency/spawn_struct_result_miscompiled: NATIVE MISCOMPILE -- a
//   spawned machine returning a struct by value joins garbage (native 71,
//   interpreter 70); reproduces WITHOUT spawn too (plain by-value struct
//   return), same family as the by-value-struct-arg gap.
#[allow(dead_code)]
const ACTIVE_PENDING_CANARIES: &[PendingCanary] = &[
    PendingCanary {
        path: "generics/machine_bound_value_call_unchecked",
        expectation: PendingCanaryExpectation::CurrentlyAccepts,
    },
    PendingCanary {
        path: "traits/equatable_string_not_equals_value",
        expectation: PendingCanaryExpectation::CurrentlyAccepts,
    },
    PendingCanary {
        path: "traits/equatable_string_equality_guard_unlowered",
        expectation: PendingCanaryExpectation::CurrentlyRejects {
            fragment: "cannot load 16-byte runtime operands",
        },
    },
    PendingCanary {
        path: "comptime/const_array_length_bare_call_arm",
        expectation: PendingCanaryExpectation::CurrentlyRejects {
            fragment: "transition target `burn` not found in current machine",
        },
    },
    PendingCanary {
        path: "concurrency/spawn_struct_result_miscompiled",
        expectation: PendingCanaryExpectation::CurrentlyAccepts,
    },
];
