use super::*;

#[test]
fn runtime_copy_then_read_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_copy_then_read_exit");
    let scratch = std::env::temp_dir().join(format!("omega-copy-then-read-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("copy-then-read canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("copy-then-read canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("copy-then-read canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a read after a copy to observe the copied value (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_i64_full_width_exit_canary_runs() {
    let canary = pass_canary("arithmetic/runtime_i64_full_width_exit");
    let scratch = std::env::temp_dir().join(format!("omega-i64-full-width-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("i64 full-width canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("i64 full-width canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("i64 full-width canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i64 store/add/compare to keep full 64-bit precision (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_chained_string_append_exit_canary_runs() {
    let canary = pass_canary("text/runtime_chained_string_append_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("chained bounded-carrier append canary should check");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);
    let scratch = std::env::temp_dir().join(format!(
        "omega-chained-string-append-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("chained bounded-carrier append canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("chained bounded-carrier append canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("chained string append canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected chained in-place appends to be visible to a later guard (exit 70), got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_string_append_in_place_exit_canary_runs() {
    let canary = pass_canary("text/runtime_string_append_in_place_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("descriptor text append-in-place canary should check");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);
    let scratch = std::env::temp_dir().join(format!(
        "omega-string-append-in-place-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("descriptor text append-in-place canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("descriptor text append-in-place canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("descriptor text append-in-place canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected descriptor text materialization followed by append to preserve the prefix and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_string_concat_two_fields_exit_canary_runs() {
    let canary = pass_canary("text/runtime_string_concat_two_fields_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("two-carrier text concat canary should compile to checked trees");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(
        interpreted.exit_code, 70,
        "interpreter should join the same two runtime bounded text carriers"
    );

    let scratch = std::env::temp_dir().join(format!(
        "omega-string-concat-two-fields-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("two-carrier text concat canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("two-carrier text concat canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("two-carrier text concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected concat of two runtime bounded text carriers (no literal anchor) to produce the joined text (exit 70), got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_machine_string_append_in_place_exit_canary_runs() {
    let canary = pass_canary("text/runtime_machine_string_append_in_place_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("bounded-carrier append-in-place canary should check");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);
    let scratch = std::env::temp_dir().join(format!(
        "omega-string-append-in-place-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("bounded-carrier append-in-place canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("bounded-carrier append-in-place canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("string append-in-place canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected in-place machine String append to preserve the prefix (exit 70), got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_local_string_field_copy_through_mut_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_local_string_field_copy_through_mut_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-local-string-field-copy-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("local string field copy canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("local string field copy canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("local string field copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a local struct String field copied through a &mut String param to reach the caller (exit 70), got {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_call_value_canary_runs() {
    // A string literal may establish a bounded text carrier as a machine
    // terminal value; the returned `{len, bytes}` value must then copy into
    // the caller's carrier field in both execution engines.
    let canary = pass_canary("calls/runtime_call_value");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("bounded-carrier return-value canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter should preserve a returned bounded carrier (exit 70), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-carrier-return-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("bounded-carrier return-value canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("bounded-carrier return-value canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bounded-carrier return-value canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native bounded-carrier return should exit 70, got {:?}",
        output.status.code()
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn boundary_operator_domain_ensures_flow_to_mutable_operand() {
    // Named boundary/operator calls do not produce ordinary state-call facts.
    // Their mutable-operand invalidation and domain postcondition flow must
    // nevertheless establish the exact caller place for the next call.
    for name in [
        "text/utf8_boundary_established",
        "text/no_nul_boundary_established",
        "text/domain_forget_validate_transitions",
    ] {
        let main_path = pass_canary(name).join("main.omg");
        compile_to_checked(&main_path, None).unwrap_or_else(|diagnostics| {
            panic!(
                "boundary operator domain establishment should check for {name}:\n{}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    }
}

/// Regression guard: a value-call (min/max builtin) result bound to a local and
/// then used in ARITHMETIC. The min-result local was elided as dead (the
/// liveness scan ignored later LocalData initializers), so `s = bounded + 70`
/// dropped its unresolved operand and s stayed ZII 0 (native exited 71). Fixed
/// by keeping the slot for any call-result initializer.
#[test]
fn runtime_min_call_result_arithmetic_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_min_call_result_arithmetic_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-min-call-result-arith-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("min-call-result arithmetic canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("min-call-result arithmetic canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("min-call-result arithmetic canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected min(seed,60)+70 to materialize and equal 70 (exit 70); 71 = the \
         write was dropped (s stayed 0); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_direct_boolean_conjunction_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_direct_boolean_conjunction_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-direct-bool-conjunction-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime direct boolean conjunction canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime direct boolean conjunction canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime direct boolean conjunction canary should run");

    assert_eq!(
        output.status.code(),
        Some(21),
        "expected runtime direct boolean conjunction canary to route to ambush exit code 21, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn executable_domain_membership_expression_exit_canary_runs() {
    let canary = pass_canary("domains/executable_domain_membership_expression_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-domain-membership-expression-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out")).expect(
        "executable domain membership expression canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "executable domain membership expression canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("executable domain membership expression canary should run");

    assert_eq!(
        output.status.code(),
        Some(81),
        "expected executable domain membership expression canary to route to exit code 81, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn executable_imported_domain_membership_exit_canary_runs() {
    let canary = pass_canary("domains/executable_imported_domain_membership_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-imported-domain-membership-expression-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out")).expect(
        "executable imported domain membership canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "executable imported domain membership canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("executable imported domain membership canary should run");

    assert_eq!(
        output.status.code(),
        Some(91),
        "expected executable imported domain membership canary to route to exit code 91, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn executable_imported_domain_membership_guard_exit_canary_runs() {
    let canary = pass_canary("domains/executable_imported_domain_membership_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-imported-domain-membership-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out")).expect(
        "executable imported domain membership guard canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "executable imported domain membership guard canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("executable imported domain membership guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(81),
        "expected executable imported domain membership guard canary to route to exit code 81, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn executable_imported_domain_membership_intersection_guard_exit_canary_runs() {
    let canary =
        pass_canary("domains/executable_imported_domain_membership_intersection_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-imported-domain-membership-intersection-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("executable imported domain membership intersection guard canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "executable imported domain membership intersection guard canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("executable imported domain membership intersection guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(219),
        "expected executable imported domain membership intersection guard canary to route to exit code 219, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn executable_imported_domain_membership_union_guard_exit_canary_runs() {
    let canary = pass_canary("domains/executable_imported_domain_membership_union_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-imported-domain-membership-union-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("executable imported domain membership union guard canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "executable imported domain membership union guard canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("executable imported domain membership union guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(217),
        "expected executable imported domain membership union guard canary to route to exit code 217, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn executable_domain_membership_intersection_guard_exit_canary_runs() {
    let canary = pass_canary("domains/executable_domain_membership_intersection_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-domain-membership-intersection-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out")).expect(
        "executable domain membership intersection canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "executable domain membership intersection canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("executable domain membership intersection canary should run");

    assert_eq!(
        output.status.code(),
        Some(231),
        "expected executable domain membership intersection canary to route to exit code 231, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn executable_domain_membership_union_guard_exit_canary_runs() {
    let canary = pass_canary("domains/executable_domain_membership_union_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-domain-membership-union-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("executable domain membership union canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("executable domain membership union canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("executable domain membership union canary should run");

    assert_eq!(
        output.status.code(),
        Some(241),
        "expected executable domain membership union canary to route to exit code 241, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn executable_domain_membership_union_value_exit_canary_runs() {
    let canary = pass_canary("domains/executable_domain_membership_union_value_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-domain-membership-union-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out")).expect(
        "executable domain membership union value canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "executable domain membership union value canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("executable domain membership union value canary should run");

    assert_eq!(
        output.status.code(),
        Some(205),
        "expected executable domain membership union value canary to route to exit code 205, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn executable_domain_membership_intersection_value_exit_canary_runs() {
    let canary = pass_canary("domains/executable_domain_membership_intersection_value_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-domain-membership-intersection-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("executable domain membership intersection value canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "executable domain membership intersection value canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("executable domain membership intersection value canary should run");

    assert_eq!(
        output.status.code(),
        Some(233),
        "expected executable domain membership intersection value canary to route to exit code 233, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn executable_imported_domain_membership_union_value_exit_canary_runs() {
    let canary = pass_canary("domains/executable_imported_domain_membership_union_value_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-imported-domain-membership-union-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("executable imported domain membership union value canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "executable imported domain membership union value canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("executable imported domain membership union value canary should run");

    assert_eq!(
        output.status.code(),
        Some(215),
        "expected executable imported domain membership union value canary to route to exit code 215, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn executable_imported_domain_membership_intersection_value_exit_canary_runs() {
    let canary =
        pass_canary("domains/executable_imported_domain_membership_intersection_value_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-imported-domain-membership-intersection-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("executable imported domain membership intersection value canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "executable imported domain membership intersection value canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("executable imported domain membership intersection value canary should run");

    assert_eq!(
        output.status.code(),
        Some(217),
        "expected executable imported domain membership intersection value canary to route to exit code 217, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_local_boolean_or_value_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_local_boolean_or_value_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-local-boolean-or-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime local boolean or value canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime local boolean or value canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime local boolean or value canary should run");

    assert_eq!(
        output.status.code(),
        Some(251),
        "expected runtime local boolean or value canary to route to exit code 251, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Straight-line value helper with NO transitions whose terminal expression is
// a LOCAL read. Pre-fix, only a bare literal terminal delivered to its caller;
// a local terminal silently fell through to the default value. Guards the
// terminal-value constant fold through local initializers.
#[test]
fn runtime_straight_line_terminal_local_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_straight_line_terminal_local_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-straight-line-terminal-local-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("straight-line terminal local canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("straight-line terminal local canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("straight-line terminal local canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the terminal local read to deliver as the exit code 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The runtime half of the straight-line helper-result shape: a field WRITE
// followed by a terminal field READ-BACK. Unlike the local variant this cannot
// constant fold — it exercises the ordinary result-register load.
#[test]
fn runtime_straight_line_terminal_field_readback_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_straight_line_terminal_field_readback_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-straight-line-terminal-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("straight-line terminal field read-back canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "straight-line terminal field read-back canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("straight-line terminal field read-back canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the terminal field read-back to deliver as the exit code 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn rooted_residual_scalar_entry_cohort_runs() {
    for (name, expected) in [
        ("control_flow/guarded_transition_dispatch", 0),
        ("collections/record_array_field_access", 0),
    ] {
        let canary = pass_canary(name);
        let build_dir = std::env::temp_dir().join(format!(
            "omega-rooted-residual-scalar-{}-{}",
            name.replace('/', "-"),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);
        let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .unwrap_or_else(|diagnostics| panic!("{name} should compile: {diagnostics:?}"));
        let executable = compilation
            .checked_native_executable_path()
            .unwrap_or_else(|| panic!("{name} should retain its executable receipt"));
        let output = Command::new(executable)
            .output()
            .unwrap_or_else(|error| panic!("{name} should run: {error}"));
        assert_eq!(
            output.status.code(),
            Some(expected),
            "unexpected rooted exit for {name}: {:?}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn runtime_negated_boolean_place_guard_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_negated_boolean_place_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-negated-bool-place-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime negated boolean place guard canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime negated boolean place guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime negated boolean place guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(73),
        "expected runtime negated boolean place guard canary to route to exit code 73, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_local_boolean_conjunction_value_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_local_boolean_conjunction_value_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-local-bool-conjunction-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("runtime local boolean conjunction value canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "runtime local boolean conjunction value canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime local boolean conjunction value canary should run");

    assert_eq!(
        output.status.code(),
        Some(74),
        "expected runtime local boolean conjunction value canary to route to exit code 74, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_local_scalar_comparison_value_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_local_scalar_comparison_value_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-local-scalar-comparison-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("runtime local scalar comparison value canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "runtime local scalar comparison value canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime local scalar comparison value canary should run");

    assert_eq!(
        output.status.code(),
        Some(76),
        "expected runtime local scalar comparison value canary to route to exit code 76, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_local_string_comparison_value_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_local_string_comparison_value_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-local-string-comparison-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("runtime local string comparison value canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "runtime local string comparison value canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime local string comparison value canary should run");

    assert_eq!(
        output.status.code(),
        Some(78),
        "expected runtime local string comparison value canary to route to exit code 78, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_boolean_or_guard_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_boolean_or_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-bool-or-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime boolean or guard canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime boolean or guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime boolean or guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(71),
        "expected runtime boolean or guard canary to route to exit code 71, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_direct_boolean_transition_argument_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_direct_boolean_transition_argument_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-direct-bool-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("runtime direct boolean transition argument canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "runtime direct boolean transition argument canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime direct boolean transition argument canary should run");

    assert_eq!(
        output.status.code(),
        Some(211),
        "expected runtime direct boolean transition argument canary to route to exit code 211, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_local_boolean_transition_argument_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_local_boolean_transition_argument_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-local-bool-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("runtime local boolean transition argument canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "runtime local boolean transition argument canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime local boolean transition argument canary should run");

    assert_eq!(
        output.status.code(),
        Some(201),
        "expected runtime local boolean transition argument canary to route to exit code 201, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_boolean_transition_argument_after_string_guard_exit_canary_runs() {
    let canary =
        pass_canary("control_flow/runtime_boolean_transition_argument_after_string_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-bool-transition-after-string-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("runtime boolean transition argument after string guard canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "runtime boolean transition argument after string guard canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime boolean transition argument after string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(247),
        "expected runtime boolean transition argument after string guard canary to route to exit code 247, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_machine_owned_indexed_nested_room_copy_exit_canary_runs() {
    let canary = pass_canary("storage/runtime_machine_owned_indexed_nested_room_copy_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-indexed-nested-room-copy-{}",
        std::process::id()
    ));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime machine-owned indexed nested room copy canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "runtime machine-owned indexed nested room copy canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime machine-owned indexed nested room copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(87),
        "expected runtime machine-owned indexed nested room copy canary to route to exit code 87, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_negated_comparison_guard_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_negated_comparison_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-negated-comparison-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime negated comparison guard canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime negated comparison guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime negated comparison guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(75),
        "expected runtime negated comparison guard canary to route to exit code 75, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_case_member_dispatch_exit_canary_runs() {
    // Payload-less `case` members (the spelling that replaces `enum`) must
    // dispatch in a transition exactly like the retired keyword did.
    let canary = pass_canary("control_flow/runtime_case_member_dispatch_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-case-member-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime case member dispatch canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime case member dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime case member dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected case-member transition dispatch to select Direction::South (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn case_payload_native_construction_canary_runs() {
    // Case payload construction (`Command::Move { steps: 70 }`) lowers natively:
    // the i32 case tag writes at offset 0, the payload field at its packed
    // offset, the transition arm compares only the 4-byte tag, and the
    // destructured `steps` binding reads the payload member into the target
    // state's argument. Promoted from pending/ when payload codegen landed.
    let canary = pass_canary("data/case_payload_native_construction");
    let scratch = std::env::temp_dir().join(format!(
        "omega-case-payload-construction-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("case payload construction canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("case payload construction canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("case payload construction canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected case payload construction + tag dispatch + payload read (exit 70), got {:?} (71 = wrong arm)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_record_field_value_pattern_exit_canary_runs() {
    // `Header { ok: 0, version }` is a plain-data destructure plus a real
    // `header.ok == 0` guard.  The matched arm must bind `version` from the
    // same evaluated subject and route its value to the target state.
    let canary = pass_canary("data/runtime_record_field_value_pattern_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-record-field-value-pattern-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("record field-value pattern canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("record field-value pattern canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("record field-value pattern canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `ok: 0` to select the arm and bind version=70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_case_payload_guard_read_exit_canary_runs() {
    // A multi-field case payload read in a destructure `if` guard: the guard
    // must read the SECOND payload field (`bonus`, packed after `power`) from
    // the enum value, not match on tag alone -- a decoy same-case arm with a
    // wrong bonus sits first and catches a dropped `if` clause.
    let canary = pass_canary("data/runtime_case_payload_guard_read_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-case-payload-guard-read-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("case payload guard read canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("case payload guard read canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("case payload guard read canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the `if bonus == 10` payload guard to select the second Strike arm (exit 70), got {:?} (71 = decoy/default arm)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn case_membership_value_exit_canary_runs() {
    // Decision 11: `cmd in Command::Move` in VALUE position lowers to a
    // tag-only compare. The constructed payload (`dx: 3`) exits 71 if the
    // compare reads payload bytes instead of clamping to the tag.
    let canary = pass_canary("data/case_membership_value_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-case-membership-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("case membership value canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("case membership value canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("case membership value canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.cmd in Command::Move` to be a true tag test (exit 70), got {:?} (71 = membership missed, e.g. payload bytes leaked into the compare)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn match_exhaustive_by_cases_canary_runs() {
    // Exhaustiveness over implicit case-domains: one arm per case counts as
    // a complete tag set (no `_`), and the counted dispatch still selects
    // the right arm at runtime.
    let canary = pass_canary("data/match_exhaustive_by_cases");
    let scratch = std::env::temp_dir().join(format!(
        "omega-match-exhaustive-by-cases-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("exhaustive-by-cases canary should compile without a `_` arm");

    let executable = compilation
        .checked_native_executable_path()
        .expect("exhaustive-by-cases canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("exhaustive-by-cases canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the held case's arm (exit 70), got {:?} (71 = dispatch missed)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn match_exhaustive_by_case_union_domain_canary_runs() {
    // A PURE case-union domain arm (the sole body fact is
    // `self in Command::Move | Command::Say`) contributes its tag set to exhaustiveness
    // -- no `_` needed -- and classifies at runtime: the held value is the
    // SECOND union member, so a lowering that drops union arms exits 71.
    let canary = pass_canary("data/match_exhaustive_by_case_union_domain");
    let scratch = std::env::temp_dir().join(format!(
        "omega-match-exhaustive-union-domain-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("case-union-domain canary should compile without a `_` arm");

    let executable = compilation
        .checked_native_executable_path()
        .expect("case-union-domain canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("case-union-domain canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the union-domain arm to classify `Command::Say` (exit 70), got {:?} (71 = union membership missed)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn case_membership_union_guard_exit_canary_runs() {
    // Decision 11: a union of implicit case domains as a transition guard
    // subject; the held value matches the SECOND (payload-bearing) arm.
    let canary = pass_canary("data/case_membership_union_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-case-membership-union-guard-{}",
        std::process::id()
    ));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("case membership union guard canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("case membership union guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("case membership union guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.cmd in Command::Quit | Command::Move` to take the matched arm (exit 70), got {:?} (71 = union membership missed)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_case_reassignment_exit_canary_runs() {
    // Overwriting one payload-carrying case with another (`Walk { pace: 9 }`
    // then `Run { speed: 70 }`) must rewrite both the tag and the overlaying
    // payload bytes: a stale tag selects the first (Walk) arm and exits 9, a
    // stale payload exits with the wrong code.
    let canary = pass_canary("data/runtime_case_reassignment_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-case-reassignment-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("case reassignment canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("case reassignment canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("case reassignment canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the second case construction to fully replace the first (exit 70), got {:?} (9 = stale tag took the Walk arm, 72 = no arm matched)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_mixed_shape_exit_canary_runs() {
    // MIXED data shape (frozen decision 7): common fields + cases in one
    // declaration. Construction names a common field alongside the payload,
    // a case change zero-initializes the unnamed common field, a common
    // field is read AND written without case knowledge, and tag dispatch
    // binds the payload. Layout: tag at 0, common fields after the tag,
    // payload overlay after the common fields.
    let canary = pass_canary("data/runtime_mixed_shape_exit");
    let scratch = std::env::temp_dir().join(format!("omega-mixed-shape-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("mixed shape canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("mixed shape canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("mixed shape canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected common-field reads/writes and payload binding to agree (exit 70), got {:?} (71 = a dispatch step observed the wrong value)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_array_literal_string_field_exit_canary_runs() {
    // ARRAY-literal struct-element initialization (`let rooms: [Room; 2] =
    // [Room { number: 11, label: "expected" }, ..]`) must write every element
    // field natively. The local-initializer mutation path had a StructLiteral
    // arm but no ArrayLiteral arm, so the whole initializer fell through to
    // the scalar path and selected NOTHING -- scalar element fields read 0 and
    // String descriptors read empty while the interpreter initialized them.
    // Guards read each element's scalar sibling and String field through a
    // runtime index (frame reads, no static folds) plus a cross check that
    // element 0 does not equal element 1's literal.
    let canary = pass_canary("data/runtime_array_literal_string_field_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-array-literal-string-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("array literal string field canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("array literal string field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("array literal string field canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected array-literal element init to write scalar and String fields natively (exit 70), got {:?} (71 = a guard observed a zeroed/incorrect element field)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_struct_literal_string_field_exit_canary_runs() {
    // Struct-LITERAL String field initialization (`let msg: T = T { label:
    // "hi" }`) must emit the native descriptor write, same as the assignment
    // form. Data planning previously collected string literals only from
    // assignments / state values / branch targets -- never from `let` local
    // initializers -- so the descriptor-write selection found no data object
    // and silently skipped the write (descriptor stayed zeroed natively).
    // Observed through the wire encoder's bytes plus a case-literal String
    // payload (`Command::Say { text: "ok" }`) destructured and compared.
    let canary = pass_canary("data/runtime_struct_literal_string_field_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-struct-literal-string-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("struct literal string field canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("struct literal string field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("struct literal string field canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected literal-form String field init to write the descriptor natively (exit 70), got {:?} (71 = empty/incorrect descriptor observed)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_param_domain_forward_exit_canary_runs() {
    // #66 domain-fact forwarding: an IMMUTABLE `&[u8] in Utf8` parameter, forwarded
    // as a call argument to another domained param, discharges `text in Utf8` via
    // the param's always-holding state invariant (caller-enforced `requires` +
    // immutability). Before the param-domain producer + direct state-invariant
    // consultation this rejected at compile time on the branch-dispatch path
    // (`consume: text in Utf8` saw 0 entry contexts).
    let canary = pass_canary("text/runtime_param_domain_forward_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("param domain forward canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for a forwarded domained param, got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-param-domain-forward-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("param domain forward canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("param domain forward canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("param domain forward canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected forwarded immutable domained param to discharge and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_case_payload_domain_forward_exit_canary_runs() {
    // #66 case-payload domain forwarding: a local constructed as `Command::First
    // { text: "ok" }` (payload `text: &[u8] in Utf8`) carries `cmd.<payload> in
    // Utf8` -- construction enforcement (#60-1c) proved it. Destructuring the
    // payload and forwarding it (`Command::First { text } -> consume(text)`)
    // discharges `consume: <payload> in Utf8` via the case-payload producer +
    // guarded-transition fallthrough threading. Before, this rejected at compile.
    let canary = pass_canary("text/runtime_case_payload_domain_forward_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("case payload domain forward canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for a forwarded case payload, got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!(
        "omega-case-payload-domain-forward-{}",
        std::process::id()
    ));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("case payload domain forward canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("case payload domain forward canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("case payload domain forward canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected forwarded case payload to discharge and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_tuple_transition_exit_canary_runs() {
    let canary = pass_canary("control_flow/runtime_tuple_transition_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-tuple-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime tuple transition canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime tuple transition canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime tuple transition canary should run");

    assert_eq!(
        output.status.code(),
        Some(22),
        "expected runtime tuple transition canary to route to tuple arm exit code 22, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_room_use_reentry_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_room_use_reentry_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-runtime-room-reentry-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime room use reentry canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime room use reentry canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime room use reentry canary should run");

    assert_eq!(
        output.status.code(),
        Some(41),
        "expected runtime room use reentry canary to route to spent-fountain exit code 41, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_enemy_clear_reentry_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_enemy_clear_reentry_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-enemy-reentry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime enemy clear reentry canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime enemy clear reentry canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime enemy clear reentry canary should run");

    assert_eq!(
        output.status.code(),
        Some(51),
        "expected runtime enemy clear reentry canary to route to cleared-hall exit code 51, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_clear_carve_render_string_fields_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_clear_carve_render_string_fields_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("runtime clear/carve/render carrier fields canary should check");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 198);
    assert_eq!(interpreted.stdout, Vec::<u8>::new());
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-clear-carve-render-string-fields-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out")).expect(
        "runtime clear/carve/render string fields canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime clear/carve/render string fields canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime clear/carve/render string fields canary should run");

    assert_eq!(
        output.status.code(),
        Some(198),
        "expected cleared then carved room label to render through lookup and exit 198, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_full_level_wrapper_lookup_string_field_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_full_level_wrapper_lookup_string_field_exit");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("runtime full-level wrapper carrier lookup canary should check");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 202);
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-full-level-wrapper-lookup-string-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out")).expect(
        "runtime full-level wrapper carrier lookup canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime full-level wrapper carrier lookup canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime full-level wrapper carrier lookup canary should run");

    assert_eq!(
        output.status.code(),
        Some(202),
        "expected full-level wrapper carrier lookup to preserve the room label, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_multi_room_reentry_exit_canary_runs() {
    let canary = pass_canary("dungeon/runtime_multi_room_reentry_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-multi-room-reentry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime multi-room reentry canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime multi-room reentry canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime multi-room reentry canary should run");

    assert_eq!(
        output.status.code(),
        Some(63),
        "expected runtime multi-room reentry canary to preserve all three room flags and exit 63, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_mutable_slice_element_write_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_mutable_slice_element_write_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-slice-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime mutable slice write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime mutable slice write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable slice write canary should run");

    assert_eq!(
        output.status.code(),
        Some(21),
        "expected runtime mutable slice write canary to preserve alias mutation and exit 21, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The promoted straight-line sibling: same mutable-slice-view write, but the
// ordinary value helper has NO transitions and delivers a field READ-BACK as
// its terminal value. Guards result delivery end to end through a slice write.
#[test]
fn runtime_mutable_slice_element_write_straight_line_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_mutable_slice_element_write_straight_line_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-slice-write-straight-line-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime mutable slice write straight-line canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "runtime mutable slice write straight-line canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable slice write straight-line canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the slice-view write to land and the terminal field read-back to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dispatch_mutable_slice_element_write_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_dispatch_mutable_slice_element_write_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dispatch-mutable-slice-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime dispatch mutable slice write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime dispatch mutable slice write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime dispatch mutable slice write canary should run");

    assert_eq!(
        output.status.code(),
        Some(31),
        "expected runtime dispatch mutable slice write canary to preserve alias mutation and exit 31, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_array_indexed_read_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_array_indexed_read_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-array-indexed-read-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime array indexed read canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime array indexed read canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime array indexed read canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.nums[self.i]` (runtime index) to read 20 and 40 and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_struct_field_write_exit_canary_runs() {
    // A runtime-indexed STRUCT-FIELD write `arr[i].field = v` (array of structs)
    // must invalidate the whole array's folded constants so a later const read
    // `arr[2].field` sees live storage. Regression for the stale-fold that the
    // earlier `arr[i] = v` fix missed (the `Member(Indexed(..))` target shape).
    let canary = pass_canary("slices/runtime_indexed_struct_field_write_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-indexed-struct-field-{}",
        std::process::id()
    ));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime indexed struct-field write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime indexed struct-field write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime indexed struct-field write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `entities[i].field = v` then const read-backs to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_particle_system_exit_canary_runs() {
    // A 2D particle system over an array of structs: runtime-indexed struct-field reads
    // and writes, integrating pos += vel each step. Self-checks three cells -> exit 70.
    let canary = pass_canary("structs/runtime_particle_system_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-particle-system-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("particle system canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("particle system canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("particle system canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the particle system to integrate pos += vel correctly (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_nested_struct_construction_exit_canary_runs() {
    // Nested struct construction `Rect { top_left: Point { .. }, .. }` PANICKED the
    // compiler (an arena span-contiguity assert: the field-value copy appended the
    // inner struct's fields mid-loop, interleaving the outer span). Fixed with
    // reserve-then-set. This canary self-checks the constructed nested fields.
    let canary = pass_canary("structs/runtime_nested_struct_construction_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-nested-struct-construct-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("nested struct construction canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested struct construction canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested struct construction canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected nested struct construction (Rect of two Points) to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_cross_machine_substate_name_exit_canary_runs() {
    // Two machines each have a `try1` sub-state (Picker::pick, Main::read_at). A named transition
    // target must resolve to a SIBLING state of the CURRENT machine, not collide on the shared
    // name and run the other machine's body. read_at(4) must return table[4]=60 even after
    // pick(2) (whose try1 yields a literal) runs -> exit 70. (Was an interp miscompile.)
    let canary = pass_canary("calls/runtime_cross_machine_substate_name_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-cross-machine-substate-{}",
        std::process::id()
    ));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("cross-machine substate-name canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("cross-machine substate-name canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("cross-machine substate-name canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected read_at(4)=60 despite pick's shared `try1` (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_value_call_to_array_element_exit_canary_runs() {
    // A single value-call result materializes correctly when written to a const-indexed array
    // element: triple(14)=42 lands at arr[2] with neighbours untouched -> exit 70. The working
    // write-side contrast to the value-call dispatch-position drop and the multi-call shared slot.
    let canary = pass_canary("calls/runtime_value_call_to_array_element_exit");
    let scratch = std::env::temp_dir().join(format!("omega-vc-array-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("value-call to array element canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("value-call to array element canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("value-call to array element canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected triple(14)=42 written to arr[2] with neighbours 0 (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_computed_transition_args_exit_canary_runs() {
    // Computed values (an addition, a subtraction, a cast) passed directly as transition
    // arguments materialize correctly. chk(7+3, 7-3, 300 as u8) sees sum=10, diff=4, byte=44
    // -> exit 70. The working contrast to the value-call-as-transition-arg silent drop.
    let canary = pass_canary("calls/runtime_computed_transition_args_exit");
    let scratch = std::env::temp_dir().join(format!("omega-computed-args-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("computed transition args canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("computed transition args canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("computed transition args canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected computed transition args (sum 10, diff 4, byte 44) to materialize (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_struct_by_value_param_exit_canary_runs() {
    // Passing a struct BY VALUE into a value-machine and reading all its fields in distinct
    // positional weights. decode(Coeffs{1,2,3}) = 1*100 + 2*10 + 3 = 123 -> exit 70. Pins
    // the working envelope around task #15 (scalar fields of a by-value struct param resolve).
    let canary = pass_canary("calls/runtime_struct_by_value_param_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-struct-by-value-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("struct by-value param canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("struct by-value param canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("struct by-value param canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected by-value struct param decode to yield 123 (exit 70); got {:?} (the decoded value on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_value_call_composition_exit_canary_runs() {
    // Function composition: chaining value-machine calls so each result feeds the next.
    // add_ten(5)=15, double(15)=30, minus_five(30)=25 -> exit 70. (Sequential binding; the
    // nested form f(g(x)) is a clean error today, documented in the canary.)
    let canary = pass_canary("calls/runtime_value_call_composition_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-value-call-composition-{}",
        std::process::id()
    ));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("value call composition canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("value call composition canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("value call composition canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected three-stage value-call pipeline to yield 25 (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_struct_value_call_exit_canary_runs() {
    // A value-machine that computes and RETURNS a struct (product type), completing the
    // value-call return-type map alongside scalars and sum-type returns. stats(7,3) returns
    // a record whose two independently-computed fields are 10 and 4 -> exit 70.
    let canary = pass_canary("calls/runtime_struct_value_call_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-struct-value-call-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("struct value call canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("struct value call canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("struct value call canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected value-call to return a record with sum 10 and diff 4 (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_option_value_call_exit_canary_runs() {
    // A value-machine that RETURNS an Option (Some/None), called in a loop with each result
    // matched -- the idiomatic functional shape for find/lookup/parse. classify(x) over
    // [5,-3,7] yields two present values and one absent; the present values sum to 12 and
    // one absent is counted -> exit 70.
    let canary = pass_canary("calls/runtime_option_value_call_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-option-value-call-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("option value call canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("option value call canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("option value call canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Option-returning value-call to sum Somes=12 and count 1 None (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_result_match_exit_canary_runs() {
    // Result-style error handling at runtime: a two-case enum (Ok/Err) produced
    // conditionally, then matched and handled in a loop. Safe-dividing 10/2, 7/0, 20/4
    // sums the Ok values to 10 and counts 1 Err -> exit 70.
    let canary = pass_canary("errors/runtime_result_match_exit");
    let scratch = std::env::temp_dir().join(format!("omega-result-match-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("result match canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("result match canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("result match canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Ok/Err handling to sum Oks=10 and count 1 Err (exit 70); got {:?} (the sum on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_entity_component_exit_canary_runs() {
    // An array of entities each holding a nested component struct (the entity-component
    // pattern): runtime-indexed access through a member path (`self.ents[i].pos.x`) read in
    // a loop and temp-RMW written back. Three entities pos.x = 1,2,3: sum 6, doubled to
    // 2,4,6 -> exit 70.
    let canary = pass_canary("structs/runtime_entity_component_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-entity-component-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("entity component canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("entity component canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("entity component canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the entity-component array (sum 6, doubled nested fields) to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_nested_struct_state_machine_exit_canary_runs() {
    // A state machine whose state lives in nested structs: a nested-vs-nested guard
    // subject, nested-field RMW, a cross-struct write, and a two-way nested verify. The
    // runtime-indexed-ARRAY guard bug does NOT extend to member paths -- these resolve the
    // correct field. Sums 1..5 = 15 -> exit 70.
    let canary = pass_canary("structs/runtime_nested_struct_state_machine_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-nested-struct-sm-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("nested struct state machine canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested struct state machine canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested struct state machine canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the nested-struct state machine to sum 1..5 = 15 (exit 70); got {:?} (a non-70 code is the bad sum)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_array_element_struct_copy_exit_canary_runs() {
    // Value semantics through an array-element struct copy: `self.f = self.arr[1]` produces an
    // independent copy, so mutating f leaves arr[1] untouched. Discriminates both ways: arr[1]
    // keeps (5,6), f holds the mutated (50,60) -> exit 70.
    let canary = pass_canary("structs/runtime_array_element_struct_copy_exit");
    let scratch = std::env::temp_dir().join(format!("omega-arr-elem-copy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("array-element struct copy canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("array-element struct copy canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("array-element struct copy canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected array-element struct copy to be independent (arr[1] unchanged, exit 70); got {:?} (the aliased value on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_nested_struct_value_semantics_exit_canary_runs() {
    // Deep nesting + whole-struct value semantics: a 3-level nested field read AND
    // write, a whole-struct copy by assignment, and copy independence (overwriting the
    // source leaves the copy intact). The data backbone of serious apps.
    let canary = pass_canary("structs/runtime_nested_struct_value_semantics_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-nested-struct-value-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("nested struct value-semantics canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested struct value-semantics canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested struct value-semantics canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected 3-level nesting + whole-struct copy + copy independence to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_struct_array_literal_exit_canary_runs() {
    // Composite literal nesting: a struct literal with an array-literal field AND an
    // array-of-struct-literals field. Guards the expression-handle + struct-field copy
    // paths near the nested-struct panic fix. Self-checks the constructed values.
    let canary = pass_canary("structs/runtime_struct_array_literal_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-struct-array-literal-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("struct-array literal canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("struct-array literal canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("struct-array literal canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a struct literal with array + struct-array fields to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_enum_struct_payload_exit_canary_runs() {
    // An enum variant with a STRUCT-typed payload `Event::Click(at: Point, ..)`. The
    // payload field's named type symbol was never resolved (the resolution pass
    // skipped variant payload fields), so the layout builder errored. Now construct +
    // match + read the struct payload's fields.
    let canary = pass_canary("structs/runtime_enum_struct_payload_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-enum-struct-payload-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("enum struct-payload canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("enum struct-payload canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("enum struct-payload canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected enum variant with a struct payload to construct/match/extract (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_enum_classify_dispatch_exit_canary_runs() {
    // A value-machine returns an enum computed through nested runtime guards (a sign classifier),
    // dispatched by a multi-arm match. All three classifications (Pos/Neg/Zero) are checked, so a
    // wrong classify arm or a wrong dispatch arm takes the false path. Value-call enum return +
    // multi-arm enum dispatch, in one program -> exit 70.
    let canary = pass_canary("structs/runtime_enum_classify_dispatch_exit");
    let scratch = std::env::temp_dir().join(format!("omega-enum-classify-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("enum classify-dispatch canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("enum classify-dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("enum classify-dispatch canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected value-call enum classification + multi-arm dispatch to route all three signs (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_nested_field_accumulate_loop_exit_canary_runs() {
    // Two-level nested struct fields (`self.body.pos.x`) mutated in place across a
    // state-machine loop -- the physics/entity-update pattern (position += velocity).
    // Two sibling nested fields must track independently (pos.x -> 70, pos.y -> 30),
    // chained-guard self-checked. Guards against nested-place read/write cross-talk.
    let canary = pass_canary("structs/runtime_nested_field_accumulate_loop_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-nested-accum-loop-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("nested-field accumulate-loop canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested-field accumulate-loop canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested-field accumulate-loop canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected nested struct fields to accumulate independently across a loop (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_indexed_write_const_read_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_indexed_write_const_read_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-indexed-write-const-read-{}",
        std::process::id()
    ));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("indexed-write/const-read canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed-write/const-read canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed-write/const-read canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a runtime-indexed write to invalidate whole-array constants so const-indexed reads see live storage (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_indexed_rmw_temp_exit_canary_runs() {
    let canary = pass_canary("slices/runtime_indexed_rmw_temp_exit");
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
    let canary = pass_canary("slices/runtime_indexed_write_adjacent_field_exit");
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
    let canary = pass_canary("slices/runtime_join_meet_bound_exit");
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
    let canary = pass_canary("collections/runtime_dual_indexed_comparison_guard_exit");
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
    let canary = pass_canary("collections/runtime_array_min_max_builtin_exit");
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
    let canary = pass_canary("collections/runtime_indexed_guard_subject_exit");
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
    let canary = pass_canary("host/runtime_tick_paced_marquee_exit");
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
    let canary = pass_canary("host/runtime_user32_key_state_exit");
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
    let canary = pass_canary("host/runtime_tick_count_monotonic_exit");
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
    let canary = pass_canary("host/runtime_gui_memory_dc_blit_exit");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
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
    let canary = pass_canary("host/runtime_gui_memory_dc_blit_exit");
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
    let canary = pass_canary("arithmetic/runtime_nested_payload_range_narrowing_exit");
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
    for name in [
        "calls/runtime_inline_recursive_walk_exit",
        "calls/runtime_value_call_direct_recursive_walk_exit",
        "calls/runtime_value_call_statement_recursive_walk_exit",
    ] {
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
    let canary = pass_canary("arithmetic/runtime_saturating_wide_boundaries_exit");
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
    let canary = pass_canary("arithmetic/runtime_saturating_param_carry_exit");
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
    let canary = pass_canary("arithmetic/runtime_saturating_expression_domain_exit");
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
    let canary = pass_canary("arithmetic/runtime_wrapping_expression_guard_exit");
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
    let canary = pass_canary("arithmetic/runtime_divide_min_edge_guard_exit");
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
    let canary = pass_canary("arithmetic/runtime_nested_unsigned_witness_exit");
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
    let canary = pass_canary("slices/runtime_local_array_element_value_operand_exit");
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
    let canary = pass_canary("slices/runtime_machine_array_element_fused_call_arg_exit");
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
    let canary = pass_canary("slices/runtime_saturating_array_element_guard_exit");
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
    let canary = pass_canary("termination/custom_ranking_field_countdown_compile");
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
    let canary = pass_canary("termination/custom_ranking_struct_view");
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
    let canary = pass_canary("arithmetic/runtime_float_nested_operand_exit");
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
    let canary = pass_canary("arithmetic/runtime_shift_count_domain_exit");
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
    let canary = pass_canary("arithmetic/runtime_exact_guarded_shift_count_exit");
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
    let canary = pass_canary("arithmetic/runtime_shift_atwidth_signed_modular_exit");
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
    let canary = pass_canary("arithmetic/runtime_shift_right_atwidth_exit");
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
    let canary = pass_canary("arithmetic/runtime_shift_atwidth_indexed_targets_exit");
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
    let canary = pass_canary("arithmetic/runtime_sat_nested_operand_domain_exit");
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
    let canary = pass_canary("arithmetic/runtime_sat_unsigned_onedirection_exit");
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
    let canary = pass_canary("arithmetic/runtime_sat_min_idiom_exit");
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
    let canary = pass_canary("arithmetic/runtime_shl_saturating_exit");
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
    let canary = pass_canary("arithmetic/runtime_shl_saturating_value_overflow_exit");
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
    let canary = pass_canary("arithmetic/runtime_shift_subword_masked_count_exit");
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
    let canary = pass_canary("arithmetic/float_to_int_saturating_exit");
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
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("saturating float->int canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.exit_code, 70, "interp Saturating cast should clamp");
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_to_int_unsigned_narrow_saturating_exit_canary_runs() {
    let canary = pass_canary("arithmetic/float_to_int_unsigned_narrow_saturating_exit");
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
    let checked = compile_to_checked(&canary.join("main.omg"), None)
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
    let canary = pass_canary("arithmetic/float_saturating_overflow_exit");
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
    let checked = compile_to_checked(&canary.join("main.omg"), None)
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
    let canary = pass_canary("arithmetic/float_trapping_overflow_traps");
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
    let checked = compile_to_checked(&canary.join("main.omg"), None)
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

fn assert_float_trapping_policy_canary_aborts(name: &str, reason_fragment: &str) {
    let canary = pass_canary(name);
    let suffix = name.replace(['/', '\\'], "-");
    let build_dir = std::env::temp_dir().join(format!("omega-f5-{suffix}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("Trapping float policy canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("Trapping float policy canary should run");
    assert_ne!(
        output.status.code(),
        Some(7),
        "expected `{name}` to trap before its sailed-past exit"
    );
    assert!(
        !output.status.success(),
        "expected `{name}` to terminate abnormally"
    );
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("Trapping float policy canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    let reason = outcome
        .error
        .expect("the interpreter must trap the float policy violation");
    assert!(
        reason.contains(reason_fragment),
        "expected `{name}` trap reason to contain `{reason_fragment}`, got: {reason}"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_trapping_divzero_traps_aborts() {
    assert_float_trapping_policy_canary_aborts(
        "arithmetic/float_trapping_divzero_traps",
        "division by zero",
    );
}

#[test]
fn float_trapping_invalid_traps_aborts() {
    assert_float_trapping_policy_canary_aborts(
        "arithmetic/float_trapping_invalid_traps",
        "invalid float operation",
    );
}

#[test]
fn trapping_float_to_int_cast_traps_aborts() {
    // F4: a Trapping float->int cast traps on an out-of-range value (1e20
    // -> i32) instead of FCVTZS's silent saturate. In-range computes first
    // (7.9 -> 7). Named without `_canary_runs` (non-clean-exit).
    let canary = pass_canary("arithmetic/trapping_float_to_int_cast_traps");
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
    let checked = compile_to_checked(&canary.join("main.omg"), None)
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
    let canary = pass_canary("arithmetic/trapping_float_to_narrow_int_cast_traps");
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
    let checked = compile_to_checked(&canary.join("main.omg"), None)
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
    let canary = pass_canary("arithmetic/trapping_shift_count_traps");
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
    let checked = compile_to_checked(&canary.join("main.omg"), None)
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
    let canary = pass_canary("arithmetic/float_literal_cast_proves_exit");
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
    let canary = pass_canary("arithmetic/u64_magnitude_transition_arg_exit");
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
    let canary = pass_canary("arithmetic/runtime_shift_count_proven_range_exit");
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
