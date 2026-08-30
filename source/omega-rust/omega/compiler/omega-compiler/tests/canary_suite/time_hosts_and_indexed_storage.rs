use super::*;

#[test]
fn runtime_i64_min_literal_exit_canary_runs() {
    // D14 anonymous literals: `-9223372036854775808` (i64::MIN) is directly spellable --
    // the magnitude parses as an uninterpreted payload and the negative fold flips the
    // sign textually. Guard proves the stored value is strictly below -(i64::MAX); exit 70.
    let canary = pass_canary("arithmetic/runtime_i64_min_literal_exit");
    let scratch = std::env::temp_dir().join(format!("omega-i64min-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("i64::MIN literal canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("i64::MIN literal canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("i64::MIN literal canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the spelled i64::MIN to compare strictly below -(i64::MAX) (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_time_host_virtual_interpreter_oracle() {
    // std::time rung 4: the TimeHost seam over the VIRTUAL clock, EXACT
    // values (D12) -- non-advancing reads, sleep(30) advances exactly 30,
    // calibration 1000/1000/0, wall = 2026-01-01 + elapsed. Interp-only
    // until rung 5 binds the ops natively.
    let canary = pass_canary("time/runtime_time_host_virtual_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("time host virtual canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.error, None,
        "time host ops should interpret cleanly"
    );
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for the virtual clock chain, got {}",
        outcome.exit_code
    );
}

#[test]
fn runtime_time_elapsed_since_exit_canary_runs() {
    // std::time rung 6 slice 2: Time::elapsed_since (the stopwatch read;
    // single-level body with `stopwatch_*`-prefixed lets -- the cross-callee
    // let-name collision dodge, see TASKS.md). DIFFERENTIAL: sleep(30) then
    // elapsed >= 30ms on both engines (interp virtual clock = exactly 30ms).
    // The caller mixes now() and elapsed_since in ONE state -- the shape
    // that #DE-crashed while the two callees shared the let name `frequency`.
    let canary = pass_canary("time/runtime_time_elapsed_since_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("elapsed-since canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.error, None,
        "elapsed-since chain should interpret cleanly"
    );
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for the elapsed-since chain, got {}",
        outcome.exit_code
    );
    let scratch = std::env::temp_dir().join(format!("omega-elapsed-since-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("elapsed-since canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("elapsed-since canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the elapsed-since chain to run natively (exit 70), got {:?} \
         (1 = under 30ms/zeroed; negative status = the frequency-collision #DE crash)",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn cross_darwin_time_host_compiles() {
    // std::time rung 10: the darwin bindings lower from any host --
    // clock_gettime_nsec_np with injected clockids (8 monotonic / 0 wall) +
    // the three POSIX calibration constants as no-call constant results.
    // COMPILE-ONLY (structural): native confirmation needs a Mac.
    let canary = pass_canary("time/cross_darwin_time_host");
    let scratch = std::env::temp_dir().join(format!("omega-darwin-time-{}", std::process::id()));
    compile_single_file_hosted_main(&canary, &scratch, "macos_arm64")
        .expect("darwin time-host cross-compile should succeed");
    let footprints = fs::read_to_string(scratch.join("out/08_boundary_footprints.json"))
        .expect("Darwin time-host boundary footprints should be written");
    assert!(
        footprints.contains("\"origin\": \"compiler_body_outbound_immediate_import\""),
        "Darwin literal _exit calls must retain their exact direct-import footprint"
    );
    assert!(
        footprints.contains("\"origin\": \"compiler_body_outbound_immediate_import_result\""),
        "Darwin clock calls with injected clock IDs must retain their result-import footprint"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn integer_result_imports_compile_on_windows_and_darwin() {
    // One source pins both closed integer-result import shapes: Windows
    // GetTickCount64 has no arguments, while Darwin injects an immediate clock
    // ID before calling clock_gettime_nsec_np.
    let canary = pass_canary("host/runtime_tick_count_monotonic_exit");
    for target in ["windows_x86_64", "macos_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-result-import-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source_dir = scratch.join("src");
        let build_dir = scratch.join("out");
        fs::create_dir_all(&source_dir).expect("integer-result import source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy integer-result import source");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write integer-result import target manifest");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("integer-result import cross-compile failed for {target}: {diagnostics:#?}")
        });
        let footprints = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
            .expect("integer-result import footprints should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_outbound_immediate_import_result\""),
            "{target} integer-result imports must retain their final replay footprint"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn storage_result_imports_compile_on_windows_and_darwin() {
    // Windows window_destroy(result, runtime hwnd) and Darwin
    // close(result, runtime fd) pin the result-plus-argument relocation class.
    for (target, canary_name) in [
        ("windows_x86_64", "host/runtime_gui_foreground_window_exit"),
        ("macos_arm64", "filesystem/native_close"),
    ] {
        let canary = pass_canary(canary_name);
        let scratch = std::env::temp_dir().join(format!(
            "omega-storage-result-import-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source_dir = scratch.join("src");
        let build_dir = scratch.join("out");
        fs::create_dir_all(&source_dir).expect("storage-result import source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy storage-result import source");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write storage-result import target manifest");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("storage-result import cross-compile failed for {target}: {diagnostics:#?}")
        });
        let footprints = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
            .expect("storage-result import footprints should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_outbound_storage_import_result\""),
            "{target} storage-result imports must retain their final replay footprint"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn darwin_open_create_retains_its_variadic_adapter_footprint() {
    let canary = pass_canary("filesystem/native_open_create");
    let scratch = std::env::temp_dir().join(format!(
        "omega-open-create-footprint-{}",
        std::process::id()
    ));
    compile_single_file_hosted_main(&canary, &scratch, "macos_arm64")
        .expect("Darwin open-create adapter should compile");

    let footprints = fs::read_to_string(scratch.join("out/08_boundary_footprints.json"))
        .expect("open-create footprints should be written");
    assert!(
        footprints.contains("\"origin\": \"compiler_body_outbound_open_create_import\""),
        "Darwin open-create must retain its exact variadic adapter footprint"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn float_parameter_result_imports_compile_on_darwin() {
    // hypotenuse returns f64 through d0/fmov, then round_nearest returns i64
    // through x0; both consume runtime float arguments from machine storage.
    let canary = pass_canary("float/native_float_two_args");
    let scratch =
        std::env::temp_dir().join(format!("omega-float-result-import-{}", std::process::id()));
    compile_single_file_hosted_main(&canary, &scratch, "macos_arm64")
        .expect("float-parameter result imports should cross-compile for macos_arm64");
    let footprints = fs::read_to_string(scratch.join("out/08_boundary_footprints.json"))
        .expect("float-parameter result import footprints should be written");
    assert!(
        footprints.contains("\"origin\": \"compiler_body_outbound_float_import_result\""),
        "Darwin float-parameter result imports must retain their final replay footprint"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn dereferenced_result_imports_compile_on_windows_and_darwin() {
    // The same source reaches `_errno()` on Windows and `___error()` on Darwin;
    // both return an integer pointer that the retained adapter dereferences
    // before writing the Omega result place.
    let canary = pass_canary("filesystem/native_errno");
    for target in ["windows_x86_64", "macos_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-dereferenced-result-import-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source_dir = scratch.join("src");
        let build_dir = scratch.join("out");
        fs::create_dir_all(&source_dir).expect("dereferenced-result import source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy dereferenced-result import source");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write dereferenced-result import target manifest");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("dereferenced-result import cross-compile failed for {target}: {diagnostics:#?}")
        });
        let footprints = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
            .expect("dereferenced-result import footprints should be written");
        assert!(
            footprints
                .contains("\"origin\": \"compiler_body_outbound_dereferenced_import_result\""),
            "{target} errno imports must retain their final dereference replay footprint"
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_outbound_data_import_result\""),
            "{target} literal-path imports must retain their final data-address replay footprint"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn authored_scalar_imports_compile_on_windows_and_darwin() {
    // Both sources call a `via Binding::DllImport` leaf with a direct integer
    // result. Darwin supplies an immediate argument to `_exit`; Windows loads
    // the runtime argument/result places around `abs`.
    for (target, canary_name) in [
        ("macos_arm64", "providers/runtime_import_call_argument_exit"),
        (
            "windows_x86_64",
            "capabilities/windows_provides_import_exit",
        ),
    ] {
        let canary = pass_canary(canary_name);
        let scratch = std::env::temp_dir().join(format!(
            "omega-authored-scalar-import-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source_dir = scratch.join("src");
        let build_dir = scratch.join("out");
        fs::create_dir_all(&source_dir).expect("authored scalar import source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy authored scalar import source");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write authored scalar import target manifest");
        compile_with_auxiliary_artifacts(CanaryCompileSpec {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("authored scalar import cross-compile failed for {target}: {diagnostics:#?}")
        });
        let footprints = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
            .expect("authored scalar import footprints should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_outbound_authored_import_result\""),
            "{target} authored scalar imports must retain their source-planned final replay footprint"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn cross_linux_time_host_compiles_on_both_architectures() {
    // Linux std::time structural slice: clock_gettime writes a two-word
    // timespec and nanosleep consumes one, so target emission must own both
    // temporary shapes, combine seconds/nanoseconds for reads, convert
    // milliseconds for sleep, and relocate the semantic Omega operands.
    // Compile-only here; runtime confirmation remains gated on Linux hosts.
    let canary = pass_canary("time/cross_linux_time_host");
    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch =
            std::env::temp_dir().join(format!("omega-linux-time-{target}-{}", std::process::id()));
        compile_single_file_hosted_main(&canary, &scratch, target).unwrap_or_else(|diagnostics| {
            panic!("Linux time-host cross-compile failed for {target}: {diagnostics:#?}")
        });
        let build_dir = scratch.join("out");
        assert!(
            build_dir.join("omega-program").exists(),
            "{target} should emit an ELF image"
        );
        let footprints = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
            .expect("Linux time-host boundary footprints should be written");
        assert!(
            footprints.contains("\"origin\": \"compiler_body_outbound_syscall\"")
                && footprints.contains("\"machine_state_bits\": 77"),
            "{target} process-exit syscalls must retain their supervisor-call footprint"
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_outbound_syscall_timespec_result\""),
            "{target} clock_gettime adapters must retain their exact composite footprint"
        );
        assert!(
            footprints.contains("\"origin\": \"compiler_body_outbound_syscall_timespec_argument\""),
            "{target} nanosleep adapters must retain their exact composite footprint"
        );
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn cross_linux_value_syscalls_compile_on_both_architectures() {
    let canary = pass_canary("filesystem/cross_linux_value_syscalls");
    for target in ["linux_x86_64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-linux-value-syscalls-{target}-{}",
            std::process::id()
        ));
        compile_single_file_hosted_main(&canary, &scratch, target).unwrap_or_else(|diagnostics| {
            panic!("Linux value-syscall cross-compile failed for {target}: {diagnostics:#?}")
        });
        let build_dir = scratch.join("out");
        assert!(
            build_dir.join("omega-program").exists(),
            "{target} should emit an ELF image"
        );
        let report = fs::read_to_string(build_dir.join("backend_report.txt"))
            .expect("read Linux backend report");
        let footprints = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
            .expect("read Linux value-syscall footprints");
        assert!(
            footprints.contains(
                "\"origin\": \"compiler_body_outbound_syscall_result_storage_arguments\""
            ),
            "{target} value syscalls with runtime buffer/address arguments must retain their certified outbound-call footprint"
        );
        assert!(
            footprints
                .contains("\"origin\": \"compiler_body_outbound_syscall_result_data_arguments\""),
            "{target} result-bearing literal-backed syscalls must retain their exact data-object argument footprint"
        );
        let expected_size = if target == "linux_x86_64" { 144 } else { 128 };
        assert!(
            report.contains(&format!(
                "data StatLayout<StatRecord>: size {expected_size}, align 8"
            )),
            "{target} must retain its kernel stat extent"
        );
        let target_source = fs::read_to_string(repo_root().join(format!(
            "source/library/std/targets/{target}/filesystem_impl.omg"
        )))
        .expect("read Linux target filesystem policy");
        if target == "linux_arm64" {
            assert!(
                target_source.contains("self.entries[2] = FieldEntry { key: schema.fields[2].key, placement: FieldPlan::IntegerAt { offset: 20, stored_width: 32")
                    && target_source.contains("self.entries[13] = FieldEntry { key: schema.fields[13].key, placement: FieldPlan::IntegerAt { offset: 56, stored_width: 32"),
                "Linux AArch64 stat must retain its 32-bit nlink and blksize encodings"
            );
        } else {
            assert!(
                target_source.contains("self.entries[2] = FieldEntry { key: schema.fields[2].key, placement: FieldPlan::At { offset: 16 }")
                    && target_source.contains("self.entries[13] = FieldEntry { key: schema.fields[13].key, placement: FieldPlan::At { offset: 56 }"),
                "Linux x86-64 stat must retain its 64-bit nlink and blksize encodings"
            );
        }
        let _ = fs::remove_dir_all(&scratch);
    }
}

#[test]
fn runtime_checked_time_arith_exit_canary_runs() {
    // std::time rung 6 slice 3: Instant + SystemTime checked_add/
    // checked_subtract, exact values. 8 legs: carry/borrow Ok arms (non-ZII),
    // u64::MAX / i64::MAX / i64::MIN overflow pins, and a duration-seconds-
    // above-i64::MAX leg pinning the biased-space detection.
    let canary = pass_canary("time/runtime_checked_time_arith_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("checked time arith canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.error, None,
        "checked arith should interpret cleanly"
    );
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for the checked-arith chain, got {}",
        outcome.exit_code
    );
    let scratch = std::env::temp_dir().join(format!("omega-checked-arith-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("checked time arith canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("checked time arith canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the checked-arith chain to run natively (exit 70), got {:?} \
         (exit N = leg N failed; see the canary header for the leg list)",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_sleep_for_exit_canary_runs() {
    // std::time rung 6 slice 3: Time::sleep_for (Duration -> one clamped u32
    // host sleep, returning the clamped request). Returned ms == 30 exactly
    // on both engines; elapsed >= 30ms.
    let canary = pass_canary("time/runtime_sleep_for_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("sleep_for canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.error, None, "sleep_for should interpret cleanly");
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for the sleep_for chain, got {}",
        outcome.exit_code
    );
    let scratch = std::env::temp_dir().join(format!("omega-sleep-for-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("sleep_for canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("sleep_for canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the sleep_for chain to run natively (exit 70), got {:?} \
         (1 = returned ms wrong; 2 = elapsed under 30ms)",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_system_time_after_2026_exit_canary_runs() {
    // std::time rung 6 slice 2: system_time_now() (one raw wall read +
    // calibration constants, all math in the wrapper) + SystemTime::
    // duration_since both directions. DIFFERENTIAL: the virtual wall clock
    // (2026-01-01 seed + slept 30ms) and the real clock both satisfy the
    // seconds>0-or-subsecond>=30ms splits; the Backwards PAYLOAD must carry
    // the real gap (ZII Backwards(ZERO) fails leg 4).
    let canary = pass_canary("time/runtime_system_time_after_2026_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("system-time canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.error, None,
        "system-time chain should interpret cleanly"
    );
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for the system-time chain, got {}",
        outcome.exit_code
    );
    let scratch = std::env::temp_dir().join(format!("omega-system-time-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("system-time canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("system-time canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the system-time chain to run natively (exit 70), got {:?} \
         (1/2 = forward leg; 3/4 = backward leg; 5/6 = Unmeasured arm fired)",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_instant_elapsed_exit_canary_runs() {
    // std::time rung 6: Time::now() normalization over the seam +
    // Instant::duration_since/checked_duration_since. DIFFERENTIAL: the
    // >= 30ms and backwards=Overflow assertions hold on the interpreter's
    // virtual clock (exactly 30_000_000 ns) AND the native QPC clock.
    let canary = pass_canary("time/runtime_instant_elapsed_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("instant elapsed canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.error, None,
        "instant chain should interpret cleanly"
    );
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for the instant chain, got {}",
        outcome.exit_code
    );
    let scratch = std::env::temp_dir().join(format!("omega-instant-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("instant elapsed canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("instant elapsed canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the instant chain to run natively (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
#[cfg(windows)]
fn runtime_time_host_native_exit_canary_runs() {
    // std::time rung 5: the TimeHost seam bound natively on windows_x64 --
    // out-param u64 imports (QPC/QPF/GetSystemTimePreciseAsFileTime) plus the
    // two constant-result calibration ops. NO interpreter oracle: the canary
    // asserts the WINDOWS calibration constants (10^7 / 11_644_473_600) and
    // real-clock inequalities; the interpreter's virtual clock (1000 / 0) is
    // asserted exactly by runtime_time_host_virtual_exit instead.
    let canary = pass_canary("time/runtime_time_host_native_exit");
    let scratch = std::env::temp_dir().join(format!("omega-time-host-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_target(&canary, scratch.clone(), "windows_x86_64")
        .expect("time host native canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("time host native canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the native time-host chain to exit 70, got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
#[cfg(target_os = "macos")]
fn runtime_time_host_native_darwin_exit_canary_runs() {
    // std::time rung 10 NATIVE CONFIRMATION (claimed by the fs lane -- it
    // has the Mac): _clock_gettime_nsec_np both reads via clockid injection
    // + the aarch64 constant-result encoder, darwin calibration constants
    // asserted exactly (10^9 units, offset 0), real-clock inequalities.
    // NO interpreter oracle (virtual clock reports 1000/0 by design).
    let canary = pass_canary("time/runtime_time_host_native_darwin_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-time-host-darwin-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_target(&canary, scratch.clone(), "macos_arm64")
        .expect("darwin time host native canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("darwin time host native canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the darwin native time-host chain to exit 70, got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
#[cfg(target_os = "macos")]
fn runtime_fs_mtime_system_time_interop_exit_canary_runs() {
    // fs <-> time interop (TASKS_TIME #9's ready leg, fs-lane claimed): a
    // real file's stat mtime bridges via SystemTime::from_unix_seconds and
    // compares against system_time_now(). Engine-agnostic assertions: BOTH
    // the interpreter (virtual mtime 10^9 vs virtual 2026 clock) and the
    // native darwin run (fresh file vs real clock) exit 70. macos-gated:
    // the decode reads darwin stat offsets (windows waits on fs #2).
    let canary = pass_canary("time/runtime_fs_mtime_system_time_interop_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("fs-time interop canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.error, None, "interop should interpret cleanly");
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for the interop chain, got {}",
        outcome.exit_code
    );
    let scratch =
        std::env::temp_dir().join(format!("omega-fs-time-interop-{}", std::process::id()));
    compile_single_file_hosted_main(&canary, &scratch, "macos_arm64")
        .expect("fs-time interop canary should compile natively");
    let output = Command::new(scratch.join("out").join(executable_name()))
        .output()
        .expect("fs-time interop canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the native fs-time interop chain to exit 70, got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
#[cfg(windows)]
fn runtime_fs_mtime_interop_windows_exit_canary_runs() {
    // fs <-> time interop, the WINDOWS leg (the darwin twin is above): a
    // real file's mtime decoded at the `_stat64` offset (st_mtime @40, the
    // windows StatLayout policy) bridges via
    // SystemTime::from_unix_seconds against system_time_now().
    // Engine-agnostic assertions: BOTH the interpreter (virtual mtime 10^9
    // at the host layout's offset vs virtual 2026 clock) and the native
    // windows run (fresh file vs real clock) exit 70. windows-gated: the
    // decode reads the `_stat64` offset.
    let canary = pass_canary("time/runtime_fs_mtime_interop_windows_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("windows fs-time interop canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.error, None, "interop should interpret cleanly");
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for the windows interop chain, got {}",
        outcome.exit_code
    );
    let scratch =
        std::env::temp_dir().join(format!("omega-fs-time-interop-win-{}", std::process::id()));
    compile_single_file_hosted_main(&canary, &scratch, "windows_x86_64")
        .expect("windows fs-time interop canary should compile natively");
    let output = Command::new(scratch.join("out").join(executable_name()))
        .output()
        .expect("windows fs-time interop canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the native windows fs-time interop chain to exit 70, got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_duration_totals_exit_canary_runs() {
    // checked_as_nanoseconds/microseconds/milliseconds exact values + the
    // Overflow arm at Duration::MAX, interpreter oracle + native. Exit 70.
    let canary = pass_canary("time/runtime_duration_totals_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("duration totals canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.error, None, "totals should interpret cleanly");
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for the totals chain, got {}",
        outcome.exit_code
    );
    let scratch = std::env::temp_dir().join(format!("omega-totals-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("duration totals canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("duration totals canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the totals chain to run natively (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_duration_constructors_interpreter_oracle() {
    // from_seconds / from_milliseconds exact values (receiverless type-scoped
    // value calls). Interp-only: the native route hits the loud 16-byte
    // value-store MVP fence; promote when that lands (see the canary header).
    let canary = pass_canary("time/runtime_duration_constructors_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("duration constructors canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.error, None, "constructors should interpret cleanly");
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for the constructor chain, got {}",
        outcome.exit_code
    );
}

#[test]
fn runtime_duration_core_exit_canary_runs() {
    // std::time rung 3: Duration checked/saturating arithmetic, exact values
    // (carry, borrow, clamp, ordering, underflow arms), interpreter oracle +
    // native differential. Exit 70. The canary routes every Duration RECEIVER
    // through the FIRST field of its type (the known same-type
    // receiver-aliasing bug, value-call flavor) and time.omg keeps payload
    // field values cascade-safe (bare `param % literal`) -- both documented
    // in tests/omega/pending/time/value_machine_receiver_field_postentry.
    let canary = pass_canary("time/runtime_duration_core_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("duration core canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for the Duration chain, got {}",
        outcome.exit_code
    );
    let scratch = std::env::temp_dir().join(format!("omega-duration-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("duration core canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("duration core canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the Duration arithmetic chain to run natively (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_scoped_const_exit_canary_runs() {
    // const-v0 (D15): scalar + struct type-scoped consts substitute their
    // literal initializers at symbol resolution; 60 + 10 == 70, exit 70.
    let canary = pass_canary("constants/runtime_scoped_const_exit");
    let scratch = std::env::temp_dir().join(format!("omega-const-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("scoped-const canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("scoped-const canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("scoped-const canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the substituted consts to sum to 70, got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_u64_max_literal_exit_canary_runs() {
    // D14 fire C: u64::MAX stores full-width into a u64 target; MAX + 1 wraps to
    // exactly 0 only if every bit was set. Exit 70.
    let canary = pass_canary("arithmetic/runtime_u64_max_literal_exit");
    let scratch = std::env::temp_dir().join(format!("omega-u64max-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("u64::MAX literal canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("u64::MAX literal canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("u64::MAX literal canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the stored u64::MAX to wrap to 0 on +1 (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn u64_literal_above_i64_max_canary_is_rejected() {
    // A u64-magnitude literal PARSES (D14) and a direct u64-classed store is ACCEPTED
    // (fire C; see runtime_u64_max_literal_exit), but every OTHER position -- here an
    // i64 target that would reinterpret the bits as negative -- still rejects at the
    // literal-width gate with a CLEAR "exceeds the i64 range" diagnostic that names
    // the accepted alternative.
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
fn runtime_guarded_computed_index_operand_exit_canary_runs() {
    // `acc + arr[k + 1]` under an explicit `k + 1 >= 0 && k + 1 < 5` guard:
    // the auto-hoisted index temp's bounds discharge from the guard facts
    // under its INITIALIZER label. Was the computed-index FAIL canary.
    let canary = pass_canary("collections/runtime_guarded_computed_index_operand_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-guarded-computed-index-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("guarded computed-index canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("guarded computed-index canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("guarded computed-index canary should run");

    assert_eq!(
        output.status.code(),
        Some(30),
        "expected `acc + arr[k + 1]` (k = arr[0] = 1) to read arr[2] = 30 and exit 30, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// DIRECT computed-index spellings (read/operand/write/reversed/backward).
#[test]
fn runtime_computed_index_direct_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_computed_index_direct_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-computed-index-direct-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("direct computed-index canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("direct computed-index canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("direct computed-index canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected the direct computed-index faces (arr[k+1] read/operand/write, arr[1+k], guarded arr[k-1]) to hit the right elements and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dual_indexed_copy_exit_canary_runs() {
    // `nums[i] = nums[j]` (both indices runtime) now LOWERS for real (task #38,
    // CopyRuntimeMachineIndexedToRuntimeMachineIndexed) instead of being fenced.
    // nums=[10,20,30,40,50], i=0, j=4 -> nums[0]=50, exited as the code. Exit 10
    // = the historic base-copy/no-op bug returned.
    let canary = pass_canary("collections/runtime_dual_indexed_copy_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-dual-copy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dual-indexed copy canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dual-indexed copy canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dual-indexed copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(50),
        "expected nums[i]=nums[j] (i=0, j=4) to copy element j -> nums[0]=50 (the exit \
         code); exit 10 = the base-copy/no-op bug. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_double_indexed_write_exit_canary_runs() {
    // Both-runtime nested writes (`grid[i][j] = v`): const value, machine and
    // frame place sources, neighbor-validated. Was the write fail canary.
    let canary = pass_canary("collections/runtime_double_indexed_write_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-double-indexed-write-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("double-indexed write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("double-indexed write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("double-indexed write canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `grid[i][j] = v` (both indices runtime) to write the right elements across all faces and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Container setter/getter/non-generic-method matrix over two instances.
#[test]
fn runtime_container_setter_matrix_exit_canary_runs() {
    let canary = pass_canary("generics/runtime_container_setter_matrix_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-container-setter-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("container setter matrix canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("container setter matrix canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("container setter matrix canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected Cell<i32>/Cell<bool> setter+getter+touch_count faces to exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Container instances: Box<i32> + Box<bool> with per-instance stored().
#[test]
fn runtime_container_method_instances_exit_canary_runs() {
    let canary = pass_canary("generics/runtime_container_method_instances_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-container-methods-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("container method instances canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("container method instances canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("container method instances canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected Box<i32>.stored() == 42 and Box<bool>.stored() == true (per-instance method clones) to exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Frame-resident 2D arrays, both-runtime reads (local + param faces).
#[test]
fn runtime_frame_double_indexed_read_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_frame_double_indexed_read_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-frame-double-read-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("frame double-indexed read canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("frame double-indexed read canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("frame double-indexed read canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected g[i][j] (frame-resident 2D array, both indices runtime) to read the right elements across the let/param faces and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Direct RMW on both-runtime nested targets (grid[i][j] += 1, member flavor)
// + the stale-fold invalidation and hoist-temp typing fixes it required.
#[test]
fn runtime_double_indexed_rmw_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_double_indexed_rmw_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-double-indexed-rmw-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("double-indexed RMW canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("double-indexed RMW canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("double-indexed RMW canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected grid[i][j] += 1 and rows[i].data[j] += 1 to read-modify-write the right elements (41 -> 42; stale folds voided) and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Indexed reads as binary operands inside TRANSITION ARGUMENTS (single and
// double index, Always and guarded arms -- the run-splice face).
#[test]
fn runtime_indexed_operand_transition_arg_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_indexed_operand_transition_arg_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-indexed-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("indexed-operand transition-arg canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed-operand transition-arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed-operand transition-arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected indexed-operand transition args (arr[i]+5, grid[i][j]+.., guarded arms) to deliver computed values and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The NON-boundary flavor of task #37's guard face: a shared &Struct param
// guard reads the right field through the alias slot.
#[test]
fn runtime_shared_ref_param_guard_exit_canary_runs() {
    let canary = pass_canary("references/runtime_shared_ref_param_guard_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-shared-ref-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("shared ref-param guard canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("shared ref-param guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("shared ref-param guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `transition r.c == 9` (r: &Pt, non-boundary) to read c and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Nested VALUE-position receiver through a plain-DATA intermediate
// (`self.p.a.get()`, `p: PairD`). The backend storage walk descends the record
// to resolve each callee's `self` base; distinct leaf types (BoxI/CellI) so the
// by-type walk hits the named instance. Receiver-place staircase rungs 2b/2a/3.
#[test]
fn runtime_nested_receiver_distinct_types_exit_canary_runs() {
    let canary = pass_canary("references/runtime_nested_receiver_distinct_types_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nested-receiver-distinct-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested-receiver distinct-types canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested-receiver canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested-receiver distinct-types canary should run");

    assert_eq!(
        output.status.code(),
        Some(9),
        "expected self.p.a.get()==5 and self.p.b.get()==9 (nested receivers, \
         distinct leaf types) to exit 9, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Member-between (`rows[i].data[j]`) + member-suffix (`boards[i][j].x`)
// double-indexed faces, read AND write, both-runtime indices.
#[test]
fn runtime_double_indexed_member_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_double_indexed_member_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-double-indexed-member-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("double-indexed member canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("double-indexed member canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("double-indexed member canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected rows[i].data[j] and boards[i][j].field faces (both indices runtime) to hit the right elements and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Both-runtime nested read as a BINARY OPERAND (`grid[i][j] + 5`).
#[test]
fn runtime_double_indexed_operand_exit_canary_runs() {
    let canary = pass_canary("collections/runtime_double_indexed_operand_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-double-indexed-operand-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("double-indexed operand canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("double-indexed operand canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("double-indexed operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected `grid[i][j] + 5` (both indices runtime, hoisted operand) to compute 42 and exit 1, got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_inplace_reverse_local_temp_exit_canary_runs() {
    // In-place reverse with a LOCAL temp (capture + dual copy + frame-source
    // write composed in a loop): [1,2,3,4,5] -> [5,4,3,2,1] -> exit 70.
    let canary = pass_canary("collections/runtime_inplace_reverse_local_temp_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-reverse-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("in-place reverse canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("in-place reverse canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("in-place reverse canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the local-temp reverse to produce [5,4,3,2,1] (exit 70); exit 71 = a          swap leg regressed (capture fold, dual copy, or frame-source write). got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_local_copy_chain_exit_canary_runs() {
    // Transitive copy chain t=arr[i]; c=t; d=c; b=d>5: the slot scan follows
    // bare copies, so b is true (exit 70); the old fold read false (exit 71).
    let canary = pass_canary("collections/runtime_indexed_local_copy_chain_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-copychain-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("copy-chain canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("copy-chain canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("copy-chain canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the copy chain to read arr[1]=8 > 5 as TRUE (exit 70); exit 71 = the          transitive fold returned. got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_write_frame_local_source_exit_canary_runs() {
    // `nums[i] = t` with t a FRAME-slot capture: nums[2] must land the captured
    // 99 (exit 70); the old stale fold wrote the post-overwrite 0.
    let canary = pass_canary("collections/runtime_indexed_write_frame_local_source_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-frame-src-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("frame-local-source indexed write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("frame-local-source canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("frame-local-source indexed write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected nums[i]=t to write the CAPTURED value 99 (exit 70); exit 71 = the stale          fold or an uninitialized slot returned. got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_captured_local_swap_exit_canary_runs() {
    // Euclid swap: let r = a % b; a = b; b = r. The capture-aware binding skip
    // keeps r in its slot -> gcd(48,36) = 12 (exit 70); the old fold gave 36.
    let canary = pass_canary("control_flow/runtime_captured_local_swap_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-gcd-swap-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("captured-local swap canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("captured-local swap canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("captured-local swap canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the slot-captured swap to compute gcd(48,36)=12 (exit 70); exit 71 = the          fold re-read the reassigned field. got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dual_indexed_copy_in_loop_exit_canary_runs() {
    // `a[i] = b[i]` element-wise in a loop (task #38's cross-array + loop face)
    // now lowers through CopyRuntimeMachineIndexedToRuntimeMachineIndexed. The
    // classifier still routes it off the static-assignment fast path (which
    // would no-op); the recorded write then selects the dual copy. b=[10,20,40]
    // copies into a; sum(a)=70. Exit 0 = the historic silent no-op returned.
    let canary = pass_canary("collections/runtime_dual_indexed_copy_in_loop_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-dual-loop-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("in-loop dual-indexed copy canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("in-loop dual-indexed copy canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("in-loop dual-indexed copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the element-wise a[i]=b[i] loop to copy [10,20,40] (sum 70, the exit \
         code); exit 0 = the historic silent no-op returned. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}
