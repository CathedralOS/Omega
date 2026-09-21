use super::fixture_roster;
use crate::{
    Command, compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host, fs,
    interpret, pass_canary,
};
use compiler::CheckedCompileRequest;

#[test]
fn runtime_copy_then_read_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_COPY_THEN_READ_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_I64_FULL_WIDTH_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_CHAINED_STRING_APPEND_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
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
    let canary = pass_canary(fixture_roster::RUNTIME_STRING_APPEND_IN_PLACE_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
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
    let canary = pass_canary(fixture_roster::RUNTIME_STRING_CONCAT_TWO_FIELDS_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
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
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_STRING_APPEND_IN_PLACE_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
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
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_STRING_FIELD_COPY_THROUGH_MUT_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_CALL_VALUE);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
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
    for &name in fixture_roster::BOUNDARY_DOMAIN_ESTABLISHMENT_PASS_CANARIES {
        let main_path = pass_canary(name).join("main.omg");
        compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
            .unwrap_or_else(|diagnostics| {
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
    let canary = pass_canary(fixture_roster::RUNTIME_MIN_CALL_RESULT_ARITHMETIC_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_DIRECT_BOOLEAN_CONJUNCTION_EXIT);
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
    let canary = pass_canary(fixture_roster::EXECUTABLE_DOMAIN_MEMBERSHIP_EXPRESSION_EXIT);
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
    let canary = pass_canary(fixture_roster::EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_EXIT);
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
    let canary = pass_canary(fixture_roster::EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_GUARD_EXIT);
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
        pass_canary(fixture_roster::EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_INTERSECTION_GUARD_EXIT);
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
    let canary =
        pass_canary(fixture_roster::EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_UNION_GUARD_EXIT);
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
    let canary = pass_canary(fixture_roster::EXECUTABLE_DOMAIN_MEMBERSHIP_INTERSECTION_GUARD_EXIT);
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
    let canary = pass_canary(fixture_roster::EXECUTABLE_DOMAIN_MEMBERSHIP_UNION_GUARD_EXIT);
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
    let canary = pass_canary(fixture_roster::EXECUTABLE_DOMAIN_MEMBERSHIP_UNION_VALUE_EXIT);
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
    let canary = pass_canary(fixture_roster::EXECUTABLE_DOMAIN_MEMBERSHIP_INTERSECTION_VALUE_EXIT);
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
    let canary =
        pass_canary(fixture_roster::EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_UNION_VALUE_EXIT);
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
        pass_canary(fixture_roster::EXECUTABLE_IMPORTED_DOMAIN_MEMBERSHIP_INTERSECTION_VALUE_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_BOOLEAN_OR_VALUE_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_STRAIGHT_LINE_TERMINAL_LOCAL_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_STRAIGHT_LINE_TERMINAL_FIELD_READBACK_EXIT);
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
    for &(name, expected) in fixture_roster::ROOTED_RESIDUAL_SCALAR_ENTRY_PASS_CANARIES {
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
    let canary = pass_canary(fixture_roster::RUNTIME_NEGATED_BOOLEAN_PLACE_GUARD_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_BOOLEAN_CONJUNCTION_VALUE_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_SCALAR_COMPARISON_VALUE_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_STRING_COMPARISON_VALUE_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_BOOLEAN_OR_GUARD_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_DIRECT_BOOLEAN_TRANSITION_ARGUMENT_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_BOOLEAN_TRANSITION_ARGUMENT_EXIT);
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
        pass_canary(fixture_roster::RUNTIME_BOOLEAN_TRANSITION_ARGUMENT_AFTER_STRING_GUARD_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_OWNED_INDEXED_NESTED_ROOM_COPY_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_NEGATED_COMPARISON_GUARD_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_CASE_MEMBER_DISPATCH_EXIT);
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
fn runtime_integer_literal_dispatch_exit_canary_runs() {
    // An ordered literal transition chain (`1 -> ..`, `2 -> ..`, `_ -> ..`)
    // must evaluate its guards in authored order and select the matching arm:
    // `choice = 2` routes to `two()` and exits 22.
    let canary = pass_canary(fixture_roster::RUNTIME_INTEGER_LITERAL_DISPATCH_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-integer-literal-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime integer literal dispatch canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime integer literal dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime integer literal dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(22),
        "expected integer literal dispatch to select arm `2 -> two()` (exit 22), got {:?}\nstderr:\n{}",
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
    let canary = pass_canary(fixture_roster::CASE_PAYLOAD_NATIVE_CONSTRUCTION);
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
    let canary = pass_canary(fixture_roster::RUNTIME_RECORD_FIELD_VALUE_PATTERN_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_CASE_PAYLOAD_GUARD_READ_EXIT);
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
