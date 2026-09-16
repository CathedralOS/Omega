#[cfg(windows)]
use crate::compile_rooted_canary_for_target;
use super::assert_float_trapping_policy_canary_aborts;
use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, compile, compile_reviewed_repository_fixture,
    compile_rooted_canary_for_native_host, executable_name, fs, interpret, pass_canary,
};
use compiler::CheckedCompileRequest;

#[test]
fn runtime_indexed_rmw_temp_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_RMW_TEMP_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-indexed-rmw-temp-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("indexed-rmw-temp canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed-rmw-temp canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed-rmw-temp canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the temp-field RMW idiom over a runtime-indexed array to accumulate (the copy write must invalidate the array's folded constants) (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_indexed_write_adjacent_field_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_WRITE_ADJACENT_FIELD_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-indexed-write-adjacent-{}",
        std::process::id()
    ));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("indexed-write-adjacent-field canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed-write-adjacent-field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed-write-adjacent-field canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a runtime-indexed write to load the index 32-bit (not pull in the adjacent field as the high dword -> OOB) (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_join_meet_bound_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_JOIN_MEET_BOUND_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-join-meet-bound-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("join-meet-bound canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("join-meet-bound canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("join-meet-bound canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the predecessor meet to carry an index bound to a multi-predecessor join (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_dual_indexed_comparison_guard_exit_canary_runs() {
    // Two runtime-indexed array elements compared in one transition guard
    // (`transition self.arr[self.lo] < self.arr[self.hi]`): the operand-hoist lifts EACH into its
    // own temp. arr[4]=20 < arr[2]=70 is true -> exit 70. A regression to the element-0 read would
    // flip the arm and diverge from the interpreter.
    let canary = pass_canary(fixture_roster::RUNTIME_DUAL_INDEXED_COMPARISON_GUARD_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-dual-idx-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("dual-indexed comparison guard canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("dual-indexed comparison guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dual-indexed comparison guard canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a guard comparing two runtime-indexed elements to read the right ones (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_array_min_max_builtin_exit_canary_runs() {
    // The min/max builtins used in a reduction over an array: `self.mx = max(self.mx, self.v)` /
    // `self.mn = min(self.mn, self.v)`, folding each element read from a runtime index. arr =
    // [30,50,70,20,60,10] -> mx 70, mn 10, both self-checked -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_ARRAY_MIN_MAX_BUILTIN_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-minmax-red-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("min/max reduction canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("min/max reduction canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("min/max reduction canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected min/max reduction over an array to compute both extremes (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_indexed_guard_subject_exit_canary_runs() {
    // A DIRECT runtime-indexed read as a transition guard subject (`transition self.arr[self.i] > 5`,
    // no local bind) -- the form that used to silently read element 0. Fixed by the frontend
    // operand-hoist now covering comparison guards. arr = [3,8,1,9,4,6]; 3 exceed 5 -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_GUARD_SUBJECT_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-guard-subj-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime-indexed guard-subject canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime-indexed guard-subject canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime-indexed guard-subject canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a direct runtime-indexed guard subject to compare the right element (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_tick_paced_marquee_exit_canary_runs() {
    // A tick-paced render loop: 24 frames of carrier writes + write_line + sleep(15), then the
    // REAL elapsed time asserted via tick_count (>= 100ms) -> exit 0.
    let canary = pass_canary(fixture_roster::RUNTIME_TICK_PACED_MARQUEE_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-marquee-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("tick marquee canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("tick marquee canary should run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "expected the tick-paced marquee to render and satisfy the elapsed-time check (exit 0), got {:?}
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[cfg(windows)]
#[test]
fn runtime_user32_key_state_exit_canary_runs() {
    // Multi-DLL proof: KERNEL32 + User32 in one PE; key_state(32) completes and stores -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_USER32_KEY_STATE_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-keystate-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("key_state canary should compile from its authored root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("key_state canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the user32 import to resolve and the call to complete (exit 70), got {:?}
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_tick_count_monotonic_exit_canary_runs() {
    // The first value-returning host import: t1 = tick_count(); sleep(30); t2 = tick_count();
    // t2 >= t1 -> exit 70 (monotonicity -- tick values are nondeterministic).
    let canary = pass_canary(fixture_roster::RUNTIME_TICK_COUNT_MONOTONIC_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-tick-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("tick_count canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("tick_count canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected tick_count monotonicity across a sleep (exit 70), got {:?}
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_gui_memory_dc_blit_canary_is_targetless_and_interprets() {
    let canary = pass_canary(fixture_roster::RUNTIME_GUI_MEMORY_DC_BLIT_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("memory-DC blit canary should compile to checked trees");
    assert_eq!(
        checked.selected_program_entry_machine(),
        None,
        "targetless checking must not select an authored target entry"
    );
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.error, None,
        "virtual GDI memory-DC blit should not decline: {:?}",
        outcome.error
    );
    assert_eq!(
        outcome.exit_code, 70,
        "virtual GDI memory-DC blit should report all eight scanlines"
    );
}

#[cfg(windows)]
#[test]
fn runtime_gui_memory_dc_blit_exit_canary_runs() {
    // The first windowed-tier pixel proof: CreateCompatibleDC(0) + StretchDIBits of an 8x8
    // 32bpp DIB (13 args -- the general import call's stack-arg + address-operand shape).
    // Blit reports the copied scanline count == height -> exit 70. CI-safe (nothing visible).
    let canary = pass_canary(fixture_roster::RUNTIME_GUI_MEMORY_DC_BLIT_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-gui-blit-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_target(&canary, build_dir.clone(), "windows_x86_64")
        .expect("gui memory-dc blit canary should compile from its Windows root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("gui memory-dc blit canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a full-height memory-DC blit (exit 70), got {:?}
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_payload_range_narrowing_exit_canary_runs() {
    // The field-stored payload's [0..=15] range narrows through the nested place
    // `self.m.dx`, discharging the decision-17 obligation for `dx * 10` -- and the
    // scaled arg discriminates at runtime (dx=7 -> 70). Exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_PAYLOAD_RANGE_NARROWING_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-npr-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("nested payload range narrowing canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested payload range narrowing canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested payload range narrowing canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the destructured nested payload to scale to 70, got {:?}
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

/// The three faces the reentrant-value-call fence used to reject -- now
/// carried by dispatch call-with-return: an effectful `terminates` walk
/// value-called inline / directly / as a statement counts the separators of
/// "a/b/c" (exit 70; the historic miscompile counted 0 natively).
#[test]
fn runtime_recursive_walk_call_with_return_canaries_run() {
    for &name in fixture_roster::RECURSIVE_WALK_PASS_CANARIES {
        let canary = pass_canary(name);
        let build_dir = std::env::temp_dir().join(format!(
            "omega-rwalk-{}-{}",
            name.rsplit('/').next().unwrap(),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);
        let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .unwrap_or_else(|d| panic!("{name} should compile: {d:?}"));
        let executable = compilation
            .checked_native_executable_path()
            .unwrap_or_else(|| panic!("{name} should retain its executable receipt"));
        let output = Command::new(executable)
            .output()
            .unwrap_or_else(|e| panic!("{name} should run: {e}"));
        assert_eq!(
            output.status.code(),
            Some(70),
            "{name}: expected the walk to count 2 separators (exit 70), got {:?}",
            output.status.code(),
        );
        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn runtime_saturating_wide_boundaries_exit_canary_runs() {
    // 64-bit saturating at the REAL boundaries -- the flag-based clamp
    // (ADDS/SUBS + CSINV on aarch64; the narrower widths' wide-result compare
    // cannot reach 64 bits). i64::MAX+1 -> MAX, MIN-1 -> MIN, u64 MAX+5 ->
    // MAX, 5-10 -> 0; exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_SATURATING_WIDE_BOUNDARIES_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-satwide-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wide saturating boundaries canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("wide saturating boundaries canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wide saturating boundaries canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected all four 64-bit saturating boundary directions to clamp (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_saturating_param_carry_exit_canary_runs() {
    // Saturating i8 arithmetic carried through recursion PARAMS and a dispatch
    // binary terminal: each hop's `acc + 50` clamps at the operation (50, 100,
    // 127), and the terminal `acc + 50` stays 127 even though its landing slot
    // (`let n: i8`, the plain `-> i8` return) is Exact -- the domain rides the
    // OPERAND's declared type. The differential oracle pins the interpreter to
    // the same 70 (it used to compute transition-arg arithmetic wide and exit
    // 71).
    let canary = pass_canary(fixture_roster::RUNTIME_SATURATING_PARAM_CARRY_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-satcarry-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("saturating param-carry canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("saturating param-carry canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("saturating param-carry canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the saturated recursion params + binary terminal to exit 70, got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_saturating_expression_domain_exit_canary_runs() {
    // Saturating arithmetic in OPERAND position (fused under guard compares,
    // no landing seam): i8 add/sub/mul overflow directions clamp at the
    // operation, the 64-bit boundary add takes the flag-based clamp, and an
    // in-range add stays exact. Exercises the register-parametric write-path
    // sequences reused by the operand evaluator.
    let canary = pass_canary(fixture_roster::RUNTIME_SATURATING_EXPRESSION_DOMAIN_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-satexpr-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("saturating expression-domain canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("saturating expression-domain canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("saturating expression-domain canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected all five operand-position saturating directions to hold (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wrapping_expression_guard_exit_canary_runs() {
    // Wrapping arithmetic fused into guard operands: the byte-width compare
    // IS the wrap natively (u8 200+100 compares as 44); the differential
    // oracle pins the interpreter's node-level wrap to the same exits.
    let canary = pass_canary(fixture_roster::RUNTIME_WRAPPING_EXPRESSION_GUARD_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-wrapexpr-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wrapping expression-guard canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("wrapping expression-guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wrapping expression-guard canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected all three wrapped guard directions to hold (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_divide_min_edge_guard_exit_canary_runs() {
    // Signed division's one overflowing corner (TYPE_MIN / -1) fused into
    // guard operands: Saturating clamps to TYPE_MAX (and MIN % -1 == 0),
    // Wrapping wraps back to TYPE_MIN. On x86_64 the fused idiv would
    // hardware-trap without its divisor guard; this pins the guard in
    // operand position on both ISAs.
    let canary = pass_canary(fixture_roster::RUNTIME_DIVIDE_MIN_EDGE_GUARD_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-divmin-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("divide min-edge guard canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("divide min-edge guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("divide min-edge guard canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected all three MIN/-1 guard directions to hold (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_unsigned_witness_exit_canary_runs() {
    // A nested binary operand carries its operands' unsignedness: high-bit
    // u32 `(a / b) % k` runs unsigned div+mod fused in a guard, the stored
    // flavor agrees, ordered compares of a nested quotient compare unsigned,
    // and `>>` of one shifts logically. The signed encodings all diverge on
    // the high-bit dividend.
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_UNSIGNED_WITNESS_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-nestuns-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested unsigned witness canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested unsigned witness canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested unsigned witness canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected all four nested-unsigned directions to hold (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_local_array_element_value_operand_exit_canary_runs() {
    // A local array's runtime-indexed element as a value-call arg and a
    // forwarded transition arg. The aarch64 indexed-operand address helpers
    // used to clobber the left operand's result (hardcoded x17 index
    // scratch) while addressing the right one -- d = i + arr[i].
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_ARRAY_ELEMENT_VALUE_OPERAND_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-localarr-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("local-array value-operand canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("local-array value-operand canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("local-array value-operand canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both local-array value-operand directions to hold (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_array_element_fused_call_arg_exit_canary_runs() {
    // A machine array's runtime-indexed element inside a fused value-call
    // arg -- the shape whose computation was silently dropped before the
    // MachineIndexed operand variant existed.
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_ARRAY_ELEMENT_FUSED_CALL_ARG_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-machidx-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("machine-array fused-call-arg canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("machine-array fused-call-arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("machine-array fused-call-arg canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the fused machine-indexed arg to deliver (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_saturating_array_element_guard_exit_canary_runs() {
    // Saturating machine-array elements fused in guard operands: the
    // MachineIndexed operand variant and the operand-domain lowering
    // intersecting (hoisted element + `Add in Saturating` bool write).
    let canary = pass_canary(fixture_roster::RUNTIME_SATURATING_ARRAY_ELEMENT_GUARD_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-satarr-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("saturating array-element guard canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("saturating array-element guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("saturating array-element guard canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both saturating-element guard directions to hold (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn custom_ranking_field_countdown_canary_runs() {
    // Custom-ranking termination proof PLUS the recursive value call's
    // terminal delivery (the aggregate unserved-recursive-call-result
    // sweep): weaken counts down to 0 and the let-bound result must land.
    let canary = pass_canary(fixture_roster::CUSTOM_RANKING_FIELD_COUNTDOWN_COMPILE);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-custom_ranking_field_countdown-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("custom-ranking recursive delivery canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("custom-ranking recursive delivery canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("custom-ranking recursive delivery canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the recursive terminal to deliver 0 (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn custom_ranking_struct_view_canary_runs() {
    // Custom-ranking termination proof PLUS the recursive value call's
    // terminal delivery (the aggregate unserved-recursive-call-result
    // sweep): weaken counts down to 0 and the let-bound result must land.
    let canary = pass_canary(fixture_roster::CUSTOM_RANKING_STRUCT_VIEW);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-custom_ranking_struct_view-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("custom-ranking recursive delivery canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("custom-ranking recursive delivery canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("custom-ranking recursive delivery canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the recursive terminal to deliver 0 (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_float_nested_operand_exit_canary_runs() {
    // Nested float binaries in operand position (write value + transition
    // arg) wire float-ness through selection; integer-op'ing the IEEE bits
    // fails both legs.
    let canary = pass_canary(fixture_roster::RUNTIME_FLOAT_NESTED_OPERAND_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-fnest-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested float operand canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested float operand canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested float operand canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "nested float operand canary should pass both legs (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_shift_count_domain_exit_canary_runs() {
    // Shift counts carry no domain weight: wrapped << exact_count resolves
    // with the lhs domain (the mixed-domain check exempts shift rhs).
    let canary = pass_canary(fixture_roster::RUNTIME_SHIFT_COUNT_DOMAIN_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-shiftdom-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("shift count domain canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("shift count domain canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("shift count domain canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "shift count domain canary should pass both legs (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_exact_guarded_shift_count_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_EXACT_GUARDED_SHIFT_COUNT_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-shiftexact-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("guard-proven Exact shift canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("guard-proven Exact shift canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("guard-proven Exact shift canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_shift_atwidth_signed_modular_exit_canary_runs() {
    // Wrapping << masks counts by the language width on every engine: the
    // i32 write path plus u32 operand-position legs (counts 40 and 70).
    let canary = pass_canary(fixture_roster::RUNTIME_SHIFT_ATWIDTH_SIGNED_MODULAR_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-shlatw-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("at-width modular shl canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("at-width modular shl canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("at-width modular shl canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "at-width modular shl canary should pass all legs (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_shift_right_atwidth_exit_canary_runs() {
    // Wrapping >> masks counts by the language width for logical and
    // arithmetic forms, in write and nested operand positions.
    let canary = pass_canary(fixture_roster::RUNTIME_SHIFT_RIGHT_ATWIDTH_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-shratw-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("at-width shr canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("at-width shr canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("at-width shr canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "at-width shr canary should pass all legs (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_shift_atwidth_indexed_targets_exit_canary_runs() {
    // Pins the planner routing that keeps masked-count Wrapping shifts correct
    // for indexed/pointee targets and Exact-count spellings: the value
    // travels the (masked) operand path, never the domain-less indexed
    // binary-write kinds.
    let canary = pass_canary(fixture_roster::RUNTIME_SHIFT_ATWIDTH_INDEXED_TARGETS_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-shlidx-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("indexed-targets shift canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed-targets shift canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed-targets shift canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "indexed-targets shift canary should pass all legs (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_sat_nested_operand_domain_exit_canary_runs() {
    // The fused write's domain witness sees through nested binary operands:
    // (a + b) + 50 at u8-Saturating clamps the OUTER add too.
    let canary = pass_canary(fixture_roster::RUNTIME_SAT_NESTED_OPERAND_DOMAIN_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-satnest-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested-operand domain canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested-operand domain canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested-operand domain canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "nested-operand domain canary should pass (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_sat_unsigned_onedirection_exit_canary_runs() {
    // Narrow unsigned Saturating ops clamp in the one direction each
    // operator can overflow; the mul leg pins the UNSIGNED upper compare
    // (a 2^63+ u32 product read signed-negative and clamped to 0 before).
    let canary = pass_canary(fixture_roster::RUNTIME_SAT_UNSIGNED_ONEDIRECTION_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-satdir-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("one-direction saturating canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("one-direction saturating canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("one-direction saturating canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "one-direction saturating canary should pass all legs (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_sat_min_idiom_exit_canary_runs() {
    // The MIN idiom `0 - 2147483648` computes MIN (immediates never
    // re-extend in the sat/trap narrow paths).
    let canary = pass_canary(fixture_roster::RUNTIME_SAT_MIN_IDIOM_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-minidm-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("MIN idiom canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("MIN idiom canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("MIN idiom canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "MIN idiom canary should compute MIN (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_shl_saturating_exit_canary_runs() {
    // Saturating << clamps on true-value overflow (u8 17 << 4 -> 255).
    let canary = pass_canary(fixture_roster::RUNTIME_SHL_SATURATING_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-shlsat-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("saturating shl canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("saturating shl canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("saturating shl canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "saturating shl canary should clamp (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_shl_saturating_value_overflow_exit_canary_runs() {
    // F8: an at-width Saturating COUNT is now a compile error (the retired
    // runtime_shl_saturating_atwidth_exit shape lives on as
    // fail/arithmetic/shift_count_saturating_oor_rejected); this keeps the
    // 32-bit VALUE-overflow clamp pinned with a PROVEN count (3 << 31).
    let canary = pass_canary(fixture_roster::RUNTIME_SHL_SATURATING_VALUE_OVERFLOW_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-shlsatvo-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("value-overflow saturating shl canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("value-overflow saturating shl canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("value-overflow saturating shl canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "value-overflow saturating shl canary should clamp (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_shift_subword_masked_count_exit_canary_runs() {
    // F8b: sub-word Wrapping shifts mask the count at the OPERAND width via
    // the explicit AND (counts chosen to uniquely witness mask 7/15).
    let canary = pass_canary(fixture_roster::RUNTIME_SHIFT_SUBWORD_MASKED_COUNT_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-shsubw-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("sub-word masked-count canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("sub-word masked-count canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("sub-word masked-count canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected sub-word masked-count shifts (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_to_int_saturating_exit_canary_runs() {
    // F4: the Saturating float->int cast clamps (NaN -> 0, OOR -> bounds,
    // in-range truncates). aarch64 FCVTZS natively IS these semantics; x86
    // classifies NaN/range before cvttsd2si and selects the policy result.
    let canary = pass_canary(fixture_roster::FLOAT_TO_INT_SATURATING_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-f2isat-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("saturating float->int canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("saturating float->int canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("saturating float->int canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the Saturating cast semantics (exit 70), got {:?}",
        output.status.code(),
    );
    // The interpreter leg mirrors the same clamp arm.
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("saturating float->int canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.exit_code, 70, "interp Saturating cast should clamp");
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_to_int_unsigned_narrow_saturating_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::FLOAT_TO_INT_UNSIGNED_NARROW_SATURATING_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-f2i-shapes-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("unsigned/narrow Saturating float->int canary should compile");
    let executable = compilation.checked_native_executable_path().expect(
        "unsigned/narrow Saturating float->int canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("unsigned/narrow Saturating float->int canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected all unsigned/narrow cast shapes to clamp (exit 70), got {:?}",
        output.status.code(),
    );
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("unsigned/narrow Saturating canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter cast shapes should agree"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_saturating_overflow_exit_canary_runs() {
    // F5: Saturating float arithmetic clamps magnitude overflow to
    // +-MAX_FINITE (div-by-zero keeps its Inf) on both native backends.
    let canary = pass_canary(fixture_roster::FLOAT_SATURATING_OVERFLOW_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-f5sat-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("saturating float overflow canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("saturating float overflow canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("saturating float overflow canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the Saturating clamp semantics (exit 70), got {:?}",
        output.status.code(),
    );
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("saturating float overflow canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interp Saturating clamp should agree"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_trapping_overflow_traps_aborts() {
    // F5: Trapping float arithmetic traps on overflow. Abort-style +
    // interpreter-checked because native termination is abnormal.
    let canary = pass_canary(fixture_roster::FLOAT_TRAPPING_OVERFLOW_TRAPS);
    let build_dir = std::env::temp_dir().join(format!("omega-f5trap-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("trapping float overflow canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("trapping float overflow canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("trapping float overflow canary should run");
    assert_ne!(
        output.status.code(),
        Some(7),
        "expected the float overflow to trap, but the program sailed past to exit 7"
    );
    assert_ne!(
        output.status.code(),
        Some(71),
        "the in-range Trapping product computed wrong"
    );
    assert!(
        !output.status.success(),
        "expected the overflow trap to terminate abnormally"
    );
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("trapping float overflow canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    let reason = outcome
        .error
        .expect("the interpreter must trap the float overflow");
    assert!(
        reason.contains("float overflow"),
        "expected the overflow trap reason, got: {reason}"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_trapping_divzero_traps_aborts() {
    assert_float_trapping_policy_canary_aborts(
        fixture_roster::FLOAT_TRAPPING_DIVZERO_TRAPS,
        "division by zero",
    );
}

#[test]
fn float_trapping_invalid_traps_aborts() {
    assert_float_trapping_policy_canary_aborts(
        fixture_roster::FLOAT_TRAPPING_INVALID_TRAPS,
        "invalid float operation",
    );
}

#[test]
fn trapping_float_to_int_cast_traps_aborts() {
    // F4: a Trapping float->int cast traps on an out-of-range value (1e20
    // -> i32) instead of FCVTZS's silent saturate. In-range computes first
    // (7.9 -> 7). Named without `_canary_runs` (non-clean-exit).
    let canary = pass_canary(fixture_roster::TRAPPING_FLOAT_TO_INT_CAST_TRAPS);
    let build_dir = std::env::temp_dir().join(format!("omega-trap-f2i-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("trapping float->int cast canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("trapping float->int cast canary should run");

    assert_ne!(
        output.status.code(),
        Some(7),
        "expected the out-of-range Trapping cast to trap (1e20 does not fit i32), but \
         the program sailed past to exit 7\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_ne!(
        output.status.code(),
        Some(71),
        "the in-range Trapping cast computed wrong (7.9 should convert to 7)"
    );
    assert!(
        !output.status.success(),
        "expected the cast trap to terminate abnormally, but it exited successfully"
    );

    // The interpreter leg: same trap, spelled as an eval error.
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("trapping float->int cast canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    let reason = outcome
        .error
        .expect("the interpreter must trap the out-of-range Trapping cast");
    assert!(
        reason.contains("float-to-int conversion failed in Trapping domain")
            && reason.contains("truncated value is out of range")
            && reason.contains("I32"),
        "expected the cast trap reason, got: {reason}"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn trapping_float_to_narrow_int_cast_traps_aborts() {
    let canary = pass_canary(fixture_roster::TRAPPING_FLOAT_TO_NARROW_INT_CAST_TRAPS);
    let build_dir = std::env::temp_dir().join(format!("omega-trap-f2i-u8-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("narrow Trapping float->int canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("narrow Trapping float->int canary should run");
    assert_ne!(
        output.status.code(),
        Some(7),
        "u8 out-of-range cast sailed past"
    );
    assert_ne!(
        output.status.code(),
        Some(71),
        "in-range u8 conversion was wrong"
    );
    assert!(!output.status.success(), "u8 out-of-range cast must trap");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("narrow Trapping canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    let reason = outcome
        .error
        .expect("interpreter must report the same narrow cast trap");
    assert!(
        reason.contains("float-to-int conversion failed in Trapping domain")
            && reason.contains("truncated value is out of range")
            && reason.contains("U8"),
        "expected the narrow cast trap reason, got: {reason}"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn trapping_shift_count_traps_aborts() {
    // F8c (ch5 shift-count ruling): a TRAPPING shift's out-of-range count
    // traps VALUE-BLIND (`0 << 40` traps even though 0 fits u32). Named
    // without `_canary_runs` (non-clean-exit; outside the RUN-list drift
    // guard, like dead_trapping_let_traps_aborts).
    let canary = pass_canary(fixture_roster::TRAPPING_SHIFT_COUNT_TRAPS);
    let build_dir = std::env::temp_dir().join(format!("omega-trap-shcnt-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("trapping shift-count canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("trapping shift-count canary should run");

    assert_ne!(
        output.status.code(),
        Some(7),
        "expected the out-of-range Trapping shift COUNT to trap (0 << 40 -- the value \
         fits, the count is invalid), but the program sailed past to exit 7\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_ne!(
        output.status.code(),
        Some(71),
        "the in-range Trapping << leg computed wrong"
    );
    assert_ne!(
        output.status.code(),
        Some(72),
        "the in-range Trapping >> leg computed wrong"
    );
    assert!(
        !output.status.success(),
        "expected the count trap to terminate abnormally, but it exited successfully"
    );

    // The interpreter leg: same trap, spelled as an eval error.
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("trapping shift-count canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    let reason = outcome
        .error
        .expect("the interpreter must trap the out-of-range Trapping shift count");
    assert!(
        reason.contains("shift count out of range"),
        "expected the count trap reason, got: {reason}"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_literal_cast_proves_exit_canary_runs() {
    // F4's proof side: a bare float->int cast with a LITERAL source proves
    // when the truncation fits the target range.
    let canary = pass_canary(fixture_roster::FLOAT_LITERAL_CAST_PROVES_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-f2ilit-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("float-literal cast canary should compile (the literal proves)");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float-literal cast canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the proven literal truncations (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn u64_magnitude_transition_arg_exit_canary_runs() {
    // D14 Fire H (the CR3 remaining face): a u64-magnitude literal in
    // transition-argument position delivers into a u64-classed param.
    let canary = pass_canary(fixture_roster::U64_MAGNITUDE_TRANSITION_ARG_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-u64arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("u64-magnitude transition-arg canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("u64-magnitude transition-arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("u64-magnitude transition-arg canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the u64-magnitude arg delivery (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_shift_count_proven_range_exit_canary_runs() {
    // F8 proof side: a RANGED runtime count (u32 [0..=7]) proves count <
    // width, so the Exact shift carries no obligation and computes exactly.
    let canary = pass_canary(fixture_roster::RUNTIME_SHIFT_COUNT_PROVEN_RANGE_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-shcntrng-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("proven-range shift-count canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("proven-range shift-count canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("proven-range shift-count canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "proven-range shift-count canary should compute exactly (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}
