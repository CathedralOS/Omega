use super::fixture_roster;
use crate::{Command, compile_rooted_canary_for_native_host, executable_name, fs, pass_canary};

#[test]
fn runtime_call_result_binary_operand_exit_canary_runs() {
    // A state-call result used as an operand of a larger value (`x = f() + 1`,
    // `x = max(y, f()+1)`) must apply the operator, not collapse to just the call's
    // result. The dispatch-body mutation path had a statement-level "copy call result
    // to target" shortcut that fired even when the call was a sub-expression, dropping
    // the `+1`/`max`. It now fires only for a bare, non-builtin call value.
    let canary = pass_canary(fixture_roster::RUNTIME_CALL_RESULT_BINARY_OPERAND_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-call-result-binary-operand-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("call-result-binary-operand canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("call-result-binary-operand canary should retain its executable receipt");
    let output = Command::new(executable)
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

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_cast_operand_exit_canary_runs() {
    // A numeric `as` cast used as a binary operand (`self.a + (self.b as f64)`) must
    // convert the source in place via a Convert value operand, not be dropped. Covers
    // int->float, float->int, and integer widening; exits 70 only when all convert.
    let canary = pass_canary(fixture_roster::RUNTIME_CAST_OPERAND_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-cast-operand-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("cast-operand canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("cast-operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `a + (b as T)` casts in operand position to convert correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_f32_arithmetic_exit_canary_runs() {
    // Single-precision f32 store/copy/compare and field arithmetic must use the
    // single-precision SSE forms (movd + ucomiss/addss/...) keyed on byte_size 4,
    // with is_float recognizing F32 -- previously f32 was compared/operated as an
    // integer or as double precision. Exits 70 only when every f32 op is correct.
    let canary = pass_canary(fixture_roster::RUNTIME_F32_ARITHMETIC_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-f32-arith-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("f32 arithmetic canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("f32 arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32 store/copy/compare + add/sub/mul/div to evaluate correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
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
    let canary = pass_canary(fixture_roster::RUNTIME_F32_LOCAL_ARITHMETIC_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-f32-local-arith-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
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
    let canary = pass_canary(fixture_roster::RUNTIME_MULTI_ARM_VALUE_TRANSITION_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-multi-arm-value-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("multi-arm value transition canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("multi-arm value transition canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("multi-arm value transition canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a 3-arm value transition to select first/middle/default correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
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
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_TRANSITION_UNSIGNED_GUARD_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-value-transition-unsigned-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("unsigned value-transition guard canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("unsigned value-transition guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("unsigned value-transition guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a u32 value-transition guard to branch unsigned (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_const_array_length_exit_canary_runs() {
    // COMPTIME STAGE 1: `slots: [i64; table_size()]` sizes a data field by an
    // build-time-admissible machine call, evaluated by the reference interpreter
    // before checking/layout (the callee computes 12 + 4, pinning evaluation,
    // not literal forwarding). Indexing slots[15] only type-checks if the
    // substituted Literal(16) reached the range checker, and the values only
    // read back if layout sized the field as 16 elements -- identically to a
    // written `[i64; 16]`. Exits 70 only when both ends hold their values.
    let canary = pass_canary(fixture_roster::RUNTIME_CONST_ARRAY_LENGTH_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-const-array-length-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("const array length canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("const array length canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("const array length canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `[i64; table_size()]` to const-evaluate to 16 and behave exactly like a literal length (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
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
    let canary = pass_canary(fixture_roster::RUNTIME_FIXED_VEC_ROUND_TRIP_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-fixed-vec-round-trip-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("fixed vec round trip canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("fixed vec round trip canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("fixed vec round trip canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the proof-discharged push/pop/get round trip to hold its values (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_vec_u32_growth_exit_canary_runs() {
    // ALLOCATOR STORY STAGE 2: `alloc::vec`'s `Vec<u32>` over caller-supplied
    // backing pushes ten elements through two growths (4 -> 8 -> 16), reads
    // the preserved prefix and the new elements back, exercises empty and
    // duplicate cleanup, a rejected growth preserving contents, and the
    // explicit drain-and-return of the retired backing. The witness keeps
    // `main` single-state (the ordinary Unit admission route — composed
    // states admit no structural call targets yet) and folds verification
    // into a bit-weighted exit code; exits 70 only when every value and
    // every custody marker matches.
    let canary = pass_canary(fixture_roster::RUNTIME_VEC_U32_GROWTH_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-vec-u32-growth-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("vec u32 growth canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("vec u32 growth canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("vec u32 growth canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the two-growth push/read/cleanup round trip to hold (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_float_negative_ops_exit_canary_runs() {
    // Float operations with negatives -- comparisons (the ucomisd unsigned-flags case),
    // a negative float->int cast (truncation toward zero), and a negative multiply.
    // Exits 70.
    let canary = pass_canary(fixture_roster::RUNTIME_FLOAT_NEGATIVE_OPS_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-float-negative-ops-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float negative ops canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("float negative ops canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected float compares/cast/multiply with negatives to be correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_float32_array_conversion_exit_canary_runs() {
    // f32 ARRAY (the separate f32 codegen path: mulss/addss/ucomiss) plus an int<->f64 round-trip
    // cast. f32 sum 1.5+2.5+2.25 = 6.25, then 7 as f64 * 2.0 truncated to i32 = 14. Exits 70.
    let canary = pass_canary(fixture_roster::RUNTIME_FLOAT32_ARRAY_CONVERSION_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-float32-array-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("f32 array + conversion canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("f32 array + conversion canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32-array sum 6.25 + int<->f64 round-trip == 14 (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_value_call_let_combine_exit_canary_runs() {
    // The SAFE side of the shared-result-slot boundary: a computing value-machine called twice in
    // one state whose results are bound to locals and combined in one expression is materialized
    // eagerly and correct (dbl(3)=6, dbl(4)=8 -> 6*100+8 == 608 -> exit 70). Guards against
    // re-broadening the shared-value-call-slot fence into a false positive on this valid pattern.
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_LET_COMBINE_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-vc-letcombine-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("value-call let-combine canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("value-call let-combine canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("value-call let-combine canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected two value-calls bound to locals and combined in one expression to keep distinct \
         results (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_float_nan_comparison_exit_canary_runs() {
    // NaN comparison honors IEEE in native codegen: `!=` is true, the other five operators
    // are false. Guards on `ucomis*` must test the parity flag (a `jp` branch) or 4 of 6 take
    // the wrong arm. The canary checks all six against a NaN operand and exits 70 iff correct.
    let canary = pass_canary(fixture_roster::RUNTIME_FLOAT_NAN_COMPARISON_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-float-nan-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float NaN comparison canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("float NaN comparison canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected NaN comparisons to follow IEEE (only != true; exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_saturating_domain_exit_canary_runs() {
    // Saturating arithmetic clamps at the type bounds (the core arithmetic-safety domain): u8
    // add/sub/mul over- and under-flow -> 255/0/255, i8 signed positive overflow -> 127, i8 signed
    // negative underflow -> -128 (checked as +128 == 0). All self-checked -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_SATURATING_DOMAIN_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-saturating-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("saturating domain canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("saturating domain canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("saturating domain canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Saturating arithmetic to clamp at type bounds in all four directions (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_i64_signed_arithmetic_exit_canary_runs() {
    // i64 signed arithmetic beyond i32: multiply past 2^32, signed div/mod with a negative dividend
    // (sign follows dividend), and a 64-bit shift (1<<40). All chained -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_I64_SIGNED_ARITHMETIC_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-i64-signed-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("i64 signed arithmetic canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("i64 signed arithmetic canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("i64 signed arithmetic canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i64 multiply/div/mod/shift at scale to be correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_cast_sign_zero_extension_exit_canary_runs() {
    // Width conversions pick the right extension (movsx vs movzx): -1 as i8 as i32 == -1
    // (sign-extend), -1 as u8 as i32 == 255 (zero-extend), 200 as i8 as i32 == -56 (truncate +
    // sign-extend). All chained -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_CAST_SIGN_ZERO_EXTENSION_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-cast-sign-zero-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("cast sign/zero extension canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("cast sign/zero extension canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("cast sign/zero extension canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected sign/zero extension + truncation casts correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_bitwise_high_ops_exit_canary_runs() {
    // Bitwise ops on u32 above i32::MAX: a=0xF0F0F0F0, b=0x0F0F0F0F. XOR/AND/OR, a shift+mask
    // nibble extract, and NOT-via-XOR all chained -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_BITWISE_HIGH_OPS_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-bitwise-high-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bitwise high ops canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("bitwise high ops canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bitwise high ops canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected u32 bitwise ops at high values to be correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_unsigned_high_comparison_exit_canary_runs() {
    // Unsigned comparison above i32::MAX (a = u32::MAX, b = 1): a signed setcc would invert every
    // ordered result. All six operators chained give the unsigned answer -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_UNSIGNED_HIGH_COMPARISON_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-unsigned-high-cmp-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("unsigned high comparison canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("unsigned high comparison canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("unsigned high comparison canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected unsigned comparisons of u32::MAX vs 1 to be correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_signed_modulo_shift_edges_exit_canary_runs() {
    // Sign-sensitive integer codegen: truncated signed modulo with negatives (-7%3==-1, 7%-3==1),
    // arithmetic vs logical right shift (-16>>2==-4 SAR, 16u32>>2==4 SHR), and a runtime shift
    // amount (1<<5==32). Exits 70.
    let canary = pass_canary(fixture_roster::RUNTIME_SIGNED_MODULO_SHIFT_EDGES_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-signed-mod-shift-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("signed modulo/shift edges canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("signed modulo/shift canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("signed modulo/shift edges canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed modulo + arithmetic/logical/runtime shifts correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}
