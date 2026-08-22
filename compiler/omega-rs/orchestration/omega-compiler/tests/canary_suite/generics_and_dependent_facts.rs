use super::*;

#[test]
fn runtime_decreases_u64_measure_exit_canary_runs() {
    // u64-typed termination measures verify like usize ones (the usize
    // retirement's stage-1 enabler; natural_measure_names_match).
    let canary = pass_canary("proofs/runtime_decreases_u64_measure_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-decu64-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("u64 decreases canary should compile (termination must accept u64 measures)");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("u64 decreases canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "u64 decreases canary should pass (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wrapping_operand_truncation_exit_canary_runs() {
    // Nested Wrapping binaries in operand position hand the parent the
    // width-wrapped value (>> / % legs pin the sign/width-sensitive reads).
    let canary = pass_canary("arithmetic/runtime_wrapping_operand_truncation_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-wraptrunc-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("wrapping operand truncation canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("wrapping operand truncation canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "wrapping operand truncation canary should pass all legs (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_float_compare_bool_exit_canary_runs() {
    // Float comparisons in value/write position (FCMP + materialized 0/1 at
    // operand width). Negative doubles pin the numeric-vs-bitwise ordering.
    let canary = pass_canary("arithmetic/runtime_float_compare_bool_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-fcmpbool-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("float compare bool canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float compare bool canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "float compare bool canary should pass all legs (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn aggregate_transition_args_exit_canary_runs() {
    // Whole-aggregate transition args: struct-by-value with ZII holes exact,
    // sum literal constructed in arg position and destructured.
    let canary = pass_canary("structs/aggregate_transition_args_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-aggarg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("aggregate transition-arg canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("aggregate transition-arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("aggregate transition-arg canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "aggregate transition-arg canary should pass all legs (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn deep_nested_write_paths_exit_canary_runs() {
    // Deep-nesting writes land without bleeding into ZII neighbors:
    // struct-in-struct, sum-in-struct, array-of-struct element field.
    let canary = pass_canary("structs/deep_nested_write_paths_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-deepw-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("deep nested write canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("deep nested write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("deep nested write canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "deep nested write canary should pass all legs (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn zii_default_composite_exit_canary_runs() {
    // ZII composites: a never-written sum dispatches as its first case with
    // zero payload; never-written array elements and nested fields read 0.
    let canary = pass_canary("core/zii_default_composite_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-ziicomp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("zii composite canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("zii composite canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("zii composite canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "zii composite canary should pass all legs (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn zii_string_host_write_exit_canary_runs() {
    // A ZII bounded carrier reaches the host adapter as an empty borrowed view.
    let canary = pass_canary("text/zii_string_host_write_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("ZII carrier host-write canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.exit_code, 70);
    assert_eq!(outcome.stdout, b"\nafter-zii\n".to_vec());
    let build_dir = std::env::temp_dir().join(format!("omega-ziihost-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("zii host-write canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("zii host-write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("zii host-write canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "zii host-write canary should print and exit 70, got {:?}",
        output.status.code(),
    );
    assert_eq!(output.stdout, b"\nafter-zii\n".to_vec());
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn zii_default_string_equality_exit_canary_runs() {
    // A ZII bounded text carrier is empty through content equality; the
    // non-empty-literal leg must not read beyond its zero length.
    let canary = pass_canary("text/zii_default_string_equality_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("ZII carrier equality canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should treat ZII carriers as empty text (exit 70), got {}",
        outcome.exit_code
    );
    let build_dir = std::env::temp_dir().join(format!("omega-ziistr-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("zii string equality canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("zii string equality canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("zii string equality canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "zii string equality canary should pass all three legs (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_owned_string_byte_view_exit_canary_runs() {
    // The honest adapter prerequisite: owned String -> borrowed text view ->
    // borrowed bytes. Native lowering copies the descriptor, and the
    // interpreter shares the same byte cell; neither path passes the owned
    // String directly as a byte-slice argument.
    let canary = pass_canary("text/runtime_owned_string_byte_view_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("owned String byte-view canary should check");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(
        interpreted.error, None,
        "owned String byte-view canary should interpret"
    );
    assert_eq!(interpreted.exit_code, 70);

    let build_dir = std::env::temp_dir().join(format!("omega-string-view-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("owned String byte-view canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("owned String byte-view canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("owned String byte-view canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn equatable_sum_stale_payload_exit_canary_runs() {
    // Synthesized sum equality is tag-aware: stale bytes from a longer
    // variant reassigned away must not leak into ==.
    let canary = pass_canary("traits/equatable_sum_stale_payload_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-sumstale-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("sum stale-payload equality canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("sum stale-payload equality canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("sum stale-payload equality canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "sum stale-payload equality canary should hold a == b (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_text_not_equals_exit_canary_runs() {
    // Text != in value + guard positions; the equal-strings leg is the pin
    // (the negation flag was ignored and != behaved as == on both ISAs).
    let canary = pass_canary("text/runtime_text_not_equals_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("carrier text not-equals canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should pass all carrier not-equals legs (exit 70), got {}",
        outcome.exit_code
    );
    let build_dir = std::env::temp_dir().join(format!("omega-texteqne-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("text not-equals canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("text not-equals canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("text not-equals canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "text not-equals canary should pass all four legs (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_text_equals_boolean_operand_exit_canary_runs() {
    // Texteq nested in a boolean AND, both operand orders x both targets;
    // the right-operand legs pin the pool-drawn address register (a fixed
    // x15 collided with the right pool's first pick and read garbage).
    let canary = pass_canary("text/runtime_text_equals_boolean_operand_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("carrier text boolean-operand canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should pass all nested carrier equality legs (exit 70), got {}",
        outcome.exit_code
    );
    let build_dir = std::env::temp_dir().join(format!("omega-texteqbool-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("texteq boolean-operand canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("texteq boolean-operand canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("texteq boolean-operand canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "texteq boolean-operand canary should pass all four legs (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn case_literal_texteq_terminal_exit_canary_runs() {
    // Text equality as a case-literal payload field in a value-machine
    // TERMINAL: the write rides the binary write's own target arms into the
    // frame staging slot, and the TextEqualsLiteral operand encoder must not
    // clobber the write's target base (x15, not x16).
    let canary = pass_canary("text/case_literal_texteq_terminal_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("carrier texteq terminal canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should deliver carrier equality in the terminal payload (exit 70), got {}",
        outcome.exit_code
    );
    let build_dir = std::env::temp_dir().join(format!("omega-texteqterm-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("texteq terminal canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("texteq terminal canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("texteq terminal canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "texteq terminal canary should deliver z == true (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn case_literal_texteq_field_store_exit_canary_runs() {
    // Text equality as a case-literal payload field in a FIELD STORE --
    // promoted from the fail tier when the literal-RHS TextEqualsLiteral arm
    // landed in the value-operand resolver (was: silently dropped, then
    // poisoned). Exit 70 proves content delivery, not just compilation.
    let canary = pass_canary("text/case_literal_texteq_field_store_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("carrier texteq field-store canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should deliver carrier equality in the stored payload (exit 70), got {}",
        outcome.exit_code
    );
    let build_dir = std::env::temp_dir().join(format!("omega-texteqstore-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("texteq field-store canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("texteq field-store canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("texteq field-store canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "texteq field-store canary should deliver z == true (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_text_equals_value_positions_exit_canary_runs() {
    // Carrier content equality in every value/write position: let-local,
    // field store vs literal, field store vs place. Exits 71/72/73 name the
    // leg that broke.
    let canary = pass_canary("text/runtime_text_equals_value_positions_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("carrier text value-position canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should pass all carrier value-position legs (exit 70), got {}",
        outcome.exit_code
    );
    let build_dir = std::env::temp_dir().join(format!("omega-texteqval-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("texteq value-positions canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("texteq value-positions canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("texteq value-positions canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "texteq value-positions canary should pass all three legs (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn sum_payload_cast_operand_field_exit_canary_runs() {
    // A case-literal terminal's payload field whose value is a BINARY WITH A
    // CAST OPERAND (`z: (x as i8) % 10`): the branch-side cascade writes each
    // field independently and its resolver had no Cast arm, so ONLY that field
    // was dropped (tag + siblings landed) and z read ZII 0 -- a silent partial
    // construction. Exit 70 proves the Convert-wrapped operand serves.
    let canary = pass_canary("control_flow/sum_payload_cast_operand_field_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-sumcast-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("sum cast-operand payload canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("sum cast-operand payload canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "sum cast-operand payload canary should deliver z == 3 (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_branching_callee_chain_exit_canary_runs() {
    // Statement calls into a dispatching entry (incl. sub-state chained) --
    // the 2026-07-04 refusal, closed by the branch-call expansion rungs.
    let canary = pass_canary("calls/runtime_branching_callee_chain_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-brchain-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("branching callee chain canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("branching callee chain canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("branching callee chain canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "branching callee chain canary should count both dispatched hits (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn recursive_result_bind_first_arg_canary_runs() {
    // The bind-first pairing (`let r = self.countdown(..); let d =
    // self.plus1(r);`) in a multi-call composition: a recursive value-call
    // result whose ONLY use is an inline-call argument. The liveness scan
    // elides `r`'s LocalStorage slot (later-`let` values are covered by the
    // alias fold) while the alias binding refuses to fold call-initialized
    // locals (they resolve to their call-result slot) -- so the serve sweep's
    // let-bound gate must be the AST question (is the statement a `let`?),
    // not a state_storage.locals scan. When it wasn't, `r` had NO storage:
    // the return edge wrote nothing and the inline `v + 1` name-captured a
    // COLLIDING caller-scope `v` (exit 73 silently) or dropped the add.
    let canary = pass_canary("calls/recursive_result_bind_first_arg");
    let build_dir = std::env::temp_dir().join(format!("omega-bindfirst-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("bind-first arg canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("bind-first arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bind-first arg canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "bind-first arg canary should deliver r through plus1 (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_recursive_result_roles_exit_canary_runs() {
    // Recursive value-call results consumed as GUARD subjects and TRANSITION
    // ARGUMENTS (the aggregate sweep's role coverage beyond let bindings).
    let canary = pass_canary("termination/runtime_recursive_result_roles_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-recroles-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("recursive result roles canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("recursive result roles canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("recursive result roles canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both recursive-result roles to deliver (exit 70), got {:?}",
        output.status.code(),
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_trapping_guard_overflow_traps_canary_runs() {
    // Trapping arithmetic in OPERAND position must TRAP: `u8 in Trapping`
    // 200+100 overflows at the guard's fused add, so the process dies before
    // either exit. A regression back to the plain fused add would truncate
    // the wide 300 to 44 at the byte-width compare and exit 70 silently.
    let canary = pass_canary("arithmetic/runtime_trapping_guard_overflow_traps");
    let build_dir = std::env::temp_dir().join(format!("omega-trapguard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("trapping guard-overflow canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("trapping guard-overflow canary should spawn");
    let code = output.status.code();
    assert_ne!(
        code,
        Some(70),
        "Trapping guard overflow must abort before the clean exit -- exit 70 means the fused add silently wrapped"
    );
    assert_ne!(
        code,
        Some(71),
        "Trapping guard overflow must abort, not fall through to the false arm"
    );
    // Windows reports the trap as a negative NTSTATUS exit code; unix hosts
    // terminate on the signal (SIGTRAP/SIGILL), where `code()` is None.
    assert!(
        code.is_none() || code.is_some_and(|code| code < 0),
        "expected a crash status (brk/ud2 kill), got {code:?}"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_trapping_overflow_traps_canary_runs() {
    // Trapping must TRAP: i32::MAX + 1 under `in Trapping` executes ud2, so the
    // process dies with a crash status and never reaches exit_process(70). If a
    // regression made Trapping silently wrap, this would exit 70 and fail.
    let canary = pass_canary("arithmetic/runtime_trapping_overflow_traps");
    let build_dir = std::env::temp_dir().join(format!("omega-trap-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("trapping overflow canary should compile (the partiality is declared)");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("trapping overflow canary should start");
    let code = output.status.code();
    assert_ne!(
        code,
        Some(70),
        "Trapping overflow must abort before the clean exit -- exit 70 means it silently wrapped"
    );
    // Windows reports the trap as a negative NTSTATUS exit code
    // (STATUS_ILLEGAL_INSTRUCTION); unix hosts terminate on the signal
    // (SIGILL/SIGTRAP), where `code()` is None.
    assert!(
        code.is_none() || code.is_some_and(|code| code < 0),
        "expected a crash status (ud2/brk -> illegal-instruction kill), got {code:?}"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_guard_proven_counter_exit_canary_runs() {
    // The de-Trapping keystone: a state entered through `count < 5` proves
    // `count = count + 1` into [0..=100] -- Exact, no domain. Exit 70.
    let canary = pass_canary("arithmetic/runtime_guard_proven_counter_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-gpc-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("guard-proven counter canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("guard-proven counter canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("guard-proven counter canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the guard-proven counter to reach 5 (exit 70), got {:?}
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_guard_narrowed_transition_arg_exit_canary_runs() {
    // The co-located face: the arm guard narrows `count + 1` into the ranged
    // parameter. Exit 70.
    let canary = pass_canary("arithmetic/runtime_guard_narrowed_transition_arg_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-gnta-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("guard-narrowed transition arg canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("guard-narrowed transition arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("guard-narrowed transition arg canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the narrowed argument to store 1 (exit 70), got {:?}
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_gui_window_lifecycle_exit_canary_runs() {
    // Message pump + lifecycle: create an invisible window, drain PeekMessageW (bounded),
    // IsWindow > 0, DestroyWindow > 0, IsWindow == 0. Exit 70. CI-safe.
    let canary = pass_canary("host/runtime_gui_window_lifecycle_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-gui-life-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("gui window lifecycle canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("gui window lifecycle canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected pump-drain + live/destroyed liveness transitions (exit 70), got {:?}
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// `foreground_window()` -- the focus gate for GLOBAL GetAsyncKeyState (an
// unfocused app must not treat a desktop-wide ESC, e.g. the Ctrl+Shift+Esc
// chord, as its quit key). No value assertion: the interp's virtual desktop
// foregrounds the last live window while a native style-0 window is invisible
// and never foreground -- the canary pins the call path, not the value.
// Windows-gated so the macOS baseline failure set gains no new test name.
#[cfg(windows)]
#[test]
fn runtime_gui_foreground_window_exit_canary_runs() {
    let canary = pass_canary("host/runtime_gui_foreground_window_exit");
    let main_path = canary.join("main.omg");

    let checked = omega_compiler::compile_to_checked(&main_path, None)
        .expect("gui foreground-window canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (virtual foreground call + destroy), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-gui-fg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("gui foreground-window canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("gui foreground-window canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected create + foreground_window + destroy (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_gui_window_blit_exit_canary_runs() {
    // The windowed integration proof: CreateWindowExA("STATIC", style 0 -- INVISIBLE, CI-safe)
    // -> GetDC -> StretchDIBits into the window DC. Real HWND end-to-end. Exit 70.
    let canary = pass_canary("host/runtime_gui_window_blit_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-gui-wnd-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("gui window blit canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("gui window blit canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected window-create + get_dc + full-height blit (exit 70), got {:?}
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_generic_value_call_agreeing_exit_canary_runs() {
    // Two value calls to one generic machine with AGREEING instantiations (both T := i32 in
    // Wrapping): the conflict detector must not fire and both results materialize. 30+40 -> 70.
    let canary = pass_canary("generics/runtime_generic_value_call_agreeing_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-gen-agree-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("agreeing generic value calls canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("agreeing generic calls canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("agreeing generic value calls canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected two agreeing generic value calls to both materialize (exit 70), got {:?}
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_generic_value_call_exit_canary_runs() {
    // A monomorphized generic VALUE call: `let v: i32 in Wrapping = self.id(70)` with
    // `id<T>(x: T) -> T`. Used to silently return 0 natively; the monomorphization pass now infers
    // T from the annotated let and the result materializes. Exit 70.
    let canary = pass_canary("generics/runtime_generic_value_call_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-gen-vcall-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("generic value call canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("generic value call canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("generic value call canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a monomorphized generic value call to materialize its result (exit 70), got {:?}
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn trait_generic_bound_static_dispatch_canary_runs() {
    let canary = pass_canary("traits/trait_generic_bound_static_dispatch");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("bounded generic call should specialize to its nominal conformance");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 1);

    let build_dir = std::env::temp_dir().join(format!(
        "omega-trait-bound-static-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("bounded generic call should compile natively from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("bounded generic call canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bounded generic call canary should run");
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected Counter::increment to run through static generic dispatch; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_generic_param_position_inference_exit_canary_runs() {
    let canary = pass_canary("generics/runtime_generic_param_position_inference_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("borrowed-place parameter inference canary should check");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let build_dir =
        std::env::temp_dir().join(format!("omega-gen-param-infer-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("borrowed-place parameter inference canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("borrowed-place parameter inference canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("borrowed-place parameter inference canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected T := Light inferred through &T and a materialized result (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_generic_multiple_specializations_exit_canary_runs() {
    let canary = pass_canary("generics/runtime_generic_multiple_specializations_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("multiple generic-machine specialization tuples should check");
    assert_eq!(
        checked
            .machine_specializations
            .iter()
            .filter(|specialization| {
                checked.machines().iter().any(|machine| {
                    machine.symbol == specialization.template
                        && machine.name.as_str() == "Main::pick"
                })
            })
            .count(),
        2
    );
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 14);

    let build_dir =
        std::env::temp_dir().join(format!("omega-gen-multi-spec-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("multiple generic-machine specialization tuples should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("multiple generic-machine specializations should retain their executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("multiple generic-machine specialization canary should run");
    assert_eq!(
        output.status.code(),
        Some(14),
        "expected both concrete clones to materialize results (exit 14), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_generic_enum_payload_exit_canary_runs() {
    // A monomorphized generic ENUM with a T-typed payload (`Maybe<i32 in Wrapping>`), constructed,
    // matched, and destructured natively -- the Option<T> shape. Its erased evidence payload
    // remains semantic but takes no runtime storage. Exit 70 via the material payload.
    let canary = pass_canary("generics/runtime_generic_enum_payload_exit");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("generic enum payload canary should reach checked semantics");
    let interpreted = psi_checked_interpreter::interpret_entry(&checked, "Main::main", &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let build_dir = std::env::temp_dir().join(format!("omega-gen-enum-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("generic enum payload canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("generic enum payload canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("generic enum payload canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a monomorphized generic enum payload to destructure natively (exit 70), got {:?}
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_generic_record_instance_exit_canary_runs() {
    // A monomorphized generic data instance (`Box<i32 in Wrapping>`) with native field access to
    // both the T-typed field and a concrete sibling: tag=30 + val=40 -> exit 70. Locks stage-1
    // generics monomorphization (recorded instance layout keyed by the definition symbol).
    let canary = pass_canary("generics/runtime_generic_record_instance_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-gen-inst-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("generic record instance canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("generic record instance canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("generic record instance canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected native field access on a monomorphized generic instance (exit 70), got {:?}
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_data_array_length_exit_canary_runs() {
    let canary = pass_canary("generics/runtime_const_data_array_length_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-const-data-array-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("literal const data argument should specialize the array extent");
    let executable = compilation
        .checked_native_executable_path()
        .expect("const-data array canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("const data array canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected specialized `[i32; 4]` storage to round-trip 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_data_forwarded_length_exit_canary_runs() {
    let canary = pass_canary("generics/runtime_const_data_forwarded_length_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-const-data-forwarded-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("forwarded const data argument should specialize the nested array extent");
    let executable = compilation
        .checked_native_executable_path()
        .expect("forwarded const-data canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("forwarded const data array canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected forwarded `[i32; 4]` storage to round-trip 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_data_multiple_instances_exit_canary_runs() {
    let canary = pass_canary("generics/runtime_const_data_multiple_instances_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-const-data-multi-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("distinct const data instances should compile to distinct layouts");
    let executable = compilation
        .checked_native_executable_path()
        .expect("multiple const-data instances should retain their executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("multiple const data instance canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected two const-specialized buffers to sum to 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_data_named_value_exit_canary_runs() {
    let canary = pass_canary("generics/runtime_const_data_named_value_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-const-data-named-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("named integer const arguments should specialize generic data");
    let executable = compilation
        .checked_native_executable_path()
        .expect("named const-data canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("named const data argument canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected named const-specialized buffers to sum to 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn structured_const_identity_and_rat_canonicality_canaries() {
    for path in [
        "generics/structured_const_canonical_identity",
        "generics/structured_const_canonical_rat",
    ] {
        let canary = pass_canary(path);
        compile_canary_without_output(&canary).unwrap_or_else(|diagnostics| {
            panic!(
                "structured const pass canary `{path}` should compile:\n{}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    }

    for (path, expected) in [
        (
            "generics/structured_const_default_domain_unproved",
            "default-domain facts whose index-site proof is not implemented",
        ),
        (
            "generics/structured_const_ineligible_float_field",
            "not eligible as a const index",
        ),
        (
            "generics/structured_const_rat_zero_denominator",
            "denominator must be positive",
        ),
        (
            "generics/structured_const_rat_uncancelled",
            "signed coordinates must be cancelled",
        ),
        (
            "generics/structured_const_rat_unreduced",
            "must be gcd-reduced",
        ),
    ] {
        let canary = fail_canary(path);
        let diagnostics = compile_canary_without_output(&canary)
            .expect_err("noncanonical structured const index should reject");
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            combined.contains(expected),
            "structured const fail canary `{path}` should contain `{expected}`:\n{combined}"
        );
    }
}

#[test]
fn closed_indexed_domain_canaries() {
    let pass = pass_canary("generics/closed_indexed_quantity");
    compile_canary_without_output(&pass).unwrap_or_else(|diagnostics| {
        panic!(
            "closed indexed domain package should compile:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let checked = compile_to_checked(&pass.join("main.omg"), None)
        .expect("closed indexed qualifications should survive checked lowering");
    let uses = &checked.facts.qualifications.vacuous_uses;
    assert_eq!(
        uses.len(),
        3,
        "closed qualification plus both concrete generic instances should be retained"
    );
    for use_fact in uses {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == use_fact.machine)
            .expect("qualification owner machine");
        let state = checked
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == use_fact.state)
            .expect("qualification owner state");
        let psi_typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } = checked
            .type_reference_table
            .type_reference(state.return_type)
        else {
            panic!("indexed qualification canary result should remain constrained");
        };
        let [psi_typed_trees::types::TypeConstraintNode::Domain(result_domain)] =
            checked.type_reference_table.constraints(*constraints)
        else {
            panic!("indexed qualification canary result should carry one domain");
        };
        assert_eq!(
            use_fact.semantic_domain, result_domain.semantic_id,
            "vacuous-use evidence must retain the exact indexed instance"
        );
    }
    let evidence = omega_visualizations::qualification_evidence_manifest_json(
        &checked,
        checked.selected_provider_plans(),
    );
    assert!(evidence.contains("\"semantic_domain_id\":"));
    assert!(evidence.contains("\"semantic_domain\":"));
    let retag = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "retag_i64")
        .expect("retag template instance");
    let specializations = checked
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.template == retag.symbol)
        .collect::<Vec<_>>();
    assert_eq!(specializations.len(), 2);
    assert!(
        specializations
            .iter()
            .all(|specialization| specialization.const_arguments.len() == 1)
    );
    assert_ne!(
        specializations[0].const_arguments,
        specializations[1].const_arguments
    );
    assert_ne!(
        specializations[0].fingerprint,
        specializations[1].fingerprint
    );
    let contracts = omega_visualizations::machine_contract_manifest_json(&checked);
    assert!(contracts.contains("\"const_arguments\":"));

    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let build_dir = std::env::temp_dir().join(format!(
        "omega-closed-indexed-qualification-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: pass.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("closed indexed generic conversion should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("closed indexed generic conversion should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected destination-specialized retag to return exit 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for path in [
        "generics/closed_indexed_domain_mismatch",
        "generics/closed_indexed_struct_field_mismatch",
        "generics/closed_indexed_array_element_mismatch",
        "generics/closed_indexed_domain_noncanonical_rat",
        "generics/closed_indexed_domain_unknown_const",
        "generics/closed_indexed_domain_wrong_arity",
        "generics/closed_indexed_domain_wrong_type",
        "generics/closed_indexed_qualification_unknown_const",
        "generics/closed_indexed_qualification_wrong_arity",
        "generics/closed_indexed_qualification_wrong_type",
        "generics/const_machine_destination_not_inferred",
    ] {
        let canary = fail_canary(path);
        let expected = fs::read_to_string(canary.join("expected.txt"))
            .expect("closed indexed domain fail canary should carry expected.txt");
        let diagnostics = compile_canary_without_output(&canary)
            .expect_err("invalid closed indexed domain should reject");
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            combined.contains(expected.trim()),
            "closed indexed domain fail canary `{path}` should contain {:?}:\n{combined}",
            expected.trim()
        );
    }
}

#[test]
fn std_units_package_conversion_and_operator_canaries() {
    let source = fs::read_to_string(repo_root().join("omega/language/std/units.omg"))
        .expect("read shipped units package");
    for published_name in [
        "Units::METER",
        "Units::KILOMETER",
        "Units::SECOND",
        "Units::METER_PER_SECOND",
        "Units::KILOMETER_PER_HOUR",
        "kilometers_to_meters_trapping_i64",
        "meters_to_kilometers_truncating_i64",
        "divide_f64_meters_by_seconds",
        "divide_f64_kilometers_by_hours",
        "kilometers_per_hour_to_meters_per_second_f64",
    ] {
        assert!(
            source.contains(published_name),
            "units package should publish `{published_name}`"
        );
    }

    let pass = pass_canary("generics/runtime_std_units_exit");
    let checked = compile_to_checked(&pass.join("main.omg"), None)
        .expect("shipped named units, conversions, and operators should check");
    assert!(
        checked
            .facts
            .index_compatibility
            .conditions
            .iter()
            .any(|condition| matches!(
                &condition.discharge,
                psi_checked_trees::IndexCompatibilityDischarge::ClosedEvaluation
            )),
        "closed unit flows should retain their closed-evaluation verification condition"
    );
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let build_dir =
        std::env::temp_dir().join(format!("omega-std-units-package-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: pass.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("shipped units package should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("shipped units canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected units conversion/operator canary to exit 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    let fail = fail_canary("generics/std_units_implicit_cross_index");
    let expected = fs::read_to_string(fail.join("expected.txt"))
        .expect("imported cross-index fail canary should carry expected.txt");
    let diagnostics = compile_canary_without_output(&fail)
        .expect_err("kilometers must not flow into a meters parameter implicitly");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(expected.trim()),
        "imported cross-index diagnostic should contain {:?}:\n{combined}",
        expected.trim()
    );
}

#[test]
fn open_computed_quantity_result_canary_runs() {
    let canary = pass_canary("generics/open_computed_quantity_result");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("generic computed index result should check");
    let selections = checked
        .open_index_normalizations
        .iter()
        .flat_map(|normalization| &normalization.operations)
        .collect::<Vec<_>>();
    assert!(!selections.is_empty());
    assert!(selections.iter().all(|selection| {
        selection
            .operation_contract_identity
            .contains("IndexAlgebra::plus")
            && selection.algebra_requirement == "add"
            && selection.algebra_alias.as_deref() == Some("Canonical")
            && selection.provider.is_valid()
            && selection.algebra_trait.is_valid()
    }));
    assert!(
        checked
            .machine_specializations
            .iter()
            .any(|specialization| specialization.template_contract_fingerprint != 0)
    );
    assert!(
        checked
            .facts
            .index_compatibility
            .conditions
            .iter()
            .any(|condition| {
                condition.name.starts_with("index-equality:")
                    && matches!(
                        &condition.discharge,
                        psi_checked_trees::IndexCompatibilityDischarge::LicensedNormalization {
                            operation_count
                        } if *operation_count > 0
                    )
            }),
        "computed result flow should retain its licensed-normalization verification condition: {:#?}",
        checked.facts.index_compatibility.conditions
    );
    let compatibility = omega_visualizations::index_compatibility_manifest_json(&checked);
    assert!(compatibility.contains("\"name\": \"index-equality:"));
    assert!(compatibility.contains("\"discharge\": \"licensed_normalization\""));
    assert!(compatibility.contains("\"operation_count\": "));
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let fail = fail_canary("generics/open_index_unlicensed_algebra");
    let expected = fs::read_to_string(fail.join("expected.txt"))
        .expect("unlicensed open-index canary should carry expected.txt");
    let diagnostics = compile_canary_without_output(&fail)
        .expect_err("an unproved index algebra must not license normalization");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(combined.contains(expected.trim()), "{combined}");
}

#[test]
fn open_index_exact_local_fact_canary_runs() {
    let canary = pass_canary("generics/open_index_local_fact");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("an exact active equality should discharge open index compatibility");
    let conditions = checked
        .facts
        .index_compatibility
        .conditions
        .iter()
        .filter(|condition| {
            matches!(
                &condition.discharge,
                psi_checked_trees::IndexCompatibilityDischarge::EstablishedLocalFacts { .. }
            )
        })
        .collect::<Vec<_>>();
    assert!(
        conditions.len() >= 2,
        "both requires and call-ensures routes should retain evidence"
    );
    assert!(
        conditions
            .iter()
            .all(|condition| condition.actual_instance != condition.expected_instance)
    );
    assert!(
        conditions.iter().any(|condition| matches!(
            &condition.discharge,
            psi_checked_trees::IndexCompatibilityDischarge::EstablishedLocalFacts { facts }
                if facts.len() == 2
        )),
        "a two-member index pack should retain both exact equality facts"
    );
    let evidence = conditions
        .iter()
        .map(|condition| {
            let psi_checked_trees::IndexCompatibilityDischarge::EstablishedLocalFacts { facts } =
                &condition.discharge
            else {
                unreachable!();
            };
            assert!(!facts.is_empty());
            facts
                .iter()
                .map(|fact| {
                    assert!(fact.is_valid());
                    checked.facts.semantic.facts.get(*fact)
                })
                .collect::<Vec<_>>()
        })
        .flatten()
        .collect::<Vec<_>>();
    assert!(
        evidence
            .iter()
            .any(|fact| matches!(fact.point, psi_facts::ProgramPoint::CallEnsures { .. }))
    );
    assert!(evidence.iter().any(|fact| !matches!(
        fact.point,
        psi_facts::ProgramPoint::CallEnsures { .. } | psi_facts::ProgramPoint::Global
    )));
    let compatibility = omega_visualizations::index_compatibility_manifest_json(&checked);
    assert!(compatibility.contains("\"discharge\": \"established_local_fact\""));
    assert!(compatibility.contains("\"evidence_facts\": ["));
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let fail = fail_canary("generics/open_index_unestablished_equality");
    let expected = fs::read_to_string(fail.join("expected.txt"))
        .expect("unestablished index equality canary should carry expected.txt");
    let diagnostics = compile_canary_without_output(&fail)
        .expect_err("an ambient but inactive theorem must not discharge index compatibility");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(expected.trim())
            && combined.contains("exact local equality fact is required"),
        "unestablished equality diagnostic should be named and fail closed:\n{combined}"
    );
}

#[test]
fn runtime_const_data_expression_exit_canary_runs() {
    let canary = pass_canary("generics/runtime_const_data_expression_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-const-data-expression-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("closed integer const expressions should specialize generic data");
    let executable = compilation
        .checked_native_executable_path()
        .expect("const-data expression canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("const data expression canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected expression-specialized buffers to sum to 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_data_symbolic_expression_exit_canary_runs() {
    let canary = pass_canary("generics/runtime_const_data_symbolic_expression_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-const-data-symbolic-expression-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("symbolic integer const expressions should specialize generic data");
    let executable = compilation
        .checked_native_executable_path()
        .expect("symbolic const-data expression should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("symbolic const data expression canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected symbolic-expression buffers to sum to 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_data_machine_call_exit_canary_runs() {
    let canary = pass_canary("generics/runtime_const_data_machine_call_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-const-data-machine-call-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("const-evaluated machine calls should specialize generic data");
    let executable = compilation
        .checked_native_executable_path()
        .expect("const-data machine-call canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("const data machine-call canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_data_where_fact_exit_canary_runs() {
    let canary = pass_canary("generics/runtime_const_data_where_fact_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-const-data-where-fact-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("const-only generic facts should discharge at instantiation");
    let executable = compilation
        .checked_native_executable_path()
        .expect("const-data where-fact canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("const-fact generic canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn domain_body_case_membership_with_default_arm_compiles() {
    compile_canary_without_output(&pass_canary("data/match_default_satisfies_exhaustiveness"))
        .expect("a domain-body fact may name an implicit case domain");
}

#[test]
fn runtime_const_data_machine_fact_exit_canary_runs() {
    let canary = pass_canary("generics/runtime_const_data_machine_fact_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-const-data-machine-fact-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("machine-backed const domain facts should discharge");
    let executable = compilation
        .checked_native_executable_path()
        .expect("const-data machine-fact canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("machine-backed const domain fact canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_signed_const_data_exit_canary_runs() {
    let canary = pass_canary("generics/runtime_signed_const_data_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-signed-const-data-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("signed const data arguments should specialize");
    let executable = compilation
        .checked_native_executable_path()
        .expect("signed const-data canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("signed const data canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_trait_default_dispatch_exit_canary_runs() {
    let canary = pass_canary("traits/runtime_trait_default_dispatch_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-trait-default-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("trait defaults and written overrides should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("trait-default dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("trait default dispatch canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_inherited_trait_default_exit_canary_runs() {
    let canary = pass_canary("traits/runtime_inherited_trait_default_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-inherited-trait-default-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("inherited trait defaults should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("inherited trait-default canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("inherited trait default canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_generic_trait_default_exit_canary_runs() {
    let canary = pass_canary("traits/runtime_generic_trait_default_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-generic-trait-default-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("generic trait defaults should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("generic trait-default canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("generic trait default canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_container_methods_exit_canary_runs() {
    let canary = pass_canary("generics/runtime_const_container_methods_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-const-container-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("const-specialized container methods should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("const-container methods canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("const container method canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected const-specialized methods to sum to 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_generic_two_instantiations_exit_canary_runs() {
    // Phase 1: TWO distinct instantiations of `Box<T>` (`Box<i32>` + `Box<bool>`)
    // coexist in one program with native field access on both -- the
    // per-instance monomorphization (pre-resolution desugar to distinct concrete
    // records) that replaces the layout builder's one-slot poison. exit 30.
    let canary = pass_canary("generics/runtime_generic_two_instantiations_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-gen-two-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("two-instantiation generic canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("two-instantiation generic canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("two-instantiation generic canary should run");
    assert_eq!(
        output.status.code(),
        Some(30),
        "expected two coexisting generic instances with native access (exit 30), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_min_max_guard_subject_hoist_exit_canary_runs() {
    // #26: a pure builtin (`min`/`max`) used directly as a guard SUBJECT is
    // hoisted into a temp automatically, so the guard compares a materialized
    // local. Builtins are effect-free, so the effectful-single-eval constraint
    // that reverted the general value-call hoist is satisfied by construction.
    // Discriminating (min=7, max=8 both match -> good, exit 70; wrong builtin
    // or vacuous guard -> bad, exit 71).
    let canary = pass_canary("calls/runtime_min_max_guard_subject_hoist_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-minguard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("min/max guard-subject hoist canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("min/max guard-subject canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("min/max guard-subject hoist canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a hoisted pure-builtin guard subject to discriminate (exit 70), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_guard_true_false_pair_exit_canary_runs() {
    // #41 indexed half: `transition arr[i] > 5 { true -> false -> }` -- the
    // natural array-element branch. hoist_comparison_match_subject shares one
    // subject temp across arms (hoisting the read inside it), so the pair pairs
    // for exhaustiveness. Discriminating: arr[1]=20 > 5 true -> ok (70).
    let canary = pass_canary("collections/runtime_indexed_guard_true_false_pair_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-idxpair-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("indexed guard true/false pair canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed guard-pair canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed guard true/false pair canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a shared-subject indexed guard pair to discriminate (exit 70), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_field_local_operand_exit_canary_runs() {
    // A local from a field read off a runtime-indexed element (`let a =
    // self.ps[self.i].x`) used as an arithmetic operand was rejected -- the local
    // alias-folded back to `arr[i].field`, which has no operand lowering. It now
    // keeps its slot (local_data_requires_storage recognizes a Member-off-a-
    // runtime-index initializer). Discriminating: ps[1].x=20 + ps[0].x=10 + 12 =
    // 42 -> 70; a dropped operand (read 0) would mismatch.
    let canary = pass_canary("collections/runtime_indexed_field_local_operand_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-idxfield-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("indexed-field-local-operand canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed-field operand canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed-field-local-operand canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an indexed-field local used as an operand to keep its slot (exit 70), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_local_bitwise_exit_canary_runs() {
    // Silent miscompile fixed (sibling of the compare case): `let t = arr[i]; let
    // m = t & 6` read m as 0 -- a bitwise operand didn't force the indexed-read
    // local's slot, so it alias-folded and dropped. is_bitwise_operator now counts
    // it. Discriminating: (20&6)+(20|1)+(20^4)+29 = 4+21+16+29 = 70; the miscompile
    // (operands read 0) would give 29 -> 71.
    let canary = pass_canary("collections/runtime_indexed_local_bitwise_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-idxbit-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("indexed-local-bitwise canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed-local bitwise canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed-local-bitwise canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an indexed-read local used as a bitwise operand to read its slot (exit 70), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_local_compare_exit_canary_runs() {
    // Silent miscompile fixed: `let hi = arr[i]; let over: bool = hi > 5` read
    // `over` as a folded default (always false) -- the alias-fold substituted the
    // indexed-read local into the fenced `arr[i] > 5` form and silently produced
    // false. A comparison operand now keeps its slot (local_data_requires_storage
    // counts comparison operators), so the compare reads the slot. Discriminating
    // (over=true vs low_over=false -> exit 70; the miscompile made over=false -> 71).
    let canary = pass_canary("collections/runtime_indexed_local_compare_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-idxcmp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("indexed-local-compare canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed-local compare canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed-local-compare canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an indexed-read local used as a compare operand to read its slot (exit 70), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_min_guard_true_false_pair_exit_canary_runs() {
    // #41: a pure-builtin guard subject with a `{ true -> false -> }` PAIR. Each
    // arm re-lowers the subject to its own temp, so the pair stopped pairing for
    // exhaustiveness; hoist_comparison_match_subject shares one subject temp
    // across arms (keyed on the syntax subject handle). Discriminating: min=7
    // matches -> good (70); a wrong min or a failed pair would exit 71.
    let canary = pass_canary("calls/runtime_min_guard_true_false_pair_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-minpair-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("min guard true/false pair canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("min guard-pair canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("min guard true/false pair canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a shared-subject builtin guard pair to discriminate (exit 70), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_generic_instantiations_exit_canary_runs() {
    // Phase 3: NESTED generic data. Pair<T> contains a Box<T> field, so Pair<i32>
    // needs Box<i32> synthesized too (and Pair<bool> -> Box<bool>). The desugar
    // runs to a fixpoint: synthesizing Pair<i32> emits a fresh Box<i32> spelling
    // the next round monomorphizes; generic template bodies are skipped so the
    // param-arg Box<T> is never mistaken for a concrete instance. exit 30.
    let canary = pass_canary("generics/runtime_nested_generic_instantiations_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-gennest-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested generic instantiations canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested generic instances should retain their executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested generic instantiations canary should run");
    assert_eq!(
        output.status.code(),
        Some(30),
        "expected nested generic instances (Pair<i32>/Pair<bool> over Box<T>) to coexist (exit 30), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_generic_let_local_instantiations_exit_canary_runs() {
    // Phase-1 type-position polish: two distinct instantiations of Box<T> as
    // LET-LOCALS (Box<i32> + Box<bool>), not fields. The desugar now scans
    // machine-body type positions (let-locals, params, returns), not just data
    // fields, so the 2nd instantiation no longer poisons the layout. exit 30.
    let canary = pass_canary("generics/runtime_generic_let_local_instantiations_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-genlet-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("generic let-local instantiations canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("generic let-local instances should retain their executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("generic let-local instantiations canary should run");
    assert_eq!(
        output.status.code(),
        Some(30),
        "expected two coexisting generic let-local instances (exit 30), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_generic_domain_instantiations_exit_canary_runs() {
    // Phase 1 domain-arg extension: two DOMAIN-CARRYING instantiations of
    // `Box<T>` (`Box<i32 in Wrapping>` + `Box<u8 in Wrapping>`) coexist. Each
    // argument carries an arithmetic domain, so before the slug extension both
    // were skipped (non-plain-Named) and fell to the one-slot poison path. Now
    // each slugs distinctly into its own synthetic record with the domain riding
    // the substituted field. exit 42.
    let canary = pass_canary("generics/runtime_generic_domain_instantiations_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-gen-domain-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("domain-arg generic canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("domain-arg generic instances should retain their executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("domain-arg generic canary should run");
    assert_eq!(
        output.status.code(),
        Some(42),
        "expected two coexisting domain-carrying generic instances (exit 42), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_array_max_and_sum_exit_canary_runs() {
    // Find the max and the sum of an array in one pass: an indexed read bound to a local, a
    // reduction (`total += v`), and an element comparison via the sound local-bind pattern
    // (`transition v > self.mx`). arr = [30,50,70,20,60,10] -> max 70, sum 240, both checked -> 70.
    let canary = pass_canary("collections/runtime_array_max_and_sum_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-max-sum-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("array max-and-sum canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("array max-and-sum canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("array max-and-sum canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a one-pass max+sum reduction to compute correctly (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_reduction_loop_exit_canary_runs() {
    // Array reduction with a runtime index (`self.sum = self.sum + self.arr[self.i]`) in a loop --
    // an indexed read as an accumulation operand, the sum/reduce primitive. Sums [5,10,15,20,8,12]
    // = 70. The index bound is proven by the loop guard.
    let canary = pass_canary("collections/runtime_indexed_reduction_loop_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-indexed-reduce-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("indexed reduction loop canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed reduction canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed reduction loop canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an array reduction over a runtime index to sum correctly (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_rmw_loop_exit_canary_runs() {
    // Read-modify-write at a runtime index (`self.arr[self.i] = self.arr[self.i] + 10`) in a loop
    // -- the count/accumulate primitive, enabled by the machine-indexed binary write accepting an
    // indexed read as its value operand. Fills [0..4], increments each by 10 -> sum 60; a non-zero
    // `marker` after the index field guards the 32-bit index load and must survive -> exit 70.
    let canary = pass_canary("collections/runtime_indexed_rmw_loop_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-indexed-rmw-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("indexed RMW loop canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed RMW canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed RMW loop canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected read-modify-write at a runtime index to increment each element correctly (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_computed_indexed_write_exit_canary_runs() {
    // A computed value written straight into a runtime-indexed machine array element
    // (`self.arr[self.j] = self.j * 10`, no field temp). A non-zero `marker` field sits right
    // after the index field, guarding the 32-bit zero-extending index load (a 64-bit load would
    // pull `marker` into the index's high dword and store out of bounds). Fills [0..40] -> sum
    // 100 and marker survives -> exit 70.
    let canary = pass_canary("collections/runtime_computed_indexed_write_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-computed-indexed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("computed indexed-write canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("computed indexed-write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("computed indexed-write canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a computed value stored straight into a runtime-indexed element to fill correctly and not corrupt the neighbouring field (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_const_product_index_exit_canary_runs() {
    // R0 of the dependent-types ladder: the direct row-major spelling
    // `pixels[y * 4 + x]` (two-level computed index, const multiplier) in
    // read + write + ranged-param positions -- interval product discharges
    // the bound; the depth-2 hoist lowers it by slot. Pinned the former
    // silent-ZII miscompile.
    let canary = pass_canary("collections/runtime_nested_const_product_index_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-nested-product-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested const-product index canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested const-product index canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested const-product index canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the row-major `pixels[y * 4 + x]` spelling to read/write the right element in every leg (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_hoisted_index_write_exit_canary_runs() {
    // A runtime value written through a hoisted computed index rides the
    // value local's frame slot + the storage-to-indexed copy (the
    // value-side slot carve-out in omega-state-storage).
    let canary = pass_canary("collections/runtime_hoisted_index_write_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-hoisted-write-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("hoisted-index write canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("hoisted-index write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("hoisted-index write canary should run");
    assert_eq!(
        output.status.code(),
        Some(7),
        "expected the runtime value to land through the hoisted index (exit 7), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_let_mut_reassign_exit_canary_runs() {
    // `let mut` reassignment reads the NEW value (slot-backed, never folded).
    let canary = pass_canary("calls/runtime_let_mut_reassign_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-let-mut-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("let-mut canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("let-mut canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("let-mut canary should run");
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected the reassigned mut local to read 2 (exit 2), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_tuple_matrix_exhaustive_exit_canary_runs() {
    // ch4 tuple-subject transitions: covering bool matrices dispatch with
    // no `_ ->` arm.
    let canary = pass_canary("control_flow/runtime_tuple_matrix_exhaustive_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-tuple-matrix-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("tuple-transition canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("tuple-transition canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("tuple-transition canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the (true, _) arm to dispatch (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_sum_tuple_matrix_exhaustive_exit_canary_runs() {
    // Multi-subject case patterns are proved over the Cartesian product; a
    // pure case-union domain contributes its finite subset to one axis.
    let canary = pass_canary("control_flow/runtime_sum_tuple_matrix_exhaustive_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-sum-tuple-matrix-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "sum-tuple transition canary should compile from its authored root without a `_` arm",
    );
    let executable = compilation
        .checked_native_executable_path()
        .expect("sum-tuple transition canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("sum-tuple transition canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the (Horizontal, _) arm for its second member (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_tuple_case_destructure_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_tuple_case_destructure_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-tuple-destructure-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("tuple case-destructure canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("tuple case-destructure canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("tuple case-destructure canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both bound payloads to reach sum (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dependent_param_range_exit_canary_runs() {
    // R1a: a state parameter ranged by a self FIELD (`i: u32
    // [0..=self.count]`) -- caller-proved at every transition, callee index
    // proofs through the substituted store-enforced high; the exclusive
    // sugar leg rides a strict `<` guard.
    let canary = pass_canary("dependent/runtime_dependent_param_range_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-dependent-param-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dependent param-range canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("dependent param-range canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dependent param-range canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the dependent-ranged parameter to prove at the caller and index at the callee (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dependent_product_index_exit_canary_runs() {
    // R0 x R1a composition: dependent-ranged params feed the row-major
    // product index with runtime arguments; both the overflow proof and the
    // hoist's temp range read substituted intervals.
    let canary = pass_canary("dependent/runtime_dependent_product_index_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-dependent-product-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dependent product-index canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("dependent product-index canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dependent product-index canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the dependent product index to read the right element (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dependent_subtract_exit_canary_runs() {
    // The relational subtraction rule: `self.count - i` proves non-negative
    // in exact u32 from i's dependent atom (capacity-minus-used).
    let canary = pass_canary("dependent/runtime_dependent_subtract_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-dependent-subtract-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dependent subtract canary should compile from its authored root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("dependent subtract canary should run");
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected `self.count - i` to prove and compute 8 - 6 (exit 2), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dependent_ordering_chain_exit_canary_runs() {
    // The minted in-callee ordering (`k <= self.count`) chains with a
    // dominating `count < 5` guard to discharge an index the substituted
    // range alone cannot.
    let canary = pass_canary("dependent/runtime_dependent_ordering_chain_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-dependent-ordering-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dependent ordering-chain canary should compile from its authored root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("dependent ordering-chain canary should run");
    assert_eq!(
        output.status.code(),
        Some(7),
        "expected the ordering chain to discharge the index (exit 7), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_requires_subtract_exit_canary_runs() {
    // Channel (b): a machine-level `requires self.a <= self.b` proves
    // `self.b - self.a` in exact u32 (machine-wide field preservation).
    let canary = pass_canary("dependent/runtime_requires_subtract_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-requires-subtract-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("requires-subtract canary should compile from its authored root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("requires-subtract canary should run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "expected the requires-ordered subtraction to prove and compute 0 (exit 0), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_requires_guarded_call_exit_canary_runs() {
    // The requires loop closed: a dominating arm guard proves the callee's
    // requires at the call site; the requires proves the subtraction inside.
    let canary = pass_canary("dependent/runtime_requires_guarded_call_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-requires-guarded-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("requires guarded-call canary should compile from its authored root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("requires guarded-call canary should run");
    assert_eq!(
        output.status.code(),
        Some(6),
        "expected the guarded requires call to prove and compute 9 - 3 (exit 6), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_sibling_len_index_exit_canary_runs() {
    // Buffer::get: `index: u64 [0..items.len]` -- caller-guarded, callee
    // indexes guard-free through the minted prove_index fact.
    let canary = pass_canary("dependent/runtime_sibling_len_index_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-sibling-len-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("sibling-length canary should compile from its authored root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("sibling-length canary should run");
    assert_eq!(
        output.status.code(),
        Some(7),
        "expected the sibling-length index to read the seeded element (exit 7), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_bounded_product_index_exit_canary_runs() {
    // R3: runtime dims coupled only by `requires rows * cols <= 12`; the
    // product rule store-proves the ranged temp and the index rides it.
    let canary = pass_canary("dependent/runtime_bounded_product_index_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("bounded-product canary should compile to checked trees");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None, "should interpret cleanly");
    assert_eq!(
        interpreted.exit_code, 7,
        "interpreter must preserve the bounded-product index result"
    );
    let build_dir =
        std::env::temp_dir().join(format!("omega-bounded-product-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("bounded-product canary should compile from its authored root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("bounded-product canary should run");
    assert_eq!(
        output.status.code(),
        Some(7),
        "expected the coupled product walk to read the seeded element (exit 7), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_depend_mapping_exit_canary_runs() {
    // M2 blocker 3: build.omg depend rows map aliases for use-resolution.
    let canary = pass_canary("build/runtime_depend_mapping_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-depend-map-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("depend-mapping canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("depend-mapping canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the aliased use to reach the depended const (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_core_roster_ops_exit_canary_runs() {
    // N4 roster slice: core add/mul (cross-machine composition) + generic
    // recursive Seq<T> + a program-side structural length lemma.
    let canary = pass_canary("proofs/runtime_core_roster_ops_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-core-roster-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("core roster ops canary should compile from its authored root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("core roster ops canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the roster machines to validate and the program to run (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}
