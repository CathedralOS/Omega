use super::*;

#[test]
fn runtime_stdin_command_branch_exit_canary_runs() {
    let canary = pass_canary("text/runtime_stdin_command_branch_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-stdin-command-branch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime stdin command branch canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime stdin command branch canary should retain its executable receipt");
    let mut child = Command::new(executable)
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
    let build_dir =
        std::env::temp_dir().join(format!("omega-contained-loop-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
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

// The ERGONOMIC Filesystem wrapper natively on windows_x64 -- every result
// SHAPE asserted end to end: write_all -> Ok; create_dir twice ->
// Error{AlreadyExists} (the error PAYLOAD through the nested last_error);
// open -> Ok{File} destructured and USED (read through the File value-call
// arg -- the alias-resolved literal `count`); a REAL close (rc 0, previously
// a silently-dropped terminal host call); remove -> Ok; remove missing ->
// Error{NotFound}. Interpreter-first differential. Windows-gated like the
// raw roundtrip.
// Previously-untested ergonomic wrapper methods on windows_x64: create
// (writable), sync (_commit), try_clone (dup -- a File round-tripped through
// a value call), set_permissions (chmod). set_permissions was BROKEN
// (passed `perms.mode`, a member of a by-value struct param, directly to
// chmod -- unresolved under some dispatch contexts); the wrapper now
// captures it into a `perm_mode` scratch field first (the `file_fd` idiom).
// Interpreter-first differential; windows-gated like the raw roundtrip.
#[cfg(windows)]
#[test]
fn windows_fs_wrapper_dark_methods_exit_canary_runs() {
    let canary = pass_canary("filesystem/windows_wrapper_dark_methods_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("dark-methods canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (create/sync/try_clone/set_permissions/remove), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-fs-dark-methods-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dark-methods canary should compile from its authored root");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("dark-methods canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the dark wrapper methods to run (exit 70), got {:?}          (71 create; 73 sync; 74 try_clone; 75 clone close; 76 set_permissions; 77 remove)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[cfg(windows)]
#[test]
fn windows_fs_wrapper_results_exit_canary_runs() {
    let canary = pass_canary("filesystem/windows_wrapper_results_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("windows fs wrapper canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (wrapper result shapes on the virtual fs), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-fs-win-wrapper-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("windows fs wrapper canary should compile from its authored root");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("windows fs wrapper canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected every wrapper result shape to deliver natively (exit 70), got {:?} \
         (71 write_all; 72/81/73 AlreadyExists leg; 74 open; 75/76/77 File read leg; \
         82 close rc; 78/79/80 remove legs)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A host-call RESULT bound to a LOCAL in a DISPATCHING state gets a frame slot
// (the dispatch-body storage builder gained a HostCall arm). Opening an absent
// path returns -1 on both the native seam and the virtual fs, so `fd < 0` is
// deterministic; a missed result slot would read ZII and take the wrong arm.
#[cfg(windows)]
#[test]
fn runtime_local_host_result_dispatch_exit_canary_runs() {
    let canary = pass_canary("filesystem/runtime_local_host_result_dispatch_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("local-host-result canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (open of absent path -> fd < 0), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-local-host-result-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("local-host-result canary should compile from its authored root");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("local-host-result canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the let-local host result to reach fd < 0 (exit 70), got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The FIRST windows_x64-native fs raw-seam roundtrip (msvcrt bindings through
// the general Win64 import call): create/write/close/open/read/close/verify/
// remove, then an ENOENT + errno probe (the deref-result `_errno()` path).
// Interpreter-first: the virtual fs is the differential oracle. Windows-gated:
// the macOS fs coverage lives in native_filesystem_canaries.
#[cfg(windows)]
#[test]
fn windows_fs_raw_roundtrip_exit_canary_runs() {
    let canary = pass_canary("filesystem/windows_raw_roundtrip_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("windows fs roundtrip canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (virtual-fs roundtrip + ENOENT errno), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-fs-win-roundtrip-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("windows fs roundtrip canary should compile from its authored root");

    // Run from the temp build dir so the probe file lands there, not the repo.
    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("windows fs roundtrip canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the full native fs roundtrip incl. errno==ENOENT (exit 70), got {:?} \
         (71-76 = create/write/open/read/verify/remove failed; 77 = removed file still \
         opens; 78 = errno wrong)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A SELF value-call that forwards a PATH LITERAL into a raw host call
// (`self.probe("lit")` -> `self.raw.open(path, ..)`). Before the fix the
// forwarded literal's data object -- keyed to the CALLER's statement -- was
// missed by the callee-keyed lookup (the alias binding resolves a SELF call to
// the CALLEE's key), so `open` got no path operand: "no encodable call
// sequence" on x86_64, "no result storage operand" on aarch64. The lookup now
// falls back to a bytes-only match (every data object with identical bytes is
// the same read-only C string). Interpreter-first oracle; native run pinned.
#[cfg(windows)]
#[test]
fn windows_fs_self_value_call_literal_path_exit_canary_runs() {
    let canary = pass_canary("filesystem/self_value_call_literal_path_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("self-value-call literal path canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (self-call literal reached open), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-fs-self-literal-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("self-value-call literal path canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("self-value-call literal path canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the self-call-forwarded literal to reach open (exit 70), got {:?} \
         (71 = create failed; 72 = self-call open failed [the bug]; 73 = cleanup \
         remove failed)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The DISCARDED-result twin of the test above: `_ = self.doit("lit")` lowers
// through the STATEMENT-call path (real argument materialization), so the
// callee's open executes with the delivered literal. Discriminating: the path
// is absent, so errno must be ENOENT (2) -- a dropped call leaves errno 0, a
// garbled path yields a different errno. Promoted from
// pending/host/self_value_call_literal_arg.
#[cfg(windows)]
#[test]
fn windows_fs_discarded_self_call_literal_errno_exit_canary_runs() {
    let canary = pass_canary("filesystem/discarded_self_call_literal_errno_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("discarded self-call literal canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (discarded open executed, errno ENOENT), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-fs-discard-literal-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("discarded self-call literal canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("discarded self-call literal canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the discarded self-call's open to execute with the literal path \
         (errno == ENOENT -> exit 70), got {:?} (71 = wrong errno -- dropped call \
         or garbled path)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Caller fields SHADOWING the wrapper's param names (`buffer`/`count`) must
// not capture its host-call operands -- pins the alias-rewrite-first operand
// ordering (a shadowed buffer swallowed the read; a shadowed ZII count
// requested 0 bytes). Discriminating: binary write/read roundtrip lands in
// the SPELLED buffer with the SPELLED count, byte-exactly.
#[cfg(windows)]
#[test]
fn windows_fs_wrapper_param_shadow_exit_canary_runs() {
    let canary = pass_canary("filesystem/wrapper_param_shadow_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("wrapper param-shadow canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (shadowed params never capture operands), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-fs-param-shadow-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("wrapper param-shadow canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("wrapper param-shadow canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the roundtrip to land in the spelled buffer/count (exit 70), got {:?} \
         (71 = write failed; 72 = short read [count shadow]; 73 = buffer shadow \
         [bin_back stayed ZII]; 74/75 = byte mismatch)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// `Filesystem::open_with` (Rust OpenOptions) end-to-end through the selected
// target package's checked foreign-flag encoder. Six legs: write+create / read /
// truncate / append / create_new-on-existing / read-absent.
#[cfg(windows)]
#[test]
fn windows_fs_wrapper_open_with_exit_canary_runs() {
    let canary = pass_canary("filesystem/wrapper_open_with_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("open_with matrix canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (full OpenOptions matrix), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-fs-open-with-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("open_with matrix canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("open_with matrix canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the full OpenOptions matrix (exit 70), got {:?} (71/72 = \
         write+create leg; 73/74 = read leg; 75 = truncate; 76 = append; \
         77 = create_new not AlreadyExists; 78 = absent not NotFound; \
         79 = cleanup)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A value-machine METHOD call through a FIELD receiver (`self.meta_f.is_file()`)
// -- the idiom the param-receiver fence points to. Discriminating: is_file()
// must be true AND agree with the inline mode-bits twin.
#[cfg(windows)]
#[test]
fn windows_fs_field_receiver_method_exit_canary_runs() {
    let canary = pass_canary("filesystem/field_receiver_method_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("field-receiver method canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (is_file true through the field receiver), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-fs-field-recv-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("field-receiver method canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("field-receiver method canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected is_file() through the field receiver (exit 70), got {:?} \
         (71 = metadata error; 72 = method mis-delivered; 73 = inline twin \
         disagreed; 74 = cleanup failed)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A host-call RESULT let in a callee reached via an ARM transition-target
// value call (`true -> self.e1()`) resolves its frame slot in the inlining
// dispatch case (branch_transition_target_key's self/sibling Nested arm).
// Discriminating: errno must be ENOENT (2), not ZII 0. Windows-gated with
// the other native-fs canaries (the raw open needs a live seam).
#[cfg(windows)]
#[test]
fn runtime_arm_target_host_result_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_arm_target_host_result_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("arm-target host-result canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (errno 2 through the arm target), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-arm-target-result-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("arm-target host-result canary should compile from its authored root");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("arm-target host-result canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected errno 2 fetched through the arm-target callee (exit 70), got {:?}          (71 = the missing file opened; 72/73 = wrong errno at either depth)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A real qualified enum case in value position resolves before the closed
// two-segment existence check. `Signal::Green` is the second case, so both
// engines must preserve its discriminator and exit 70; a silent ZII/Red
// resolution would take the exit-71 arm.
#[test]
fn runtime_qualified_case_value_exit_canary_runs() {
    let canary = pass_canary("expressions/runtime_qualified_case_value_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("qualified-case-value canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (Signal::Green in value position), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-qualified-case-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("qualified-case-value canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("qualified-case-value canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("qualified-case-value canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a real qualified case in value position to stay accepted, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// SINGLE-TARGET PARADIGM INTERNALS: a machine implemented by exactly ONE
// non-selected target (the windows dir-walk's find-enumeration helpers on a
// posix compile) filters SILENTLY with its callers -- the loud missing-row
// edge is reserved for names two or more targets implement (the fail canary's
// demo_target/demo_target2 pair). Both engines run the program to 70 with the
// inert internal present.
#[test]
fn single_target_internal_machine_skipped_canary_runs() {
    let canary = pass_canary("targets/single_target_internal_machine_skipped");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("single-target internal canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 past the filtered internal, got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-single-target-internal-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("single-target internal canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("single-target internal canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("single-target internal canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the program to run past the filtered single-target internal (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn target_machine_gating_exit_canary_runs() {
    // TARGET-SCOPED MACHINES (fs portable-contract settle 2026-07-18):
    // `pick` comes from `local_unchecked` (= host everywhere) and `delta`
    // from the host's real target while THREE inert same-name machines sit
    // beside it -- 63 + 7 reaches 70 only if selection and inertness both
    // hold in both engines.
    let canary = pass_canary("targets/target_machine_gating_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("target-machine canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (63 + 7 through two selected target machines), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-target-machine-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("target-machine canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("target-machine canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("target-machine canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the selected target machines to deliver 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// create_new UNFENCED on windows_x64: the wrapper now asks the selected target
// policy to encode its semantic OpenOptions, so Windows emits msvcrt
// O_CREAT|O_EXCL (not darwin's O_CREAT
// 0x200 == msvcrt O_TRUNC). Discriminating: create_new on an existing file
// returns AlreadyExists (proves O_EXCL took effect, no truncation). NATIVE-ONLY:
// the interpreter now decodes the HOST's flag numerology (host_open_flags), so
// it matches the host-targeted program -- interp oracle + native, differential.
#[cfg(windows)]
#[test]
fn windows_wrapper_create_new_exit_canary_runs() {
    let canary = pass_canary("filesystem/windows_wrapper_create_new_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("create_new canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (create_new + O_EXCL AlreadyExists), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-fs-create-new-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("create_new canary should compile from its authored root");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("create_new canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected create_new + O_EXCL AlreadyExists (exit 70), got {:?}          (71 create; 72 close; 73 second create not Error; 74 kind not AlreadyExists)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn ring_requirement_satisfies_exit_canary_runs() {
    // SINGLE-REQUIREMENT trait conformance (rearrange settle, rung A): the
    // settled CommutativeSemiring surface -- free-shaped requirements, an
    // ensures LAW, `satisfies Trait::req [as Alias]`, Self binding the
    // carrier -- with the satisfier machines actually RUN (2 + 1 = depth 3).
    let canary = pass_canary("traits/ring_requirement_satisfies_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("ring-requirement canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (2 + 1 = depth 3 through the conformed ops), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-ring-requirement-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("ring-requirement canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("ring-requirement canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("ring-requirement canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the conformed ring ops to deliver depth 3 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The find-enumeration seam trio (fs portable-contract rung 3a):
// find_first/find_next/find_close over kernel32 FindFirstFileA/FindNextFileA/
// FindClose, the windows dir-walk paradigm behind the portable contract.
// WINDOWS-HOST ONLY by design: posix targets have no lowering for the trio
// (their impls walk dirent records), so the canary lives outside the
// cross-host sweep lists and this gated test is its runner. Interp + native
// differential (the hermetic find-cursor model mirrors Win32 semantics:
// dots first, snapshot-at-open, directory bit at data[0], name at data[44]).
#[cfg(windows)]
#[test]
fn windows_find_enumeration_exit_canary_runs() {
    let canary = pass_canary("filesystem/windows_find_enumeration_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("find-enumeration canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (dots + a.txt + b.txt + end + close), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-find-enum-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("find-enumeration canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("find-enumeration canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the find trio walk to exit 70 (71 setup; 72 find_first; 73 \".\"; 74 \"..\"; 75 a.txt; 76 b.txt/dir-bit; 77 end; 78 close), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The read_dir_nth WRAPPER composition over the find seam (the seam trio is
// pinned above). The trailing-dir shape is the KIND-LATCH witness: the scan
// drain keeps classifying records after capturing the target, so reading
// the RUNNING w_scan_kind after the drain reported the LAST record's kind
// -- every file child claimed is_dir whenever the directory's last record
// was a dir. The impl latches w_hit_kind at the hit now. Interp + native.
#[cfg(windows)]
#[test]
fn windows_read_dir_nth_exit_canary_runs() {
    let canary = pass_canary("filesystem/windows_read_dir_nth_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("read_dir_nth canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (file aaa, dir ccc, then End), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-read-dir-nth-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("read_dir_nth canary should compile from its authored root");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("read_dir_nth canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the read_dir_nth walk to exit 70 (75 = child 0 reported as a dir, the kind-latch regression; header lists the rest), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The RAW set_file_time seam op (session slice 4b): kernel32 SetFileTime
// over the handle bridge, hand-built FILETIME, stat round-trip @40. The
// wrapper has its own end-to-end round-trip; this raw pin keeps the seam and
// calibration independently honest. WINDOWS-HOST ONLY (raw windows ops have no
// posix lowering), outside the cross-host sweep lists like the find trio.
#[cfg(windows)]
#[test]
fn windows_set_file_time_exit_canary_runs() {
    let canary = pass_canary("filesystem/windows_set_file_time_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("set_file_time canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (stamp + stat round-trip), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-win-sft-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("set_file_time canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("set_file_time canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the set_file_time round-trip to exit 70 (75 = a FILETIME calibration slip; header lists the rest), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Wrapper half of the windows SetFileTime slice. Unlike the raw seam canary,
// this value-calls Filesystem::set_times into a UnitResult field and therefore
// pins the mutation-heavy-entry expansion. Its build file also keeps the three
// POSIX target implementations selected and checked after the portable body's
// migration into target files.
#[test]
fn filesystem_set_times_target_implementations_compile() {
    let canary = pass_canary("filesystem/windows_wrapper_set_times_exit");
    for target in [
        "windows_x86_64",
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
    ] {
        compile_to_checked(&canary.join("main.omg"), Some(target))
            .unwrap_or_else(|d| panic!("set_times wrapper should check for {target}:\n{d:#?}"));
    }
}

#[cfg(windows)]
#[test]
fn windows_wrapper_set_times_exit_canary_runs() {
    let canary = pass_canary("filesystem/windows_wrapper_set_times_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("windows set_times wrapper canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (wrapper stamp + metadata round-trip), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-win-wrapper-times-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("windows set_times wrapper canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("windows set_times wrapper canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the windows set_times wrapper round-trip to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn filesystem_lock_target_implementations_compile() {
    let canary = pass_canary("filesystem/windows_wrapper_lock_exit");
    for target in [
        "windows_x86_64",
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
    ] {
        compile_to_checked(&canary.join("main.omg"), Some(target))
            .unwrap_or_else(|d| panic!("lock wrappers should check for {target}:\n{d:#?}"));
    }
}

#[cfg(windows)]
#[test]
fn windows_wrapper_lock_exit_canary_runs() {
    let canary = pass_canary("filesystem/windows_wrapper_lock_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("windows lock wrapper canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (exclusive/shared contention), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-win-wrapper-lock-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("windows lock wrapper canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("windows lock wrapper canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the windows lock wrapper canary to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The WRAPPER canonicalize contract on windows (session slice 4a): msvcrt
// has no realpath, so the windows impl composes the HANDLE BRIDGE -- open,
// _get_osfhandle, GetFinalPathNameByHandleA (the \\?\-prefixed DOS path),
// close. The canary discriminates per-model first bytes ('\\' native / 'o'
// hermetic) and pins the NotFound leg (the open's errno is captured before
// the trailing close can clobber it). Interp + native.
#[test]
fn windows_canonicalize_canary_is_targetless_and_interprets() {
    let canary = pass_canary("filesystem/windows_canonicalize_exit");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("windows canonicalize canary should compile to checked trees");
    assert_eq!(
        checked.selected_program_entry_machine(),
        None,
        "targetless checking must not select an authored target entry"
    );
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.error, None,
        "hermetic canonicalize should not decline: {:?}",
        outcome.error
    );
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should resolve files/directories and preserve NotFound"
    );
}

#[cfg(windows)]
#[test]
fn windows_canonicalize_exit_canary_runs() {
    let canary = pass_canary("filesystem/windows_canonicalize_exit");

    let build_dir = std::env::temp_dir().join(format!("omega-win-canon-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_target(&canary, build_dir.clone(), "windows_x86_64")
        .expect("windows canonicalize canary should compile from its Windows root");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("windows canonicalize canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the canonicalize walk to exit 70 (73 = the resolved buffer starts with neither model's spelling; header lists the rest), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The WRAPPER hard-link contract on windows (session slice 3): msvcrt has
// no link(2), so the windows impl rides the designed create_hard_link seam
// op (kernel32 CreateHardLinkA -- Win32 arg order (NEW link, existing) +
// the NULL security-attributes arg, BOOL result). Engine-agnostic legs:
// create+readback, link-survives-removal, taken-name refuses (kind
// pinned as AlreadyExists through immediate GetLastError capture). Interp + native.
#[cfg(windows)]
#[test]
fn windows_hard_link_exit_canary_runs() {
    let canary = pass_canary("filesystem/windows_hard_link_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("windows hard-link canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (link + survive + taken-name refusal), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-win-hardlink-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("windows hard-link canary should compile from its authored root");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("windows hard-link canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the hard-link walk to exit 70 (72 = CreateHardLinkA refused; header lists the rest), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The WRAPPER positioned-io contract on windows (session slice 2): msvcrt
// fds have no pread/pwrite, so the windows_x64 impl COMPOSES save-cursor /
// seek / op / restore over the wired _lseeki64/_read/_write rows. The
// canary pins the cursor contract directly (a plain read after both
// positioned ops still starts at byte 0). Interp + native.
#[cfg(windows)]
#[test]
fn windows_positioned_io_exit_canary_runs() {
    let canary = pass_canary("filesystem/windows_positioned_io_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("windows positioned-io canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (write_at + read_at + cursor unmoved), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-win-pio-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("windows positioned-io canary should compile from its authored root");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("windows positioned-io canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the positioned-io walk to exit 70 (77 = the cursor moved, the restore leg regressed; header lists the rest), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Native windows metadata via msvcrt `_stat64` (the per-target stat-offset
// migration payoff). The wrapper's `decode_metadata` reads `stat_buf[ST_*_OFF + k]`
// -- a pure-const binary index that now const-folds -- at the windows `_stat64`
// offsets from the selected StatLayout policy. Discriminating: a written 6-byte
// file reports len 6 and a regular-file st_mode (S_IFREG 0x8000, not S_IFDIR).
// NATIVE + interp differential: the interpreter mirrors the host stat layout
// (host_stat_offsets), so it matches the host-targeted program.
#[cfg(windows)]
#[test]
fn windows_wrapper_metadata_exit_canary_runs() {
    let canary = pass_canary("filesystem/windows_wrapper_metadata_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("windows metadata canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (metadata len + regular-file mode), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-fs-metadata-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("windows metadata canary should compile from its authored root");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("windows metadata canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected native _stat64 metadata (exit 70), got {:?}          (71 write; 72 not Ok; 73 wrong len; 74 not regular; 75 is dir)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// `exists`/`try_exists` UNFENCED on windows as a side effect of the stat
// migration: both consult only `read_metadata`'s return code (not the decoded
// record), so wiring msvcrt `_stat64` made them lower natively with no extra
// seam work. Discriminating: absent -> false/No, after write -> true/Yes.
// NATIVE + interp differential.
#[cfg(windows)]
#[test]
fn windows_wrapper_exists_exit_canary_runs() {
    let canary = pass_canary("filesystem/windows_wrapper_exists_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("windows exists canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (exists/try_exists absent->present), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-fs-exists-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("windows exists canary should compile from its authored root");

    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("windows exists canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected native stat-based exists/try_exists (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// `set_len` UNFENCED on windows via msvcrt `_chsize_s` (ftruncate's 64-bit
// analogue): create empty -> extend to 10 -> metadata len == 10. NATIVE + interp.
#[cfg(windows)]
#[test]
fn windows_wrapper_set_len_exit_canary_runs() {
    let canary = pass_canary("filesystem/windows_wrapper_set_len_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("set_len canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "set_len interpreter oracle should exit 70, got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-fs-set-len-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("set_len canary should compile from its authored root");
    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("set_len canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected native set_len extend (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// `copy` UNFENCED on windows (set_len wired + the chmod mode arg moved to a field
// so it no longer elides into a computed host-call argument). NATIVE + interp.
#[cfg(windows)]
#[test]
fn windows_wrapper_copy_exit_canary_runs() {
    let canary = pass_canary("filesystem/windows_wrapper_copy_exit");
    let main_path = canary.join("main.omg");
    let checked =
        compile_to_checked(&main_path, None).expect("copy canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "copy interpreter oracle should exit 70, got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-fs-copy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("copy canary should compile from its authored root");
    let output = Command::new(build_dir.join(executable_name()))
        .current_dir(&build_dir)
        .output()
        .expect("copy canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected native copy (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn cross_windows_general_imports_compile() {
    // ENT2c: exercise general and composite plan-driven imports through full PE
    // layout/emission on every development host: seven-argument GUI, key/time,
    // the dedicated byte-at-a-time line reader, and a source external call.
    for canary_name in [
        "host/runtime_gui_window_lifecycle_exit",
        "host/runtime_user32_key_state_exit",
        "time/runtime_time_host_native_exit",
        "text/runtime_stdin_line_buffering_exit",
        "capabilities/windows_provides_import_exit",
    ] {
        let canary = pass_canary(canary_name);
        let scratch = std::env::temp_dir().join(format!(
            "omega-win-plan-import-{}-{}",
            canary_name.replace('/', "-"),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let src_dir = scratch.join("src");
        fs::create_dir_all(&src_dir).expect("scratch source directory");
        fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
        fs::write(src_dir.join("build.omg"), "target windows_x86_64 {\n}\n")
            .expect("write windows target manifest");
        compile(CanaryCompileSpec {
            root_path: src_dir.join("main.omg"),
            build_dir: Some(scratch.join("out")),
            target_name: Some("windows_x86_64".to_owned()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostic| {
            panic!("{canary_name} should cross-compile for windows_x64: {diagnostic:?}")
        });
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn cross_aarch64_stack_import_compiles_with_planned_layout() {
    let canary = pass_canary("capabilities/aarch64_stack_import_compile");
    let scratch =
        std::env::temp_dir().join(format!("omega-aarch64-stack-import-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(src_dir.join("build.omg"), "target macos_arm64 {\n}\n")
        .expect("write macos_arm64 target manifest");

    compile(CanaryCompileSpec {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("macos_arm64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("nine-argument import should compile for macos_arm64");

    let image = fs::read(out_dir.join("omega-program")).expect("read emitted AArch64 Mach-O");
    for (name, instruction) in [
        ("16-byte outgoing reserve", 0xd100_43ffu32),
        ("ninth argument store", 0xf900_03eau32),
        ("outgoing stack restore", 0x9100_43ffu32),
    ] {
        let bytes = instruction.to_le_bytes();
        assert!(
            image.windows(4).any(|window| window == bytes),
            "AArch64 stack-import image missing {name}"
        );
    }
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn native_fixed_arrays_classify_by_value_without_pointer_decay() {
    let canary = pass_canary("capabilities/native_fixed_array_import_compile");
    for (target, expected_float) in [
        ("windows_x86_64", "aggregate 16/4"),
        ("linux_x86_64", "hfa4x32"),
        ("macos_arm64", "hfa4x32"),
    ] {
        let scratch = unique_no_output_build_dir();
        let src_dir = scratch.join("src");
        let build_dir = scratch.join("out");
        fs::create_dir_all(&src_dir).expect("fixed-array scratch source directory");
        fs::copy(canary.join("main.omg"), src_dir.join("main.omg"))
            .expect("copy fixed-array canary");
        fs::write(
            src_dir.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write fixed-array target manifest");
        let compile_result = compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: src_dir.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some(target.to_owned()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        });
        if let Err(diagnostics) = compile_result {
            let only_unbound_elf_imports = target == "linux_x86_64"
                && diagnostics.iter().all(|diagnostic| {
                    diagnostic
                        .message
                        .contains("relocation references unknown symbol")
                });
            assert!(
                only_unbound_elf_imports,
                "fixed-array boundary canary should reach image binding for {target}: {diagnostics:#?}"
            );
        }
        let report = fs::read_to_string(build_dir.join("backend_report.txt"))
            .expect("fixed-array compile should publish its backend report");
        assert!(
            report.contains("aggregate 16/1"),
            "{target} must classify `[u8; 16]` as a by-value aggregate:\n{report}"
        );
        assert!(
            report.contains(expected_float),
            "{target} must preserve `[f32; 4]`'s target aggregate class:\n{report}"
        );
        assert!(
            report.contains("address &"),
            "{target} must keep `&[u8; 16]` as the distinct pointer form:\n{report}"
        );
        let _ = fs::remove_dir_all(scratch);
    }
}

#[test]
fn cross_win64_distinguishes_separate_pointer_length_from_descriptor_record() {
    let canary = pass_canary("capabilities/win64_pointer_length_vs_descriptor_compile");
    let scratch = unique_no_output_build_dir();
    let src_dir = scratch.join("src");
    let build_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("pointer/length scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg"))
        .expect("copy pointer/length canary");
    fs::write(src_dir.join("build.omg"), "target windows_x86_64 {\n}\n")
        .expect("write windows_x64 target manifest");

    compile_with_auxiliary_artifacts(CanaryCompileSpec {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("windows_x86_64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("pointer/length shape canary should compile for windows_x64");

    let report = fs::read_to_string(build_dir.join("backend_report.txt"))
        .expect("pointer/length compile should publish its backend report");
    assert!(
        report.contains("(scalar i32 omega_machine_Main::main_storage@32, scalar i64 omega_machine_Main::main_storage@0, scalar i64 omega_machine_Main::main_storage@8)"),
        "the separate declaration must remain two scalar Win64 arguments:\n{report}"
    );
    assert!(
        report.contains("(scalar i32 omega_machine_Main::main_storage@32, aggregate 16/8 omega_machine_Main::main_storage@16)"),
        "the descriptor declaration must remain one 16-byte aggregate argument:\n{report}"
    );

    let image = fs::read(build_dir.join("omega-program.exe"))
        .expect("read emitted pointer/length Win64 PE");
    assert!(
        image
            .windows(4)
            .any(|window| window == [0x48, 0x83, 0xec, 40]),
        "the separate scalar call must need only ordinary Win64 shadow space"
    );
    assert!(
        image
            .windows(4)
            .any(|window| window == [0x48, 0x83, 0xec, 56]),
        "the descriptor call must reserve shadow space plus its aligned caller copy"
    );
    for copy_offset in [32u32, 40] {
        let mut store = vec![0x48, 0x89, 0x84, 0x24];
        store.extend(copy_offset.to_le_bytes());
        assert!(
            image.windows(store.len()).any(|window| window == store),
            "expected descriptor fragment at outgoing stack offset {copy_offset}"
        );
    }
    assert!(
        image
            .windows(8)
            .any(|window| window == [0x48, 0x8d, 0x8c, 0x24, 32, 0, 0, 0]),
        "the descriptor must occupy one argument slot as an RCX pointer to the caller copy"
    );
    let _ = fs::remove_dir_all(scratch);
}

#[test]
fn cross_aarch64_hfa_import_compiles_with_fragmented_plan() {
    let canary = pass_canary("capabilities/aarch64_hfa_import_compile");
    let scratch =
        std::env::temp_dir().join(format!("omega-aarch64-hfa-import-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(src_dir.join("build.omg"), "target macos_arm64 {\n}\n")
        .expect("write macos_arm64 target manifest");

    compile(CanaryCompileSpec {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("macos_arm64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("by-value HFA import should compile for macos_arm64");

    let image = fs::read(out_dir.join("omega-program")).expect("read emitted AArch64 Mach-O");
    let fmov_d0_x17 = (0x9e67_0000u32 | (17 << 5)).to_le_bytes();
    let fmov_d1_x17 = (0x9e67_0000u32 | (17 << 5) | 1).to_le_bytes();
    assert!(
        image
            .windows(12)
            .any(|window| { window[0..4] == fmov_d0_x17 && window[8..12] == fmov_d1_x17 }),
        "expected one HFA source to feed consecutive d0/d1 fragments"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn cross_aarch64_erased_hfa_import_keeps_two_vector_fragments() {
    let canary = pass_canary("capabilities/aarch64_erased_hfa_import_compile");
    let scratch = std::env::temp_dir().join(format!(
        "omega-aarch64-erased-hfa-import-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let src_dir = scratch.join("src");
    let out_dir = scratch.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(src_dir.join("build.omg"), "target macos_arm64 {\n}\n")
        .expect("write macos_arm64 target manifest");

    compile(CanaryCompileSpec {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("macos_arm64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("erased-stripped HFA import should compile for macos_arm64");

    let image = fs::read(out_dir.join("omega-program")).expect("read emitted AArch64 Mach-O");
    let fmov_d0_x17 = (0x9e67_0000u32 | (17 << 5)).to_le_bytes();
    let fmov_d1_x17 = (0x9e67_0000u32 | (17 << 5) | 1).to_le_bytes();
    assert!(
        image
            .windows(12)
            .any(|window| { window[0..4] == fmov_d0_x17 && window[8..12] == fmov_d1_x17 }),
        "erased evidence must not interrupt the HFA's d0/d1 fragments"
    );
    let _ = fs::remove_dir_all(&scratch);
}
