use super::fixture_roster;
use crate::{
    Command, compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host,
    executable_name, fs, interpret, pass_canary,
};
use compiler::CheckedCompileRequest;

#[test]
fn mutual_cycle_tail_admitted_canary_runs() {
    // MR4 admission: a measured all-tail mutual pair with proven per-edge
    // decrease compiles and runs on CONSTANT stack -- every cross-machine
    // tail arm target lowers as a SetDispatchState jump in the one dispatch
    // loop. n = 100000 keeps the interpreter oracle inside its 10M-step
    // budget; the native probe ran 40M alternations on constant stack.
    let canary = pass_canary(fixture_roster::MUTUAL_CYCLE_TAIL_ADMITTED_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("admitted mutual tail cycle should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should run the admitted cycle to 0 (exit 70), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-mutual-tail-admitted-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("admitted mutual tail cycle should compile natively");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("admitted mutual tail cycle should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the admitted cycle to run on constant stack (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn const_fold_unsigned_landed_ops_canary_runs() {
    // CM2 first rung: the state-values folder folds at the LANDED type (the
    // destination landing from the let's declared type). `0u32 - 2` wraps to
    // width (4294967294, not i64 -2) and the sign-sensitive ops fold unsigned:
    // `b >> 1` logical = 2147483647, `b / 3` = 1431655764, `b % 3` = 2; their
    // sum is guard-checked in the SAME state (the transition-arg delivery face
    // is a separate open bug, pinned pending). exit 71 = a fold regressed to
    // the bare-i64 window.
    let canary = pass_canary(fixture_roster::CONST_FOLD_UNSIGNED_LANDED_OPS_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("landed-ops const-fold canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (u32 wrap + logical shift/unsigned div), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-const-fold-landed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("landed-ops const-fold canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("landed-ops const-fold canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("landed-ops const-fold canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected landed-type const folds (exit 70), got {:?} \
         (71 = a sign-sensitive fold ran in the bare-i64 window)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn const_fold_unsigned_shift_right_arg_canary_runs() {
    // The const-fold sign class END TO END through the transition-ARG delivery
    // path: the arg re-derives from substituted locals as a typeless nested
    // runtime binary, and the write's signedness falls back to the WRITE
    // TARGET's primitive (the callee param's u32) so `>>` emits logical shr.
    // exit 71 = the target-primitive fallback (or the landed fold) regressed.
    let canary = pass_canary(fixture_roster::CONST_FOLD_UNSIGNED_SHIFT_RIGHT_ARG_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("shift-right arg-delivery canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (u32 logical shift through the arg), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-const-fold-shr-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("shift-right arg-delivery canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("shift-right arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("shift-right arg-delivery canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the arg-delivered logical shift (exit 70), got {:?} \
         (71 = the typeless arg write emitted `sar`)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn const_fold_unsigned_divide_arg_canary_runs() {
    // Unsigned DIVISION and MODULO through the transition-arg delivery path --
    // the other two sign-sensitive ops of the same class; both values ride
    // chained transition args, so both typeless writes must take the
    // target-primitive signedness fallback. exit 71 = a signed idiv slipped in.
    let canary = pass_canary(fixture_roster::CONST_FOLD_UNSIGNED_DIVIDE_ARG_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("divide/mod arg-delivery canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (u32 unsigned div + mod through args), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-const-fold-div-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("divide/mod arg-delivery canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("divide/mod arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("divide/mod arg-delivery canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected unsigned div/mod through the args (exit 70), got {:?} \
         (71 = a signed division/modulo fold or delivery)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn unsigned_min_max_wrapping_local_canary_runs() {
    // max/min on a `u64 in Wrapping` LOCAL: `z - 1` folds to the bit-faithful
    // but signless `-1` (u64-at-width-64 can't ride a positive i64), so the
    // operand probe finds no type and the non-table mutation write's operator
    // adjustment falls back to the WRITE TARGET's u64 -> MaxUnsigned picks
    // u64::MAX. exit 78 = a signed Max compared -1 < 5 again.
    let canary = pass_canary(fixture_roster::UNSIGNED_MIN_MAX_WRAPPING_LOCAL_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("unsigned min/max local canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter oracle should exit 77 (unsigned max witness), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-minmax-local-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("unsigned min/max local canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("unsigned min/max local canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("unsigned min/max local canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected the target-fallback unsigned max (exit 77), got {:?} \
         (78 = a signed Max on the folded u64 local)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn unsigned_min_max_operand_position_canary_runs() {
    // The OPERAND-POSITION twin (carrier CR3 acceptance, promoted
    // 2026-07-18): `max(big, 5) + 0` has no write target for the max, so the
    // signedness must come from the CONSTANT itself -- the binding-capture
    // stamp + the operand-derived anonymous-destination fold carry big's
    // u64/Wrapping landing to the probe. exit 78 = a signed Max compared
    // -1 < 5 again.
    let canary = pass_canary(fixture_roster::UNSIGNED_MIN_MAX_OPERAND_POSITION_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("operand-position min/max canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter oracle should exit 77 (unsigned max witness), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-minmax-operand-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("operand-position min/max canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("operand-position min/max canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("operand-position min/max canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected the landed-constant unsigned max (exit 77), got {:?} \
         (78 = a signed Max on the stamped u64 constant)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn suffix_boundary_magnitudes_canary_runs() {
    // CR4 magnitude fit -- the BOUNDARY pins: -128i8 / 127i8 / 255u8 /
    // i64::MIN all fit their suffixes (the parse-time negation fold makes
    // `-128i8` one literal valued -128) and must stay legal while 200i8 and
    // -1u8 are loud errors (the fail twins).
    let canary = pass_canary(fixture_roster::SUFFIX_BOUNDARY_MAGNITUDES_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("suffix boundary canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (all boundary magnitudes intact), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-suffix-boundary-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("suffix boundary canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("suffix boundary canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("suffix boundary canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the boundary magnitudes to arrive intact (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn float_value_call_return_canary_runs() {
    // The frame-slot value writer's FLOAT arm: a float value-call return
    // delivers instead of leaving the call-result slot ZII.
    let canary = pass_canary(fixture_roster::FLOAT_VALUE_CALL_RETURN_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("float return canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (passthru(3.5) == 3.5), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-float-vret-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float return canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("float return canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the float return to deliver (exit 70), got {:?}          (71 = the call-result slot stayed ZII)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn expansion_float_local_guard_canary_runs() {
    // The float-literal guard arm in the EXPANSION path: an inlined callee's
    // float-local guard (`d == 0.0`) lowers instead of refusing.
    let canary = pass_canary(fixture_roster::EXPANSION_FLOAT_LOCAL_GUARD_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("expansion float guard canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (finite literal routes true), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-exp-float-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("expansion float guard canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("expansion float guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the inlined float-local guard to lower and route true (exit 70), got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn float_value_call_runtime_arg_canary_runs() {
    // The arm-guard failure distance: an inlined callee's failed guard
    // (`d == 0.0` with d = inf - inf = NaN) lands on its SIBLING no-arm,
    // not the caller's failure trailer -- is_zeroish(inf) returns false.
    let canary = pass_canary(fixture_roster::FLOAT_VALUE_CALL_RUNTIME_ARG_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("float runtime-arg canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (is_zeroish(inf) == false), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-float-varg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("float runtime-arg canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float runtime-arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the failed arm guard to route to the no-arm (exit 70), got {:?} (71 = the failure branch sailed past the sibling arm into the state trailer)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn f32_chain_per_op_rounding_canary_runs() {
    // F2c: an f32 arithmetic chain rounds per OP at f32 -- the nested
    // binary operand plans the 4-byte op from the literal's LANDED format
    // instead of defaulting float literals to F64 (which computed `addsd`
    // over f32 bit patterns and diverged from the interpreter).
    let canary = pass_canary(fixture_roster::F32_CHAIN_PER_OP_ROUNDING_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("f32 chain canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (per-op f32 rounding), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-f32-chain-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("f32 chain canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("f32 chain canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected per-op f32 rounding natively (exit 70), got {:?} (71 = a double-width intermediate crept into the f32 chain)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_std_is_finite_canary_runs() {
    // std is_finite three-leg: finite -> true, runtime inf -> false,
    // runtime NaN -> false; the false legs ride the inlined arm-guard
    // failure branch (the no() arm).
    let canary = pass_canary(fixture_roster::RUNTIME_STD_IS_FINITE_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("is_finite canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (all three is_finite legs), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-std-isfin-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("is_finite canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("is_finite canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected all three is_finite legs to hold (exit 70), got {:?} (72 = finite leg, 73 = inf leg, 74 = NaN leg)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn bool_value_call_return_canary_runs() {
    // Bool-returning value calls deliver in all three faces (attached
    // let-bound, free let-bound, direct-in-guard) -- the 07-08-era
    // mis-delivery no longer reproduces; this pins the closed class.
    let canary = pass_canary(fixture_roster::BOOL_VALUE_CALL_RETURN_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("bool value-call canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (all three bool faces), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-bool-vcall-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bool value-call canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("bool value-call canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bool value-call canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected all three bool value-call faces to deliver (exit 70), got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn struct_literal_transition_arg_canary_runs() {
    // The record arm of struct-literal ARG materialization (was missing:
    // a plain record arg planned nothing and the callee param stayed ZII).
    // Both field shapes: constant (int fast path) + runtime (general writer).
    let canary = pass_canary(fixture_roster::STRUCT_LITERAL_TRANSITION_ARG_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("struct-literal arg canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (13 + 6 across both legs), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-struct-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("struct-literal arg canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("struct-literal arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("struct-literal arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both struct-literal arg legs to deliver (exit 70), got {:?}          (71 = a leg's fields arrived ZII)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_indexed_element_copy_write_canary_runs() {
    // Place 1c-ii: the runtime-indexed whole-element slice write
    // (`exits[index] = e`, runtime index + runtime struct source) -- the
    // write face x86_64 refused with the zero-width blocker until the
    // materializer's shared-base indexed-target shape provided it.
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_ELEMENT_COPY_WRITE_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("indexed element write canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (element 1 = {{9,4}}), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-idx-elem-write-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("indexed element write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed element write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed element write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the runtime-indexed element write to land (exit 70), got {:?}          (71 = the element missed or the source never materialized)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn suffix_landed_operand_position_canary_runs() {
    // CR4a: a width-suffixed literal is BORN LANDED, so `z - 1u64` folds at
    // the suffix's u64 landing and the operand-position max compares
    // UNSIGNED. exit 78 = the suffix was stripped and a signed max picked 5.
    let canary = pass_canary(fixture_roster::SUFFIX_LANDED_OPERAND_POSITION_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("suffix-landed canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter oracle should exit 77 (unsigned max witness), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-suffix-landed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("suffix-landed canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("suffix-landed canary should retain its executable receipt");

    let output = Command::new(executable)
        .output()
        .expect("suffix-landed canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected the suffix-landed unsigned max (exit 77), got {:?} \
         (78 = the suffix stripped, signed Max)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn suffix_f32_single_rounding_canary_runs() {
    // F2a: the double-rounding witness -- an f32-suffixed literal parses
    // ONCE, correctly, to f32 (8388609.0); the old f64 route rounds twice
    // (8388610.0). Both engines key the landed read identically.
    let canary = pass_canary(fixture_roster::SUFFIX_F32_SINGLE_ROUNDING_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("f32 single-rounding canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter oracle should exit 77 (single-rounded f32 witness), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-f32-rounding-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("f32 single-rounding canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("f32 single-rounding canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected the single-rounded f32 witness (exit 77), got {:?} \
         (78 = the retired f64 double-rounding route)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn unsuffixed_f32_destination_single_rounding_canary_runs() {
    // F2b: the double-rounding witness UNSUFFIXED -- the destination's
    // declared f32 lands the format on the literal's text carrier
    // (land_float_literal_destinations, pre-fork on the typed tree), so all
    // three stamped faces (let local, field assignment, struct-literal
    // field) parse ONCE to f32 in both engines.
    let canary = pass_canary(fixture_roster::UNSUFFIXED_F32_DESTINATION_SINGLE_ROUNDING_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("unsuffixed f32 destination canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter oracle should exit 77 (all three faces single-rounded), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-f32-dest-rounding-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("unsuffixed f32 destination canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("unsuffixed f32 destination canary should run");

    assert_eq!(
        output.status.code(),
        Some(77),
        "expected all three destination faces single-rounded (exit 77), got {:?} \
         (78 = let face, 79 = field-assignment face, 80 = struct-field face \
         still on the f64-then-narrow route)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn unsuffixed_f32_argument_single_rounding_canary_runs() {
    // F2c: typed call/transition parameter and return destinations stamp the
    // same f32 format before the native/interpreter fork.
    let canary = pass_canary(fixture_roster::UNSUFFIXED_F32_ARGUMENT_SINGLE_ROUNDING_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("unsuffixed f32 argument canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter should single-round call, return, and transition faces"
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-f32-arg-rounding-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("unsuffixed f32 argument canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("unsuffixed f32 argument canary should run");
    assert_eq!(
        output.status.code(),
        Some(77),
        "expected call, return, and transition faces single-rounded (exit 77), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn f32_per_operation_rounding_canary_runs() {
    // F2d: the constant guard folder, substituted-local native operands, and
    // interpreter runtime evaluator all round after each binary32 operation.
    // At 2^24, two sequential +1 legs plateau; an f64-window fold followed by
    // one narrowing reaches 2^24+2.
    let canary = pass_canary(fixture_roster::F32_PER_OPERATION_ROUNDING_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("f32 per-operation rounding canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter should round every f32 arithmetic node"
    );

    let build_dir = std::env::temp_dir().join(format!("omega-f32-per-op-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("f32 per-operation rounding canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("f32 per-operation rounding canary should run");
    assert_eq!(
        output.status.code(),
        Some(77),
        "expected constant and runtime f32 chains to plateau (exit 77), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn anonymous_exact_rat_const_canary_runs() {
    // Anonymous integer/decimal trees evaluate as exact rationals, then round
    // once at a destination or guard-context format. The special-value leg
    // uses an explicitly typed operand to select IEEE division on both engines.
    let canary = pass_canary(fixture_roster::ANONYMOUS_EXACT_RAT_CONST_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("anonymous exact-Rat canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter should consume exact folds"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-float-exact-rat-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("anonymous exact-Rat canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("anonymous exact-Rat canary should run");
    assert_eq!(
        output.status.code(),
        Some(77),
        "expected all exact-Rat destination/guard/special faces (exit 77), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn finite_core_domain_range_discharge_canary_runs() {
    // F3a: the built-in value domain crosses the full type pipeline, and a
    // declared float range proves Finite when passed into a constrained state.
    let canary = pass_canary(fixture_roster::FINITE_CORE_DOMAIN_RANGE_DISCHARGE);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("Finite core-domain canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 77,
        "interpreter should preserve ranged f32 value"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-finite-domain-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("Finite core-domain canary should compile natively");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("Finite core-domain canary should run");
    assert_eq!(
        output.status.code(),
        Some(77),
        "expected range-to-Finite discharge and value preservation (exit 77), got {:?}",
        output.status.code()
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn struct_literal_field_coercion_canary_runs() {
    // A struct-literal field init coerces the field value to the field's declared
    // width/domain (interpreter eval_struct_literal): `Point { x: a+b }` with
    // `a+b`=300 into a u8 field reads 44. The field is read DIRECTLY (`p.x`), so
    // the coercion must happen at construction. exit 71 = field carried raw 300.
    let canary = pass_canary(fixture_roster::STRUCT_LITERAL_FIELD_COERCION);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("struct-literal coercion canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (struct field truncates to u8 width), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-struct-lit-coerce-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("struct-literal coercion canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("struct-literal coercion canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("struct-literal coercion canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected struct field init to truncate to u8 width (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn array_element_write_width_domain_canary_runs() {
    // An array-element write coerces the stored value to the element WIDTH and the
    // ARRAY's arithmetic DOMAIN (interpreter assignment_target_coercion): a u8
    // element given `a+b`=300 truncates to 44; a `[u8;N] in Saturating` element
    // clamps to 255. exit 72 = wrap element wrong; 73 = saturating did not clamp.
    let canary = pass_canary(fixture_roster::ARRAY_ELEMENT_WRITE_WIDTH_DOMAIN);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("array-element coercion canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (element width truncation + array Saturating clamp), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-array-elem-coerce-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("array-element coercion canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("array-element coercion canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("array-element coercion canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected element width truncation + array Saturating clamp (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn int_transition_arg_width_wrap_canary_runs() {
    // An integer argument is wrapped to the param's declared width at the binding
    // (interpreter bind_frame apply_arithmetic_domain), matching native's
    // truncating store at the call boundary: `a+b`=300 into a u8 param reads 44.
    // exit 71 = the interpreter carried the un-wrapped 300 into the u8 param.
    let canary = pass_canary(fixture_roster::INT_TRANSITION_ARG_WIDTH_WRAP);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("int transition-arg width canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (int arg wraps to u8 param width), got {}",
        outcome.exit_code
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-int-transition-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("int transition-arg width canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("int transition-arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("int transition-arg width canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected int arg to wrap to the u8 param width (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn f32_transition_arg_rounding_canary_runs() {
    // An f32 passed through a transition ARGUMENT rounds to f32 at the param
    // binding (interpreter bind_frame), not just at stores: accumulating via
    // inline `+ 1.0` args past 2^24 plateaus at 16777216, matching native.
    // exit 71 = the interpreter carried f64 through params to 16777218.
    let canary = pass_canary(fixture_roster::F32_TRANSITION_ARG_ROUNDING);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("f32 transition-arg canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (f32 rounds at param binding), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-f32-transition-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("f32 transition-arg canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("f32 transition-arg canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32 transition-arg to round at param binding (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn f32_field_store_rounding_canary_runs() {
    // An f32 field/local rounds each stored result to f32 (interpreter store
    // rounding, matching native SSE): stepping past 2^24 by `+ 1.0` plateaus at
    // 16777216. Regression lock for the interpreter Value::Float store-rounding
    // fix; exit 71 = the interpreter kept f64 and over-accumulated to 16777218.
    let canary = pass_canary(fixture_roster::F32_FIELD_STORE_ROUNDING);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("f32 field store canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (f32 store rounds, plateaus at 16777216), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-f32-field-store-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("f32 field store canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("f32 field store canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32 field store to round to f32 and plateau (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn const_fold_cast_signedness_canary_runs() {
    // Const-folded integer casts stay correct across truncation + sign
    // reinterpret, including a wrapping-produced high-bit value cast to a signed
    // type (`(0u32-1) as i8 == -1`). Positive contrast to the const-fold
    // arithmetic miscompile class: a cast node carries its target type, so
    // folding never drops width/signedness. Guards against a future arithmetic-
    // folder fix breaking cast folding. exit 71 = a fold dropped width/sign.
    let canary = pass_canary(fixture_roster::CONST_FOLD_CAST_SIGNEDNESS);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("const-fold cast canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for const-folded casts, got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-const-fold-cast-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("const-fold cast canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("const-fold cast canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("const-fold cast canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected const-folded casts (truncate + sign reinterpret) to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}
