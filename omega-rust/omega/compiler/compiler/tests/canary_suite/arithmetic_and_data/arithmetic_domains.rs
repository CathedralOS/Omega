use super::fixture_roster;
use crate::{
    Command, compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host,
    executable_name, fs, interpret, pass_canary,
};
use compiler::CheckedCompileRequest;

#[test]
fn runtime_float_local_arithmetic_exit_canary_runs() {
    // Float arithmetic whose result is a `let`-bound LOCAL must lower to an SSE
    // op (addsd/...), not an integer add over the IEEE bits. The local-target
    // binary write used to emit an integer op; the canary guards the exact result
    // (6.5) and exits 70 only when correct (71 otherwise).
    let canary = pass_canary(fixture_roster::RUNTIME_FLOAT_LOCAL_ARITHMETIC_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-float-local-arith-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float local arithmetic canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("float local arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected float arithmetic into locals to use SSE ops and yield 6.5 (exit 70), got {:?} (71 = integer op over float bits)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn float_array_binary_op_zero_exit_canary_runs() {
    // Float binary arithmetic where BOTH operands are fixed-array elements of
    // type f64 (`self.vals[0] + self.vals[1]`) must emit an SSE addsd, not an
    // integer add.  Root cause: `resolve_machine_owned_collection_in_table`
    // returned the array type `[f64; 2]` instead of the element type `f64`,
    // so `binary_value_operands_are_float` returned false.  Fixed to apply the
    // element index from the root-field member_index when the suffix is empty.
    let canary = pass_canary(fixture_roster::FLOAT_ARRAY_BINARY_OP_ZERO);
    let scratch = std::env::temp_dir().join(format!(
        "omega-float-array-binary-op-zero-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float_array_binary_op_zero canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("float_array_binary_op_zero canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f64 array element binary op to yield 7.0 and exit 70, got {:?} (71 = integer add over float bits)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn f32_array_binary_op_zero_exit_canary_runs() {
    // Same as float_array_binary_op_zero but for f32 array elements.
    // Both operands `self.vals[0]` and `self.vals[1]` are f32; their sum
    // 3.0f32 + 4.0f32 = 7.0f32 must use addss and exit 70.
    let canary = pass_canary(fixture_roster::F32_ARRAY_BINARY_OP_ZERO);
    let scratch = std::env::temp_dir().join(format!(
        "omega-f32-array-binary-op-zero-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("f32_array_binary_op_zero canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("f32_array_binary_op_zero canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32 array element binary op to yield 7.0f32 and exit 70, got {:?} (71 = integer add over float bits)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_wrapping_exit_canary_runs() {
    // Decision 17 S1a: `u8 in Wrapping` parses and wraps (200+100 -> 44).
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_WRAPPING_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-wrapping-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_wrapping canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("arithmetic-domain wrapping canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("arithmetic_domain_wrapping canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected u8 in Wrapping (200+100) to wrap to 44 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_saturating_exit_canary_runs() {
    // Decision 17 S1b: `u8 in Saturating` clamps on overflow (200+100 -> 255),
    // NOT wraps to 44. Native emits a width-correct add + carry-flag cmov.
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_SATURATING_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-saturating-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_saturating canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("arithmetic-domain saturating canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("arithmetic_domain_saturating canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected u8 in Saturating (200+100) to clamp to 255 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_saturating_div_mod_exit_canary_runs() {
    // Decision 17: SATURATING signed divide/modulo. TYPE_MIN / -1 (the only
    // overflowing division, and the corner `idiv` traps on) clamps to TYPE_MAX, and
    // TYPE_MIN % -1 -> 0, instead of trapping. The divisor reaches -1 via a loop so
    // it is a genuine runtime value (defeats const-folding), exercising the native
    // divisor==-1 guard + cmovo saturation.
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_SATURATING_DIV_MOD_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-sat-div-mod-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_saturating_div_mod canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("saturating divide/modulo canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("arithmetic_domain_saturating_div_mod canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected saturating i32::MIN/-1 -> i32::MAX, MIN%-1 -> 0, -8/-1 -> 8 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_guard_divide_modulo_exit_canary_runs() {
    // Division and modulo in a transition GUARD subject (`self.x / 3 > 5`,
    // `self.x % 5 == 3`). The planner whitelist excluded Divide+Modulo and the
    // guard value-operand resolver did not map Divide, so a div/mod guard silently
    // took the true arm. Every arm here is reached only on a correct guard, so the
    // regression would exit 71-74 instead of 70.
    let canary = pass_canary(fixture_roster::RUNTIME_GUARD_DIVIDE_MODULO_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-guard-div-mod-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("runtime_guard_divide_modulo canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("divide/modulo guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime_guard_divide_modulo canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected div/mod guard subjects to evaluate correctly (exit 70), got {:?} (71-74 = a guard took the wrong arm)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_guard_negative_arithmetic_exit_canary_runs() {
    // Negative-i32 arithmetic in a transition guard subject (`self.x - 1 == -9` for
    // x=-8) took the wrong arm natively: a computed value-operand zero-extended the
    // i32 but the compare ran 64-bit. Fixed by sizing a Binary value-operand from
    // the non-immediate operand so the compare runs at the i32 width. Every arm is
    // reached only on a correct guard, so a regression exits 71-74.
    let canary = pass_canary(fixture_roster::RUNTIME_GUARD_NEGATIVE_ARITHMETIC_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-guard-neg-arith-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("runtime_guard_negative_arithmetic canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("negative guard arithmetic canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime_guard_negative_arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected negative-i32 guard arithmetic to evaluate correctly (exit 70), got {:?} (71-74 = a guard took the wrong arm)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_guard_divide_modulo_signedness_exit_canary_runs() {
    // Division/modulo in a guard subject with a NEGATIVE i32 dividend (`neg / 2 ==
    // -4`) and a large UNSIGNED dividend. Div/mod are not modular, so the op runs at
    // the operand width (32-bit) -- signed idiv for i32, Divide->DivideUnsigned for
    // u32 so a large u32 is not misread as negative. A regression exits 71-74.
    let canary = pass_canary(fixture_roster::RUNTIME_GUARD_DIVIDE_MODULO_SIGNEDNESS_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-guard-divmod-sign-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("runtime_guard_divide_modulo_signedness canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("signedness guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime_guard_divide_modulo_signedness canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed/unsigned div/mod guard subjects to evaluate correctly (exit 70), got {:?} (71-74 = wrong arm)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_nested_loop_grid_sum_exit_canary_runs() {
    // A nested loop (outer over i, inner over j, each its own self-transition state) summing
    // i*3+j over a 3x3 grid -> 36. Exercises nested control flow + per-outer inner-counter reset.
    // Exit 70 iff sum == 36.
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_LOOP_GRID_SUM_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-nested-loop-grid-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nested loop grid canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested loop grid canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested loop grid canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected nested 3x3 grid sum (i*3+j) == 36 (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_multi_field_payload_arith_exit_canary_runs() {
    // A two-field sum-case payload `case Rect(w, h)`: both fields bind in the match arm and drive
    // a computed transition arg (w * h + 58), discriminating arm selection AND both field binds
    // (Circle computes r * 7). Rect{3,4} -> 70.
    let canary = pass_canary(fixture_roster::RUNTIME_MULTI_FIELD_PAYLOAD_ARITH_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-multi-field-payload-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("multi-field payload canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("multi-field payload canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("multi-field payload canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Rect{{3,4}} -> 3*4+58 = 70 (both payload fields bound); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn case_payload_shared_field_name_exit_canary_runs() {
    // Regression: destructuring `Tx::Transfer { to, amount }` must read Transfer's
    // `amount` (40), not a same-named field in an earlier variant (would read to=3).
    let canary = pass_canary(fixture_roster::CASE_PAYLOAD_SHARED_FIELD_NAME_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-case-payload-collision-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("case_payload_shared_field_name canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("shared payload field-name canary should retain its executable receipt");
    let output = Command::new(executable)
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
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn sum_field_storage_roundtrip_canary_runs() {
    // A sum value stored in a field, then read + dispatched across a machine call,
    // carries its TAG and PAYLOAD intact through the storage round-trip: the Pong
    // arm fires (not Ping) and both payload fields read back. Distinct from the
    // construct-and-dispatch sum canaries. exit 71 = wrong variant; 72 = payload
    // field read wrong.
    let canary = pass_canary(fixture_roster::SUM_FIELD_STORAGE_ROUNDTRIP);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("sum field-storage canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (sum tag+payload survive field round-trip), got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-sum-field-roundtrip-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("sum field-storage canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("sum field-storage canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("sum field-storage canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected sum tag+payload to survive a field store round-trip (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn sum_mixed_width_payload_layout_canary_runs() {
    // Sum-type payload LAYOUT across mixed widths: variant B packs (i16, i16,
    // i64). Each destructured field must be read at the correct byte offset AND
    // width -- the i64 sits after two i16s. Complements the shared-field-NAME
    // collision canary with an offset/width axis. exit 72 = a field read the
    // wrong offset/width; 71 = wrong variant dispatched.
    let canary = pass_canary(fixture_roster::SUM_MIXED_WIDTH_PAYLOAD_LAYOUT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("sum mixed-width payload canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for mixed-width payload reads, got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-sum-mixed-width-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("sum mixed-width payload canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("sum mixed-width payload canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("sum mixed-width payload canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected (i16,i16,i64) payload fields read at correct offset/width (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_saturating_mul_exit_canary_runs() {
    // Decision 17: `u8 in Saturating` multiply clamps 100*100=10000 to 255 (a
    // 64-bit imul gives the exact product, then range-compare + cmov to the max).
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_SATURATING_MUL_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-arith-domain-sat-mul-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_saturating_mul canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("unsigned saturating multiply canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("arithmetic_domain_saturating_mul canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected u8 in Saturating (100*100) to clamp to 255 and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_saturating_mul_signed_exit_canary_runs() {
    // Decision 17: signed saturating multiply clamps both ways (2500->127 cmovg,
    // -2500->-128 cmovl).
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_SATURATING_MUL_SIGNED_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-sat-mul-signed-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_saturating_mul_signed canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("signed saturating multiply canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("arithmetic_domain_saturating_mul_signed canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed sat mul (2500->127, -2500->-128) to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_trapping_div_exit_canary_runs() {
    // Decision 17: Trapping divide routes to the normal idiv (which traps on
    // overflow / div-by-zero); in range 140/2 = 70.
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_TRAPPING_DIV_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-trap-div-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_trapping_div canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("in-range trapping divide canary should retain its executable receipt");
    let output = Command::new(executable)
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
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn arithmetic_domain_trapping_mul_exit_canary_runs() {
    // Decision 17: in-range Trapping multiply (10*10=100) does not trap.
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_TRAPPING_MUL_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-trap-mul-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_trapping_mul canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("in-range trapping multiply canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("arithmetic_domain_trapping_mul canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected in-range Trapping mul (10*10=100) to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

/// Decision 17 transition-arg enforcement + dominating-guard narrowing: the
/// recursive arm arg `count_down(n - 1)` carries the exact-arith obligation and
/// proves Exact ONLY because the guard `n > 0` narrows `n` to `[1, ..]`. Runs to
/// 70 (the unguarded form is rejected — fail/arithmetic/transition_arg_unguarded_overflow).
#[test]
fn runtime_transition_arg_guard_narrowing_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_TRANSITION_ARG_GUARD_NARROWING_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-transition-arg-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("guarded transition-arg decrement should compile (guard narrows n-1 to Exact)");
    let executable = compilation
        .checked_native_executable_path()
        .expect("guarded transition-arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("transition-arg guard narrowing canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected guarded count_down(n-1) to prove Exact and run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

/// Decision 17 + S4 requires-seeding precision: a ONE-sided `requires x < 100`
/// bounds only x's high end, yet `x + 1` proves Exact because the operand's env
/// interval is intersected with its declared type range (`[None, 99] ∩ i32 =
/// [i32::MIN, 99]`). Before that intersection the low end stayed unbounded and
/// this over-rejected. inc(41) = 42.
#[test]
fn runtime_requires_one_sided_bound_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_REQUIRES_ONE_SIDED_BOUND_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-requires-one-sided-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("one-sided requires x<100 should prove x+1 Exact (env interval ∩ type range)");
    let executable = compilation
        .checked_native_executable_path()
        .expect("one-sided requires canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("requires one-sided bound canary should run");
    assert_eq!(
        output.status.code(),
        Some(42),
        "expected requires x<100 to prove x+1 Exact and run to 42; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

/// Decision 17 transition-VALUE return + dominating-guard narrowing: `n > 0`
/// narrows `n` so the value return `(n - 1)` proves Exact. Mirrors the
/// transition-arg canary for the return-value boundary (which previously used the
/// un-narrowed env and over-rejected). dec(43) = 42.
#[test]
fn runtime_transition_value_guard_narrowing_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_TRANSITION_VALUE_GUARD_NARROWING_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-transition-value-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("guarded transition-value decrement should compile (guard narrows n-1 to Exact)");
    let executable = compilation
        .checked_native_executable_path()
        .expect("guarded transition-value canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("transition-value guard narrowing canary should run");
    assert_eq!(
        output.status.code(),
        Some(42),
        "expected guarded (n-1) return to prove Exact and run to 42; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

/// Decision 17 transition-arg narrowing on the FALSE arm: the arm fires when
/// `n >= 70` is FALSE (negate `>=` -> `<`), so `n + 1` proves Exact. Runs to 70.
#[test]
fn runtime_transition_arg_false_arm_narrowing_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_TRANSITION_ARG_FALSE_ARM_NARROWING_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-transition-arg-false-arm-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone()).expect(
        "false-arm transition-arg increment should compile (negated guard narrows n+1 to Exact)",
    );
    let executable = compilation
        .checked_native_executable_path()
        .expect("false-arm transition-arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("transition-arg false-arm narrowing canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected false-arm climb(n+1) to prove Exact and run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

/// Decision 17 transition-arg enforcement respects domains: a Saturating-domain
/// accumulator argument carries no exact-arith obligation, so `acc + (s[0] as
/// i32 in Saturating)` compiles with no guard / no range proof. Runs to 70.
#[test]
fn runtime_transition_arg_saturating_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_TRANSITION_ARG_SATURATING_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-transition-arg-saturating-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("Saturating transition-arg accumulator should compile (no exact-arith obligation)");
    let executable = compilation
        .checked_native_executable_path()
        .expect("saturating transition-arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("transition-arg saturating canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Saturating accumulator to run to 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

/// Regression for the #59 native miscompile: a domain-cast of a slice ELEMENT in
/// a recursive accumulator (`acc + (s[0] as i32 in Wrapping)`) silently read 0
/// because the cast could not classify its element source. Fixed by classifying
/// a slice-element read from the collection's element type. Sums to 70.
#[test]
fn runtime_cast_element_accumulator_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CAST_ELEMENT_ACCUMULATOR_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-cast-element-accum-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("cast-of-element accumulator should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("cast-element accumulator canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("cast-element accumulator canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected cast-of-element accumulator to sum to 70 (not 0); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_domain_boundaries_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DOMAIN_BOUNDARIES_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-domain-boundaries-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("domain-boundaries canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("domain-boundaries canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("domain-boundaries canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Saturating/Wrapping at i32 & u8 boundaries to clamp/wrap correctly (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_comparison_signedness_exit_canary_runs() {
    // Comparison-operator signedness across widths: a signed compare used for
    // unsigned operands (or vice versa) flips the branch past the signed/unsigned
    // boundary. The canary self-checks u32/u8/u16 unsigned cases and i32/i64 signed
    // cases; the wrong arm exits 71.
    let canary = pass_canary(fixture_roster::RUNTIME_COMPARISON_SIGNEDNESS_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-comparison-signedness-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("comparison-signedness canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("comparison-signedness canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("comparison-signedness canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed/unsigned compares to pick the right branch at each width's boundary (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_shift_signedness_exit_canary_runs() {
    // Shift signedness: a signed right shift must be arithmetic (sar), an unsigned
    // one logical (shr). The canary builds the shift value at runtime (a loop) and
    // self-checks a negative arithmetic >>, a high-bit unsigned >>, and a <<.
    let canary = pass_canary(fixture_roster::RUNTIME_SHIFT_SIGNEDNESS_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-shift-signedness-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("shift-signedness canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("shift-signedness canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("shift-signedness canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed (arithmetic) vs unsigned (logical) shifts to compute correctly (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_shift_in_guard_exit_canary_runs() {
    // Shifts used DIRECTLY in a guard subject (`self.x >> n == k`). `<<` is
    // signedness-agnostic; `>>` threads the shifted value's signedness (arithmetic
    // sar for signed, logical shr for unsigned). Values are built at runtime so the
    // shifts run in codegen. Was rejected by the dispatch-guard blocker until the
    // guard value-operand path learned to thread shift signedness.
    let canary = pass_canary(fixture_roster::RUNTIME_SHIFT_IN_GUARD_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-shift-in-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("shift-in-guard canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("shift-in-guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("shift-in-guard canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed/unsigned/left shifts in guard subjects to compute correctly (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_cast_in_guard_exit_canary_runs() {
    // A numeric `as` cast used directly in a guard subject (`self.x as u8 == c`).
    // The guard value-operand path wraps it in a Convert and the compare derives the
    // cast target's width. Covers narrowing (300 as u8), widening signed (-4 as i64,
    // sign-extended), and widening unsigned (200u8 as i32, zero-extended). Values are
    // built at runtime so the casts run in codegen. Was rejected by the dispatch-guard
    // blocker until the guard resolver learned to resolve a Cast operand.
    let canary = pass_canary(fixture_roster::RUNTIME_CAST_IN_GUARD_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-cast-in-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("cast-in-guard canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("cast-in-guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("cast-in-guard canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected narrowing/widening casts in guard subjects to compute correctly (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_parenthesized_guard_subjects_exit_canary_runs() {
    // Parenthesized guard subjects: `(a as i8) > 0`, `(a + b) > 6`, and a DNF
    // `(a > 0 && b > 0) || c > 100`. The parser now routes a leading-`(` subject
    // with no top-level comma through the general expression parser. Values built
    // at runtime; all guards must hold -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_PARENTHESIZED_GUARD_SUBJECTS_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-paren-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("parenthesized-guard-subjects canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("parenthesized guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("parenthesized-guard-subjects canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected parenthesized cast/arith/DNF guard subjects to evaluate correctly (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_and_of_or_guard_exit_canary_runs() {
    // And-of-Or in a guard subject (`a && (b || c)`) now lowers: the guard build
    // distributes it to DNF without re-factoring, and the disjunction lowering
    // (which already handles a full DNF) takes it. The canary discriminates true
    // and false arms via different operands; all must be correct -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_AND_OF_OR_GUARD_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-and-of-or-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("and-of-or-guard canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("and-of-or guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("and-of-or-guard canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected And-of-Or guard subjects to evaluate + discriminate correctly (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_negated_boolean_nesting_guard_exit_canary_runs() {
    // `!(a && b)` and `!(a || b)` (De Morgan) in a guard subject: the negation is
    // pushed through and each comparison inverted, then distributed to DNF and
    // lowered. Complements the positive And-of-Or canary. Values built at runtime;
    // discriminates both arms -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_NEGATED_BOOLEAN_NESTING_GUARD_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-neg-bool-nest-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("negated-boolean-nesting canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("negated boolean guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("negated-boolean-nesting canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `!(a && b)` / `!(a || b)` guards to evaluate + discriminate correctly (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_guard_feature_composition_exit_canary_runs() {
    // Shift + cast comparisons composed INSIDE boolean nesting (&&, ||, and an Or
    // nested in an And) -- the guard value-operand path and distribute-to-DNF
    // together. Locks the integration of the guard-subject features (each canaried
    // alone). Values built at runtime; discriminates -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_GUARD_FEATURE_COMPOSITION_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-guard-compose-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("guard-feature-composition canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("guard feature-composition canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("guard-feature-composition canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected composed shift/cast + boolean-nested guards to evaluate + discriminate (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_saturating_narrow_add_sub_exit_canary_runs() {
    // Runtime narrow SATURATING add/sub at type boundaries (i8/u8/i16, field
    // operands so it exercises the backend clamp not the fold): add overflow clamps
    // to max, sub underflow clamps to min, unsigned underflow clamps to 0, in-range
    // stays exact. Differential-checked native==interp.
    let canary = pass_canary(fixture_roster::RUNTIME_SATURATING_NARROW_ADD_SUB_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-sat-narrow-as-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("saturating narrow add/sub canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("saturating narrow add/sub canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("saturating narrow add/sub canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected narrow saturating add/sub boundary clamps to be correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_unsigned_high_bit_u32_ops_exit_canary_runs() {
    // Runtime unsigned divide/modulo/shift/compare on a high-bit u32 field (> 2^31,
    // negative as i32). The field path must pick the unsigned form of each op; the
    // compare is the sharpest check (signed `3e9 > 2e9` would be false). Differential
    // native==interp.
    let canary = pass_canary(fixture_roster::RUNTIME_UNSIGNED_HIGH_BIT_U32_OPS_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-u32-highbit-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("unsigned high-bit u32 ops canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("unsigned high-bit u32 canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("unsigned high-bit u32 ops canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected runtime unsigned divide/modulo/shift/compare on a high-bit u32 to be correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_narrow_signed_wrap_boundaries_exit_canary_runs() {
    // Signed two's-complement wrap-around at narrow boundaries (i8: 127->-128, -128->127;
    // i16 analogues), both ends, in-Wrapping. Complements the saturating narrow canaries.
    // All four corners must hold -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_NARROW_SIGNED_WRAP_BOUNDARIES_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-narrow-wrap-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("narrow signed wrap canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("narrow signed wrap canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("narrow signed wrap canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i8/i16 signed Wrapping wrap-around at both boundaries (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_narrow_signed_guard_ops_exit_canary_runs() {
    // Narrow (i8) signed compare/sub/mul with negative values as guard subjects -- the
    // working siblings of the narrow-signed-divide-guard fix; guards the area.
    let canary = pass_canary(fixture_roster::RUNTIME_NARROW_SIGNED_GUARD_OPS_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-narrow-signed-guard-ops-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("narrow-signed-guard-ops canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("narrow signed guard-ops canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("narrow-signed-guard-ops canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i8 signed compare/sub/mul with negatives in guards to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_narrow_signed_divide_guard_exit_canary_runs() {
    // Narrow (i8/i16) signed div/mod evaluated as a GUARD SUBJECT with a negative
    // result. Guard-subject operands arrive zero-extended, so the 32-bit idiv divided
    // i8 -20 as 236 -- the divide core now sign-extends narrow signed operands.
    let canary = pass_canary(fixture_roster::RUNTIME_NARROW_SIGNED_DIVIDE_GUARD_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-narrow-signed-divide-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("narrow-signed-divide-guard canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("narrow signed divide-guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("narrow-signed-divide-guard canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i8/i16 signed div/mod in a guard with a negative result to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_saturating_narrow_divide_exit_canary_runs() {
    // i8/i16 saturating signed divide (previously a hard "not implemented" error):
    // normal divide, and the TYPE_MIN/-1 overflow clamped to TYPE_MAX (i8 127, i16
    // 32767). The narrow path clamps -a > TYPE_MAX instead of using neg's overflow flag.
    let canary = pass_canary(fixture_roster::RUNTIME_SATURATING_NARROW_DIVIDE_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-saturating-narrow-divide-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("saturating-narrow-divide canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("saturating narrow-divide canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("saturating-narrow-divide canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i8/i16 saturating divide (normal + TYPE_MIN/-1 -> TYPE_MAX) to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_mixed_width_sign_exit_canary_runs() {
    // Mixed-width / mixed-sign arithmetic auto-promotes and extends the narrower
    // operand correctly: sign-extension (i32(-5)+i64), zero-extension (u8+i32),
    // narrower-signed (i16(-3)+i32), and a mixed-sign add (i32+u32).
    let canary = pass_canary(fixture_roster::RUNTIME_MIXED_WIDTH_SIGN_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-mixed-width-sign-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("mixed-width-sign canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("mixed-width sign canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("mixed-width-sign canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected mixed-width/sign arithmetic with correct sign/zero extension to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}
