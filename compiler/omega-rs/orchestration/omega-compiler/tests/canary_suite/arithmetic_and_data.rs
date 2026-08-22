use super::*;

#[test]
fn runtime_float_local_arithmetic_exit_canary_runs() {
    // Float arithmetic whose result is a `let`-bound LOCAL must lower to an SSE
    // op (addsd/...), not an integer add over the IEEE bits. The local-target
    // binary write used to emit an integer op; the canary guards the exact result
    // (6.5) and exits 70 only when correct (71 otherwise).
    let canary = pass_canary("expressions/runtime_float_local_arithmetic_exit");
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
    let canary = pass_canary("expressions/float_array_binary_op_zero");
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
    let canary = pass_canary("expressions/f32_array_binary_op_zero");
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
    let canary = pass_canary("expressions/arithmetic_domain_wrapping_exit");
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
    let canary = pass_canary("expressions/arithmetic_domain_saturating_exit");
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
    let canary = pass_canary("expressions/arithmetic_domain_saturating_div_mod_exit");
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
    let canary = pass_canary("expressions/runtime_guard_divide_modulo_exit");
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
    let canary = pass_canary("expressions/runtime_guard_negative_arithmetic_exit");
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
    let canary = pass_canary("expressions/runtime_guard_divide_modulo_signedness_exit");
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
    let canary = pass_canary("control_flow/runtime_nested_loop_grid_sum_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-nested-loop-grid-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nested loop grid canary should compile");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("control_flow/runtime_multi_field_payload_arith_exit");
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
    let canary = pass_canary("control_flow/case_payload_shared_field_name_exit");
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
    let canary = pass_canary("control_flow/sum_field_storage_roundtrip");
    let main_path = canary.join("main.omg");

    let checked = omega_compiler::compile_to_checked(&main_path, None)
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
    let canary = pass_canary("control_flow/sum_mixed_width_payload_layout");
    let main_path = canary.join("main.omg");

    let checked = omega_compiler::compile_to_checked(&main_path, None)
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
    let canary = pass_canary("expressions/arithmetic_domain_saturating_mul_exit");
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
    let canary = pass_canary("expressions/arithmetic_domain_saturating_mul_signed_exit");
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
    let canary = pass_canary("expressions/arithmetic_domain_trapping_div_exit");
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
    let canary = pass_canary("expressions/arithmetic_domain_trapping_mul_exit");
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
    let canary = pass_canary("arithmetic/runtime_transition_arg_guard_narrowing_exit");
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
    let canary = pass_canary("arithmetic/runtime_requires_one_sided_bound_exit");
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
    let canary = pass_canary("arithmetic/runtime_transition_value_guard_narrowing_exit");
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
    let canary = pass_canary("arithmetic/runtime_transition_arg_false_arm_narrowing_exit");
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
    let canary = pass_canary("arithmetic/runtime_transition_arg_saturating_exit");
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
    let canary = pass_canary("arithmetic/runtime_cast_element_accumulator_exit");
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
    let canary = pass_canary("arithmetic/runtime_domain_boundaries_exit");
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
    let canary = pass_canary("arithmetic/runtime_comparison_signedness_exit");
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
    let canary = pass_canary("arithmetic/runtime_shift_signedness_exit");
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
    let canary = pass_canary("arithmetic/runtime_shift_in_guard_exit");
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
    let canary = pass_canary("arithmetic/runtime_cast_in_guard_exit");
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
    let canary = pass_canary("arithmetic/runtime_parenthesized_guard_subjects_exit");
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
    let canary = pass_canary("arithmetic/runtime_and_of_or_guard_exit");
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
    let canary = pass_canary("arithmetic/runtime_negated_boolean_nesting_guard_exit");
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
    let canary = pass_canary("arithmetic/runtime_guard_feature_composition_exit");
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
    let canary = pass_canary("arithmetic/runtime_saturating_narrow_add_sub_exit");
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
    let canary = pass_canary("arithmetic/runtime_unsigned_high_bit_u32_ops_exit");
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
    let canary = pass_canary("arithmetic/runtime_narrow_signed_wrap_boundaries_exit");
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
    let canary = pass_canary("arithmetic/runtime_narrow_signed_guard_ops_exit");
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
    let canary = pass_canary("arithmetic/runtime_narrow_signed_divide_guard_exit");
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
    let canary = pass_canary("arithmetic/runtime_saturating_narrow_divide_exit");
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
    let canary = pass_canary("arithmetic/runtime_mixed_width_sign_exit");
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

#[test]
fn runtime_integer_casts_exit_canary_runs() {
    // Integer width/sign casts (sign-extend / zero-extend / truncate / reinterpret),
    // with each cast result threaded through a transition PARAM. This last part also
    // guards the fix for the dispatch-arg fold missing Cast/Binary arms: a let-local
    // whose initializer is a cast (or binary) reading a prior local was re-materialized
    // in the target state -- where the source local has no slot -- and read 0.
    let canary = pass_canary("arithmetic/runtime_integer_casts_exit");
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
    let canary = pass_canary("arithmetic/runtime_i64_divide_modulo_exit");
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
    let canary = pass_canary("arithmetic/runtime_float_compare_cast_exit");
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
    let canary = pass_canary("arithmetic/runtime_float_operations_exit");
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
    let canary = pass_canary("arithmetic/runtime_inferred_multipath_return_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-inferred-multipath-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("multi-path inferred return range should let the caller prove Exact");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("arithmetic/runtime_inferred_return_range_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-inferred-return-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("inferred return range should let the caller's arithmetic prove Exact");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("arithmetic/runtime_provable_field_construction_exit");
    let scratch = std::env::temp_dir().join(format!("omega-provable-field-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("provable non-literal field construction should compile");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("arithmetic/runtime_struct_field_range_narrowing_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-struct-field-range-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone()).expect(
        "struct-field range narrowing should compile (constrained field discharges the obligation)",
    );
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("arithmetic/runtime_payload_range_narrowing_exit");
    let scratch = std::env::temp_dir().join(format!("omega-payload-range-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone()).expect(
        "payload range narrowing should compile (constrained payload discharges the obligation)",
    );
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("ranges/sum_payload_range_narrowed_exit");
    let scratch = std::env::temp_dir().join(format!("omega-payload-narrow-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("direct payload pass-through should prove under the case-arm guard");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("ranges/sum_payload_range_arith_narrowed_exit");
    let scratch = std::env::temp_dir().join(format!("omega-payload-arith-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("payload operand arithmetic should prove under the case-arm guard");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("arithmetic/runtime_exclusive_range_constraint_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-exclusive-range-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("exclusive/inclusive range-constraint syntax should compile");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("arithmetic/runtime_fnv1a_hash_exit");
    let scratch = std::env::temp_dir().join(format!("omega-fnv1a-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("FNV-1a hash canary should compile");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("arithmetic/runtime_min_max_clamp_narrowing_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-min-max-clamp-narrowing-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("min/max clamp narrowing canary should compile (narrowing proves the bound)");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("arithmetic/runtime_modulo_div_narrowing_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-modulo-div-narrowing-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("modulo/div narrowing canary should compile (narrowing proves the bound)");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("expressions/arithmetic_domain_trapping_mul_overflow");
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-trap-mul-of-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_trapping_mul_overflow canary should compile");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("expressions/arithmetic_domain_saturating_signed_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-sat-signed-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_saturating_signed canary should compile");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("expressions/arithmetic_domain_requires_proven_exact_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-requires-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_requires_proven_exact canary should compile");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("expressions/arithmetic_domain_range_proven_exact_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-arith-domain-range-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_range_proven_exact canary should compile");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("expressions/arithmetic_domain_cast_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-arith-domain-cast-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_cast canary should compile");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("expressions/arithmetic_domain_trapping_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-trapping-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_trapping canary should compile");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("expressions/arithmetic_domain_trapping_overflow");
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-trapping-of-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_trapping_overflow canary should compile");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("expressions/arithmetic_domain_trapping_let_overflow");
    let scratch =
        std::env::temp_dir().join(format!("omega-trapping-let-of-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("arithmetic_domain_trapping_let_overflow canary should compile");
    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("expressions/arithmetic_domain_return_range_proven_exact_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-return-range-exact-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("return-range proven-exact canary should compile");
    let output = Command::new(scratch.join("out").join(executable_name()))
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
    let canary = pass_canary("expressions/arithmetic_domain_trapping_const_fold_overflow");
    let scratch = std::env::temp_dir().join(format!(
        "omega-arith-domain-trapping-const-of-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("trapping const-fold overflow canary should compile");
    let output = Command::new(scratch.join("out").join(executable_name()))
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
    let canary = pass_canary("arithmetic/constant_trapping_shift_value_overflow_traps");
    let scratch = std::env::temp_dir().join(format!(
        "omega-constant-trapping-shift-value-overflow-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("constant trapping shift-overflow canary should compile");
    let output = Command::new(scratch.join(executable_name()))
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
fn dead_trapping_let_traps_aborts() {
    // Abort-as-effect first sentence (owner, 2026-07-18): a trap is an
    // EFFECT, so a DEAD trapping computation is not dead -- the storage layer
    // keeps a trap-carrying initializer's slot and the trap lowers. Before,
    // native DCE'd the write AND the trap and exited 7 while the interpreter
    // trapped. Named without `_canary_runs` (non-clean-exit; outside the
    // RUN-list drift guard, like the const-fold overflow twin above).
    let canary = pass_canary("expressions/dead_trapping_let_traps");
    let scratch =
        std::env::temp_dir().join(format!("omega-dead-trapping-let-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("dead trapping let canary should compile");
    let output = Command::new(scratch.join("out").join(executable_name()))
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
    let canary = pass_canary("expressions/f32_field_binary_to_local_cast");
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
    let canary = pass_canary("expressions/f32_to_f64_local_cast");
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
    let canary = pass_canary("expressions/f32_deep_chain_binary");
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
    let canary = pass_canary("control_flow/no_payload_case_variant_after_payload_dispatch_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-no-payload-variant-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("no_payload_case_variant_after_payload_dispatch canary should compile");

    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("calls/transition_arg_local_from_embedded_call_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-transition-arg-embedded-call-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("transition_arg_local_from_embedded_call canary should compile");

    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("calls/value_call_embedded_in_binary_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-value-call-embedded-binary-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("value_call_embedded_in_binary canary should compile");

    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("calls/sequential_self_field_rmw_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-sequential-self-field-rmw-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("sequential_self_field_rmw canary should compile");

    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("expressions/runtime_literal_source_cast_exit");
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
    let canary = pass_canary("expressions/runtime_float_constant_store_exit");
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
    let canary = pass_canary("expressions/runtime_match_value_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-runtime-match-value-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("match value canary should compile");

    let output = Command::new(scratch.join(executable_name()))
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
    let canary = pass_canary("expressions/runtime_flat_boolean_logic_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-flat-boolean-logic-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("flat-boolean-logic canary should compile");
    let output = Command::new(scratch.join(executable_name()))
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

#[test]
fn runtime_enum_match_breadth_exit_canary_runs() {
    // Enum matching breadth + the SOUND pattern for a runtime-indexed enum element
    // (bind to a local first). grid[2]=Goal (non-first variant) and a field-name
    // collision (Potion.power vs Weapon.power) self-check to exit 70.
    let canary = pass_canary("expressions/runtime_enum_match_breadth_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-enum-match-breadth-{}", std::process::id()));
    compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("enum match breadth canary should compile");

    let output = Command::new(scratch.join("out").join(executable_name()))
        .output()
        .expect("enum match breadth canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected indexed-via-local enum match + payload extraction to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_conformance_item_exit_canary_runs() {
    let canary = pass_canary("traits/runtime_conformance_item_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-conformance-item-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("conformance item canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("conformance item canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `Circle satisfies Shape;` to validate against the written member and run unchanged (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn equatable_record_equality_exit_canary_runs() {
    // Equatable synthesis (decisions 8 + 11): `Point satisfies Equatable;`
    // makes `==`/`!=` on the record structural -- equal values match, one
    // differing middle field misses.
    let canary = pass_canary("traits/equatable_record_equality_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-equatable-record-equality-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("equatable record equality canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("equatable record equality canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `==`/`!=` on `Point` to compare field by field (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn equatable_sum_payload_equality_exit_canary_runs() {
    // Equatable synthesis on a payload-bearing sum: tag equality AND the
    // matching case's payload fields. Same-case-equal matches; same-case-
    // different-payload and different-case miss; the constructed-literal
    // compare pins the single-arm form.
    let canary = pass_canary("traits/equatable_sum_payload_equality_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-equatable-sum-payload-equality-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("equatable sum payload equality canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("equatable sum payload equality canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `==` on `Command` to compare tag AND payload (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn equatable_mixed_shape_equality_exit_canary_runs() {
    // Equatable synthesis on a MIXED shape: common fields AND tag AND the
    // matching case's payload. The second compare differs ONLY in a common
    // field (the reconstruction zero-initialized it), so equality that skips
    // common fields exits 71. Also regression net for the boolean-folding
    // factor/distribute mutual recursion this expansion first exposed.
    let canary = pass_canary("traits/equatable_mixed_shape_equality_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-equatable-mixed-shape-equality-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("equatable mixed shape equality canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("equatable mixed shape equality canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `==` on `RoomEvent` to compare common fields AND tag AND payload (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn equatable_string_field_equality_exit_canary_runs() {
    // Equatable synthesis over a String field: `==` compares text CONTENT
    // (length AND bytes) through the value-position text-equals operand --
    // equal contents match; same-length-different-bytes, different-length,
    // and equal-text-different-scalar-sibling all miss.
    let canary = pass_canary("traits/equatable_string_field_equality_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-equatable-string-field-equality-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("equatable string field equality canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("equatable string field equality canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `==` on `Tag` to compare String content (length AND bytes) plus the scalar sibling (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn equatable_string_not_equals_exit_canary_runs() {
    // Equatable `!=` over a String-bearing record in VALUE position: the
    // simplifier De-Morgans `equality == false` into per-field `!=` compares,
    // so the String term lowers as the negated text-equals leaf
    // (`text_equals(..) == 0`). The names differ ("gold" vs "iron") while the
    // scalar siblings are equal, so dropping the String term (the old
    // miscompile: the whole initializer write silently vanished and the ZII
    // false took the bad arm) flips the exit code.
    let canary = pass_canary("traits/equatable_string_not_equals_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-equatable-string-not-equals-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("equatable string not-equals canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("equatable string not-equals canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `!=` on `Tag` to see the differing String content (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn equatable_string_equality_guard_exit_canary_runs() {
    // Equatable structural `==` over a String-bearing record DIRECTLY in
    // GUARD position: the conjunction's String clause routes through the
    // value-position TextEquals content compare (the raw 16-byte descriptor
    // place compare cannot encode), the scalar clause stays a place compare.
    // Equal contents take the `true` arm (exit 70).
    let canary = pass_canary("traits/equatable_string_equality_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-equatable-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("equatable string guard canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("equatable string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected guard-position structural `==` on `Tag` to compare String content (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_deep_nested_field_exit_canary_runs() {
    // A 5-level nested field chain (self.l1.l2.l3.l4.v) written and read back; offsets must compose
    // through every level. v + w = 30 + 40 = 70 (a sibling `tag` decoy discriminates the offsets).
    let canary = pass_canary("data/runtime_deep_nested_field_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-deep-nested-field-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("deep nested field canary should compile from its authored root");
    let output = Command::new(scratch.join("out").join(executable_name()))
        .output()
        .expect("deep nested field canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected 5-level nested field access to resolve correctly (30+40 == 70, exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_struct_value_copy_exit_canary_runs() {
    // Struct assignment is a value copy, not an alias: copy a->b, mutate a, b stays unchanged;
    // same between array-of-structs elements. Both sums stay 14 -> exit 70.
    let canary = pass_canary("data/runtime_struct_value_copy_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-struct-value-copy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("struct value copy canary should compile from its authored root");
    let output = Command::new(scratch.join("out").join(executable_name()))
        .output()
        .expect("struct value copy canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected struct + array-element value copies to stay unchanged after mutating the source (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_whole_struct_mutation_copy_canary_runs() {
    // The first migrated CopyPlaces sites (Place rung 2a): cross-region
    // field writes + a same-region whole-struct copy, relocations patched
    // BY PLACE REGION from the materializer's site list.
    let canary = pass_canary("data/runtime_whole_struct_mutation_copy_exit");
    let main_path = canary.join("main.omg");

    let checked = omega_compiler::compile_to_checked(&main_path, None)
        .expect("whole-struct mutation copy canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (both fields survive the copy), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-copy-places-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("whole-struct mutation copy canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("whole-struct mutation copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the CopyPlaces mutation copies to deliver (exit 70), got {:?} (71 = a base patched to the wrong region's symbol)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_data_properties_exit_canary_runs() {
    let canary = pass_canary("data/runtime_data_properties_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-data-properties-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("data properties canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("data properties canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `[copy]` declarations to verify and run identically to property-free data (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn case_first_payload_zero_established_canary_compiles() {
    let canary = pass_canary("data/case_first_payload_zero_established");

    compile_canary_without_output(&canary)
        .expect("a zero-established first-case payload should compile");
}

#[test]
fn compound_assignment_exit_canary_runs() {
    let canary = pass_canary("operators/compound_assignment_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-compound-assignment-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("compound assignment canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("compound assignment canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `+= -= *= /= %=` to chain correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_chained_field_mutation_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_chained_field_mutation_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-chained-field-mutation-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("chained field mutation canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("chained field mutation canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected chained read-modify-write to observe prior writes (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_comparison_guard_signedness_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_comparison_guard_signedness_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-comparison-guard-signedness-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("comparison guard signedness canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("comparison guard signedness canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected guard comparisons to respect operand signedness (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_comparison_value_signedness_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_comparison_value_signedness_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-comparison-value-signedness-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("comparison value signedness canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("comparison value signedness canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected value-position comparisons to respect operand signedness (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_min_max_signedness_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_min_max_signedness_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-min-max-signedness-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("min/max signedness canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("min/max signedness canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected min/max to respect operand signedness (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_unsigned_division_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_unsigned_division_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-unsigned-division-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("unsigned division canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("unsigned division canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected unsigned division/remainder/logical-shift on high-bit u32 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_unsigned_min_max_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_unsigned_min_max_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-unsigned-min-max-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("unsigned min/max canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("unsigned min/max canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("unsigned min/max canary should run");

    assert_eq!(
        output.status.code(),
        Some(88),
        "expected max(u64::MAX, 5)==u64::MAX and min==5 (unsigned witness) to exit 88, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_unsigned_modulo_call_argument_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_unsigned_modulo_call_argument_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-unsigned-modulo-call-argument-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("unsigned modulo call-argument canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("unsigned modulo call-argument canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("unsigned modulo call-argument canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the inline `raw % 100` call argument to use UNSIGNED modulo \
         so the dispatch ladder selects the satisfied arm (exit 70 = 3 RNG \
         draws, interpreter semantics; exit 71 = the signed-remainder misfire \
         routed the second event into the enemy arm and drew once extra -- the \
         dungeon seed-7 14-vs-15 residual), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_named_conversion_alias_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_nested_named_conversion_alias_exit");
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nested-named-conversion-alias-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("nested named-conversion alias canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("nested named-conversion alias canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the nested conversion to read the caller's RandomState alias \
         and return its high word (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_unsigned_modulo_cast_operand_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_unsigned_modulo_cast_operand_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-unsigned-modulo-cast-operand-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("unsigned modulo cast-operand canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("unsigned modulo cast-operand canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("unsigned modulo cast-operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `((random.seed >> 32) as u32) % 199` to use UNSIGNED modulo \
         (the cast's TARGET type decides operand signedness; exit 70 = roll 158, \
         interpreter semantics; exit 71 = the signed-remainder misfire stored \
         -87 in the u32 slot), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn saturating_multiply_overflow_both_signs_canary_runs() {
    // Saturating i32 multiply overflow clamps to the SIGN-CORRECT bound: positive
    // overflow -> INT_MAX, negative overflow -> INT_MIN (clamping negative to
    // INT_MAX is the classic bug). exit 72 = positive wrong; 73 = negative bound.
    let canary = pass_canary("arithmetic/saturating_multiply_overflow_both_signs");
    let main_path = canary.join("main.omg");

    let checked = omega_compiler::compile_to_checked(&main_path, None)
        .expect("saturating multiply canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (+overflow->INT_MAX, -overflow->INT_MIN), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-sat-mul-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("saturating multiply canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("saturating multiply canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected sign-correct saturating multiply clamp (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn saturating_signed_divide_min_by_neg_one_canary_runs() {
    // Saturating signed divide/modulo of TYPE_MIN by -1: the same idiv #DE corner
    // as the Wrapping case, but CLAMPED (INT_MIN / -1 -> INT_MAX, % -> 0).
    // Verifies append_saturating_signed_divide_modulo's -1 guard. exit 72 = divide
    // did not clamp; 73 = modulo not 0.
    let canary = pass_canary("arithmetic/saturating_signed_divide_min_by_neg_one");
    let main_path = canary.join("main.omg");

    let checked = omega_compiler::compile_to_checked(&main_path, None)
        .expect("saturating INT_MIN/-1 canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (INT_MIN/-1 clamps to INT_MAX, %0), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-sat-div-min-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("saturating INT_MIN/-1 canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("saturating INT_MIN/-1 canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected INT_MIN/-1 to clamp to INT_MAX (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn wrapping_signed_divide_min_by_neg_one_canary_runs() {
    // Wrapping signed divide/modulo of TYPE_MIN by -1: x86 `idiv` raises #DE
    // (integer-overflow) for this corner, so the Wrapping domain guards it and
    // produces the wrapped result (INT_MIN / -1 -> INT_MIN, INT_MIN % -1 -> 0).
    // Before the guard the native binary crashed with STATUS_INTEGER_OVERFLOW.
    // exit 72 = divide did not wrap; 73 = modulo not 0.
    let canary = pass_canary("arithmetic/wrapping_signed_divide_min_by_neg_one");
    let main_path = canary.join("main.omg");

    let checked = omega_compiler::compile_to_checked(&main_path, None)
        .expect("wrapping INT_MIN/-1 canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (INT_MIN/-1 wraps to INT_MIN, %0), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-wrap-div-min-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wrapping INT_MIN/-1 canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("wrapping INT_MIN/-1 canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected INT_MIN/-1 to wrap (exit 70), got {:?} (a crash would be a large negative code = idiv #DE)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_signed_division_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_signed_division_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-signed-division-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("signed division canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("signed division canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed division/remainder of a negative dividend (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_shift_right_signedness_canary_runs() {
    // Right-shift is signedness-sensitive: a signed operand must lower to `sar`
    // (arithmetic), an unsigned operand to `shr` (logical). `-8 >> 1 == -4` AND
    // `0xFFFFFFFE >> 1 == 0x7FFFFFFF` both hold only when the two shifts pick
    // different instructions. exit 71 = a `sar` misfire on the unsigned value
    // (0xFFFFFFFF instead of 0x7FFFFFFF). Values are field-held so they are
    // genuine runtime operands (instruction selection resolves the field's
    // signedness); the const-folded high-bit case is a separate documented gap.
    let canary = pass_canary("arithmetic/runtime_shift_right_signedness");
    let main_path = canary.join("main.omg");

    // Interpreter oracle first: it must agree the exit is 70.
    let checked = omega_compiler::compile_to_checked(&main_path, None)
        .expect("shift-right signedness canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (sar for signed, shr for unsigned), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!(
        "omega-shift-right-signedness-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("shift-right signedness canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("shift-right signedness canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed `sar` AND unsigned `shr` right shifts (exit 70), got {:?} \
         (71 = unsigned >> emitted `sar`)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn const_fold_saturating_narrow_canary_runs() {
    // CM3 differential legs: fold_landed's Saturating CLAMP at NARROW widths
    // (i8 clamps to 127/-128, u8 to 255; the division folds unsigned at u8).
    // exit 71 = a fold regressed to the bare-i64 window (no clamp).
    let canary = pass_canary("arithmetic/const_fold_saturating_narrow_exit");
    let main_path = canary.join("main.omg");

    let checked = omega_compiler::compile_to_checked(&main_path, None)
        .expect("saturating narrow const-fold canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should clamp narrow saturating folds (exit 70), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!(
        "omega-const-fold-sat-narrow-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("saturating narrow const-fold canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("saturating narrow const-fold canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected narrow saturating clamps at fold (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn const_fold_wrapping_narrow_canary_runs() {
    // CM3 differential legs: fold_landed's wrap-to-width face at NARROW
    // widths (i8: 100+100 -> -56; u16: 65535+2 -> 1). exit 71 = a fold
    // regressed to the bare-i64 window (no wrap).
    let canary = pass_canary("arithmetic/const_fold_wrapping_narrow_exit");
    let main_path = canary.join("main.omg");

    let checked = omega_compiler::compile_to_checked(&main_path, None)
        .expect("wrapping narrow const-fold canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should wrap narrow folds to width (exit 70), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!(
        "omega-const-fold-wrap-narrow-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wrapping narrow const-fold canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("wrapping narrow const-fold canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected narrow wrapping folds at width (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}
