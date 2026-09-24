use super::assert_native_exit_code;
use super::fixture_roster;
use crate::{compile_rooted_canary_for_native_host, fs, pass_canary};

#[test]
fn runtime_shift_operators_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SHIFT_OPERATORS_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-shift-operators-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("shift operators canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "shift-operators canary",
        "left shift and arithmetic right shift should preserve their signed semantics",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_bitwise_operators_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_BITWISE_OPERATORS_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-bitwise-operators-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bitwise operators canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "bitwise-operators canary",
        "and, or, and xor should retain their exact scalar results",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_popcount_loop_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_POPCOUNT_LOOP_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-popcount-loop-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("popcount loop canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "popcount-loop canary",
        "the shift-and-mask loop should count the exact number of set bits",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_xorshift_prng_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_XORSHIFT_PRNG_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-xorshift-prng-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("xorshift prng canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "xorshift PRNG canary",
        "the composed xor and shifts should produce the exact seeded draw",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_bitwise_guard_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_BITWISE_GUARD_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-bitwise-guard-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bitwise guard canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "bitwise-guard canary",
        "bitwise expressions should remain valid dispatch-guard subjects",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn integer_literal_suffix_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::INTEGER_LITERAL_SUFFIX_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-integer-literal-suffix-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("integer literal suffix canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "integer literal-suffix canary",
        "the i64, u32, and usize suffixes should roundtrip exactly",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_value_position_branching_call_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_POSITION_BRANCHING_CALL_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-value-position-branching-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("value-position branching call canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "value-position branching-call canary",
        "the value binding should receive the selected call arm",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_free_machine_value_call_exit_canary_runs() {
    // A value-position call to a FREE stateful machine (top-level `machine pick`,
    // no attached data, 2-arm guarded value transition) must deliver the selected
    // arm's value. The backend state-call collector resolved only local states and
    // attached (method) machines, so `pick(self.v)` was never collected: `let n =
    // pick(self.v)` silently left n at 0 and a field target failed loudly. Covers
    // both a `let` local and a field target, both arms; exits 70 only when correct.
    let canary = pass_canary(fixture_roster::RUNTIME_FREE_MACHINE_VALUE_CALL_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-free-machine-value-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("free-machine value call canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "free-machine value-call canary",
        "both local and field targets should receive the selected free-machine arm value",
    );

    let _ = fs::remove_dir_all(&scratch);
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
    let canary = pass_canary(fixture_roster::RUNTIME_FREE_MACHINE_STRUCT_ARG_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-free-machine-struct-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("free-machine struct arg canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "free-machine struct-argument canary",
        "by-value struct arguments should deliver every caller field to the free machine",
    );

    let _ = fs::remove_dir_all(&scratch);
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
    let canary = pass_canary(fixture_roster::BY_VALUE_CASE_PARAM_SELF_WRITE_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-by-value-case-param-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("by-value case param self-write canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "by-value case-parameter self-write canary",
        "the case-bearing argument should select the substate that persists the self write",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_owned_case_temporaries_exhaustive_dispatch_exit_canary_runs() {
    // The by-value case canary's variation that challenges its assumptions:
    // two case values constructed inline as call arguments in successive
    // statements, one with a payload and one without, each moved whole into
    // a callee that dispatches on it exhaustively, so every arm (not a `_`
    // fallback) consumes the owned subject. Exits 70 only when both
    // dispatches reached their arms.
    let canary =
        pass_canary(fixture_roster::RUNTIME_OWNED_CASE_TEMPORARIES_EXHAUSTIVE_DISPATCH_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-owned-case-temporaries-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("owned case temporaries canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "owned case temporaries canary",
        "each inline case argument should reach its callee's arm exactly once",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_attached_machine_struct_arg_exit_canary_runs() {
    // The attached (data-scoped, receiverless `Worker::run`) spelling of the
    // by-value struct argument shape: the same leaf expansion path lowers it
    // (binding rewrite + struct-literal member projection + caller-context
    // value resolution), but resolution routes through the attached machine
    // lookup, so it gets its own rung. Exits 70 only when the callee saw the
    // real runtime field values (a dropped result-slot write reads 0 -> 71).
    let canary = pass_canary(fixture_roster::RUNTIME_ATTACHED_MACHINE_STRUCT_ARG_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-attached-machine-struct-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("attached-machine struct arg canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "attached-machine struct-argument canary",
        "by-value struct arguments should deliver every caller field to the attached machine",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_record_forwarding_statement_call_exit_canary_runs() {
    // An inlined value call may execute ordinary statement callees before its
    // terminal value is delivered. The deferred leaf selector used to stop at
    // the outer callee's last direct mutation, so it copied `self.observed`
    // before the nested `capture()` mutation ran (native 0, interpreter 70).
    // The complete contiguous splice, including nested-callee operations, must
    // finish before the outer result slot is written. The same canary seeds an
    // omitted record field with 1 first, pinning whole-construction ZII reset
    // rather than merely the two explicitly named field writes.
    let canary = pass_canary(fixture_roster::RUNTIME_RECORD_FORWARDING_STATEMENT_CALL_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-record-forwarding-statement-call-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("record-forwarding statement-call canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "record-forwarding statement-call canary",
        "nested statement effects should precede outer value delivery",
    );

    let _ = fs::remove_dir_all(&scratch);
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
    let canary = pass_canary(fixture_roster::RUNTIME_FREE_MACHINE_STRUCT_RETURN_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-free-machine-struct-return-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("free-machine struct return canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "free-machine struct-return canary",
        "by-value struct returns should deliver every field to the caller local",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_free_machine_value_call_mut_arg_exit_canary_runs() {
    // A free-machine value call carrying a `&mut` tally argument alongside the
    // returned value: the callee increments the caller's field through the
    // reference AND returns the selected arm value. The tally is a counting probe
    // pinning call-count semantics (exactly one call). Exits 70 only when both
    // the returned value and the tally are correct.
    let canary = pass_canary(fixture_roster::RUNTIME_FREE_MACHINE_VALUE_CALL_MUT_ARG_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-free-machine-mut-arg-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("free-machine mut-arg value call canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "free-machine mutable-argument value-call canary",
        "the call should return its value and mutate the tally exactly once",
    );

    let _ = fs::remove_dir_all(&scratch);
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
    let canary = pass_canary(fixture_roster::RUNTIME_FREE_MACHINE_LOOPING_VALUE_CALL_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-free-machine-looping-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("looping free-machine value call canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "looping free-machine value-call canary",
        "the looping call should deliver its final accumulator to the caller",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_numeric_cast_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NUMERIC_CAST_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-runtime-numeric-cast-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("numeric cast canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "numeric-cast canary",
        "float-to-integer, integer-to-float, and signed widening casts should agree",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_widened_comparison_exit_canary_runs() {
    // The `as`-widen is the sanctioned way to compare different-width integers
    // (fail canary mismatched_width_comparison_rejected). Lock that the widened
    // compare does NOT truncate the wider operand: `44 as i32 == 300` is FALSE
    // (a truncating compare would read `44 == (300 & 0xFF == 44)` -> TRUE), while
    // `44 as i32 == 44` stays TRUE. Both correct -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_WIDENED_COMPARISON_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-runtime-widened-cmp-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("widened comparison canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "widened-comparison canary",
        "the explicitly widened comparison should not truncate the wider operand",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_widened_bitwise_exit_canary_runs() {
    // Bitwise companion to runtime_widened_comparison_exit: `self.big | self.small
    // as u32` (u32 256 | widened u8 1) must be 257, not 1. A truncation to u8 width
    // (the rejected mismatched_width_bitwise bug) would drop the 256. -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_WIDENED_BITWISE_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-widened-bitor-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("widened bitwise canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "widened-bitwise canary",
        "the explicitly widened bitwise operation should retain every high bit",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_16bit_cast_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_16BIT_CAST_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-runtime-16bit-cast-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("16-bit cast canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "16-bit cast canary",
        "i16 and u16 truncation, extension, and reinterpretation should agree",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_float_place_comparison_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_FLOAT_PLACE_COMPARISON_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-float-place-compare-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float place comparison canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "float place-comparison canary",
        "field-to-field float comparisons should preserve ordering and negative operands",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_float_comparison_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_FLOAT_COMPARISON_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-float-compare-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float comparison canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "float comparison-guard canary",
        "float equality and ordering guards should preserve negative operands",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_float_arithmetic_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_FLOAT_ARITHMETIC_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-runtime-float-arith-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float arithmetic canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "float-arithmetic canary",
        "float addition, subtraction, multiplication, division, and field operands should agree",
    );

    let _ = fs::remove_dir_all(&scratch);
}
