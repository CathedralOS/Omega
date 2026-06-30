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

// Atomics end-to-end across architectures. The host (windows_x64) RUNS the
// program (fetch_add + compare_exchange, exit 70). aarch64 cannot execute on
// this box, so the linux_arm64 build is verified by the emitted ELF carrying the
// real LSE atomic instructions: LDADDAL (fetch_add) and CASAL (compare_exchange).
#[test]
fn atomics_cross_platform_emits_real_atomics() {
    let sample = repo_root().join("samples").join("atomics_cross");
    let main_path = sample.join("main.omg");

    // --- windows_x64: compile + run ---
    let win_dir = std::env::temp_dir().join(format!("omega-atomics-win-{}", std::process::id()));
    let _ = fs::remove_dir_all(&win_dir);
    compile(CompileOptions {
        root_path: main_path.clone(),
        build_dir: Some(win_dir.clone()),
        target_name: Some("windows_x64".to_owned()),
        write_output: true,
    })
    .expect("atomics_cross should compile for windows_x64");
    let output = Command::new(win_dir.join("omega-program.exe"))
        .output()
        .expect("windows_x64 atomics_cross should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected fetch_add(5) old==10/counter==15 then compare_exchange(15,99) \
         prior==15/counter==99 (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&win_dir);

    // --- linux_arm64: cross-emit + disassemble-by-bytes ---
    let arm_dir = std::env::temp_dir().join(format!("omega-atomics-arm-{}", std::process::id()));
    let _ = fs::remove_dir_all(&arm_dir);
    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(arm_dir.clone()),
        target_name: Some("linux_arm64".to_owned()),
        write_output: true,
    })
    .expect("atomics_cross should compile for linux_arm64");
    let elf = fs::read(arm_dir.join("omega-program")).expect("linux_arm64 ELF should be emitted");

    assert_eq!(
        u16::from_le_bytes([elf[18], elf[19]]),
        183,
        "e_machine should be EM_AARCH64"
    );
    // LDADDAL w17, wzr, [x16] = 0xB8F1021F (fetch_add; prior discarded into WZR).
    assert!(
        elf.windows(4).any(|w| w == [0x1f, 0x02, 0xf1, 0xb8]),
        "linux_arm64 ELF should contain LDADDAL w17,wzr,[x16] for fetch_add"
    );
    // CASAL w26, w17, [x16] = 0x88FAFE11 (compare_exchange).
    assert!(
        elf.windows(4).any(|w| w == [0x11, 0xfe, 0xfa, 0x88]),
        "linux_arm64 ELF should contain CASAL w26,w17,[x16] for compare_exchange"
    );
    let _ = fs::remove_dir_all(&arm_dir);
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
fn value_call_as_host_arg_rejected_canary_is_rejected() {
    // A value-call result (or any non-trivial computed value) used directly as a host-call
    // argument is not yet encodable; the selector rejects it cleanly rather than silently
    // miscompiling (#40). Workaround: bind to a field first. Guards the value-call positions
    // that ARE cleanly rejected against the value-call-in-guard silent-miscompile failure mode.
    let canary = fail_canary("calls/value_call_as_host_arg_rejected");
    let diagnostics = match compile_canary_without_output(&canary) {
        Ok(report) => panic!(
            "expected value-call-as-host-arg canary to reject, but it compiled: {}",
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
        combined.contains("encodable"),
        "expected a 'no encodable call selection' rejection diagnostic, got:\n{combined}"
    );
}

#[test]
fn cast_in_guard_rejected_canary_is_rejected() {
    // A cast in a guard subject is not yet lowerable; the dispatch-guard blocker rejects it
    // cleanly rather than silently miscompiling (#40). Workaround: cast into a field first,
    // then guard the field. Sibling of shift_in_guard_rejected; guards against the value-call
    // -in-guard silent-miscompile failure mode reaching casts. Remove when casts-in-guards land.
    let canary = fail_canary("arithmetic/cast_in_guard_rejected");
    let diagnostics = match compile_canary_without_output(&canary) {
        Ok(report) => panic!(
            "expected cast-in-guard canary to reject, but it compiled: {}",
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
        combined.contains("guard"),
        "expected a guard-lowering rejection diagnostic, got:\n{combined}"
    );
}

#[test]
fn shift_in_guard_rejected_canary_is_rejected() {
    // A shift (<< / >>) directly in a guard subject is not yet lowerable; the dispatch-guard
    // blocker rejects it cleanly rather than silently miscompiling (#40). Value-position
    // shifts work (bind the shifted value to a local first). This guards the #40 boundary:
    // if shift-in-guard ever compiles, the canary fails. Remove when shifts-in-guards land.
    let canary = fail_canary("arithmetic/shift_in_guard_rejected");
    let diagnostics = match compile_canary_without_output(&canary) {
        Ok(report) => panic!(
            "expected shift-in-guard canary to reject, but it compiled: {}",
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
        combined.contains("guard"),
        "expected a guard-lowering rejection diagnostic, got:\n{combined}"
    );
}

#[test]
fn u64_literal_above_i64_max_canary_is_rejected() {
    // A u64 literal above i64::MAX (the literal value is carried as i64 through the IR) is
    // rejected with a CLEAR "exceeds the i64 range" diagnostic that names the real
    // limitation, not the misleading "invalid integer literal". Remove this test when
    // full-width u64 literals land (the i128 literal-widening fix).
    let canary = fail_canary("arithmetic/u64_literal_above_i64_max");
    let diagnostics = match compile_canary_without_output(&canary) {
        Ok(report) => panic!(
            "expected u64-literal-too-large canary to reject, but it compiled: {}",
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
        combined.contains("exceeds the i64 range"),
        "expected a clear i64-range-overflow diagnostic (not 'invalid integer literal'), got:\n{combined}"
    );
}

#[test]
fn computed_index_operand_canary_is_rejected() {
    // `arr[k + 1]` as a value operand silently read 0 (computed index not lowerable);
    // even with `k + 1`'s bound explicitly guarded it must be refused, not miscompiled.
    let canary = fail_canary("collections/computed_index_operand_rejected");
    let diagnostics = match compile_canary_without_output(&canary) {
        Ok(report) => panic!(
            "expected computed-index canary to reject, but it compiled: {}",
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
        combined.contains("is a computed expression"),
        "expected computed-index diagnostic, got:\n{combined}"
    );
}

#[test]
fn indexed_write_from_indexed_read_canary_is_rejected() {
    // `nums[i] = nums[j]` with both indices runtime silently copied the array base
    // (exit 10 not 50). A blocker-level stopgap now refuses it; this pins that it
    // errors rather than miscompiles. Sound workaround: a field temp.
    let canary = fail_canary("collections/indexed_write_from_indexed_read_rejected");
    let diagnostics = match compile_canary_without_output(&canary) {
        Ok(report) => panic!(
            "expected indexed-write-from-indexed-read canary to reject, but it compiled: {}",
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
        combined.contains("runtime-indexed element from a runtime-indexed read"),
        "expected dual-runtime-indexed diagnostic, got:\n{combined}"
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

// #66 B1c / GAP #2 (value-call arg descriptor): a string literal passed to a
// `&[u8] in Utf8` parameter materializes a correct `{ptr, len}` slice descriptor
// so the callee's `text.len` reads the literal's byte length 5 (exit 70). The
// value-call mechanism and arg passing already worked; the gap was the
// param-slice `.len` value-source read in the value-call splice path.
#[test]
fn utf8_literal_len_exit_canary_runs() {
    let canary = pass_canary("domains/utf8_literal_len_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-utf8-literal-len-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("utf8 literal len canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("utf8 literal len canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a string literal passed to a `&[u8] in Utf8` param to read len 5 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66: the literal-grant mechanism is GENERAL -- not hardcoded to the `Utf8`
// domain name. A USER domain `[u8]::Ascii` with a DIFFERENT classifier predicate
// (`ascii_only(self)`) grants an ASCII string literal its domain, discharging the
// param's `in Ascii` requirement; under the old `name == "Utf8"` hardcode this
// would have failed. The literal also flows as a real `&[u8]` view, so
// `measure("hi")` reads len 2 and exits 70.
#[test]
fn user_domain_literal_grant_canary_runs() {
    let canary = pass_canary("domains/user_domain_literal_grant");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-user-domain-grant-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("user-domain literal grant canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("user-domain literal grant canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an ASCII literal to be granted a user `[u8]::Ascii` domain via its \
         `ascii_only(self)` classifier and read len 2 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 GAP #4 (slice-`.len`-to-field write): a `&[u8] in Utf8` PARAM is a runtime
// `{ptr, len}` descriptor in a frame slot, so `self.result = text.len` reads the
// descriptor's len field (NOT a compile-time constant -- that is GAP #2). This
// already lowers via the `<root>.len`-over-a-descriptor-slot value-source; the
// canary pins it end-to-end. `store("hello")` records 5; the caller guards == 5.
#[test]
fn utf8_param_len_field_exit_canary_runs() {
    let canary = pass_canary("domains/utf8_param_len_field_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-utf8-param-len-field-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("utf8 param len field canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("utf8 param len field canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.result = text.len` of a `&[u8] in Utf8` param to record len 5 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 GAP #1 (regular-call arg descriptor) + GAP #3 (sub-state requires flow): a
// string literal passed to a `&[u8] in Utf8` param in a REGULAR statement call
// materializes a correct `{ptr, len}` descriptor (String/slice share the fat
// layout, so the existing String-literal frame-slot writer populates the slot),
// and the callee's synthesized `requires text in [u8]::Utf8` is assumed once on
// entry -- NOT re-imposed at the internal `true -> ok() _ -> nope()` sub-state
// dispatch. `check("hello")` sees len 5 and exits 70.
#[test]
fn utf8_regular_call_len_exit_canary_runs() {
    let canary = pass_canary("domains/utf8_regular_call_len_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-utf8-regular-call-len-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("utf8 regular call len canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("utf8 regular call len canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a regular call passing a string literal to a `&[u8] in Utf8` param to read len 5 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 (`&[u8] in Utf8 == "literal"` content compare): a domained-slice-view value
// `==` a string literal lowers through the SAME TextEquals leaf String uses, NOT a
// scalar compare of the descriptor's pointer words. `classify("quit")` matches and
// exits 70; the interpreter agrees (differential). Before this, the guard fell to
// the generic scalar path: native compared the descriptor's POINTER words and took
// the wrong arm, silently diverging from the interpreter's content equality.
#[test]
fn utf8_equals_literal_exit_canary_runs() {
    let canary = pass_canary("domains/utf8_equals_literal_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-utf8-equals-literal-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("utf8 equals literal canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("utf8 equals literal canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `&[u8] in Utf8 == \"quit\"` content equality to match and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 (`&[u8] in Utf8 == &[u8] in Utf8` content compare): comparing two
// domained-slice views lowers through the TextEquals content leaf. `cmp("Gate",
// "Gate")` matches and exits 70; the interpreter agrees. Before this, the generic
// scalar path tried to load the 16-byte descriptor as a runtime operand (the
// encoder rejects it) and compared only the pointer words.
#[test]
fn utf8_equals_view_exit_canary_runs() {
    let canary = pass_canary("domains/utf8_equals_view_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-utf8-equals-view-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("utf8 equals view canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("utf8 equals view canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `&[u8] in Utf8 == &[u8] in Utf8` content equality to match and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 read-narrowing: a DECLARED-domain field (`out: &[u8] in Utf8`) carries its
// `in Utf8` fact when READ. `self.out = "Gate"` (write-enforced) then
// `self.check(self.out)` passes the field read to a `&[u8] in Utf8` parameter --
// which only discharges because the read carries the field's declared domain.
// `check` guards `text == "Gate"` and exits 70; the interpreter agrees. Before the
// read-narrowing fix this rejected with "cannot prove requires contract ...
// self.out in [u8]::Utf8".
#[test]
fn utf8_field_read_carries_domain_exit_canary_runs() {
    let canary = pass_canary("domains/utf8_field_read_carries_domain_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-utf8-field-read-domain-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("utf8 field-read domain canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("utf8 field-read domain canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the declared-domain field read `self.out` to carry `in Utf8` so \
         `self.check(self.out)` discharges and `text == \"Gate\"` exits 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 soundness-gate COMPLETENESS: an empty-violating domain (`[u8]::NonEmpty`,
// `non_empty(self)`) gets NO machine-entry field-invariant (the empty/ZII value
// violates it -- see fail canary domain_field_read_no_write_unproven), but after
// an ENFORCED write the re-established fact still flows to a read. `self.f = "x"`
// stores a literal accepted by the write-enforcement construction-grant
// (non-empty bytes); the subsequent `self.check(self.f)` read carries the
// re-established `in NonEmpty` and discharges the `&[u8] in NonEmpty` parameter.
// `check` guards `text == "x"` and exits 73; the interpreter agrees.
#[test]
fn domain_field_write_then_read_exit_canary_runs() {
    let canary = pass_canary("domains/domain_field_write_then_read_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-domain-field-write-then-read-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("domain field write-then-read canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("domain field write-then-read canary should run");

    assert_eq!(
        output.status.code(),
        Some(73),
        "expected the enforced write `self.f = \"x\"` to re-establish `in NonEmpty` so the \
         read `self.check(self.f)` discharges and `text == \"x\"` exits 73, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 owned `[u8; N] in Utf8` bounded byte carrier, end to end on native: the
// literal write materializes into the carrier's `{len, bytes}` inline storage
// (the carrier OWNS its bytes -- not a {ptr,len} descriptor aliasing rodata),
// and the `==` guard reads it back with carrier addressing (len @ 0, bytes @
// pointer_size) and content-compares. `self.label == "Gate"` matches -> exit 70.
// `[u8; 8]` is the 16-byte case (8 + 8 == the string descriptor size) that the
// String text-write pass would otherwise claim as a descriptor.
#[test]
fn runtime_bounded_carrier_write_read_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_write_read_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-bounded-carrier-write-read-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("bounded carrier write-read canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bounded carrier write-read canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the owned `[u8; 8] in Utf8` carrier to write `\"Gate\"` into its inline \
         {{len, bytes}} storage and read it back so `self.label == \"Gate\"` exits 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 owned `[u8; N] in Utf8` carrier `.len` as a marshaled host-call argument:
// `self.message = "ALERT " + self.label` builds a length-10 carrier, and
// `exit_process(self.message.len)` reads the carrier's length word (at the
// carrier's own offset 0, not a fat-slice descriptor's `+pointer_size`) -> exit 10.
// `.len` already resolved in guards; this exercises it in value/argument position.
#[test]
fn runtime_bounded_carrier_length_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_length_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-bounded-carrier-length-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("bounded carrier length canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bounded carrier length canary should run");

    assert_eq!(
        output.status.code(),
        Some(10),
        "expected `exit_process(self.message.len)` to read the carrier's length word \
         (\"ALERT temp\" = 10), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 owned `[u8; N] in Utf8` carrier `.len` as a MUTATION-WRITE value:
// `self.count = self.message.len` reads the length-10 carrier's length word into a
// plain i32 field (a 4-byte read narrowing exactly into the i32 target), then exits
// the field -> 10. Covers the mutation value-operand consumer of the shared
// resolver's carrier-`.len` resolution (the host-call consumer is _length_exit).
#[test]
fn runtime_bounded_carrier_length_field_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_length_field_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-bounded-carrier-length-field-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("bounded carrier length field canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bounded carrier length field canary should run");

    assert_eq!(
        output.status.code(),
        Some(10),
        "expected `self.count = self.message.len` to store the carrier length (10) \
         into the i32 field, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 owned `[u8; N] in Utf8` carrier byte indexing in guard subjects:
// `message[i]` reads the byte at `base + pointer_size + i` (content after the
// length word, u8 elements). The compound guard `message[0] == 'A' &&
// message[2] == 'E'` reads two bytes of "ALERT"; both hold -> ok arm exits 70.
// (Indexing in guards is the parsing workhorse; widening a byte's value into a
// wider int, e.g. exiting it, needs a separate u8->i32 zero-extension still TODO.)
#[test]
fn runtime_bounded_carrier_byte_index_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_byte_index_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-bounded-carrier-byte-index-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("bounded carrier byte index canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bounded carrier byte index canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the compound byte-index guard `message[0]=='A' && message[2]=='E'` \
         to hold and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_carrier_indexed_read_exit_canary_runs() {
    let canary = pass_canary("text/runtime_carrier_indexed_read_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-carrier-indexed-read-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("carrier indexed read canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("carrier indexed read canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.text[self.i]` (runtime index on a [u8;N] carrier) to read 'a'/'c' past the len prefix and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_number_to_decimal_exit_canary_runs() {
    // Numeric output (itoa): build n=12345 at runtime, render it to the decimal text
    // "12345" via divide/modulo + computed carrier byte writes, and assert the
    // carrier equals it. A round-trip proving printable numbers, a serious-app need.
    let canary = pass_canary("text/runtime_number_to_decimal_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-number-to-decimal-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("number-to-decimal canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("number-to-decimal canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected integer->decimal-text round-trip to produce \"12345\" and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_decimal_to_number_exit_canary_runs() {
    // Numeric input (atoi): parse the decimal text "12345" into the integer 12345 via
    // carrier byte reads + accumulation, and assert it. The read-side complement of
    // the itoa canary -- carrier reads used as arithmetic operands.
    let canary = pass_canary("text/runtime_decimal_to_number_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-decimal-to-number-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("decimal-to-number canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("decimal-to-number canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected decimal-text->integer parse of \"12345\" to yield 12345 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_carrier_indexed_write_exit_canary_runs() {
    let canary = pass_canary("text/runtime_carrier_indexed_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-carrier-indexed-write-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("carrier indexed write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("carrier indexed write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.out[self.i] = self.ch` (runtime index on a [u8;N] carrier, runtime value) to write bytes past the len prefix and read 2 back at index 2 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_carrier_indexed_read_operand_exit_canary_runs() {
    let canary = pass_canary("text/runtime_carrier_indexed_read_operand_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-carrier-read-operand-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("carrier indexed-read-operand canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("carrier indexed-read-operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a carrier indexed read in operand position (`sum + self.text[self.i] as u32`, temp typed `u8` not `u8 in Utf8`) to sum 'ABCD' to 266 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_carrier_cipher_exit_canary_runs() {
    let canary = pass_canary("text/runtime_carrier_cipher_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-carrier-cipher-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("carrier cipher canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("carrier cipher canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a Caesar-cipher loop (read `text[i]` in an expression, Wrapping-shift, write `out[i]`) to map \"HELLO\" to \"KHOOR\" and read it back (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_carrier_indexed_const_write_exit_canary_runs() {
    let canary = pass_canary("text/runtime_carrier_indexed_const_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-carrier-const-write-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("carrier const-write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("carrier const-write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a CONSTANT byte written into a carrier at a RUNTIME index (`self.out[self.i] = 88`) to respect the index at both ends (out[0] and out[3]) (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_carrier_len_guard_exit_canary_runs() {
    let canary = pass_canary("text/runtime_carrier_len_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-carrier-len-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("carrier len guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("carrier len guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a carrier `.len` guard to actually evaluate (len==3 true, len==9 false; exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_carrier_fnv_loop_exit_canary_runs() {
    let canary = pass_canary("text/runtime_carrier_fnv_loop_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-carrier-fnv-loop-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("carrier fnv loop canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("carrier fnv loop canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected FNV-1a over a carrier string (`.len`-bounded loop + byte reads) to hash 'abc' to 11 and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mandelbrot_render_exit_canary_runs() {
    let canary = pass_canary("text/runtime_mandelbrot_render_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-mandelbrot-render-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("mandelbrot render canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("mandelbrot render canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the 40x18 Mandelbrot renderer (f64 escape-time over a 1D carrier framebuffer) to produce 140 in-set pixels and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_crc32_exit_canary_runs() {
    // CRC-32 (the ZIP/PNG/Ethernet checksum): polynomial division over GF(2), bit by bit
    // with shifts + XOR (reflected poly 0xEDB88320, init/final 0xFFFFFFFF). CRC-32("abc")
    // is 891568578, verified against zlib -> exit 70.
    let canary = pass_canary("text/runtime_crc32_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-crc32-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("crc32 canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("crc32 canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected CRC-32(\"abc\") == 891568578 (exit 70); got {:?} -- a shift/XOR or u32 regression\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_base64_encode_exit_canary_runs() {
    // Base64 encoding: three input bytes regrouped into four 6-bit values (shifts + masks +
    // OR), each indexing the 64-char alphabet. "Man" -> "TWFu", all four bytes checked ->
    // exit 70.
    let canary = pass_canary("text/runtime_base64_encode_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-base64-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("base64 encode canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("base64 encode canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected base64(\"Man\") == \"TWFu\" (exit 70); got {:?} -- a bit-op regression\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_run_length_encode_exit_canary_runs() {
    // Run-length encoding (compression): scan counting consecutive equal bytes, emit
    // byte+count at each run boundary and at the end (shared emit dispatched by a mode
    // field). "aaabbbbcc" -> "a3b4c2", six output bytes checked -> exit 70.
    let canary = pass_canary("text/runtime_run_length_encode_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-rle-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("run-length encode canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("run-length encode canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected RLE of \"aaabbbbcc\" to be \"a3b4c2\" (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_binary_format_exit_canary_runs() {
    // Format a number as an 8-bit binary string: `(n >> (7-i)) & 1` per bit (runtime shift
    // amount + bitwise AND in value position), written to a carrier. 42 -> "00101010",
    // all eight bytes checked -> exit 70.
    let canary = pass_canary("text/runtime_binary_format_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-binary-format-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("binary format canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("binary format canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected 42 to format as binary \"00101010\" (exit 70); got {:?} -- a shift/bitwise regression\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_substring_search_exit_canary_runs() {
    // Naive substring search (find a needle in a haystack): nested loop, carrier byte
    // comparison, the index guarded against `.len` directly. "world" in "hello world"
    // rejects i=0..5 and matches at i=6 -> exit 70.
    let canary = pass_canary("text/runtime_substring_search_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-substring-search-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("substring search canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("substring search canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected substring search to find \"world\" at position 6 (exit 70); got {:?} (a non-70 code is the wrong position)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_string_palindrome_exit_canary_runs() {
    let canary = pass_canary("text/runtime_string_palindrome_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-string-palindrome-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("string palindrome canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("string palindrome canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a two-pointer string palindrome check -- text[i] proved via the relational chain (i <= j < len), bytes compared through local temps -- to detect 'ABCBA' (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_carrier_itoa_exit_canary_runs() {
    let canary = pass_canary("text/runtime_carrier_itoa_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-carrier-itoa-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("carrier itoa canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("carrier itoa canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `itoa` (computed digit chars written into a carrier) to render 150 as \"150\" and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 owned `[u8; N] in Utf8` carrier byte WRITE `self.buffer[i] = <byte>`: the byte
// stores inline at `base + pointer_size + i`. Both a byte literal (`buffer[0] = 67`
// = 'C') and a u8 field (`buffer[1] = self.ch` = 'D') work; from "AB" the writes
// yield "CD" -> `==` exits 70.
#[test]
fn runtime_bounded_carrier_byte_write_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_byte_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-bounded-carrier-byte-write-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("bounded carrier byte write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bounded carrier byte write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected indexed byte writes (literal 'C' + u8-field 'D') to turn \"AB\" into \
         \"CD\" so `==` exits 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A runtime slice `.len` (descriptor read of a slice PARAM, not a folded
// fixed-array constant) narrows into an i32 field: `self.count = s.len` where
// `s: &[i32]` -> exit 5 for a 5-element view. The length value is 32-bit, so its
// low 4-byte word lowers into the i32 target (an 8-byte read does not) -- the same
// width convention as carrier `.len`.
#[test]
fn runtime_slice_length_field_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_slice_length_field_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-slice-length-field-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("slice length field canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("slice length field canary should run");

    assert_eq!(
        output.status.code(),
        Some(5),
        "expected `self.count = s.len` to store the slice param's length (5) into \
         the i32 field, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 owned `[u8; N] in Utf8` carrier builder/concat, native: `self.text =
// "Room " + self.label` materializes into the target carrier's inline storage --
// the first literal initializes it, then the source carrier's content is appended
// (running offset + running len). `self.text == "Room A1"` matches -> exit 70.
#[test]
fn runtime_bounded_carrier_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_concat_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-bounded-carrier-concat-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("bounded carrier concat canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bounded carrier concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the owned `[u8; 16] in Utf8` carrier to materialize `\"Room \" + self.label` \
         into its inline storage so `self.text == \"Room A1\"` exits 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 owned `[u8; N] in Utf8` carrier MULTI-SEGMENT concat into a `&mut` OUT-PARAM
// (the dungeon render-line shape): `out_line = "== " + self.label + " =="` writes
// across a machine boundary into a borrowed carrier -- init literal, append the
// source carrier, append the trailing literal at the running length. Exits 70.
#[test]
fn runtime_bounded_carrier_alias_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_alias_concat_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-bounded-carrier-alias-concat-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("bounded carrier alias concat canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bounded carrier alias concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `out_line = \"== \" + self.label + \" ==\"` to materialize `== Gate ==` into the \
         borrowed carrier so `self.line == \"== Gate ==\"` exits 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 owned carrier concat with a FRAME-LOCAL source: `out_line = "== " + src +
// " =="` where `src` is a `let`-local carrier read from the runtime frame base.
#[test]
fn runtime_bounded_carrier_local_source_concat_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_local_source_concat_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-bounded-carrier-local-source-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("bounded carrier local source concat canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bounded carrier local source concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `out_line = \"== \" + src + \" ==\"` (frame-local source) to render `== Gate ==` \
         so `self.line == \"== Gate ==\"` exits 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Carrier sibling of the slice-view-element test: a value-call guards on the
// element's CARRIER field (`room.label == "Gate"`, room = r[0], r =
// self.rooms.as_slice()). Carrier RECOGNITION now traces the elided local and
// sees through the as_slice view to resolve the field descriptor against the
// underlying array; before, the carrier `==` failed to lower (the arm was
// poisoned). Exits 70.
#[test]
fn runtime_value_call_slice_view_carrier_guard_exit_canary_runs() {
    let canary = pass_canary("text/runtime_value_call_slice_view_carrier_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-value-call-slice-view-carrier-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("value-call slice-view carrier guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("value-call slice-view carrier guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the carrier guard `room.label == \"Gate\"` (room = r[0], r a \
         slice view) to resolve and take the true arm -> exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A value-call forwarded a SLICE-VIEW element by value (`read(r[0])`, r a local
// `self.rooms.as_slice()`): the body reads `room.id` through the BranchParameter
// alias `room = r[0]` -> `(self.rooms.as_slice())[0].id`. The place resolver now
// sees through the as_slice view AND traces the elided local to its initializer
// so the element resolves against the underlying array; before, it read a zero
// slot and the call returned 0. Exits 70.
#[test]
fn runtime_value_call_slice_view_element_arg_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_value_call_slice_view_element_arg_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-value-call-slice-view-elem-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("value-call slice-view element canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("value-call slice-view element canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `read(r[0])` (r = self.rooms.as_slice()) to resolve room.id to the \
         underlying array element and return 7 -> exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A `let`-local capturing `self.vm.sp + 1`, where `self.vm.sp` is reassigned
// before the local is forwarded through a nested dispatch (try_push1 -> push1).
// Argument materialization used to inline-fold the local back into its
// initializer and re-evaluate it AFTER the field was overwritten, so a deeper
// substate's guard saw the wrong value and branched into the wrong arm. The fix
// keeps the captured slot. Exits 70 (both pushes land: stack[0]=3, stack[1]=4).
#[test]
fn runtime_loop_patterns_exit_canary_runs() {
    // Loop patterns via self-transition: a LARGE counting loop (1..10000) stays
    // iterative (no stack growth) and nested loops re-initialize the inner counter.
    // Guards the state-recursion lowering that serious apps lean on.
    let canary = pass_canary("control_flow/runtime_loop_patterns_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-loop-patterns-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("loop-patterns canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("loop-patterns canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a 10000-iteration counting loop + nested loops to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_composite_initializer_local_arg_exit_canary_runs() {
    // A let-local whose initializer is a composite (binary / unary / cast) reading a
    // prior local or field, forwarded as a transition argument. The dispatch-arg fold
    // must recurse into the composite to resolve the inner local; missing Cast/Binary/
    // Unary arms re-materialized it in the target frame (no slot) and read 0.
    let canary = pass_canary("control_flow/runtime_composite_initializer_local_arg_exit");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-composite-initializer-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("composite-initializer-local-arg canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("composite-initializer-local-arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected binary/unary/field-read composite initializers forwarded as args to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_captured_local_remutated_field_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_captured_local_remutated_field_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-captured-local-remutated-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("captured-local-remutated-field canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("captured-local-remutated-field canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the captured `new_sp` slot (not a re-folded `self.vm.sp + 1`) to \
         drive the nested dispatch so both pushes land and exit is 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 carrier compare through a POINTEE in a VALUE-CALL guard: the value-call
// `Finder::check(level) -> i32` branches on `r[0].label == "Gate"` where `r:
// &[Room]` indexes the by-value `level` param, so `r[0].label` is a carrier
// reached through the slice pointer. The guard resolves the pointee place and
// lowers the bounded-buffer compare; before the fix the resolver bailed, the
// leaf branch dropped the arm write (the literal-guard poison-skip), and the
// value-call returned a stale 0. Exits 70 (the `== "Gate"` true arm).
#[test]
fn runtime_bounded_carrier_pointee_guard_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_pointee_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-bounded-carrier-pointee-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("bounded carrier pointee guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bounded carrier pointee guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the value-call guard `r[0].label == \"Gate\"` (carrier through a \
         slice-element pointee) to take the true arm and return 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 owned `[u8; N] in Utf8` carrier field reached THROUGH a slice pointer:
// `cells[0].label = "Gate"` writes the carrier inline through the `&mut [Room]`
// pointer (a pointee write), then reads it back through the same pointer. Exits 70.
#[test]
fn runtime_bounded_carrier_slice_field_write_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_slice_field_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-bounded-carrier-slice-field-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("bounded carrier slice field write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bounded carrier slice field write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `cells[0].label = \"Gate\"` to write the carrier through the slice pointer so \
         `cells[0].label == \"Gate\"` exits 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 owned `[u8; N] in Utf8` carrier through HOST OUTPUT, native: build a carrier
// by concat and `write_line` it. The host-call path reads the carrier with carrier
// addressing (len @ 0, content pointer = place + pointer_size). Prints "Room A1"
// and exits 70.
#[test]
fn runtime_bounded_carrier_write_line_exit_canary_runs() {
    let canary = pass_canary("text/runtime_bounded_carrier_write_line_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-bounded-carrier-write-line-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("bounded carrier write_line canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bounded carrier write_line canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the carrier write_line canary to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end_matches(['\r', '\n']),
        "Room A1",
        "expected the carrier `write_line` to print the materialized content `Room A1`",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 builder over NESTED-field carriers, CROSS-STATE: `self.line.text = "Room " +
// self.room.label` is built in `main` and `write_line`d in a later `shutdown`
// state. The nested fields carry their declared `in Utf8` domain across the state
// transition (entry-invariant seeded for nested fields, enforced at the nested
// write), so the carrier persists and prints. Prints "Room A1", exits 0.
#[test]
fn runtime_text_builder_canary_runs() {
    let canary = pass_canary("text/runtime_text_builder");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-runtime-text-builder-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("nested-field carrier builder canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("nested-field carrier builder canary should run");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected the nested-field carrier builder canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end_matches(['\r', '\n']),
        "Room A1",
        "expected the cross-state nested-field carrier builder to print `Room A1`",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 (return a `&[u8] in Utf8` view from a machine): a value-position call
// returning a `&[u8] in Utf8` literal view flows as a real 16-byte `{ptr,len}`
// descriptor into a `==` content compare. `pick() == "Gate"` matches and exits 70;
// the interpreter agrees. Exercises the value-call-result descriptor reaching the
// TextEquals leaf.
#[test]
fn utf8_return_view_equals_exit_canary_runs() {
    let canary = pass_canary("domains/utf8_return_view_equals_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-utf8-return-view-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("utf8 return view canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("utf8 return view canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a returned `&[u8] in Utf8` view compared `== \"Gate\"` to match and exit 70, got {:?}\nstderr:\n{}",
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
fn runtime_bitwise_operators_exit_canary_runs() {
    let canary = pass_canary("operators/runtime_bitwise_operators_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-bitwise-operators-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("bitwise operators canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bitwise operators canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `&`, `|`, `^` to evaluate correctly (12&10==8, 12|10==14, 12^10==6; exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_popcount_loop_exit_canary_runs() {
    let canary = pass_canary("operators/runtime_popcount_loop_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-popcount-loop-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("popcount loop canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("popcount loop canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the shift-and-mask popcount loop to count 24 bits and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_xorshift_prng_exit_canary_runs() {
    let canary = pass_canary("operators/runtime_xorshift_prng_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-xorshift-prng-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("xorshift prng canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("xorshift prng canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected xorshift32 (XOR + shifts composed) to draw 99 and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_bitwise_guard_exit_canary_runs() {
    let canary = pass_canary("operators/runtime_bitwise_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-bitwise-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("bitwise guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bitwise guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected bitwise guard SUBJECTS (`flags & 2 == 0`, `& 4 == 4`, `| 2 == 7`, `^ 5 == 0`; exit 70), got {:?}\nstderr:\n{}",
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
fn runtime_free_machine_struct_arg_exit_canary_runs() {
    // A BY-VALUE STRUCT argument to a FREE machine (`machine work(job: Job)`
    // called as `work(job)` / `combine(move pair)`) must deliver the caller's
    // field values. Three stacked selection bugs dropped the call's
    // result-slot write so the callee computed from a stale 0: the same-named
    // caller arg was rejected as a no-op self-binding (caller args arrive
    // symbol-less), caller-local initializer substitution had no Member arm
    // to project `job.id` through the struct literal, and the leaf terminal
    // value write resolved the substituted CALLER-context value in the
    // CALLEE's context. Rung 1 = same-name 1-field struct (71 on miss),
    // rung 2 = 2-field struct with explicit `move` (72). Exits 70 only when
    // both callees saw the real runtime field values.
    let canary = pass_canary("calls/runtime_free_machine_struct_arg_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-free-machine-struct-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("free-machine struct arg canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("free-machine struct arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected by-value struct args to free machines to deliver the caller's field values (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn by_value_case_param_self_write_exit_canary_runs() {
    // A `&mut self` machine taking a BY-VALUE CASE-BEARING parameter must
    // persist writes to `self.<field>` made in a dispatched substate.
    // Root cause: InlineBranching argument materialization had no handler for
    // StructLiteral arguments -- `Event::Insert { cents: 50 }` was never
    // written to the parameter slot, so the case tag stayed 0 (Idle), the
    // dispatch guard failed, the substate was never entered, and
    // `self.register.balance` stayed 0. Exits 70 when the write-back lands.
    let canary = pass_canary("calls/by_value_case_param_self_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-by-value-case-param-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("by-value case param self-write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("by-value case param self-write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a by-value case-bearing arg to persist the self write-back (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_attached_machine_struct_arg_exit_canary_runs() {
    // The attached (data-scoped, receiverless `Worker::run`) spelling of the
    // by-value struct argument shape: the same leaf expansion path lowers it
    // (binding rewrite + struct-literal member projection + caller-context
    // value resolution), but resolution routes through the attached machine
    // lookup, so it gets its own rung. Exits 70 only when the callee saw the
    // real runtime field values (a dropped result-slot write reads 0 -> 71).
    let canary = pass_canary("calls/runtime_attached_machine_struct_arg_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-attached-machine-struct-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("attached-machine struct arg canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("attached-machine struct arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a by-value struct arg to an attached machine to deliver the caller's field values (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_free_machine_struct_return_exit_canary_runs() {
    // A FREE machine RETURNING a struct BY VALUE (`let lit: Pair = make(seed)`)
    // must deliver both field values into the caller's local. Two leaf
    // terminal-value resolution gaps dropped every per-field result-slot write
    // (the local read ZII zeroes): the caller-local initializer substitution
    // had no StructLiteral arm (folded caller locals never substituted inside
    // field values), and a local backed by a CALL's result slot (`let bumped =
    // bump(30)`) was substituted with the unloweable call expression instead
    // of keeping its name resolving against the result slot. Rung 1 = struct
    // from a folded literal seed, rung 2 = struct from a chained call-result
    // seed. Exits 70 only when all four returned fields are correct.
    let canary = pass_canary("calls/runtime_free_machine_struct_return_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-free-machine-struct-return-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("free-machine struct return canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("free-machine struct return canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected by-value struct returns from free machines to deliver both field values (exit 70), got {:?}\nstderr:\n{}",
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
fn runtime_16bit_cast_exit_canary_runs() {
    let canary = pass_canary("expressions/runtime_16bit_cast_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-16bit-cast-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("16-bit cast canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("16-bit cast canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i16/u16 casts (truncate, sign/zero-extend, reinterpret) in every direction to evaluate correctly and exit 70, got {:?}\nstderr:\n{}",
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
    // String-bearing variants: pass/traits/equatable_string_not_equals_exit
    // (value position) and pass/traits/equatable_string_equality_guard_exit
    // (guard position).
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
fn runtime_const_array_length_bare_call_arm_exit_canary_runs() {
    // Comptime: the const-position callee's value arm is a PARENTHESIZED
    // BARE CALL (`_ -> (burn(4, 12))`). The parenthesized lone call is a
    // value expression (not a transition target), so const evaluation
    // resolves the free machine `burn` like the arithmetic-wrapped spelling
    // does: 16 slots, both the write and the index-15 typecheck land.
    let canary = pass_canary("comptime/runtime_const_array_length_bare_call_arm_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-const-length-bare-call-arm-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("bare-call-arm const length canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bare-call-arm const length canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the bare-call-arm const length (burn(4, 12) = 16) to size the array (exit 70), got {:?}\nstderr:\n{}",
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
fn runtime_wire_encode_byte_slice_exit_canary_runs() {
    // Wire stage 2 (#43), borrowed `&[u8]` fields: a fat-slice bytes field
    // constructed from a fixed-array subslice (`{ bytes: self.source[0..2] }`)
    // materializes a `{ptr, len}` descriptor, and `encode_wire` frames it as RAW
    // bytes (length varint + the bytes) through the same text-bytes append a
    // String uses. The canary checks the five expected bytes + the written count
    // in-language; exits 70 when byte-exact.
    let canary = pass_canary("wire/runtime_wire_encode_byte_slice_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-wire-byte-slice-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("wire encode byte-slice canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("wire encode byte-slice canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `&[u8]` field construction + encode to frame raw bytes (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_decode_byte_slice_exit_canary_runs() {
    // Wire stage 2 (#43), borrowed `&[u8]` ZERO-COPY decode: `decode_wire` reads
    // a byte-length varint and stores a fat `{ptr, len}` descriptor viewing the
    // buffer in place (the `ReadWireByteSlice` op). The canary round-trips and
    // RE-ENCODES the decoded value to prove the view is content-correct (ptr +
    // len point at the right buffer bytes); exits 70 when byte-exact.
    let canary = pass_canary("wire/runtime_wire_decode_byte_slice_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-wire-decode-byte-slice-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("wire decode byte-slice canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("wire decode byte-slice canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `&[u8]` zero-copy decode to recover a content-correct buffer view (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_decoded_byte_slice_index_exit_canary_runs() {
    // Consuming a decoded zero-copy `&[u8]`: a runtime-indexed element read
    // (`data[i]`) in transition-ARGUMENT position must be materialized by reading
    // through the descriptor's data pointer. It used to fall through every
    // argument strategy and was never written (parameter kept uninitialized
    // bytes); now resolved as a value operand. Exits 70 when decoded.bytes[0]==72.
    let canary = pass_canary("wire/runtime_wire_decoded_byte_slice_index_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-wire-decoded-byte-slice-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("wire decoded byte-slice index canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("wire decoded byte-slice index canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected indexing a decoded `&[u8]` under a length guard to read the right byte (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_decoded_byte_slice_len_exit_canary_runs() {
    // `.len` of a `&[u8]` held as a struct FIELD must resolve to the descriptor's
    // runtime len slot. The place resolver used to drop a
    // `<struct>.<descriptor-field>.len` path (the `.len` step has no data layout),
    // so `let n = decoded.bytes.len` emitted no write and `n` held garbage. The
    // length is genuinely runtime (decoded from a varint), so a correct read
    // proves the len slot is targeted; exits 70 when n == 2.
    let canary = pass_canary("wire/runtime_wire_decoded_byte_slice_len_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-wire-decoded-byte-slice-len-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("wire decoded byte-slice .len canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("wire decoded byte-slice .len canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `.len` of a decoded `&[u8]` field to read the descriptor len slot (exit 70), got {:?}\nstderr:\n{}",
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
fn runtime_spawn_struct_result_exit_canary_runs() {
    // Promoted from pending/concurrency/spawn_struct_result_miscompiled when
    // by-value struct RETURNS landed natively. `spawn { Worker::make(move
    // seed) }` joins a machine returning a STRUCT by value; under the
    // synchronous-spawn desugar the joined `pair` must read back both fields
    // ({ a: 34, b: 35 }, a + b == 69 -> exit 70). The struct-typed terminal
    // value's per-field result-slot writes used to drop silently (ZII zeroes,
    // exit 71). The direct no-spawn spelling is pinned by
    // calls/runtime_free_machine_struct_return_exit.
    let canary = pass_canary("concurrency/runtime_spawn_struct_result_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-spawn-struct-result-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("spawn struct result canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("spawn struct result canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the joined by-value struct result to deliver both field values (exit 70), got {:?}\nstderr:\n{}",
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
fn runtime_float_negative_ops_exit_canary_runs() {
    // Float operations with negatives -- comparisons (the ucomisd unsigned-flags case),
    // a negative float->int cast (truncation toward zero), and a negative multiply.
    // Exits 70.
    let canary = pass_canary("arithmetic/runtime_float_negative_ops_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-float-negative-ops-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("float negative ops canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float negative ops canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected float compares/cast/multiply with negatives to be correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_newton_sqrt_exit_canary_runs() {
    // Newton's method for a square root (an iterative numerical algorithm): x <- (x + S/x)/2
    // over f64, six iterations from 1.0 on S=2.0 -> sqrt(2) ~= 1.41421; checks
    // 1.414 < x < 1.415 -> exit 70.
    let canary = pass_canary("arithmetic/runtime_newton_sqrt_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-newton-sqrt-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("newton sqrt canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("newton sqrt canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Newton's method to converge to sqrt(2) in (1.414, 1.415) (exit 70); got {:?} -- a float div/compare regression\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_monte_carlo_pi_exit_canary_runs() {
    // Monte Carlo pi estimation driven by the xorshift32 PRNG: 64 random points, count
    // those inside the quarter circle (px*px+py*py < 100*100). Deterministic from seed 1:
    // 53 inside, scaled estimate 400*53/64 = 331 (pi ~= 3.31) -> exit 70.
    let canary = pass_canary("arithmetic/runtime_monte_carlo_pi_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-monte-carlo-pi-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("monte carlo pi canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("monte carlo pi canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Monte Carlo pi (seed 1, 64 points) to count 53 inside / estimate 331 (exit 70); got {:?} -- the count on regression\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_gcd_euclid_exit_canary_runs() {
    // The iterative Euclidean GCD: `(a,b) = (b, a%b)` until b==0. gcd(1071,462)=21.
    // A two-variable loop with a runtime modulo; self-checks the result -> exit 70.
    let canary = pass_canary("arithmetic/runtime_gcd_euclid_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-gcd-euclid-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("gcd euclid canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("gcd euclid canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the Euclidean GCD to reduce 1071,462 to 21 (exit 70); got {:?} (a non-70 code is the wrong gcd)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_rpn_evaluator_exit_canary_runs() {
    // A reverse-Polish stack evaluator (a stack VM): push numbers, pop-pop-op-push for
    // operators, over a token array. Evaluates `3 4 + 5 *` to 35 -> exit 70.
    let canary = pass_canary("collections/runtime_rpn_evaluator_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-rpn-eval-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("rpn evaluator canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("rpn evaluator canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the RPN stack VM to evaluate 3 4 + 5 * to 35 (exit 70); got {:?} (a non-70 code is the wrong result)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_activity_selection_greedy_exit_canary_runs() {
    // Greedy activity selection: given activities sorted by finish, take each that starts
    // no earlier than the last chosen finish. Six activities yield 3 non-overlapping ->
    // exit 70.
    let canary = pass_canary("collections/runtime_activity_selection_greedy_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-activity-greedy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("activity selection canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("activity selection canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected greedy activity selection to pick 3 non-overlapping (exit 70); got {:?} (the count on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_maze_pathfind_exit_canary_runs() {
    // Shortest-path BFS on a 5x5 grid maze (implicit grid neighbours + walls, distinct from
    // the adjacency-matrix BFS). The shortest distance from cell 0 to cell 24 through the
    // snaking corridor is 16 -> exit 70.
    let canary = pass_canary("collections/runtime_maze_pathfind_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-maze-pathfind-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("maze pathfind canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("maze pathfind canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected grid-BFS shortest distance 0->24 to be 16 (exit 70); got {:?} (the distance on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nqueens_backtracking_exit_canary_runs() {
    // N-queens count by backtracking (try/prune/undo): cols[r] is the column tried for row
    // r and doubles as the state stack; conflicts are column or diagonal. N=4 has exactly 2
    // solutions -> exit 70 (a discriminating count).
    let canary = pass_canary("collections/runtime_nqueens_backtracking_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-nqueens-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("nqueens backtracking canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("nqueens backtracking canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected N=4 queens to have exactly 2 solutions (exit 70); got {:?} (the count on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_coin_change_dp_exit_canary_runs() {
    // Coin-change minimisation by dynamic programming: dp[a] = fewest coins for amount a,
    // relaxing dp[a] toward 1 + dp[a-c] over a computed subproblem index. Coins {1,3,4},
    // amount 6 -> 2 coins (3+3) -> exit 70.
    let canary = pass_canary("collections/runtime_coin_change_dp_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-coin-change-dp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("coin change dp canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("coin change dp canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected DP min coins for 6 with {{1,3,4}} to be 2 (exit 70); got {:?} (dp[6] on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_bfs_traversal_exit_canary_runs() {
    // Breadth-first search over a 4-node graph (adjacency matrix + FIFO queue + visited
    // set): from node 0 the frontier expands level by level, visit order 0,1,2,3, all four
    // reached -> exit 70.
    let canary = pass_canary("collections/runtime_bfs_traversal_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-bfs-traversal-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("bfs traversal canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bfs traversal canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected BFS to visit 0,1,2,3 in order and reach all 4 nodes (exit 70); got {:?} (the visit count on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_hash_table_exit_canary_runs() {
    // An open-addressing hash table with linear probing (the associative map): parallel
    // keys/vals/used arrays, hash k%8, probe forward with wrap past occupied slots, look
    // back up. Keys 6,14,7,15 collide and force a wrap; their values sum to 246 -> exit 70.
    let canary = pass_canary("collections/runtime_hash_table_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-hash-table-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("hash table canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("hash table canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the hash table (probe + wrap) to sum looked-up values to 246 (exit 70); got {:?} (the sum on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_matrix_multiply_exit_canary_runs() {
    // 2x2 matrix multiply (row-major flat storage, triple i/j/k loop, inner-product
    // accumulation with computed flat indices). [[1,2],[3,4]] * [[5,6],[7,8]] =
    // [[19,22],[43,50]] -> exit 70.
    let canary = pass_canary("collections/runtime_matrix_multiply_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-matrix-mul-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("matrix multiply canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("matrix multiply canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected 2x2 matmul to yield [[19,22],[43,50]] (exit 70); got {:?} (a non-70 code is the wrong C[0])\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_ring_buffer_queue_exit_canary_runs() {
    // A FIFO ring-buffer queue: a fixed [i32;4] with head/tail advancing modulo the
    // capacity (explicit wrap) and a count guard. Interleaved enqueue/dequeue forces both
    // pointers to wrap; each dequeue is checked against a running counter so FIFO order is
    // pinned. All of 1..6 dequeued in order -> exit 70.
    let canary = pass_canary("collections/runtime_ring_buffer_queue_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-ring-buffer-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("ring buffer queue canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("ring buffer queue canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the ring buffer to preserve FIFO order 1..6 (exit 70); got {:?} (a non-70 code is where order broke)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_bubble_sort_exit_canary_runs() {
    // Bubble sort with nested loops, the adjacent index `j+1` via a field, a field-bound
    // compare, and a value-swap. Sorts [5,2,8,1,9,3] and self-checks four cells -> 70.
    let canary = pass_canary("collections/runtime_bubble_sort_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-bubble-sort-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("bubble sort canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bubble sort canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected bubble sort to order the array (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_2d_transpose_exit_canary_runs() {
    // A 2D matrix transpose over a flat array via the linear-counter sidestep: the
    // (row,col) and transposed output index are computed into fields, then used as plain
    // indices. Self-checks four transposed cells -> exit 70. Proves 2D/matrix data.
    let canary = pass_canary("collections/runtime_2d_transpose_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-2d-transpose-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("2d transpose canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("2d transpose canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the 2D transpose to place cells correctly (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_through_guard_chain_exit_canary_runs() {
    // An index bound carried across a CHAIN of convergent-arm guards (`d<0 {true->t
    // _->t}`) that neither name nor rewrite x. Before convergent arms were treated as
    // a single unconditional predecessor, each guard split dropped the bound. Compiling
    // + reading arr[3]=70 -> exit 70 confirms the bound survives the chain.
    let canary = pass_canary("collections/runtime_indexed_through_guard_chain_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-guard-chain-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("indexed-through-guard-chain canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("indexed-through-guard-chain canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the index bound to survive the convergent-guard chain (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_binary_search_exit_canary_runs() {
    // Binary search for 50 in a sorted 7-element array narrows in BOTH directions
    // (lo=mid+1 then hi=mid-1) and must find it at exactly index 4. Locks the computed
    // midpoint, the indexed read into a field, and both pointer updates. Exits 70.
    let canary = pass_canary("collections/runtime_binary_search_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-binary-search-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("binary search canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("binary search canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected binary search to find 50 at index 4 (exit 70); got {:?} (71=wrong index, 72=not found)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_two_pointer_palindrome_exit_canary_runs() {
    // A two-pointer palindrome check whose DECREASING pointer `j` stays >= 0 only
    // because j > i >= 0 -- proven by chaining the loop ordering `i < j` with `i`'s
    // non-negativity (non_negative_is_proven_via_ordering). Compiling + exiting 70
    // confirms the decreasing-counter lower bound is derived and the walk is correct.
    let canary = pass_canary("collections/runtime_two_pointer_palindrome_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-two-pointer-palindrome-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("two-pointer palindrome canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("two-pointer palindrome canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the two-pointer palindrome walk to confirm [3,7,9,7,3] (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_rule90_automaton_exit_canary_runs() {
    // A self-checking Rule 90 cellular automaton (the engine behind
    // samples/cellular_automaton): a sliding 3-cell window, the value-position rule
    // shift `(90 >> window) & 1`, plain-index array reads/writes, and a field-temp
    // double buffer. The live-cell counts of the first four generations (1,2,2,4) sum
    // to 9, so it exits 70 only when the computation is exactly right.
    let canary = pass_canary("collections/runtime_rule90_automaton_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-rule90-automaton-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("rule90 automaton canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("rule90 automaton canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the Rule 90 automaton's first-four-generation live-cell sum to be 9 (exit 70); got {:?} (a non-70 code is the actual sum)\n{}",
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
fn float_array_binary_op_zero_exit_canary_runs() {
    // Float binary arithmetic where BOTH operands are fixed-array elements of
    // type f64 (`self.vals[0] + self.vals[1]`) must emit an SSE addsd, not an
    // integer add.  Root cause: `resolve_machine_owned_collection_in_table`
    // returned the array type `[f64; 2]` instead of the element type `f64`,
    // so `binary_value_operands_are_float` returned false.  Fixed to apply the
    // element index from the root-field member_index when the suffix is empty.
    let canary = pass_canary("expressions/float_array_binary_op_zero");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-float-array-binary-op-zero-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("float_array_binary_op_zero canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float_array_binary_op_zero canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f64 array element binary op to yield 7.0 and exit 70, got {:?} (71 = integer add over float bits)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn f32_array_binary_op_zero_exit_canary_runs() {
    // Same as float_array_binary_op_zero but for f32 array elements.
    // Both operands `self.vals[0]` and `self.vals[1]` are f32; their sum
    // 3.0f32 + 4.0f32 = 7.0f32 must use addss and exit 70.
    let canary = pass_canary("expressions/f32_array_binary_op_zero");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-f32-array-binary-op-zero-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("f32_array_binary_op_zero canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("f32_array_binary_op_zero canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32 array element binary op to yield 7.0f32 and exit 70, got {:?} (71 = integer add over float bits)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_wrapping_exit_canary_runs() {
    // Decision 17 S1a: `u8 in Wrapping` parses and wraps (200+100 -> 44).
    let canary = pass_canary("expressions/arithmetic_domain_wrapping_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-arith-domain-wrapping-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("arithmetic_domain_wrapping canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("arithmetic_domain_wrapping canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected u8 in Wrapping (200+100) to wrap to 44 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_saturating_exit_canary_runs() {
    // Decision 17 S1b: `u8 in Saturating` clamps on overflow (200+100 -> 255),
    // NOT wraps to 44. Native emits a width-correct add + carry-flag cmov.
    let canary = pass_canary("expressions/arithmetic_domain_saturating_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-arith-domain-saturating-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("arithmetic_domain_saturating canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("arithmetic_domain_saturating canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected u8 in Saturating (200+100) to clamp to 255 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_saturating_div_mod_exit_canary_runs() {
    // Decision 17: SATURATING signed divide/modulo. TYPE_MIN / -1 (the only
    // overflowing division, and the corner `idiv` traps on) clamps to TYPE_MAX, and
    // TYPE_MIN % -1 -> 0, instead of trapping. The divisor reaches -1 via a loop so
    // it is a genuine runtime value (defeats const-folding), exercising the native
    // divisor==-1 guard + cmovo saturation.
    let canary = pass_canary("expressions/arithmetic_domain_saturating_div_mod_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-arith-domain-sat-div-mod-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("arithmetic_domain_saturating_div_mod canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("arithmetic_domain_saturating_div_mod canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected saturating i32::MIN/-1 -> i32::MAX, MIN%-1 -> 0, -8/-1 -> 8 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_guard_divide_modulo_exit_canary_runs() {
    // Division and modulo in a transition GUARD subject (`self.x / 3 > 5`,
    // `self.x % 5 == 3`). The planner whitelist excluded Divide+Modulo and the
    // guard value-operand resolver did not map Divide, so a div/mod guard silently
    // took the true arm. Every arm here is reached only on a correct guard, so the
    // regression would exit 71-74 instead of 70.
    let canary = pass_canary("expressions/runtime_guard_divide_modulo_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-runtime-guard-div-mod-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime_guard_divide_modulo canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime_guard_divide_modulo canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected div/mod guard subjects to evaluate correctly (exit 70), got {:?} (71-74 = a guard took the wrong arm)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_guard_negative_arithmetic_exit_canary_runs() {
    // Negative-i32 arithmetic in a transition guard subject (`self.x - 1 == -9` for
    // x=-8) took the wrong arm natively: a computed value-operand zero-extended the
    // i32 but the compare ran 64-bit. Fixed by sizing a Binary value-operand from
    // the non-immediate operand so the compare runs at the i32 width. Every arm is
    // reached only on a correct guard, so a regression exits 71-74.
    let canary = pass_canary("expressions/runtime_guard_negative_arithmetic_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-runtime-guard-neg-arith-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime_guard_negative_arithmetic canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime_guard_negative_arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected negative-i32 guard arithmetic to evaluate correctly (exit 70), got {:?} (71-74 = a guard took the wrong arm)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_guard_divide_modulo_signedness_exit_canary_runs() {
    // Division/modulo in a guard subject with a NEGATIVE i32 dividend (`neg / 2 ==
    // -4`) and a large UNSIGNED dividend. Div/mod are not modular, so the op runs at
    // the operand width (32-bit) -- signed idiv for i32, Divide->DivideUnsigned for
    // u32 so a large u32 is not misread as negative. A regression exits 71-74.
    let canary = pass_canary("expressions/runtime_guard_divide_modulo_signedness_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-runtime-guard-divmod-sign-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime_guard_divide_modulo_signedness canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime_guard_divide_modulo_signedness canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed/unsigned div/mod guard subjects to evaluate correctly (exit 70), got {:?} (71-74 = wrong arm)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn case_payload_shared_field_name_exit_canary_runs() {
    // Regression: destructuring `Tx::Transfer { to, amount }` must read Transfer's
    // `amount` (40), not a same-named field in an earlier variant (would read to=3).
    let canary = pass_canary("control_flow/case_payload_shared_field_name_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-case-payload-collision-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("case_payload_shared_field_name canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("case_payload_shared_field_name canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected destructured Transfer.amount==40 to exit 70 (93 = read `to`=3), got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_saturating_mul_exit_canary_runs() {
    // Decision 17: `u8 in Saturating` multiply clamps 100*100=10000 to 255 (a
    // 64-bit imul gives the exact product, then range-compare + cmov to the max).
    let canary = pass_canary("expressions/arithmetic_domain_saturating_mul_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-arith-domain-sat-mul-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("arithmetic_domain_saturating_mul canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("arithmetic_domain_saturating_mul canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected u8 in Saturating (100*100) to clamp to 255 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_saturating_mul_signed_exit_canary_runs() {
    // Decision 17: signed saturating multiply clamps both ways (2500->127 cmovg,
    // -2500->-128 cmovl).
    let canary = pass_canary("expressions/arithmetic_domain_saturating_mul_signed_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-arith-domain-sat-mul-signed-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("arithmetic_domain_saturating_mul_signed canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("arithmetic_domain_saturating_mul_signed canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed sat mul (2500->127, -2500->-128) to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_trapping_div_exit_canary_runs() {
    // Decision 17: Trapping divide routes to the normal idiv (which traps on
    // overflow / div-by-zero); in range 140/2 = 70.
    let canary = pass_canary("expressions/arithmetic_domain_trapping_div_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-arith-domain-trap-div-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("arithmetic_domain_trapping_div canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("arithmetic_domain_trapping_div canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Trapping div (140/2=70) to exit 70, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_trapping_mul_exit_canary_runs() {
    // Decision 17: in-range Trapping multiply (10*10=100) does not trap.
    let canary = pass_canary("expressions/arithmetic_domain_trapping_mul_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-arith-domain-trap-mul-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("arithmetic_domain_trapping_mul canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("arithmetic_domain_trapping_mul canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected in-range Trapping mul (10*10=100) to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

/// Decision 17 transition-arg enforcement + dominating-guard narrowing: the
/// recursive arm arg `count_down(n - 1)` carries the exact-arith obligation and
/// proves Exact ONLY because the guard `n > 0` narrows `n` to `[1, ..]`. Runs to
/// 70 (the unguarded form is rejected — fail/arithmetic/transition_arg_unguarded_overflow).
#[test]
fn runtime_transition_arg_guard_narrowing_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_transition_arg_guard_narrowing_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-transition-arg-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("guarded transition-arg decrement should compile (guard narrows n-1 to Exact)");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("transition-arg guard narrowing canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected guarded count_down(n-1) to prove Exact and run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

/// Decision 17 transition-arg narrowing on the FALSE arm: the arm fires when
/// `n >= 70` is FALSE (negate `>=` -> `<`), so `n + 1` proves Exact. Runs to 70.
#[test]
fn runtime_transition_arg_false_arm_narrowing_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_transition_arg_false_arm_narrowing_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-transition-arg-false-arm-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("false-arm transition-arg increment should compile (negated guard narrows n+1 to Exact)");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("transition-arg false-arm narrowing canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected false-arm climb(n+1) to prove Exact and run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

/// Decision 17 transition-arg enforcement respects domains: a Saturating-domain
/// accumulator argument carries no exact-arith obligation, so `acc + (s[0] as
/// i32 in Saturating)` compiles with no guard / no range proof. Runs to 70.
#[test]
fn runtime_transition_arg_saturating_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_transition_arg_saturating_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-transition-arg-saturating-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("Saturating transition-arg accumulator should compile (no exact-arith obligation)");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("transition-arg saturating canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Saturating accumulator to run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

/// Regression for the #59 native miscompile: a domain-cast of a slice ELEMENT in
/// a recursive accumulator (`acc + (s[0] as i32 in Wrapping)`) silently read 0
/// because the cast could not classify its element source. Fixed by classifying
/// a slice-element read from the collection's element type. Sums to 70.
#[test]
fn runtime_cast_element_accumulator_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_cast_element_accumulator_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-cast-element-accum-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("cast-of-element accumulator should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("cast-element accumulator canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected cast-of-element accumulator to sum to 70 (not 0); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_domain_boundaries_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_domain_boundaries_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-domain-boundaries-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("domain-boundaries canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("domain-boundaries canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Saturating/Wrapping at i32 & u8 boundaries to clamp/wrap correctly (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_comparison_signedness_exit_canary_runs() {
    // Comparison-operator signedness across widths: a signed compare used for
    // unsigned operands (or vice versa) flips the branch past the signed/unsigned
    // boundary. The canary self-checks u32/u8/u16 unsigned cases and i32/i64 signed
    // cases; the wrong arm exits 71.
    let canary = pass_canary("arithmetic/runtime_comparison_signedness_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-comparison-signedness-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("comparison-signedness canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("comparison-signedness canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed/unsigned compares to pick the right branch at each width's boundary (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_shift_signedness_exit_canary_runs() {
    // Shift signedness: a signed right shift must be arithmetic (sar), an unsigned
    // one logical (shr). The canary builds the shift value at runtime (a loop) and
    // self-checks a negative arithmetic >>, a high-bit unsigned >>, and a <<.
    let canary = pass_canary("arithmetic/runtime_shift_signedness_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-shift-signedness-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("shift-signedness canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("shift-signedness canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed (arithmetic) vs unsigned (logical) shifts to compute correctly (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_narrow_signed_wrap_boundaries_exit_canary_runs() {
    // Signed two's-complement wrap-around at narrow boundaries (i8: 127->-128, -128->127;
    // i16 analogues), both ends, in-Wrapping. Complements the saturating narrow canaries.
    // All four corners must hold -> exit 70.
    let canary = pass_canary("arithmetic/runtime_narrow_signed_wrap_boundaries_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-narrow-wrap-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("narrow signed wrap canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("narrow signed wrap canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i8/i16 signed Wrapping wrap-around at both boundaries (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_narrow_signed_guard_ops_exit_canary_runs() {
    // Narrow (i8) signed compare/sub/mul with negative values as guard subjects -- the
    // working siblings of the narrow-signed-divide-guard fix; guards the area.
    let canary = pass_canary("arithmetic/runtime_narrow_signed_guard_ops_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-narrow-signed-guard-ops-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("narrow-signed-guard-ops canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("narrow-signed-guard-ops canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i8 signed compare/sub/mul with negatives in guards to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_narrow_signed_divide_guard_exit_canary_runs() {
    // Narrow (i8/i16) signed div/mod evaluated as a GUARD SUBJECT with a negative
    // result. Guard-subject operands arrive zero-extended, so the 32-bit idiv divided
    // i8 -20 as 236 -- the divide core now sign-extends narrow signed operands.
    let canary = pass_canary("arithmetic/runtime_narrow_signed_divide_guard_exit");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-narrow-signed-divide-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("narrow-signed-divide-guard canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("narrow-signed-divide-guard canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i8/i16 signed div/mod in a guard with a negative result to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_saturating_narrow_divide_exit_canary_runs() {
    // i8/i16 saturating signed divide (previously a hard "not implemented" error):
    // normal divide, and the TYPE_MIN/-1 overflow clamped to TYPE_MAX (i8 127, i16
    // 32767). The narrow path clamps -a > TYPE_MAX instead of using neg's overflow flag.
    let canary = pass_canary("arithmetic/runtime_saturating_narrow_divide_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-saturating-narrow-divide-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("saturating-narrow-divide canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("saturating-narrow-divide canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i8/i16 saturating divide (normal + TYPE_MIN/-1 -> TYPE_MAX) to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mixed_width_sign_exit_canary_runs() {
    // Mixed-width / mixed-sign arithmetic auto-promotes and extends the narrower
    // operand correctly: sign-extension (i32(-5)+i64), zero-extension (u8+i32),
    // narrower-signed (i16(-3)+i32), and a mixed-sign add (i32+u32).
    let canary = pass_canary("arithmetic/runtime_mixed_width_sign_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-mixed-width-sign-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("mixed-width-sign canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("mixed-width-sign canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected mixed-width/sign arithmetic with correct sign/zero extension to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_integer_casts_exit_canary_runs() {
    // Integer width/sign casts (sign-extend / zero-extend / truncate / reinterpret),
    // with each cast result threaded through a transition PARAM. This last part also
    // guards the fix for the dispatch-arg fold missing Cast/Binary arms: a let-local
    // whose initializer is a cast (or binary) reading a prior local was re-materialized
    // in the target state -- where the source local has no slot -- and read 0.
    let canary = pass_canary("arithmetic/runtime_integer_casts_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-integer-casts-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("integer-casts canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("integer-casts canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected integer width/sign casts threaded through params to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_i64_divide_modulo_exit_canary_runs() {
    // i64 signed divide/modulo with both operands immediate (constant/constant): the
    // byte-size resolver must fall back to the i64 target width, not 4, or the encoder
    // emits a 32-bit idiv (width mismatch + a truncated 64-bit dividend).
    let canary = pass_canary("arithmetic/runtime_i64_divide_modulo_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-i64-divmod-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("i64 divide/modulo canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("i64 divide/modulo canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i64 constant divide/modulo to run 64-bit and self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_float_compare_cast_exit_canary_runs() {
    // Float breadth: comparisons with negatives (the ucomisd unsigned-flag case),
    // f64/f32 arithmetic, int<->float and f32<->f64 casts, and nested-field float
    // arithmetic (a dot product). Self-checks to exit 70.
    let canary = pass_canary("arithmetic/runtime_float_compare_cast_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-float-breadth-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("float compare/cast canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float compare/cast canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected float comparisons/arith/casts/nested-field to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_float_operations_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_float_operations_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-float-ops-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("float-arithmetic canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float-arithmetic canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f64/f32 arithmetic, casts, local & nested-field float arith to be correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

/// ch15 stage 2 -- multi-path return-range inference: a callee returning via two
/// transition arms (3 / 7) infers the UNION [3,7], so the caller's `pick(b) + 63`
/// proves Exact. run(false) -> 70.
#[test]
fn runtime_inferred_multipath_return_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_inferred_multipath_return_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-inferred-multipath-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("multi-path inferred return range should let the caller prove Exact");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("multi-path inferred return canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected multi-path inferred-return narrowing to run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

/// ch15 stage 2 (modular return-range inference): a callee with NO declared
/// return range whose body bounds the result (`min(x, 3)`) lets the caller's
/// `classify(x) + 67` prove Exact via the INFERRED bound. run(100) -> 70.
#[test]
fn runtime_inferred_return_range_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_inferred_return_range_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-inferred-return-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("inferred return range should let the caller's arithmetic prove Exact");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("inferred return range canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected inferred-return-range narrowing to run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

/// #62: constructing a range-refined field from a PROVABLE non-literal value (a
/// same-range field) is accepted, not just integer literals. copy_box -> 70.
#[test]
fn runtime_provable_field_construction_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_provable_field_construction_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-provable-field-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("provable non-literal field construction should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("provable field construction canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected provable non-literal field construction to run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

/// Fact catalog over PLAIN STRUCT fields: a range-refined field `v: i32 [0..=15]`
/// of a param flows into the reader so `b.v + 65` proves Exact. Box{v:5} -> 70.
#[test]
fn runtime_struct_field_range_narrowing_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_struct_field_range_narrowing_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-struct-field-range-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("struct-field range narrowing should compile (constrained field discharges the obligation)");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("struct-field range narrowing canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected constrained struct field `b.v + 65` to run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

/// Stage 1 of the fact catalog over sum cases: a case-payload field's range
/// refinement (`index: i32 [0..=15]`) flows into the destructure arm so
/// `index + 65` proves Exact. Sound because construction enforces the range.
/// Found{index:5} -> 70.
#[test]
fn runtime_payload_range_narrowing_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_payload_range_narrowing_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-payload-range-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("payload range narrowing should compile (constrained payload discharges the obligation)");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("payload range narrowing canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected constrained payload `index + 65` to run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

/// The `a..b` (exclusive) / `a..=b` (inclusive) range refinement syntax that
/// replaced the removed `range<a, b>`: `x in 0..16` and `y in 0..=100` keep
/// `x + y` Exact. Runs to 70.
#[test]
fn runtime_exclusive_range_constraint_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_exclusive_range_constraint_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-exclusive-range-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("exclusive/inclusive range-constraint syntax should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("exclusive range constraint canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `0..16` + `0..=100` constrained sum to run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

/// Decision 17 S4: min/max clamp narrowing. `max(self.seed, 0)` lower-bounds at
/// 0 and `min(_, 60)` upper-bounds at 60, so `+ 70` stays EXACT. Without the
/// narrowing both clamps are unbounded and `+ 70` is a decision-17 overflow
/// error — so the program only COMPILES because the narrowing proves the bound
/// (and runs because the value-call-result materialization bug is fixed).
#[test]
fn runtime_min_max_clamp_narrowing_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_min_max_clamp_narrowing_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-min-max-clamp-narrowing-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("min/max clamp narrowing canary should compile (narrowing proves the bound)");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("min/max clamp narrowing canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected max(seed,0) then min(_,60) + 70 == 70 with seed=0, proven Exact by S4 \
         min/max narrowing (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

/// Decision 17 S4: modulo + division result-interval narrowing. `self.seed %
/// 100` bounds the remainder ([-99,99]) and `/ 2` keeps it bounded, so `+ 70`
/// stays EXACT. Without the narrowing both `%` and `/` are unbounded and the
/// `+ 70` is a decision-17 overflow error — so this program only COMPILES
/// because the narrowing proves the bound (seed ZII 0 → exit 70).
#[test]
fn runtime_modulo_div_narrowing_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_modulo_div_narrowing_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-modulo-div-narrowing-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("modulo/div narrowing canary should compile (narrowing proves the bound)");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("modulo/div narrowing canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected (seed%100)/2 + 70 == 70 with seed=0, proven Exact by S4 modulo/div \
         narrowing (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_trapping_mul_overflow_aborts() {
    // Decision 17: Trapping multiply overflow (100*100) aborts via ud2 -- never
    // reaches the transition. (No `_canary_runs` suffix so the differential drift
    // guard skips it.)
    let canary = pass_canary("expressions/arithmetic_domain_trapping_mul_overflow");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-arith-domain-trap-mul-of-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("arithmetic_domain_trapping_mul_overflow canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("arithmetic_domain_trapping_mul_overflow canary should run");
    assert!(
        !output.status.success()
            && output.status.code() != Some(70)
            && output.status.code() != Some(71),
        "expected Trapping mul overflow (100*100) to abort, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_saturating_signed_exit_canary_runs() {
    // Decision 17 S1b: signed `i8 in Saturating` clamps 100+100=200 to 127.
    let canary = pass_canary("expressions/arithmetic_domain_saturating_signed_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-arith-domain-sat-signed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("arithmetic_domain_saturating_signed canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("arithmetic_domain_saturating_signed canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i8 in Saturating (100+100) to clamp to 127 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_requires_proven_exact_exit_canary_runs() {
    // Decision 17 S4: a `requires`-bounded param (amount in [0,100]) proves
    // `amount + amount` in [0,200] -> exact (no domain). compute(35) -> 70.
    let canary = pass_canary("expressions/arithmetic_domain_requires_proven_exact_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-arith-domain-requires-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("arithmetic_domain_requires_proven_exact canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("arithmetic_domain_requires_proven_exact canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected requires-bounded exact `amount + amount` (compute(35)) to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_range_proven_exact_exit_canary_runs() {
    // Decision 17 S4: range-constraint narrowing proves `x + y` (each in [0,100])
    // is in [0,200], so it stays EXACT (no domain needed). 40+30=70.
    let canary = pass_canary("expressions/arithmetic_domain_range_proven_exact_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-arith-domain-range-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("arithmetic_domain_range_proven_exact canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("arithmetic_domain_range_proven_exact canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected range-bounded exact `x + y` (40+30) to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_cast_exit_canary_runs() {
    // Decision 17 S2: a domain `as` cast crosses domains -- `(a as u8 in
    // Saturating) + b` lets an exact `a` join saturating arithmetic; 200+100->255.
    let canary = pass_canary("expressions/arithmetic_domain_cast_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-arith-domain-cast-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("arithmetic_domain_cast canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("arithmetic_domain_cast canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `(a as u8 in Saturating) + b` (200+100) to clamp to 255 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_trapping_exit_canary_runs() {
    // Decision 17 S1b: `u8 in Trapping` runs normally when in range (100+50=150).
    let canary = pass_canary("expressions/arithmetic_domain_trapping_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-arith-domain-trapping-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("arithmetic_domain_trapping canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("arithmetic_domain_trapping canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected u8 in Trapping (100+50=150, in range) to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_trapping_overflow_aborts() {
    // Decision 17 S1b: `u8 in Trapping` ABORTS on overflow (200+100=300). The
    // native backend emits `ud2`, so the process never reaches the transition and
    // never exits 70/71 -- it terminates abnormally. (Named without the
    // `_canary_runs` suffix so the differential drift guard does not treat it as a
    // clean-exit run canary.)
    let canary = pass_canary("expressions/arithmetic_domain_trapping_overflow");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-arith-domain-trapping-of-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("arithmetic_domain_trapping_overflow canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("arithmetic_domain_trapping_overflow canary should run");

    assert_ne!(
        output.status.code(),
        Some(70),
        "expected u8 in Trapping overflow (200+100) to trap (abnormal exit), but it exited 70 \
         as if no overflow occurred\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_ne!(
        output.status.code(),
        Some(71),
        "expected u8 in Trapping overflow to trap BEFORE the transition, but it reached the \
         bad() arm (exit 71)\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "expected u8 in Trapping overflow to terminate abnormally, but it exited successfully"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_return_range_proven_exact_exit_canary_runs() {
    // Decision 17 S4: a range-constrained return (`-> i32 [0..=10]`) lets a
    // caller's exact arithmetic on the result stay Exact (5+5+60=70). Enforcement
    // (callee must return in range) makes trusting the range sound.
    let canary = pass_canary("expressions/arithmetic_domain_return_range_proven_exact_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-return-range-exact-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("return-range proven-exact canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("return-range proven-exact canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected return-range-narrowed exact arithmetic to exit 70; got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_trapping_const_fold_overflow_aborts() {
    // Decision 17 / task #39: a Trapping op with CONST operands that overflows
    // (u8 100*100=10000) must trap, even though the operands fold to a constant.
    // The const-store path re-emits a guaranteed-overflowing trapping op so the
    // encoder trap fires -- the process terminates abnormally (never 70/71).
    // Before the fix it silently wrapped to 16 and exited 70. Named without
    // `_canary_runs` so the differential drift guard treats it as non-clean-exit.
    let canary = pass_canary("expressions/arithmetic_domain_trapping_const_fold_overflow");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-arith-domain-trapping-const-of-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("trapping const-fold overflow canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("trapping const-fold overflow canary should run");

    assert_ne!(
        output.status.code(),
        Some(70),
        "expected const u8 Trapping 100*100 to trap (abnormal exit), but it exited 70 as if no \
         overflow occurred (silently wrapped)\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "expected const Trapping overflow to terminate abnormally, but it exited successfully"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn f32_field_binary_to_local_cast_exit_canary_runs() {
    // Scalar-width-rederivation fix: a folded f32 binary (`self.a + self.b`)
    // feeding `as i32` must compute single-precision (`addss`), not the old
    // hardcoded `addsd` over f32 bits. The binary operand threads its resolved
    // 4-byte width so producer (addss) and convert consumer (cvttss2si) agree.
    let canary = pass_canary("expressions/f32_field_binary_to_local_cast");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-f32-field-binary-local-cast-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("f32_field_binary_to_local_cast canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("f32_field_binary_to_local_cast canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32 field-binary-to-local-cast to yield 4 and exit 70, got {:?} (71 = addsd over f32 bits)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn f32_to_f64_local_cast_exit_canary_runs() {
    // Nested-cast width fix: `(self.src as f64) as i32` (a cast whose source is
    // a folded cast). classify now types a Cast as its target, so the convert
    // chain (cvtss2sd -> cvttsd2si) builds instead of the write being dropped.
    let canary = pass_canary("expressions/f32_to_f64_local_cast");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-f32-to-f64-local-cast-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("f32_to_f64_local_cast canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("f32_to_f64_local_cast canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32->f64-local->i32 cast chain to yield 7 and exit 70, got {:?} (71 = write dropped, n stayed 0)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn f32_deep_chain_binary_exit_canary_runs() {
    // Scalar-width-rederivation fix at depth: a left-chain f32 `a + b + c + d`
    // in a guard `s > 9.5`. Each nested binary threads its 4-byte width so
    // every level emits `addss`, not `addsd`. Depth 3 was where the old
    // re-derivation stopped agreeing.
    let canary = pass_canary("expressions/f32_deep_chain_binary");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-f32-deep-chain-binary-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("f32_deep_chain_binary canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("f32_deep_chain_binary canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32 depth-3 chain to sum to 10.0 (> 9.5) and exit 70, got {:?} (71 = wrong XMM result)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn no_payload_case_variant_after_payload_dispatch_exit_canary_runs() {
    // A no-payload case variant declared AFTER payload-bearing variants
    // (`AlarmEvent::Trigger`, ordinal 3) must be reachable when dispatched.
    // Was a native miscompile (bare-variant arg materialized as a place-copy,
    // not a tag write -> slot held ZII 0 -> only ordinal-0 matched -> exit 71).
    let canary = pass_canary("control_flow/no_payload_case_variant_after_payload_dispatch_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-no-payload-variant-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("no_payload_case_variant_after_payload_dispatch canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("no_payload_case_variant_after_payload_dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the Trigger arm (ordinal 3) to run twice and exit 70, got {:?} (71 = bare variant materialized as place, tag stayed 0)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn transition_arg_local_from_embedded_call_exit_canary_runs() {
    // A local whose initializer contains a value call, passed as a transition
    // argument, must copy the local's slot -- not fold+re-materialize the call
    // in the target state (whose scratch is unreachable). Was native exit 73.
    let canary = pass_canary("calls/transition_arg_local_from_embedded_call_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-transition-arg-embedded-call-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("transition_arg_local_from_embedded_call canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("transition_arg_local_from_embedded_call canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected r1 (=46) to pass as a transition arg and exit 70, got {:?} (73 = param slot never materialized)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn value_call_embedded_in_binary_exit_canary_runs() {
    // `let r = self.base + self.calc.double_val(6) * 3`: a value call embedded in
    // a binary. A read of `r` must resolve to its local slot (46), not the
    // embedded call's scratch result slot (12). Was a slot-name collision that
    // made the guard read the scratch -> native exit 71.
    let canary = pass_canary("calls/value_call_embedded_in_binary_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-call-embedded-binary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("value_call_embedded_in_binary canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("value_call_embedded_in_binary canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected base + double_val(6)*3 == 46 and exit 70, got {:?} (71 = read the embedded call's scratch slot)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sequential_self_field_rmw_exit_canary_runs() {
    // Sequential read-modify-write on a self field across 5 sub-machine calls
    // (`self.s.total = self.s.total + 1` in accum, called 5x) must accumulate
    // to 5. Guards against the stale-static-fold regression (the read folding
    // to the ZII entry value, emitting a constant store of 1 every call).
    let canary = pass_canary("calls/sequential_self_field_rmw_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sequential-self-field-rmw-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("sequential_self_field_rmw canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("sequential_self_field_rmw canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected 5 sequential RMW increments to total 5 and exit 70, got {:?} (72 = stale fold left total at 1)\nstderr:\n{}",
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
fn runtime_flat_boolean_logic_exit_canary_runs() {
    // Flat boolean logic in guards + value position: a && b, a || b, !b, a && c && !b,
    // and `let r = a && !b`. (The nested mix (a||b)&&c is a documented separate gap.)
    let canary = pass_canary("expressions/runtime_flat_boolean_logic_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-flat-boolean-logic-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("flat-boolean-logic canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("flat-boolean-logic canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected flat boolean logic (&&, ||, !, three-term, value-position) to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_enum_match_breadth_exit_canary_runs() {
    // Enum matching breadth + the SOUND pattern for a runtime-indexed enum element
    // (bind to a local first). grid[2]=Goal (non-first variant) and a field-name
    // collision (Potion.power vs Weapon.power) self-check to exit 70.
    let canary = pass_canary("expressions/runtime_enum_match_breadth_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-enum-match-breadth-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("enum match breadth canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("enum match breadth canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected indexed-via-local enum match + payload extraction to self-check (exit 70), got {:?}\nstderr:\n{}",
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
fn equatable_string_not_equals_exit_canary_runs() {
    // Equatable `!=` over a String-bearing record in VALUE position: the
    // simplifier De-Morgans `equality == false` into per-field `!=` compares,
    // so the String term lowers as the negated text-equals leaf
    // (`text_equals(..) == 0`). The names differ ("gold" vs "iron") while the
    // scalar siblings are equal, so dropping the String term (the old
    // miscompile: the whole initializer write silently vanished and the ZII
    // false took the bad arm) flips the exit code.
    let canary = pass_canary("traits/equatable_string_not_equals_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-equatable-string-not-equals-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("equatable string not-equals canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("equatable string not-equals canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `!=` on `Tag` to see the differing String content (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn equatable_string_equality_guard_exit_canary_runs() {
    // Equatable structural `==` over a String-bearing record DIRECTLY in
    // GUARD position: the conjunction's String clause routes through the
    // value-position TextEquals content compare (the raw 16-byte descriptor
    // place compare cannot encode), the scalar clause stays a place compare.
    // Equal contents take the `true` arm (exit 70).
    let canary = pass_canary("traits/equatable_string_equality_guard_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-equatable-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("equatable string guard canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("equatable string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected guard-position structural `==` on `Tag` to compare String content (exit 70), got {:?}\nstderr:\n{}",
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

/// Regression guard: a value-call (min/max builtin) result bound to a local and
/// then used in ARITHMETIC. The min-result local was elided as dead (the
/// liveness scan ignored later LocalData initializers), so `s = bounded + 70`
/// dropped its unresolved operand and s stayed ZII 0 (native exited 71). Fixed
/// by keeping the slot for any call-result initializer.
#[test]
fn runtime_min_call_result_arithmetic_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_min_call_result_arithmetic_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-min-call-result-arith-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("min-call-result arithmetic canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("min-call-result arithmetic canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected min(seed,60)+70 to materialize and equal 70 (exit 70); 71 = the \
         write was dropped (s stayed 0); got {:?}\n{}",
        output.status.code(),
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
fn runtime_param_domain_forward_exit_canary_runs() {
    // #66 domain-fact forwarding: an IMMUTABLE `&[u8] in Utf8` parameter, forwarded
    // as a call argument to another domained param, discharges `text in Utf8` via
    // the param's always-holding state invariant (caller-enforced `requires` +
    // immutability). Before the param-domain producer + direct state-invariant
    // consultation this rejected at compile time on the branch-dispatch path
    // (`consume: text in Utf8` saw 0 entry contexts).
    let canary = pass_canary("text/runtime_param_domain_forward_exit");
    let main_path = canary.join("main.omg");

    let checked = omega_compiler::compile_to_checked(&main_path, None)
        .expect("param domain forward canary should compile to checked trees");
    let outcome = omega_interpreter::interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for a forwarded domained param, got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-param-domain-forward-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("param domain forward canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("param domain forward canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected forwarded immutable domained param to discharge and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_case_payload_domain_forward_exit_canary_runs() {
    // #66 case-payload domain forwarding: a local constructed as `Command::First
    // { text: "ok" }` (payload `text: &[u8] in Utf8`) carries `cmd.<payload> in
    // Utf8` -- construction enforcement (#60-1c) proved it. Destructuring the
    // payload and forwarding it (`Command::First { text } -> consume(text)`)
    // discharges `consume: <payload> in Utf8` via the case-payload producer +
    // guarded-transition fallthrough threading. Before, this rejected at compile.
    let canary = pass_canary("text/runtime_case_payload_domain_forward_exit");
    let main_path = canary.join("main.omg");

    let checked = omega_compiler::compile_to_checked(&main_path, None)
        .expect("case payload domain forward canary should compile to checked trees");
    let outcome = omega_interpreter::interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for a forwarded case payload, got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-case-payload-domain-forward-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("case payload domain forward canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("case payload domain forward canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected forwarded case payload to discharge and exit 70, got {:?}\nstderr:\n{}",
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
fn runtime_array_indexed_read_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_array_indexed_read_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-array-indexed-read-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime array indexed read canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime array indexed read canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.nums[self.i]` (runtime index) to read 20 and 40 and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_struct_field_write_exit_canary_runs() {
    // A runtime-indexed STRUCT-FIELD write `arr[i].field = v` (array of structs)
    // must invalidate the whole array's folded constants so a later const read
    // `arr[2].field` sees live storage. Regression for the stale-fold that the
    // earlier `arr[i] = v` fix missed (the `Member(Indexed(..))` target shape).
    let canary = pass_canary("slices/runtime_indexed_struct_field_write_exit");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-runtime-indexed-struct-field-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime indexed struct-field write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime indexed struct-field write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `entities[i].field = v` then const read-backs to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_particle_system_exit_canary_runs() {
    // A 2D particle system over an array of structs: runtime-indexed struct-field reads
    // and writes, integrating pos += vel each step. Self-checks three cells -> exit 70.
    let canary = pass_canary("structs/runtime_particle_system_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-particle-system-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("particle system canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("particle system canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the particle system to integrate pos += vel correctly (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_struct_construction_exit_canary_runs() {
    // Nested struct construction `Rect { top_left: Point { .. }, .. }` PANICKED the
    // compiler (an arena span-contiguity assert: the field-value copy appended the
    // inner struct's fields mid-loop, interleaving the outer span). Fixed with
    // reserve-then-set. This canary self-checks the constructed nested fields.
    let canary = pass_canary("structs/runtime_nested_struct_construction_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-nested-struct-construct-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("nested struct construction canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("nested struct construction canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected nested struct construction (Rect of two Points) to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_call_to_array_element_exit_canary_runs() {
    // A single value-call result materializes correctly when written to a const-indexed array
    // element: triple(14)=42 lands at arr[2] with neighbours untouched -> exit 70. The working
    // write-side contrast to the value-call dispatch-position drop and the multi-call shared slot.
    let canary = pass_canary("calls/runtime_value_call_to_array_element_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-vc-array-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("value-call to array element canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("value-call to array element canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected triple(14)=42 written to arr[2] with neighbours 0 (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_computed_transition_args_exit_canary_runs() {
    // Computed values (an addition, a subtraction, a cast) passed directly as transition
    // arguments materialize correctly. chk(7+3, 7-3, 300 as u8) sees sum=10, diff=4, byte=44
    // -> exit 70. The working contrast to the value-call-as-transition-arg silent drop.
    let canary = pass_canary("calls/runtime_computed_transition_args_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-computed-args-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("computed transition args canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("computed transition args canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected computed transition args (sum 10, diff 4, byte 44) to materialize (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_struct_by_value_param_exit_canary_runs() {
    // Passing a struct BY VALUE into a value-machine and reading all its fields in distinct
    // positional weights. decode(Coeffs{1,2,3}) = 1*100 + 2*10 + 3 = 123 -> exit 70. Pins
    // the working envelope around task #15 (scalar fields of a by-value struct param resolve).
    let canary = pass_canary("calls/runtime_struct_by_value_param_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-struct-by-value-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("struct by-value param canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("struct by-value param canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected by-value struct param decode to yield 123 (exit 70); got {:?} (the decoded value on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_call_composition_exit_canary_runs() {
    // Function composition: chaining value-machine calls so each result feeds the next.
    // add_ten(5)=15, double(15)=30, minus_five(30)=25 -> exit 70. (Sequential binding; the
    // nested form f(g(x)) is a clean error today, documented in the canary.)
    let canary = pass_canary("calls/runtime_value_call_composition_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-value-call-composition-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("value call composition canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("value call composition canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected three-stage value-call pipeline to yield 25 (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_struct_value_call_exit_canary_runs() {
    // A value-machine that computes and RETURNS a struct (product type), completing the
    // value-call return-type map alongside scalars and sum-type returns. stats(7,3) returns
    // a record whose two independently-computed fields are 10 and 4 -> exit 70.
    let canary = pass_canary("calls/runtime_struct_value_call_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-struct-value-call-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("struct value call canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("struct value call canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected value-call to return a record with sum 10 and diff 4 (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_option_value_call_exit_canary_runs() {
    // A value-machine that RETURNS an Option (Some/None), called in a loop with each result
    // matched -- the idiomatic functional shape for find/lookup/parse. classify(x) over
    // [5,-3,7] yields two present values and one absent; the present values sum to 12 and
    // one absent is counted -> exit 70.
    let canary = pass_canary("calls/runtime_option_value_call_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-option-value-call-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("option value call canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("option value call canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Option-returning value-call to sum Somes=12 and count 1 None (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_result_match_exit_canary_runs() {
    // Result-style error handling at runtime: a two-case enum (Ok/Err) produced
    // conditionally, then matched and handled in a loop. Safe-dividing 10/2, 7/0, 20/4
    // sums the Ok values to 10 and counts 1 Err -> exit 70.
    let canary = pass_canary("errors/runtime_result_match_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-result-match-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("result match canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("result match canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Ok/Err handling to sum Oks=10 and count 1 Err (exit 70); got {:?} (the sum on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_entity_component_exit_canary_runs() {
    // An array of entities each holding a nested component struct (the entity-component
    // pattern): runtime-indexed access through a member path (`self.ents[i].pos.x`) read in
    // a loop and temp-RMW written back. Three entities pos.x = 1,2,3: sum 6, doubled to
    // 2,4,6 -> exit 70.
    let canary = pass_canary("structs/runtime_entity_component_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-entity-component-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("entity component canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("entity component canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the entity-component array (sum 6, doubled nested fields) to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_struct_state_machine_exit_canary_runs() {
    // A state machine whose state lives in nested structs: a nested-vs-nested guard
    // subject, nested-field RMW, a cross-struct write, and a two-way nested verify. The
    // runtime-indexed-ARRAY guard bug does NOT extend to member paths -- these resolve the
    // correct field. Sums 1..5 = 15 -> exit 70.
    let canary = pass_canary("structs/runtime_nested_struct_state_machine_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-nested-struct-sm-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("nested struct state machine canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("nested struct state machine canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the nested-struct state machine to sum 1..5 = 15 (exit 70); got {:?} (a non-70 code is the bad sum)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_struct_value_semantics_exit_canary_runs() {
    // Deep nesting + whole-struct value semantics: a 3-level nested field read AND
    // write, a whole-struct copy by assignment, and copy independence (overwriting the
    // source leaves the copy intact). The data backbone of serious apps.
    let canary = pass_canary("structs/runtime_nested_struct_value_semantics_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-nested-struct-value-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("nested struct value-semantics canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("nested struct value-semantics canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected 3-level nesting + whole-struct copy + copy independence to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_struct_array_literal_exit_canary_runs() {
    // Composite literal nesting: a struct literal with an array-literal field AND an
    // array-of-struct-literals field. Guards the expression-handle + struct-field copy
    // paths near the nested-struct panic fix. Self-checks the constructed values.
    let canary = pass_canary("structs/runtime_struct_array_literal_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-struct-array-literal-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("struct-array literal canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("struct-array literal canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a struct literal with array + struct-array fields to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_enum_struct_payload_exit_canary_runs() {
    // An enum variant with a STRUCT-typed payload `Event::Click(at: Point, ..)`. The
    // payload field's named type symbol was never resolved (the resolution pass
    // skipped variant payload fields), so the layout builder errored. Now construct +
    // match + read the struct payload's fields.
    let canary = pass_canary("structs/runtime_enum_struct_payload_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-enum-struct-payload-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("enum struct-payload canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("enum struct-payload canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected enum variant with a struct payload to construct/match/extract (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_write_const_read_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_indexed_write_const_read_exit");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-indexed-write-const-read-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("indexed-write/const-read canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("indexed-write/const-read canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a runtime-indexed write to invalidate whole-array constants so const-indexed reads see live storage (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_rmw_temp_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_indexed_rmw_temp_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-indexed-rmw-temp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("indexed-rmw-temp canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("indexed-rmw-temp canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the temp-field RMW idiom over a runtime-indexed array to accumulate (the copy write must invalidate the array's folded constants) (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_write_adjacent_field_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_indexed_write_adjacent_field_exit");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-indexed-write-adjacent-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("indexed-write-adjacent-field canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("indexed-write-adjacent-field canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a runtime-indexed write to load the index 32-bit (not pull in the adjacent field as the high dword -> OOB) (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_join_meet_bound_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_join_meet_bound_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-join-meet-bound-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("join-meet-bound canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("join-meet-bound canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the predecessor meet to carry an index bound to a multi-predecessor join (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_array_indexed_loop_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_array_indexed_loop_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-array-indexed-loop-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime array indexed loop canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime array indexed loop canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a runtime-indexed loop to sum the array to 100 and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_decreasing_index_exit_canary_runs() {
    // A DECREASING runtime counter indexing an inline array: the bound
    // `self.i < 4` that the body's `self.nums[self.i]` needs is a loop
    // INVARIANT (entry `i = 3`, each `i = i - 1` decrement preserves `i < 4`),
    // not the loop guard (`self.i >= 0`). The loop head is multi-predecessor, so
    // single-predecessor incoming-guard seeding can't reach it; the inductive
    // loop-invariant fact discharges the index obligation. Sums [1,2,3,4]
    // backwards to 10 and self-checks (exit 70).
    let canary = pass_canary("slices/runtime_decreasing_index_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-decreasing-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime decreasing index canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime decreasing index canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a decreasing-counter loop (loop-invariant bound) to sum [1,2,3,4] \
         backwards to 10 and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_slice_indexed_read_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_slice_indexed_read_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-slice-indexed-read-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime slice indexed read canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime slice indexed read canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `s[self.i]` (runtime index on a &[T] slice) to read 20 and 40 and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_array_adjacent_index_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_array_adjacent_index_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-runtime-adjacent-index-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime adjacent-index canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime adjacent-index canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the DERIVED index `nums[j + 1]` (bound carried across `jp = j + 1`) to walk adjacent pairs and confirm the array is sorted (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_decreasing_index_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_nested_decreasing_index_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-runtime-nested-decreasing-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime nested-decreasing-index canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime nested-decreasing-index canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected NESTED decreasing loops -- the inner counter's invariant proven via dominance-based back edges, the outer invariant held through the inner loop -- to sum to 54 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_narrow_widen_cast_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_narrow_widen_cast_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-runtime-narrow-widen-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime narrow-widen-cast canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime narrow-widen-cast canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an inline narrow widening cast to extend by signedness -- u8>127 zero-extends (sum 806), i8<0 sign-extends (-5) (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_signed_index_guarded_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_signed_index_guarded_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-runtime-signed-index-guarded-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime signed-index-guarded canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime signed-index-guarded canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a SIGNED i32 index proven non-negative by its `>= 0` guard to be accepted and sum nums[3..0] to 10 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_two_pointer_sum_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_two_pointer_sum_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-runtime-two-pointer-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime two-pointer-sum canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime two-pointer-sum canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the two-pointer traversal to prove nums[i] via the relational chain (i <= j < len) and sum converging pairs to 210 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_two_pointer_reverse_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_two_pointer_reverse_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-runtime-two-pointer-reverse-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime two-pointer-reverse canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime two-pointer-reverse canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected two-pointer in-place reverse (indexed WRITE targets proved via the relational chain) to reverse [1..5] to [5..1] (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_branched_index_bound_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_branched_index_bound_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-runtime-branched-bound-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime branched-index-bound canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime branched-index-bound canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a loop bound to carry TRANSITIVELY across a conditional branch so the indexed read in the branch target proves, re-reading 99 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_array_write_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_indexed_array_write_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-runtime-indexed-write-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime indexed-array-write canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime indexed-array-write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a runtime-indexed array WRITE of a field value (`nums[self.i] = self.v`) to fill nums[i]=i+100 and read 103 back at index 3 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn recursive_subslice_element_accumulator_exit_canary_runs() {
    // `sum(s[1..], acc + s[0])`: the element read s[0] must happen before the
    // s descriptor is retargeted to s[1..]. Was an off-by-one (descriptor
    // advanced first -> summed the next window's head -> native exit 71).
    let canary = pass_canary("slices/recursive_subslice_element_accumulator_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-recursive-subslice-accum-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("recursive subslice element accumulator canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("recursive subslice element accumulator canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected sum([5,10,15,20]) == 50 via sum(s[1..], acc + s[0]) and exit 70, got {:?} (71 = descriptor advanced before s[0] read)\nstderr:\n{}",
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
fn runtime_machine_field_subslice_arg_index_exit_canary_runs() {
    // Passing a BARE subslice of a machine fixed-array field (`self.source[0..3]`,
    // no `.as_slice()`) as a `&[u8]` argument must materialize a correct
    // {ptr,len} descriptor. The literal-subslice descriptor writer only knew
    // `x.as_slice()[a..b]` bases, so a bare base declined and the argument fell
    // through to a garbage copy (wrong len AND elements natively). Exits 70.
    let canary = pass_canary("slices/runtime_machine_field_subslice_arg_index_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-machine-field-subslice-arg-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("machine-field subslice arg canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("machine-field subslice arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a bare machine-field subslice passed as a slice arg to carry a correct descriptor (exit 70), got {:?}\nstderr:\n{}",
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
fn runtime_indexed_read_operand_exit_canary_runs() {
    // A runtime-indexed read `self.nums[self.i]` used as a SUB-EXPRESSION OPERAND
    // (a child of `+` and of an `as i64` cast), hoisted into synthetic
    // `let __hoist_N = self.nums[self.i];` temps. Exits 70 when acc == 20 and
    // big == 20.
    let canary = pass_canary("slices/runtime_indexed_read_operand_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-indexed-operand-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("runtime indexed read operand canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("runtime indexed read operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected hoisted runtime-indexed operand reads (binary + cast) to lower and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_subslice_len_exit_canary_runs() {
    // A `&[u8]` bound to a literal fixed-array subslice (`self.source[0..2]`)
    // and used only for `.len` is inlined to `(self.source[0..2]).len`; the
    // length must FOLD to the window width `b - a` (2), not fall through to a
    // place read with no descriptor slot. Exits 70 when `s.len == 2`.
    let canary = pass_canary("slices/runtime_subslice_len_exit");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-subslice-len-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("subslice len canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("subslice len canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `(arr[0..2]).len` to fold to 2 (exit 70), got {:?}\nstderr:\n{}",
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
fn runtime_local_aggregate_into_let_exit_canary_runs() {
    // A local ARRAY literal read by a subsequent `let` (`let arr = [..]; let e = arr[1]`)
    // silently yielded 0: the liveness scan never inspected LocalData (`let`) values, so
    // the read-only array was elided (no slot) and the indexed read resolved against a
    // missing slot. Fixed by keeping the slot for an array-literal local referenced in a
    // later let value (array-only -- borrow-carrying structs must stay folded).
    let canary = pass_canary("slices/runtime_local_aggregate_into_let_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-local-aggregate-into-let-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("local-aggregate-into-let canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("local-aggregate-into-let canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a local array element read into a subsequent let (and used as a value) to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_field_array_element_value_operand_exit_canary_runs() {
    // A field array's indexed element as a VALUE OPERAND: passed to a value-call, and
    // read into a let then forwarded as a transition arg. Works for FIELD arrays; the
    // local-array form (`let arr = [..]; let e = arr[i]`) silently yields 0 -- a
    // machine-indexed-value-operand gap tracked separately.
    let canary = pass_canary("slices/runtime_field_array_element_value_operand_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-field-array-value-operand-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("field-array value-operand canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("field-array value-operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a field-array element used as a value-call arg / let-then-transition-arg to self-check (exit 70), got {:?}\nstderr:\n{}",
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
fn runtime_value_call_return_types_exit_canary_runs() {
    // Value-returning calls across return types (i32 / struct / enum / bool) + the
    // un-nested nested-call pattern. Locks the working value-call core. (A value-call
    // written directly as an arg to another VALUE-call miscompiles -- tracked
    // separately; the sound form is to bind the inner call to a local first.)
    let canary = pass_canary("calls/runtime_value_call_return_types_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-value-call-return-types-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("value-call return-types canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("value-call return-types canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected value-calls returning i32/struct/enum/bool to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_call_struct_result_to_target_exit_canary_runs() {
    // Delivering a value-call STRUCT result: dispatch scalar -> field, bare-body struct
    // -> field, and dispatch struct -> local -> field all work. (A dispatch-body value-
    // call returning a struct assigned DIRECTLY to a field silently stores 0 -- tracked
    // separately; bind to a local first.)
    let canary = pass_canary("calls/runtime_value_call_struct_result_to_target_exit");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-value-call-struct-result-target-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("value-call struct-result-to-target canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("value-call struct-result-to-target canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected value-call struct-result delivery (working cases + the local workaround) to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_call_self_field_enum_match_exit_canary_runs() {
    // A value-call dispatching on an ENUM FIELD of self (`transition self.s { .. }`),
    // called twice with different field values to prove real dispatch. (A method on the
    // enum TYPE matching bare `self`, called `self.s.sides()`, mis-dispatches -- tracked
    // separately; dispatching on a self field or a param both work.)
    let canary = pass_canary("calls/runtime_value_call_self_field_enum_match_exit");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-value-call-self-field-enum-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("value-call self-field-enum-match canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("value-call self-field-enum-match canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a value-call dispatching on a self enum field to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_call_struct_literal_arms_exit_canary_runs() {
    // A value-call whose transition arms return STRUCT / enum-CASE literals
    // (`transition d { Dir::E -> Vec2 { dx: 1, dy: 0 } ... }`). This was a parse error
    // (a struct-literal arm value is name-like, so the target parser read only the
    // leading path and left the `{`); fixed by re-parsing a path-followed-by-`{` arm
    // value as an expression. The natural "dispatch on an enum, return a struct" shape.
    let canary = pass_canary("calls/runtime_value_call_struct_literal_arms_exit");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-value-call-struct-lit-arms-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("value-call struct-literal-arms canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("value-call struct-literal-arms canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a value-call returning struct/case literals from its arms to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_contained_machine_exit_canary_runs() {
    // A contained machine (component with state): single-instance method calls --
    // statement-call mutation, arg, and a value-call return -- all work. (Multiple
    // contained machines of the SAME type alias to the first; tracked separately.)
    let canary = pass_canary("calls/runtime_contained_machine_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-contained-machine-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("contained-machine canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("contained-machine canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected contained-machine method calls (increment/add_to/get) to self-check (exit 70), got {:?}\nstderr:\n{}",
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
fn runtime_sleep_exit_canary_runs() {
    // Clock.sleep -- a kernel32 `Sleep(ms)` native call, the first host op beyond the
    // original four. Reaching exit_process(70) after the call proves the Win64 ABI
    // (shadow space, ecx arg, clean non-terminal return) is correct.
    let canary = pass_canary("host/runtime_sleep_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-runtime-sleep-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("sleep canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("sleep canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `sleep(2)` and `sleep(self.delay)` to return cleanly and the program to reach exit_process(70); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_write_no_newline_exit_canary_runs() {
    // `write` (Stdout, no trailing newline) vs `write_line`. The differential oracle
    // checks the exact stdout ("ABC\n"); this run-test just confirms it exits 70.
    let canary = pass_canary("host/runtime_write_no_newline_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-write-no-newline-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("write-no-newline canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("write-no-newline canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `write`+`write_line` to print 'ABC\\n' and exit 70; got {:?}\nstderr:\n{}",
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
fn borrow_carrying_data_field_exit_canary_runs() {
    // Borrow-carrying data (decision 15 stage 2/3): constructing `Msg { body:
    // &self.cell }` and reading the reference field `message.body` extracts the
    // borrowed `&Cell`, which is dereferenced through a `&Cell` ref parameter.
    // Both the interpreter oracle AND the native backend must exit 70 (a 0/71
    // exit is the pre-fix bug where a struct-literal-rooted field read resolved
    // to no place and left the target zero).
    let canary = pass_canary("expressions/borrow_carrying_data_field_exit");
    let main_path = canary.join("main.omg");

    let checked = omega_compiler::compile_to_checked(&main_path, None)
        .expect("borrow-carrying data canary should compile to checked trees");
    let outcome = omega_interpreter::interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should read the borrowed field as 70, got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-borrow-carrying-data-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("borrow-carrying data canary should compile to a PE");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("borrow-carrying data canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native backend should read the borrowed field as 70 (matching the interpreter); \
         got {:?}\nstderr:\n{}",
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

// #66 carrier stdin round-trip: `read_line` into a `[u8; 64] in Utf8` carrier
// (stdin straight into the inline bytes + len), then `write_line` the carrier back.
// #66 carrier command-LOOP: each prompt reads a line into a `[u8; 16]` carrier,
// resolves it to a Command enum via a value-call, and loops until `quit`. Exercises
// every branch (Look loops, Invalid loops, Quit exits) so the loop genuinely
// re-reads + re-resolves -- the String original (reverted) was a broken orphan that
// always returned Look.
#[test]
fn contained_loop_command_branch_carrier_canary_runs() {
    let canary = run_canary("contained_loop_command_branch");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-contained-loop-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("carrier command-loop canary should compile");

    let mut child = Command::new(build_dir.join(executable_name()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("carrier command-loop canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"look\nzzz\nlook\nquit\n")
        .expect("carrier command-loop input should be written");
    let output = child
        .wait_with_output()
        .expect("carrier command-loop canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected carrier command-loop canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "look\ninvalid\nlook\n",
        "expected each loop iteration to re-resolve its own command (Look, Invalid, Look) then quit"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 carrier command-loop with a health gate: each iteration checks health, reads
// a line into a `[u8; 16]` carrier, resolves a Command, and loops until `quit`.
#[test]
fn contained_health_loop_command_branch_carrier_canary_runs() {
    let canary = run_canary("contained_health_loop_command_branch");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-contained-health-loop-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("carrier health-loop canary should compile");

    let mut child = Command::new(build_dir.join(executable_name()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("carrier health-loop canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"look\nzzz\nquit\n")
        .expect("carrier health-loop input should be written");
    let output = child
        .wait_with_output()
        .expect("carrier health-loop canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected carrier health-loop canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "look\ninvalid\n",
        "expected the health-gated loop to resolve each command (Look, Invalid) then quit"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 carrier sequential reads: two `read_line`s into the same `[u8; 64]` carrier,
// each echoed -- the second read must overwrite the first line's bytes + length.
#[test]
fn runtime_stdin_line_buffering_carrier_canary_runs() {
    let canary = pass_canary("text/runtime_stdin_line_buffering_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-runtime-line-buffering-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("carrier line buffering canary should compile");

    let mut child = Command::new(build_dir.join(executable_name()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("carrier line buffering canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"first\nsecond\n")
        .expect("carrier line buffering input should be written");
    let output = child
        .wait_with_output()
        .expect("carrier line buffering canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected carrier line buffering canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "first\nsecond\n",
        "expected each carrier read_line to echo its own line, the second overwriting the first"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// #66 carrier stdin round-trip: `read_line` into a `[u8; 64] in Utf8` carrier
// (stdin straight into the inline bytes + len), then `write_line` the carrier back.
#[test]
fn runtime_text_storage_carrier_canary_runs() {
    let canary = pass_canary("text/runtime_text_storage");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir()
        .join(format!("omega-runtime-text-storage-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("carrier text storage canary should compile");

    let mut child = Command::new(build_dir.join(executable_name()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("carrier text storage canary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(b"echo me\n")
        .expect("carrier text storage input should be written");
    let output = child
        .wait_with_output()
        .expect("carrier text storage canary should finish");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected carrier text storage canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "> echo me\n",
        "expected the carrier read_line to round-trip the input line back through write_line"
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
fn value_call_sequential_result_slots_exit_canary_runs() {
    // Two sequential value-position calls where callee 1 (`f`) has an internal
    // `let rr = r * r` binding and callee 2 (`g`) takes MORE arguments.
    //
    // Root cause: the leaf branch expansion for `f` fired at the StateCall op,
    // emitting a copy from frame[rr] to frame[a1_result] BEFORE the callee's
    // spliced LocalStorage op wrote `rr = r*r = 9` into frame[rr].  The stale
    // read (rr == 0 at that point) set a1_result = 0, so `a1 + 61` yielded 61
    // and the program exited 71.
    //
    // After the fix: the deferral condition detects callee-body LocalStorage
    // ops after the StateCall and defers the leaf expansion to after the LAST
    // such op, so `rr` is written before the copy fires.
    let canary = pass_canary("calls/value_call_sequential_result_slots_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-call-sequential-result-slots-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("sequential result slots canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("sequential result slots canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a1 = f(3) = 9, a2 = g(5,8) = 40, self.v = a1 + 61 = 70 (exit 70); \
         exit 71 = a1 was 0 (stale read: leaf expansion fired before rr was written), \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_saturating_const_fold_exit_canary_runs() {
    // Decision 17 / task #39: const*const Saturating overflow must clamp, not wrap.
    let canary = pass_canary("expressions/arithmetic_domain_saturating_const_fold_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-sat-const-fold-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("saturating const-fold canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("saturating const-fold canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected u8 Saturating 100*100 to clamp to 255 (exit 70); exit 71 = wrapped to 16          (const-fold dropped the domain). got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn value_call_sequential_self_capture_exit_canary_runs() {
    // Regression coverage for sequential value-position calls whose terminal
    // value is a SELF-CAPTURED local (`let s = self.seed; transition { _ -> s }`)
    // -- a bare-local return, not arithmetic. This is the self-capture variant
    // of the leaf-expansion stale-read family; it is already handled by the
    // callee-body LocalStorage deferral landed for
    // value_call_sequential_result_slots_exit (the captured-field READ is a
    // LocalStorage op the deferral waits on). Pinned so the bare self-capture
    // shape cannot silently regress. exit 70 = a1 = cap() = self.seed = 9,
    // a2 = add(40) = 49, self.v = a1 + 61 = 70.
    let canary = pass_canary("calls/value_call_sequential_self_capture_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-call-sequential-self-capture-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("sequential self-capture canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("sequential self-capture canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a1 = cap() = self.seed = 9, a2 = add(40) = 49, self.v = a1 + 61 = 70; \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_f64_state_arg_exit_canary_runs() {
    // Verifies that f64 values forwarded through a transition arm state
    // argument (`transition { _ -> store_flt(x) }`) arrive with the correct
    // IEEE-754 bits in the callee state.  Previously the Float literal path
    // was absent from `static_runtime_argument_value`, so the 8-byte parameter
    // slot was never written and the callee received zero-bits.  Bug 11
    // (2026-06-12): fixed in argument_materialization.rs by adding an explicit
    // ExpressionNode::Float branch that writes the bit-pattern via
    // WriteRuntimeStorageInteger.  exit 72 = bad_flt (wrong bits); exit 71 =
    // bad_int (regression); exit 70 = both args arrived correctly.
    let canary = pass_canary("expressions/runtime_f64_state_arg_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-f64-state-arg-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("f64 state arg canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("f64 state arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f64 state arg (3.14 > 3.0) and i32 state arg (42 == 42) to pass (exit 70); \
         exit 72 = f64 bits wrong, exit 71 = i32 wrong, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_let_local_nested_state_arg_exit_canary_runs() {
    // Verifies that a `let`-bound local whose initializer is a pure place read
    // (e.g. `let slot: i32 = self.s.count`) is forwarded correctly through a
    // nested state argument chain across repeated calls.  Previously argument
    // materialization folded the Name expression back to its initializer
    // (re-evaluating `self.s.count`) instead of reading the LocalStorage frame
    // slot that captured the pre-mutation value.  On the second call the
    // already-incremented count was substituted, causing `try1` to take the
    // wrong dispatch arm.  Bug 10 (2026-06-12): fixed in
    // argument_materialization.rs by blocking the fold when the local has a
    // LocalStorage slot and its initializer is a pure place expression.  exit
    // 72 = wrong arm (set2 taken instead of set1); exit 70 = correct.
    let canary = pass_canary("calls/runtime_let_local_nested_state_arg_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-let-local-nested-state-arg-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("let-local nested state arg canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("let-local nested state arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected arr[0]==500 and arr[1]==200 after two `put` calls (exit 70); \
         exit 72 = set2 arm wrongly taken (slot re-read post-increment), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
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
        .nth(4)
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

fn run_canary(path: &str) -> PathBuf {
    repo_root().join("canaries/run").join(path)
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
    "concurrency/runtime_spawn_struct_result_exit",
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
    "domains/domain_param_membership_satisfied",
    "domains/domain_param_forwarded",
    "domains/slice_carrier_domain",
    "domains/slice_domain_validator",
    "domains/utf8_slice_ops",
    "domains/utf8_literal_arg",
    "domains/utf8_value_call_field_write",
    "domains/utf8_field_write_from_param",
    "domains/utf8_field_read_carries_domain_exit",
    "domains/domain_field_write_then_read_exit",
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
    "control_flow/no_payload_case_variant_after_payload_dispatch_exit",
    "control_flow/case_payload_shared_field_name_exit",
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
    "calls/runtime_attached_machine_struct_arg_exit",
    "calls/by_value_case_param_self_write_exit",
    "calls/runtime_explicit_discard_executes_exit",
    "calls/runtime_free_machine_struct_arg_exit",
    "calls/runtime_free_machine_struct_return_exit",
    "calls/sequential_self_field_rmw_exit",
    "calls/transition_arg_local_from_embedded_call_exit",
    "calls/value_call_embedded_in_binary_exit",
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
    "slices/runtime_array_indexed_read_exit",
    "slices/runtime_array_indexed_loop_exit",
    "slices/runtime_decreasing_index_exit",
    "slices/runtime_slice_indexed_read_exit",
    "slices/runtime_array_adjacent_index_exit",
    "slices/runtime_nested_decreasing_index_exit",
    "slices/runtime_narrow_widen_cast_exit",
    "slices/runtime_signed_index_guarded_exit",
    "slices/runtime_two_pointer_sum_exit",
    "slices/runtime_two_pointer_reverse_exit",
    "slices/runtime_branched_index_bound_exit",
    "slices/runtime_indexed_array_write_exit",
    "arithmetic/runtime_modulo_value",
    "arithmetic/runtime_modulo_div_narrowing_exit",
    "arithmetic/runtime_min_max_clamp_narrowing_exit",
    "arithmetic/runtime_transition_arg_guard_narrowing_exit",
    "arithmetic/runtime_transition_arg_false_arm_narrowing_exit",
    "arithmetic/runtime_transition_arg_saturating_exit",
    "arithmetic/runtime_cast_element_accumulator_exit",
    "arithmetic/runtime_exclusive_range_constraint_exit",
    "arithmetic/runtime_payload_range_narrowing_exit",
    "arithmetic/runtime_struct_field_range_narrowing_exit",
    "arithmetic/runtime_provable_field_construction_exit",
    "arithmetic/runtime_inferred_return_range_exit",
    "arithmetic/runtime_inferred_multipath_return_exit",
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
    "slices/runtime_indexed_read_operand_exit",
    "slices/runtime_subslice_len_exit",
    "slices/runtime_machine_field_subslice_arg_index_exit",
    "slices/recursive_subslice_element_accumulator_exit",
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
    "traits/equatable_string_not_equals_exit",
    "traits/equatable_string_equality_guard_exit",
    "traits/equatable_sum_payload_equality_exit",
    "termination/default_order_nat_countdown_compile",
    "termination/default_order_slice_length_compile",
    "termination/default_order_bounded_distance_compile",
    "termination/bounded_distance_named_view",
    "termination/default_order_unsigned_width_countdown_compile",
    "termination/runtime_shrinking_slice_recursion_exit",
    // --- Language-guide chapter coverage (Ch1-22) ---
    "calls/runtime_local_string_field_copy_through_mut_exit",
    "calls/runtime_min_call_result_arithmetic_exit",
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
    "calls/value_call_sequential_result_slots_exit",
    "calls/value_call_sequential_self_capture_exit",
    "operators/integer_literal_suffix_exit",
    "operators/runtime_shift_operators_exit",
    "operators/runtime_bitwise_operators_exit",
    "operators/runtime_bitwise_guard_exit",
    "operators/runtime_xorshift_prng_exit",
    "operators/runtime_popcount_loop_exit",
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
    "expressions/float_array_binary_op_zero",
    "expressions/f32_array_binary_op_zero",
    "expressions/arithmetic_domain_wrapping_exit",
    "expressions/arithmetic_domain_saturating_exit",
    "expressions/arithmetic_domain_saturating_mul_exit",
    "expressions/arithmetic_domain_saturating_const_fold_exit",
    "expressions/arithmetic_domain_return_range_proven_exact_exit",
    "expressions/arithmetic_domain_saturating_mul_signed_exit",
    "expressions/arithmetic_domain_trapping_mul_exit",
    "expressions/arithmetic_domain_trapping_div_exit",
    "expressions/arithmetic_domain_trapping_mul_overflow",
    "expressions/arithmetic_domain_saturating_signed_exit",
    "expressions/arithmetic_domain_trapping_exit",
    "expressions/arithmetic_domain_trapping_overflow",
    "expressions/arithmetic_domain_cast_exit",
    "expressions/arithmetic_domain_range_proven_exact_exit",
    "expressions/arithmetic_domain_requires_proven_exact_exit",
    "expressions/f32_field_binary_to_local_cast",
    "expressions/f32_deep_chain_binary",
    "expressions/f32_to_f64_local_cast",
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
    "generics/machine_bound_satisfied_at_value_call",
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
    // version block with MORE fields than the current body (regression 2026-06-12)
    "versioning/version_block_more_fields_than_current",
    "versioning/version_block_three_fields_vs_one",
    "versioning/version_block_v1_more_than_current",
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
    "wire/runtime_wire_encode_byte_slice_exit",
    "wire/runtime_wire_decode_byte_slice_exit",
    "wire/runtime_wire_decoded_byte_slice_len_exit",
    "wire/runtime_wire_decoded_byte_slice_index_exit",
    "wire/runtime_wire_roundtrip_repeated_exit",
    "wire/runtime_wire_decode_rejects_repeated_overflow_exit",
    // --- 2026-06-12 canary coverage sweep (feature-edge additions) ---
    "wire/runtime_wire_roundtrip_repeated_max_one_exit",
    "wire/runtime_wire_encode_repeated_then_string_exit",
    "wire/runtime_wire_roundtrip_nested_and_repeated_exit",
    "comptime/runtime_const_array_length_transitive_exit",
    "comptime/runtime_const_array_length_bare_call_arm_exit",
    "data/property_send_declared",
    "data/property_zero_init_nested_array",
    "data/runtime_case_membership_mixed_shape_exit",
    "traits/runtime_equatable_scalar_not_equals_guard_exit",
    "borrow/runtime_view_of_view_chain_exit",
    "borrow/runtime_method_view_write_after_last_use_exit",
    // --- ch17 atomics (concurrency stage 1) ---
    "atomics/atomic_field_declared",
    "atomics/runtime_atomic_load_store_exit",
    "atomics/runtime_atomic_fetch_add_exit",
    "atomics/runtime_atomic_compare_exchange_exit",
];

const ACTIVE_FAIL_CANARIES: &[&str] = &[
    "expressions/arithmetic_domain_mixed",
    "expressions/nested_i32_mul_overflow",
    "arithmetic/removed_range_constraint_syntax",
    "arithmetic/construction_payload_out_of_range",
    "arithmetic/unconstrained_payload_arithmetic",
    "arithmetic/bounded_assignment_unproven",
    "arithmetic/struct_field_arithmetic_unproven",
    "arithmetic/transition_arg_unguarded_overflow",
    "domains/type_constraint_unknown_domain",
    "domains/domain_carrier_mismatch",
    "domains/domain_param_requires_membership",
    "domains/domain_field_write_raw_value",
    "domains/literal_violates_classifier",
    "domains/domain_field_read_no_write_unproven",
    "expressions/arithmetic_domain_literal_target_overflow",
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
    "generics/machine_bound_value_call_unchecked",
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
    "ranges/loop_increment_index_unbounded",
    "ranges/loop_body_resets_index",
    "ranges/loop_init_exceeds_capacity",
    "ranges/index_join_unbounded_arm",
    "ranges/index_read_after_increment_oob",
    "ranges/index_read_after_decrement_negative",
    "ranges/index_signed_guard_below_zero",
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
// machine_bound_value_call_unchecked was promoted to fail/generics/ when
// `validate_value_position_calls` landed in omega-validation/src/calls.rs:
// the machine-call type-parameter bound check now runs for VALUE-position
// calls (`let r = self.pick(&self.h)`) via an expression-tree walker that
// mirrors the statement-position `validate_call_node` path.  A companion
// pass canary (generics/machine_bound_satisfied_at_value_call) pins the
// accepted side: `[copy]`-satisfying data types and scalars compile fine.
// versioned_match_missing_current_arm was promoted to fail/versioning/ when
// version-match exhaustiveness counting landed (the decidable arm set of a
// `Versioned<T>` subject is {each declared era vN} + {current};
// crate::exhaustiveness in omega-symbol-resolved-trees-to-typed-trees).
// const_array_length_bare_call_arm was promoted to
// pass/comptime/runtime_const_array_length_bare_call_arm_exit when the
// parenthesized-lone-call arm body became a VALUE expression (the parser
// defers; symbol assignment re-classifies back into a state transition only
// for sibling-state and self-recursion callees).
// 2026-06-12 canary coverage sweep: all five bugs it pinned as pending were
// fixed and promoted the same day --
// - traits/equatable_string_not_equals_value -> pass/traits/
//   equatable_string_not_equals_exit (String `!=` lowers as the negated
//   TextEquals leaf instead of dropping the String term).
// - traits/equatable_string_equality_guard_unlowered -> pass/traits/
//   equatable_string_equality_guard_exit (guard-position String place
//   compares route through TextEquals).
// - concurrency/spawn_struct_result_miscompiled -> pass/concurrency/
//   runtime_spawn_struct_result_exit (by-value struct RETURNS: leaf
//   terminal-value StructLiteral substitution + call-result-backed locals
//   keep their name); the direct no-spawn spelling is pinned by
//   calls/runtime_free_machine_struct_return_exit.
// - versioning/versioned_match_missing_current_arm -> fail/versioning/
//   (version-match exhaustiveness counting landed).
// - comptime/const_array_length_bare_call_arm -> pass/comptime/
//   runtime_const_array_length_bare_call_arm_exit (parenthesized lone-call
//   arm bodies are value expressions; sibling-state callees re-classify).
#[allow(dead_code)]
// The f32 scalar-width family + the sequential self-field RMW stale-fold all
// closed 2026-06-14 and are now pass RUN canaries. The one remaining pending
// entry is a native/interpreter DIVERGENCE (not a one-sided miscompile) whose
// fix is gated on a maintainer SEMANTICS decision about i32 overflow in a
// nested value operand -- see the canary header. Tracked as CurrentlyAccepts
// (it compiles); deliberately NOT a RUN canary until the semantics is settled,
// so the differential oracle is not asked to adjudicate an undecided question.
// Empty: nested_i32_mul_overflow_divergence was promoted to a FAIL canary
// (expressions/nested_i32_mul_overflow) once decision 17 S3 made the unprovable
// i32 multiply a compile error -- the divergence was a symptom of accepting an
// unprovable overflow, now rejected.
const ACTIVE_PENDING_CANARIES: &[PendingCanary] = &[];

// =============================================================================
// ch17 Atomics (concurrency stage 1) RUN canaries
// =============================================================================

/// M2 -- AtomicU32 load/store round-trip.  `store(42, Relaxed)` writes then
/// `load(Relaxed)` reads back.  On x86_64 both lower to plain aligned `mov`
/// (TSO gives acquire-release for free; SeqCst store frontier documented).
#[test]
fn runtime_atomic_load_store_exit_canary_runs() {
    let canary = pass_canary("atomics/runtime_atomic_load_store_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-atomic-load-store-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("atomic load/store canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("atomic load/store canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected atomic store(42)+load to read back 42 and exit 70; \
         exit 71 = value mismatch; got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

/// M3 -- AtomicU32 fetch_add: returns PRIOR value, increments cell.
/// Stage-1 desugar: `let old = place.fetch_add(n, ord)` → `let old = place;
/// place = place + n;`.  Two successive fetch_add calls; guard ladder checks
/// both prior values AND the post-add cell values.
#[test]
fn runtime_atomic_fetch_add_exit_canary_runs() {
    let canary = pass_canary("atomics/runtime_atomic_fetch_add_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-atomic-fetch-add-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("atomic fetch_add canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("atomic fetch_add canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected fetch_add(5) old=10/new=15, fetch_add(8) old=15/new=23 (exit 70); \
         exit 71=bad old1, 72=bad after first, 73=bad old2, 74=bad after second; \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

/// M4 -- AtomicU32 compare_exchange: returns PRIOR value; swaps only when
/// *place == expected.
/// Stage-1 desugar: `let prior = place.compare_exchange(expected, new, s, f)`
/// → `let prior = place; place = prior + (prior == expected) * (new - prior);`
/// Success path: CAS(10, 99) when counter==10 → prior==10, counter becomes 99.
/// Failure path: CAS(10, 42) when counter==99 → prior==99, counter stays 99.
#[test]
fn runtime_atomic_compare_exchange_exit_canary_runs() {
    let canary = pass_canary("atomics/runtime_atomic_compare_exchange_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-atomic-cmpxchg-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("atomic compare_exchange canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("atomic compare_exchange canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected compare_exchange success(10→99, prior=10) + failure(10 vs 99, prior=99, cell=99) \
         (exit 70); exit 71=bad prior_s, 72=bad after swap, 73=bad prior_f, 74=bad after fail; \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}
