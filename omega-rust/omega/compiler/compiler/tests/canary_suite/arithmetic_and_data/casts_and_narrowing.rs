use super::fixture_roster;
use crate::{
    Command, compile_native_canary_without_output, compile_rooted_canary_for_native_host,
    executable_name, fs, pass_canary, unique_no_output_build_dir,
};

#[test]
fn runtime_integer_casts_exit_canary_runs() {
    // Integer width/sign casts (sign-extend / zero-extend / truncate / reinterpret),
    // with each cast result threaded through a transition PARAM. This last part also
    // guards the fix for the dispatch-arg fold missing Cast/Binary arms: a let-local
    // whose initializer is a cast (or binary) reading a prior local was re-materialized
    // in the target state -- where the source local has no slot -- and read 0.
    let canary = pass_canary(fixture_roster::RUNTIME_INTEGER_CASTS_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-integer-casts-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("integer-casts canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("integer-casts canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("integer-casts canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected integer width/sign casts threaded through params to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_i64_divide_modulo_exit_canary_runs() {
    // i64 signed divide/modulo with both operands immediate (constant/constant): the
    // byte-size resolver must fall back to the i64 target width, not 4, or the encoder
    // emits a 32-bit idiv (width mismatch + a truncated 64-bit dividend).
    let canary = pass_canary(fixture_roster::RUNTIME_I64_DIVIDE_MODULO_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-i64-divmod-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("i64 divide/modulo canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("i64 divide/modulo canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("i64 divide/modulo canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i64 constant divide/modulo to run 64-bit and self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_float_compare_cast_exit_canary_runs() {
    // Float breadth: comparisons with negatives (the ucomisd unsigned-flag case),
    // f64/f32 arithmetic, int<->float and f32<->f64 casts, and nested-field float
    // arithmetic (a dot product). Self-checks to exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_FLOAT_COMPARE_CAST_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-float-breadth-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float compare/cast canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("float compare/cast canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("float compare/cast canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected float comparisons/arith/casts/nested-field to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_float_operations_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_FLOAT_OPERATIONS_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-float-ops-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float-arithmetic canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("float-arithmetic canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("float-arithmetic canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f64/f32 arithmetic, casts, local & nested-field float arith to be correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

/// ch15 stage 2 -- multi-path return-range inference: a callee returning via two
/// transition arms (3 / 7) infers the UNION [3,7], so the caller's `pick(b) + 63`
/// proves Exact. run(false) -> 70.
#[test]
fn runtime_inferred_multipath_return_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_INFERRED_MULTIPATH_RETURN_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-inferred-multipath-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("multi-path inferred return range should let the caller prove Exact");
    let executable = compilation
        .checked_native_executable_path()
        .expect("multipath return canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("multi-path inferred return canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected multi-path inferred-return narrowing to run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

/// ch15 stage 2 (modular return-range inference): a callee with NO declared
/// return range whose body bounds the result (`min(x, 3)`) lets the caller's
/// `classify(x) + 67` prove Exact via the INFERRED bound. run(100) -> 70.
#[test]
fn runtime_inferred_return_range_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_INFERRED_RETURN_RANGE_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-inferred-return-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("inferred return range should let the caller's arithmetic prove Exact");
    let executable = compilation
        .checked_native_executable_path()
        .expect("inferred return-range canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("inferred return range canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected inferred-return-range narrowing to run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

/// #62: constructing a range-refined field from a PROVABLE non-literal value (a
/// same-range field) is accepted, not just integer literals. copy_box -> 70.
#[test]
fn runtime_provable_field_construction_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_PROVABLE_FIELD_CONSTRUCTION_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-provable-field-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("provable non-literal field construction should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("provable field construction canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("provable field construction canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected provable non-literal field construction to run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

/// Fact catalog over PLAIN STRUCT fields: a range-refined field `v: i32 [0..=15]`
/// of a param flows into the reader so `b.v + 65` proves Exact. Box{v:5} -> 70.
#[test]
fn runtime_struct_field_range_narrowing_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_STRUCT_FIELD_RANGE_NARROWING_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-struct-field-range-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone()).expect(
        "struct-field range narrowing should compile (constrained field discharges the obligation)",
    );
    let executable = compilation
        .checked_native_executable_path()
        .expect("struct-field range canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("struct-field range narrowing canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected constrained struct field `b.v + 65` to run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

/// Stage 1 of the fact catalog over sum cases: a case-payload field's range
/// refinement (`index: i32 [0..=15]`) flows into the destructure arm so
/// `index + 65` proves Exact. Sound because construction enforces the range.
/// Found{index:5} -> 70.
#[test]
fn runtime_payload_range_narrowing_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_PAYLOAD_RANGE_NARROWING_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-payload-range-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone()).expect(
        "payload range narrowing should compile (constrained payload discharges the obligation)",
    );
    let executable = compilation
        .checked_native_executable_path()
        .expect("payload range canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("payload range narrowing canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected constrained payload `index + 65` to run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

/// Sum-payload-range narrowing for a DIRECT pass-through (2026-07-08): the
/// destructured binding `v` (`P::One { v } -> use_v(v)`) rewrites to `self.p.v`
/// and is passed WITHOUT arithmetic to a same-ranged param. The arm's
/// co-located guard proves `self.p`'s case is `One`, so the payload field's
/// declared range [0..=50] discharges the argument obligation. Complements the
/// arithmetic-use narrowing above (that path handles `index + 65`; this the bare
/// pass-through). Guard-gated -- direct access outside a case-arm stays unproven
/// (fail canaries sum_payload_direct_access_unproven / _non_case_guard_unproven).
/// P::One { v: 20 } -> use_v(20) -> 20.
#[test]
fn runtime_sum_payload_range_narrowed_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::SUM_PAYLOAD_RANGE_NARROWED_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-payload-narrow-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("direct payload pass-through should prove under the case-arm guard");
    let executable = compilation
        .checked_native_executable_path()
        .expect("direct payload range canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("sum payload range narrowed canary should run");
    assert_eq!(
        output.status.code(),
        Some(20),
        "expected direct payload `use_v(v)` under the `P::One` arm to run to 20; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

/// Sum-payload-range narrowing through ARITHMETIC in a transition arg
/// (2026-07-08): a bounded payload operand narrows the arithmetic it feeds.
/// `Cmd::Dim { amount } -> apply(amount * 10)` with `amount: [0..=10]` proves
/// `amount * 10` fits `apply`'s `target: [0..=100]` -- the payload operand's
/// range is resolved under the arm guard and folded through the binary. Extends
/// the direct pass-through; the too-wide sibling (`amount * 100`) stays unproven.
/// Cmd::Dim { amount: 7 } -> apply(70) -> 70.
#[test]
fn runtime_sum_payload_range_arith_narrowed_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::SUM_PAYLOAD_RANGE_ARITH_NARROWED_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-payload-arith-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("payload operand arithmetic should prove under the case-arm guard");
    let executable = compilation
        .checked_native_executable_path()
        .expect("payload arithmetic range canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("sum payload range arith narrowed canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `apply(amount * 10)` with amount in [0..=10] to run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

/// The `a..b` (exclusive) / `a..=b` (inclusive) range refinement syntax that
/// replaced the removed `range<a, b>`: `x in 0..16` and `y in 0..=100` keep
/// `x + y` Exact. Runs to 70.
#[test]
fn runtime_exclusive_range_constraint_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_EXCLUSIVE_RANGE_CONSTRAINT_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-exclusive-range-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("exclusive/inclusive range-constraint syntax should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("exclusive range canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("exclusive range constraint canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `0..16` + `0..=100` constrained sum to run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

/// Decision 17 S4: min/max clamp narrowing. `max(self.seed, 0)` lower-bounds at
/// 0 and `min(_, 60)` upper-bounds at 60, so `+ 70` stays EXACT. Without the
/// narrowing both clamps are unbounded and `+ 70` is a decision-17 overflow
/// error — so the program only COMPILES because the narrowing proves the bound
/// (and runs because the value-call-result materialization bug is fixed).
#[test]
fn runtime_fnv1a_hash_exit_canary_runs() {
    // FNV-1a-32 hash of [72,105,33] folded in a loop (hash = (hash ^ byte) * prime, wrapping u32),
    // checked against the independently computed reference 844955649 -> exit 70. Proves Omega's u32
    // wrapping XOR+multiply computes the correct hash of a real algorithm.
    let canary = pass_canary(fixture_roster::RUNTIME_FNV1A_HASH_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-fnv1a-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("FNV-1a hash canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("FNV-1a hash canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("FNV-1a hash canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected FNV-1a to hash to the reference value (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_min_max_clamp_narrowing_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MIN_MAX_CLAMP_NARROWING_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-min-max-clamp-narrowing-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("min/max clamp narrowing canary should compile (narrowing proves the bound)");
    let executable = compilation
        .checked_native_executable_path()
        .expect("min/max clamp canary should retain its executable receipt");
    let output = Command::new(executable)
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
    let _ = fs::remove_dir_all(&scratch);
}

/// Decision 17 S4: modulo + division result-interval narrowing. `self.seed %
/// 100` bounds the remainder ([-99,99]) and `/ 2` keeps it bounded, so `+ 70`
/// stays EXACT. Without the narrowing both `%` and `/` are unbounded and the
/// `+ 70` is a decision-17 overflow error — so this program only COMPILES
/// because the narrowing proves the bound (seed ZII 0 → exit 70).
#[test]
fn runtime_modulo_div_narrowing_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MODULO_DIV_NARROWING_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-modulo-div-narrowing-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("modulo/div narrowing canary should compile (narrowing proves the bound)");
    let executable = compilation
        .checked_native_executable_path()
        .expect("modulo/div narrowing canary should retain its executable receipt");
    let output = Command::new(executable)
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
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_trapping_mul_overflow_aborts() {
    // Decision 17: Trapping multiply overflow (100*100) aborts via ud2 -- never
    // reaches the transition. (No `_canary_runs` suffix so the differential drift
    // guard skips it.)
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_TRAPPING_MUL_OVERFLOW);
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-trap-mul-of-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_trapping_mul_overflow canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("trapping multiply overflow canary should retain its executable receipt");
    let output = Command::new(executable)
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
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_saturating_signed_exit_canary_runs() {
    // Decision 17 S1b: signed `i8 in Saturating` clamps 100+100=200 to 127.
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_SATURATING_SIGNED_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-sat-signed-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_saturating_signed canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("signed saturation canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("arithmetic_domain_saturating_signed canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i8 in Saturating (100+100) to clamp to 127 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_requires_proven_exact_exit_canary_runs() {
    // Decision 17 S4: a `requires`-bounded param (amount in [0,100]) proves
    // `amount + amount` in [0,200] -> exact (no domain). compute(35) -> 70.
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_REQUIRES_PROVEN_EXACT_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-requires-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_requires_proven_exact canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("requires-proven Exact canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("arithmetic_domain_requires_proven_exact canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected requires-bounded exact `amount + amount` (compute(35)) to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_range_proven_exact_exit_canary_runs() {
    // Decision 17 S4: range-constraint narrowing proves `x + y` (each in [0,100])
    // is in [0,200], so it stays EXACT (no domain needed). 40+30=70.
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_RANGE_PROVEN_EXACT_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-arith-domain-range-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_range_proven_exact canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("range-proven Exact canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("arithmetic_domain_range_proven_exact canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected range-bounded exact `x + y` (40+30) to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_cast_exit_canary_runs() {
    // Decision 17 S2: a domain `as` cast crosses domains -- `(a as u8 in
    // Saturating) + b` lets an exact `a` join saturating arithmetic; 200+100->255.
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_CAST_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-arith-domain-cast-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_cast canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("arithmetic-domain cast canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("arithmetic_domain_cast canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `(a as u8 in Saturating) + b` (200+100) to clamp to 255 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_trapping_exit_canary_runs() {
    // Decision 17 S1b: `u8 in Trapping` runs normally when in range (100+50=150).
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_TRAPPING_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-trapping-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_trapping canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("in-range trapping canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("arithmetic_domain_trapping canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected u8 in Trapping (100+50=150, in range) to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_trapping_overflow_aborts() {
    // Decision 17 S1b: `u8 in Trapping` ABORTS on overflow (200+100=300). The
    // native backend emits `ud2`, so the process never reaches the transition and
    // never exits 70/71 -- it terminates abnormally. (Named without the
    // `_canary_runs` suffix so the differential drift guard does not treat it as a
    // clean-exit run canary.)
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_TRAPPING_OVERFLOW);
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-trapping-of-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_trapping_overflow canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("trapping overflow canary should retain its executable receipt");
    let output = Command::new(executable)
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

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_trapping_let_overflow_aborts() {
    // Decision 17: a Trapping overflow in a `let` LOCAL const-fold ABORTS, like the
    // field path. `let b: i32 in Trapping = a + a` (a+a overflows i32) traps (ud2)
    // and never reaches exit(70). REGRESSION for the fix: the frame-slot (`let`)
    // store path used to write the folded constant RAW, silently running past the
    // overflow. (No `_canary_runs` suffix -- a trap aborts, not a clean exit, so
    // the differential drift guard must not treat it as a run canary.)
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_TRAPPING_LET_OVERFLOW);
    let scratch =
        std::env::temp_dir().join(format!("omega-trapping-let-of-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_trapping_let_overflow canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("trapping let overflow canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("arithmetic_domain_trapping_let_overflow canary should run");

    assert_ne!(
        output.status.code(),
        Some(70),
        "expected a Trapping `let` overflow (2e9 + 2e9) to trap before exit(70), but it exited 70 \
         as if no overflow occurred (frame-slot store wrote the constant raw)\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "expected a Trapping `let` overflow to terminate abnormally, but it exited successfully"
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_return_range_proven_exact_exit_canary_runs() {
    // Decision 17 S4: a range-constrained return (`-> i32 [0..=10]`) lets a
    // caller's exact arithmetic on the result stay Exact (5+5+60=70). Enforcement
    // (callee must return in range) makes trusting the range sound.
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_RETURN_RANGE_PROVEN_EXACT_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-return-range-exact-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("return-range proven-exact canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("return-range Exact canary should retain its executable receipt");
    let output = Command::new(executable)
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
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_trapping_const_fold_overflow_aborts() {
    // Decision 17 / task #39: a Trapping op with CONST operands that overflows
    // (u8 100*100=10000) must trap, even though the operands fold to a constant.
    // The const-store path re-emits a guaranteed-overflowing trapping op so the
    // encoder trap fires -- the process terminates abnormally (never 70/71).
    // Before the fix it silently wrapped to 16 and exited 70. Named without
    // `_canary_runs` so the differential drift guard treats it as non-clean-exit.
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_TRAPPING_CONST_FOLD_OVERFLOW);
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-trapping-const-of-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("trapping const-fold overflow canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("trapping const-fold overflow canary should retain its executable receipt");
    let output = Command::new(executable)
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

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn constant_trapping_shift_value_overflow_aborts() {
    // The landed folder must not turn `u8 in Trapping` 200 << 1 into the
    // wrapped value 144. Native lowering must retain the overflow trap.
    let canary = pass_canary(fixture_roster::CONSTANT_TRAPPING_SHIFT_VALUE_OVERFLOW_TRAPS);
    let scratch = std::env::temp_dir().join(format!(
        "omega-constant-trapping-shift-value-overflow-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("constant trapping shift-overflow canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("constant trapping shift canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("constant trapping shift-overflow canary should run");
    let code = output.status.code();
    assert_ne!(
        code,
        Some(70),
        "constant Trapping shift overflow silently wrapped instead of trapping"
    );
    assert!(
        !output.status.success(),
        "expected a crash status from the Trapping overflow, got {code:?}"
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn wrapping_overwidth_shift_reaches_native_artifact_without_panicking() {
    // The regression was a constant-folder panic, so checked trees alone do
    // not exercise it. Keep the fixture's explicitly Wrapping left operand.
    // NativeArtifact (not publish) keeps the retained artifact readable;
    // publishing consumes it into the executable receipt.
    let canary = pass_canary(fixture_roster::SHIFT_AMOUNT_OVER_WIDTH_COMPILES);
    let compilation = compile_native_canary_without_output(&canary)
        .expect("overwidth Wrapping shift should compile without a folder panic");
    compilation
        .retained_native_artifact()
        .expect("overwidth Wrapping shift should retain its native artifact")
        .validate()
        .expect("overwidth Wrapping shift artifact should replay");
}

#[test]
fn runtime_trapping_shift_count_canary_aborts() {
    // A zero left operand does not make an invalid Trapping count safe.
    let canary = pass_canary(fixture_roster::RUNTIME_TRAPPING_SHIFT_COUNT_TRAPS);
    let scratch = unique_no_output_build_dir();
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("out-of-range Trapping shift canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("Trapping shift-count canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("out-of-range Trapping shift canary should spawn");
    let code = output.status.code();
    assert!(
        code.is_none() || code.is_some_and(|code| code < 0),
        "Trapping shift count must abort, got {code:?}"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn dead_trapping_let_traps_aborts() {
    // Abort-as-effect first sentence (owner, 2026-07-18): a trap is an
    // EFFECT, so a DEAD trapping computation is not dead -- the storage layer
    // keeps a trap-carrying initializer's slot and the trap lowers. Before,
    // native DCE'd the write AND the trap and exited 7 while the interpreter
    // trapped. Named without `_canary_runs` (non-clean-exit; outside the
    // RUN-list drift guard, like the const-fold overflow twin above).
    let canary = pass_canary(fixture_roster::DEAD_TRAPPING_LET_TRAPS);
    let scratch =
        std::env::temp_dir().join(format!("omega-dead-trapping-let-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("dead trapping let canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("dead trapping let canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dead trapping let canary should run");

    assert_ne!(
        output.status.code(),
        Some(7),
        "expected the DEAD `i32 in Trapping` overflow to trap, but the program ran past it to \
         exit 7 (the trap was dead-code-eliminated)\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "expected the dead trapping computation to terminate abnormally, but it exited successfully"
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn f32_field_binary_to_local_cast_exit_canary_runs() {
    // Scalar-width-rederivation fix: a folded f32 binary (`self.a + self.b`)
    // feeding `as i32` must compute single-precision (`addss`), not the old
    // hardcoded `addsd` over f32 bits. The binary operand threads its resolved
    // 4-byte width so producer (addss) and convert consumer (cvttss2si) agree.
    let canary = pass_canary(fixture_roster::F32_FIELD_BINARY_TO_LOCAL_CAST);
    let scratch = std::env::temp_dir().join(format!(
        "omega-f32-field-binary-local-cast-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("f32_field_binary_to_local_cast canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("f32_field_binary_to_local_cast canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32 field-binary-to-local-cast to yield 4 and exit 70, got {:?} (71 = addsd over f32 bits)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn f32_to_f64_local_cast_exit_canary_runs() {
    // Nested-cast width fix: `(self.src as f64) as i32` (a cast whose source is
    // a folded cast). classify now types a Cast as its target, so the convert
    // chain (cvtss2sd -> cvttsd2si) builds instead of the write being dropped.
    let canary = pass_canary(fixture_roster::F32_TO_F64_LOCAL_CAST);
    let scratch = std::env::temp_dir().join(format!(
        "omega-f32-to-f64-local-cast-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("f32_to_f64_local_cast canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("f32_to_f64_local_cast canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32->f64-local->i32 cast chain to yield 7 and exit 70, got {:?} (71 = write dropped, n stayed 0)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn f32_deep_chain_binary_exit_canary_runs() {
    // Scalar-width-rederivation fix at depth: a left-chain f32 `a + b + c + d`
    // in a guard `s > 9.5`. Each nested binary threads its 4-byte width so
    // every level emits `addss`, not `addsd`. Depth 3 was where the old
    // re-derivation stopped agreeing.
    let canary = pass_canary(fixture_roster::F32_DEEP_CHAIN_BINARY);
    let scratch = std::env::temp_dir().join(format!(
        "omega-f32-deep-chain-binary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("f32_deep_chain_binary canary should compile");

    let output = Command::new(scratch.join("out").join(executable_name()))
        .output()
        .expect("f32_deep_chain_binary canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32 depth-3 chain to sum to 10.0 (> 9.5) and exit 70, got {:?} (71 = wrong XMM result)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn no_payload_case_variant_after_payload_dispatch_exit_canary_runs() {
    // A no-payload case variant declared AFTER payload-bearing variants
    // (`AlarmEvent::Trigger`, ordinal 3) must be reachable when dispatched.
    // Was a native miscompile (bare-variant arg materialized as a place-copy,
    // not a tag write -> slot held ZII 0 -> only ordinal-0 matched -> exit 71).
    let canary = pass_canary(fixture_roster::NO_PAYLOAD_CASE_VARIANT_AFTER_PAYLOAD_DISPATCH_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-no-payload-variant-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("no_payload_case_variant_after_payload_dispatch canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("no-payload case dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("no_payload_case_variant_after_payload_dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the Trigger arm (ordinal 3) to run twice and exit 70, got {:?} (71 = bare variant materialized as place, tag stayed 0)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn transition_arg_local_from_embedded_call_exit_canary_runs() {
    // A local whose initializer contains a value call, passed as a transition
    // argument, must copy the local's slot -- not fold+re-materialize the call
    // in the target state (whose scratch is unreachable). Was native exit 73.
    let canary = pass_canary(fixture_roster::TRANSITION_ARG_LOCAL_FROM_EMBEDDED_CALL_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-transition-arg-embedded-call-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("transition_arg_local_from_embedded_call canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("embedded-call transition argument canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("transition_arg_local_from_embedded_call canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected r1 (=46) to pass as a transition arg and exit 70, got {:?} (73 = param slot never materialized)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn value_call_embedded_in_binary_exit_canary_runs() {
    // `let r = self.base + self.calc.double_val(6) * 3`: a value call embedded in
    // a binary. A read of `r` must resolve to its local slot (46), not the
    // embedded call's scratch result slot (12). Was a slot-name collision that
    // made the guard read the scratch -> native exit 71.
    let canary = pass_canary(fixture_roster::VALUE_CALL_EMBEDDED_IN_BINARY_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-value-call-embedded-binary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("value_call_embedded_in_binary canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("embedded binary value-call canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("value_call_embedded_in_binary canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected base + double_val(6)*3 == 46 and exit 70, got {:?} (71 = read the embedded call's scratch slot)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn sequential_self_field_rmw_exit_canary_runs() {
    // Sequential read-modify-write on a self field across 5 sub-machine calls
    // (`self.s.total = self.s.total + 1` in accum, called 5x) must accumulate
    // to 5. Guards against the stale-static-fold regression (the read folding
    // to the ZII entry value, emitting a constant store of 1 every call).
    let canary = pass_canary(fixture_roster::SEQUENTIAL_SELF_FIELD_RMW_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-sequential-self-field-rmw-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("sequential_self_field_rmw canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("sequential self-field RMW canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("sequential_self_field_rmw canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected 5 sequential RMW increments to total 5 and exit 70, got {:?} (72 = stale fold left total at 1)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_literal_source_cast_exit_canary_runs() {
    // A numeric `as` cast whose source folds to a literal (`10.0 as i32`) must
    // still emit a convert. The selector used to bail (no place type for a
    // literal source) and emit nothing, leaving the destination 0. Guards both
    // float->int and int->float results, exits 70 only when both are correct.
    let canary = pass_canary(fixture_roster::RUNTIME_LITERAL_SOURCE_CAST_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-literal-source-cast-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("literal source cast canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("literal source cast canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected literal-source casts (10.0 as i32, 7 as f64) to emit converts and exit 70, got {:?} (71 = wrong/missing convert)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_float_constant_store_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_FLOAT_CONSTANT_STORE_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-runtime-float-store-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float constant store canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("float constant store canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected float constant stores (f64 + f32 + 0.0) to execute and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_match_value_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MATCH_VALUE_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-runtime-match-value-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("match value canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("match value canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("match value canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected value-position match (enum + integer + wildcard) to select the right arm (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_flat_boolean_logic_exit_canary_runs() {
    // Flat boolean logic in guards + value position: a && b, a || b, !b, a && c && !b,
    // and `let r = a && !b`. (The nested mix (a||b)&&c is a documented separate gap.)
    let canary = pass_canary(fixture_roster::RUNTIME_FLAT_BOOLEAN_LOGIC_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-flat-boolean-logic-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("flat-boolean-logic canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("flat boolean logic canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("flat-boolean-logic canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected flat boolean logic (&&, ||, !, three-term, value-position) to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}
