use super::application_build;
use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, Stdio, compile,
    compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host,
    fixture_path_dependencies, fs, hosted_main_program_entry_build_for, interpret, pass_canary,
    repo_root, run_bounded_canary_jobs,
};
#[cfg(not(windows))]
use crate::{copy_dir_recursive, executable_name, sample_project};
use compiler::CheckedCompileRequest;
use std::io::Write;

#[test]
fn runtime_local_struct_string_field_concat_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_STRUCT_STRING_FIELD_CONCAT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-local-struct-string-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime local struct string field concat canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "runtime local struct string field concat canary should retain its executable receipt",
    );
    let output = Command::new(executable)
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
fn runtime_string_stored_suffix_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_STRING_STORED_SUFFIX_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-string-stored-suffix-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime string stored-suffix canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime string stored-suffix canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime string stored-suffix canary should run");

    assert_eq!(
        output.status.code(),
        Some(193),
        "expected segmented stored-suffix text assembly to exit 193, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_lookup_struct_field_concat_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_LOOKUP_STRUCT_FIELD_CONCAT_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("runtime lookup carrier concat should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.exit_code, 190, "interpreter lookup carrier concat");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-lookup-struct-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime lookup struct field concat canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime lookup struct field concat canary should retain its executable receipt");
    let output = Command::new(executable)
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
    let canary = pass_canary(fixture_roster::RUNTIME_LARGE_LOOKUP_STRUCT_FIELD_CONCAT_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("large lookup carrier concat should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 192,
        "interpreter large lookup carrier concat"
    );
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-large-lookup-struct-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime large lookup struct field concat canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime large lookup struct field concat canary should retain its executable receipt",
    );
    let output = Command::new(executable)
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
    let canary = pass_canary(fixture_roster::RUNTIME_LARGE_ROOM_LOOKUP_STRUCT_FIELD_CONCAT_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("large room lookup carrier concat should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 200,
        "interpreter large-room lookup carrier concat"
    );
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-large-room-lookup-struct-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime large room lookup struct field concat canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime large room lookup struct field concat canary should retain its executable receipt",
    );
    let output = Command::new(executable)
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
    let canary =
        pass_canary(fixture_roster::RUNTIME_CALL_ARGUMENT_STRUCT_STRING_FIELD_SLICE_ALIAS_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-call-argument-struct-string-slice-alias-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime call argument struct string slice alias canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime call argument struct string slice alias canary should retain its executable receipt",
    );
    let output = Command::new(executable)
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
fn bounded_carrier_regressions_compile_on_aarch64() {
    for (index, &canary_name) in fixture_roster::BOUNDED_CARRIER_PASS_CANARIES
        .iter()
        .enumerate()
    {
        let canary = pass_canary(canary_name);
        let scratch = std::env::temp_dir().join(format!(
            "omega-carrier-place-arm64-{}-{index}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source = scratch.join("src");
        fs::create_dir_all(&source).expect("AArch64 carrier scratch source directory");
        fs::copy(canary.join("main.omg"), source.join("main.omg"))
            .expect("copy carrier canary into AArch64 scratch source");
        // Members whose fixture authors a dependency keep it in the scratch
        // project; a bare application build leaves module resolution looking
        // for the package beside the copied source instead of at the
        // repository root it was authored against.
        let build = if fixture_path_dependencies(&canary).is_empty() {
            application_build()
        } else {
            hosted_main_program_entry_build_for(&canary, "linux_arm64")
        };
        fs::write(source.join("build.omg"), build).expect("write AArch64 carrier build source");
        compile(CanaryCompileSpec {
            root_path: source.join("main.omg"),
            build_dir: Some(scratch.join("out")),
            target_name: Some("linux_arm64".into()),
            product: CanaryCompileProduct::Check,
        })
        .unwrap_or_else(|diagnostics| {
            panic!(
                "AArch64 carrier place/view/return lowering should compile for {canary_name}:\n{}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn runtime_mutable_struct_string_field_copy_concat_write_line_canary_runs() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_MUTABLE_STRUCT_STRING_FIELD_COPY_CONCAT_WRITE_LINE);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("runtime mutable struct carrier field copy/write canary should check");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 77);
    assert_eq!(interpreted.stdout, b"prefix omega done\n".to_vec());
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-struct-string-field-copy-concat-write-line-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime mutable struct carrier field copy concat write_line canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime mutable struct carrier field copy concat write_line canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable struct carrier field copy concat write_line canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected runtime mutable struct carrier field copy concat write_line canary to print copied-field text and exit 77, got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"prefix omega done\n");

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_indexed_integer_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_OWNED_INDEXED_INTEGER_WRITE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-indexed-integer-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime machine-owned indexed integer write canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime machine-owned indexed integer write canary should retain its executable receipt",
    );
    let output = Command::new(executable)
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
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_OWNED_FIXED_INDEXED_STRUCT_COPY_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-fixed-indexed-struct-copy-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime machine-owned fixed indexed struct copy canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime machine-owned fixed indexed struct copy canary should retain its executable receipt",
    );
    let output = Command::new(executable)
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
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_OWNED_INDEXED_STRUCT_COPY_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-indexed-struct-copy-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime machine-owned indexed struct copy canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime machine-owned indexed struct copy canary should retain its executable receipt",
    );
    let output = Command::new(executable)
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
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_OWNED_INDEXED_NESTED_EXIT_WRITE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-indexed-nested-exit-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime machine-owned indexed nested exit write canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime machine-owned indexed nested exit write canary should retain its executable receipt",
    );
    let output = Command::new(executable)
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
    let canary = pass_canary(fixture_roster::RUNTIME_ORDERED_ROOM_DISPATCH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime ordered room dispatch canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime ordered room dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
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
    let canary = pass_canary(fixture_roster::RUNTIME_ORDERED_ROOM_DISPATCH_AFTER_CALL_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-after-call-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime ordered room dispatch after call canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime ordered room dispatch after call canary should retain its executable receipt",
    );
    let output = Command::new(executable)
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
    let canary = pass_canary(fixture_roster::RUNTIME_ORDERED_ROOM_DISPATCH_GAME_SHAPE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-game-shape-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime ordered room dispatch game-shape canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime ordered room dispatch game-shape canary should retain its executable receipt",
    );
    let output = Command::new(executable)
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
    let canary = pass_canary(fixture_roster::RUNTIME_ORDERED_ROOM_DISPATCH_LARGE_MACHINE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-large-machine-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime ordered room dispatch large-machine canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime ordered room dispatch large-machine canary should retain its executable receipt",
    );
    let output = Command::new(executable)
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
    let canary = pass_canary(fixture_roster::RUNTIME_ORDERED_ROOM_DISPATCH_LOOP_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-loop-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime ordered room dispatch loop canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime ordered room dispatch loop canary should retain its executable receipt");
    let mut child = Command::new(executable)
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
    let canary = pass_canary(fixture_roster::RUNTIME_GUARDED_INLINE_LEAF_ARM_SKIP_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-guarded-inline-leaf-arm-skip-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime guarded inline leaf arm skip canary should compile from its authored root",
    );

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime guarded inline leaf arm skip canary should retain its executable receipt");
    let output = Command::new(executable)
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
    let canary = pass_canary(fixture_roster::RUNTIME_ORDERED_ROOM_DISPATCH_REAL_SHOW_STATES_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-ordered-room-dispatch-real-show-states-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime ordered room dispatch real-show-states canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime ordered room dispatch real-show-states canary should retain its executable receipt",
    );
    let mut child = Command::new(executable)
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
    let canary = pass_canary(fixture_roster::RUNTIME_THREADED_MUT_ARG_INTERRUPT_SOAK_EXIT);
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-threaded-mut-arg-interrupt-soak-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("threaded mut-arg interrupt soak canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("threaded mut-arg interrupt soak canary should retain its executable receipt");
    let output = Command::new(executable)
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
fn named_integer_conversion_prng_cohort_reaches_checked_trees() {
    for &relative in fixture_roster::PRNG_REPOSITORY_PASS_CANARIES {
        let main_path = repo_root().join(relative).join("main.omg");
        compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "named integer-conversion PRNG canary {relative} should reach checked trees: \
                 {diagnostics:#?}"
                )
            });
    }
}

#[test]
fn named_integer_conversion_filesystem_decode_cohort_reaches_checked_trees() {
    for &relative in fixture_roster::FILESYSTEM_REPOSITORY_PASS_CANARIES {
        let main_path = repo_root().join(relative).join("main.omg");
        compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "named integer-conversion filesystem decode canary {relative} should reach \
                 checked trees: {diagnostics:#?}"
                )
            });
    }

    let windows_relative = fixture_roster::REPOSITORY_WINDOWS_SET_FILE_TIME_EXIT;
    let windows_main = repo_root().join(windows_relative).join("main.omg");
    compile_reviewed_repository_fixture(CheckedCompileRequest::new(&windows_main, None))
        .unwrap_or_else(|diagnostics| {
            panic!(
                "named integer-conversion filesystem decode canary {windows_relative} should reach \
             checked trees: {diagnostics:#?}"
            )
        });
}

#[test]
fn named_integer_conversion_filesystem_cross_targets_reach_checked_trees() {
    let canary = pass_canary(fixture_roster::WINDOWS_POSITIONED_IO_EXIT);
    let targets = ["linux_x86_64", "linux_arm64", "windows_x86_64"];
    let results = run_bounded_canary_jobs(&targets, |target| {
        compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &canary.join("main.omg"),
            Some(target),
        ))
        .map(|_| ())
        .map_err(|diagnostics| format!("{diagnostics:#?}"))
    });
    for (target, result) in targets.into_iter().zip(results) {
        result.unwrap_or_else(|diagnostic| {
            panic!(
                "named integer-conversion filesystem cohort should reach checked trees for \
                 {target}: {diagnostic}"
            )
        });
    }
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
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_VALUE_CALL_CALLER_LOCAL_GUARD_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nested-value-call-caller-local-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested value-call caller-local guard canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested value-call caller-local guard canary should retain its executable receipt");
    let output = Command::new(executable)
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
    let sample = sample_project("cli/games/dungeon_crawler_cli");
    let main_path = sample.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-native-dungeon-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
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
    let source = sample_project("cli/games/dungeon_crawler_cli");
    let package_dir = std::env::temp_dir().join(format!(
        "omega-dungeon-direct-movement-{}",
        std::process::id()
    ));
    let build_dir = package_dir.join("build");
    let _ = fs::remove_dir_all(&package_dir);
    copy_dir_recursive(&source, &package_dir).expect("sample package should copy into temp repro");

    compile(CanaryCompileSpec {
        root_path: package_dir.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
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
