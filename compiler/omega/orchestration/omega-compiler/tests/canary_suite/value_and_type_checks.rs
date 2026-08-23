use super::*;

#[test]
fn write_only_whole_scalar_replace_and_forward_are_accepted() {
    let canary = pass_canary("borrow/write_only_whole_scalar_replace");
    check_canary(&canary).expect("whole-scalar write-only replacement should be checked");
}

#[test]
fn write_only_aggregate_projection_is_rejected() {
    let canary = fail_canary("borrow/write_only_reference_operation_gate");
    let diagnostics =
        check_canary(&canary).expect_err("write-only aggregate projection must remain gated");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("whole, unrestricted primitive scalars")
            && combined.contains("whole-root replacement"),
        "expected directed aggregate and projection diagnostics, got:\n{combined}"
    );
}

#[test]
fn write_only_observation_is_rejected() {
    let canary = fail_canary("borrow/write_only_observation");
    let diagnostics = check_canary(&canary).expect_err("write-only observation must reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("reads write-only parameter `source`")
            && combined.contains("never observation"),
        "expected directed non-observation diagnostic, got:\n{combined}"
    );
}

#[test]
fn write_only_mutable_widening_is_rejected() {
    let canary = fail_canary("borrow/write_only_mutable_widening");
    let diagnostics = check_canary(&canary).expect_err("write-only widening must reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("widens write-only parameter `value` to `&mut`")
            && combined.contains("&write value"),
        "expected directed widening diagnostic, got:\n{combined}"
    );
}

#[test]
fn write_only_attenuation_must_be_explicit() {
    let canary = fail_canary("borrow/write_only_implicit_attenuation");
    let diagnostics = check_canary(&canary)
        .expect_err("mutable syntax must not silently attenuate to write-only");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("requires explicit write-only attenuation")
            && combined.contains("pass `&write ...`")
            && combined.contains("`&mut ...` does not establish the no-read contract"),
        "expected exact borrow-mode diagnostic, got:\n{combined}"
    );
}

#[test]
fn write_only_boundary_provider_requires_admitted_claim() {
    let canary = fail_canary("borrow/write_only_provider_gate");
    let diagnostics = check_canary(&canary)
        .expect_err("opaque/boundary write-only implementations must remain gated");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("proves non-observation only for checked Omega bodies")
            && combined.contains("admitted write-only boundary claim"),
        "expected directed provider-boundary diagnostic, got:\n{combined}"
    );
}

fn assert_native_exit_code(report: &CompileReport, expected: i32, fixture: &str) {
    let executable = report
        .checked_native_executable_path()
        .unwrap_or_else(|| panic!("{fixture} lost its exact executable publication receipt"));
    let output = Command::new(executable)
        .output()
        .unwrap_or_else(|error| panic!("{fixture} should run: {error}"));
    assert_eq!(
        output.status.code(),
        Some(expected),
        "{fixture} should exit {expected}, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn value_call_as_host_arg_exit_canary_runs() {
    let canary = pass_canary("calls/value_call_as_host_arg_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-value-call-host-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);

    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("nested value call used as a host argument should compile");
    assert_native_exit_code(&compilation, 70, "nested value-call host argument canary");

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn computed_host_arg_exit_canary_runs() {
    let canary = pass_canary("calls/computed_host_arg_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-computed-host-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);

    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("computed scalar host argument should compile");
    assert_native_exit_code(&compilation, 70, "computed scalar host argument canary");

    let _ = fs::remove_dir_all(&scratch);

    let cross_dir = std::env::temp_dir().join(format!(
        "omega-computed-host-arg-arm64-{}",
        std::process::id()
    ));
    compile_single_file_hosted_main(&canary, &cross_dir, "linux_arm64")
        .expect("computed scalar host argument should cross-compile for AArch64");
    let _ = fs::remove_dir_all(&cross_dir);
}

#[test]
fn computed_host_cast_arg_exit_canary_runs() {
    let canary = pass_canary("calls/computed_host_cast_arg_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-computed-host-cast-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);

    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("computed cast host argument should compile");
    assert_native_exit_code(&compilation, 70, "computed cast host argument canary");
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn computed_host_builtin_arg_exit_canary_runs() {
    let canary = pass_canary("calls/computed_host_builtin_arg_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-computed-host-builtin-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);

    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("computed builtin host argument should compile");
    assert_native_exit_code(&compilation, 70, "computed builtin host argument canary");
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn computed_host_indexed_arg_exit_canary_runs() {
    let canary = pass_canary("calls/computed_host_indexed_arg_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-computed-host-indexed-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);

    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime-indexed host argument should compile");
    assert_native_exit_code(&compilation, 70, "runtime-indexed host argument canary");
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn exact_overflow_value_call_hint_canary_is_rejected() {
    // Exact arithmetic over a value-machine call with an unconstrained return is a
    // decision-17 overflow; the diagnostic must NAME the call and point at annotating
    // the callee's return (not just the generic "constrain the operands' range").
    let canary = fail_canary("arithmetic/exact_overflow_value_call_hint");
    let diagnostics = check_canary(&canary)
        .expect_err("expected exact-overflow-over-value-call canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("value-machine call") && combined.contains("annotate its return"),
        "expected the overflow diagnostic to name the value-call operand and point at the \
         callee's return annotation, got:\n{combined}"
    );
    // The generic overflow message also names the constraining mechanisms.
    assert!(
        combined.contains("requires` clause") && combined.contains("dominating guard"),
        "expected the overflow diagnostic to name the `requires`-clause / dominating-guard \
         constraining mechanisms, got:\n{combined}"
    );
}

#[test]
fn unknown_field_write_rejected_canary_is_rejected() {
    // A direct `self.<field>` write to a nonexistent field (a typo) is rejected at
    // type-check with a clear "data X has no field Y", not the misleading "not
    // mutable" or an opaque backend lowering error.
    let canary = fail_canary("arithmetic/unknown_field_write_rejected");
    let diagnostics =
        check_canary(&canary).expect_err("expected unknown-field write canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("has no field `cont`"),
        "expected a clear unknown-field diagnostic naming the missing field, got:\n{combined}"
    );
}

#[test]
fn literal_class_mismatch_rejected_canary_is_rejected() {
    // A cross-class literal assignment (`self.i32 = true`) is rejected at
    // type-check. This was a SILENT MISCOMPILE -- the backend stored the bool
    // literal as an integer with no error at any phase.
    let canary = fail_canary("arithmetic/literal_class_mismatch_rejected");
    let diagnostics =
        check_canary(&canary).expect_err("expected literal-class-mismatch canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("stores a boolean into a `i32` place"),
        "expected a clear cross-class literal diagnostic, got:\n{combined}"
    );
}

#[test]
fn member_class_mismatch_rejected_canary_is_rejected() {
    // A cross-class assignment through a PLACE (`self.i32 = self.bool_field`) is
    // rejected. Like the literal case this was a SILENT MISCOMPILE -- the bool
    // field was stored into the integer field with no error at any phase.
    let canary = fail_canary("arithmetic/member_class_mismatch_rejected");
    let diagnostics =
        check_canary(&canary).expect_err("expected member-class-mismatch canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("stores a boolean into a `i32` place"),
        "expected a clear cross-class place diagnostic, got:\n{combined}"
    );
}

#[test]
fn arg_class_mismatch_rejected_canary_is_rejected() {
    // A cross-class call ARGUMENT (`exit_process(self.bool_field)` for an i32
    // parameter) is rejected. Like the assignment cases this was a SILENT
    // MISCOMPILE -- the bool arg reached the host encoder as a raw byte.
    let canary = fail_canary("arithmetic/arg_class_mismatch_rejected");
    let diagnostics =
        check_canary(&canary).expect_err("expected arg-class-mismatch canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("stores a boolean into a `i32` parameter"),
        "expected a clear cross-class argument diagnostic, got:\n{combined}"
    );
}

#[test]
fn value_call_arg_class_mismatch_rejected_canary_is_rejected() {
    // A cross-class argument at a VALUE-position call site
    // (`let v: i32 = self.take(self.bool_field)`) is rejected. This was the 4th
    // silent miscompile in the cross-class family -- the value-position path
    // validated only type-parameter bounds, not argument classes.
    let canary = fail_canary("arithmetic/value_call_arg_class_mismatch_rejected");
    let diagnostics =
        check_canary(&canary).expect_err("expected value-call-arg-class-mismatch canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("stores a boolean into a `i32` parameter"),
        "expected a clear cross-class value-position argument diagnostic, got:\n{combined}"
    );
}

#[test]
fn narrowing_call_arg_rejected_canary_is_rejected() {
    // Decision-17 narrowing enforced at the call-argument boundary: passing an
    // i64 (300) to an i8 state parameter was a SILENT MISCOMPILE (truncated to
    // 44); now rejected like the analogous assignment.
    let canary = fail_canary("arithmetic/narrowing_call_arg_rejected");
    let diagnostics =
        check_canary(&canary).expect_err("expected narrowing-call-arg canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("narrowing store") && combined.contains("argument `amount`"),
        "expected a narrowing diagnostic naming the argument, got:\n{combined}"
    );
}

#[test]
fn narrowing_value_call_arg_rejected_canary_is_rejected() {
    // Decision-17 narrowing enforced at the VALUE-position call-argument boundary
    // (`let v: i32 = self.take_i8(self.big)`, i64 -> i8): the last silent
    // miscompile in the call-argument narrowing family, now rejected.
    let canary = fail_canary("arithmetic/narrowing_value_call_arg_rejected");
    let diagnostics =
        check_canary(&canary).expect_err("expected narrowing value-call-arg canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("narrowing store") && combined.contains("argument `x`"),
        "expected a narrowing diagnostic naming the argument, got:\n{combined}"
    );
}

#[test]
fn transition_value_overflow_rejected_canary_is_rejected() {
    // Decision-17 exact-arithmetic overflow at the transition-value return
    // boundary (`transition { _ -> (x + y) }`): this boundary used to SKIP the
    // overflow proof obligation the other boundaries enforce, so the sum wrapped
    // silently. Now checked uniformly.
    let canary = fail_canary("arithmetic/transition_value_overflow_rejected");
    let diagnostics =
        check_canary(&canary).expect_err("expected transition-value-overflow canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("exact arithmetic") && combined.contains("return value"),
        "expected an exact-arithmetic overflow diagnostic at the return boundary, got:\n{combined}"
    );
}

#[test]
fn joint_add_guard_cannot_bound_a_different_operand() {
    let canary = fail_canary("arithmetic/joint_add_guard_wrong_operand");
    let diagnostics =
        check_canary(&canary).expect_err("a guard over `other` must not authorize `left + right`");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("transition argument may overflow"),
        "expected the exact-add obligation to reject:\n{combined}"
    );
}

#[test]
fn struct_literal_class_mismatch_rejected_canary_is_rejected() {
    // A cross-class struct-literal field value (`Point { x: true }`, bool -> i32
    // field) is rejected. This was a SILENT MISCOMPILE at construction, the
    // sibling of the assignment / call-arg cross-class holes.
    let canary = fail_canary("arithmetic/struct_literal_class_mismatch_rejected");
    let diagnostics =
        check_canary(&canary).expect_err("expected struct-literal-class-mismatch canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("stores a boolean into a `i32` field"),
        "expected a clear cross-class construction diagnostic, got:\n{combined}"
    );
}

#[test]
fn struct_literal_narrowing_rejected_canary_is_rejected() {
    // Decision-17 narrowing at the struct-literal construction boundary
    // (`Small { v: self.i64_field }` into an i8 field): a SILENT truncation, now
    // rejected. The construction check enforces the field's type width for every
    // primitive field, not only `[a..=b]`-refined ones.
    let canary = fail_canary("arithmetic/struct_literal_narrowing_rejected");
    let diagnostics =
        check_canary(&canary).expect_err("expected struct-literal-narrowing canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("narrowing store")
            && combined.contains("construction of `Small` field `v`"),
        "expected a narrowing diagnostic at the construction boundary, got:\n{combined}"
    );
}

#[test]
fn array_literal_element_narrowing_rejected_canary_is_rejected() {
    // Decision-17 narrowing at the array-literal element boundary (`[300, 0, 0]`
    // into a `[i8; 3]`): a SILENT truncation (300 -> 44), now rejected. Each
    // element is checked against the array's element type.
    let canary = fail_canary("arithmetic/array_literal_element_narrowing_rejected");
    let diagnostics = check_canary(&canary)
        .expect_err("expected array-literal-element-narrowing canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("narrowing store") && combined.contains("array literal element"),
        "expected a narrowing diagnostic at the array-literal element boundary, got:\n{combined}"
    );
}

#[test]
fn array_literal_let_init_narrowing_rejected_canary_is_rejected() {
    // Array-literal element narrowing at the LET-INITIALIZER position -- a distinct
    // wiring of the shared array-element check from the assignment one.
    let canary = fail_canary("arithmetic/array_literal_let_init_narrowing_rejected");
    let diagnostics = check_canary(&canary)
        .expect_err("expected array-literal let-init narrowing canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("narrowing store") && combined.contains("array literal element"),
        "expected a narrowing diagnostic at the array-literal let-init boundary, got:\n{combined}"
    );
}

#[test]
fn let_init_class_mismatch_rejected_canary_is_rejected() {
    // A cross-class let-initializer (`let x: i32 = true`) is rejected. This was a
    // SILENT MISCOMPILE (the LocalData path had the narrowing check but not the
    // class check); now every scalar value-binding position class-checks.
    let canary = fail_canary("arithmetic/let_init_class_mismatch_rejected");
    let diagnostics =
        check_canary(&canary).expect_err("expected let-init class-mismatch canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("stores a boolean into a `i32` local"),
        "expected a clear cross-class let-init diagnostic, got:\n{combined}"
    );
}

#[test]
fn return_value_class_mismatch_rejected_canary_is_rejected() {
    // A cross-class transition-value return (`_ -> (true)` from an -> i32 machine)
    // is rejected. This was a SILENT MISCOMPILE -- the terminal `{ true }` form was
    // shape-gated, but the transition-value form was not class-checked.
    let canary = fail_canary("arithmetic/return_value_class_mismatch_rejected");
    let diagnostics =
        check_canary(&canary).expect_err("expected return-value class-mismatch canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("stores a boolean into a `i32` return value"),
        "expected a clear cross-class return-value diagnostic, got:\n{combined}"
    );
}

#[test]
fn terminal_return_class_mismatch_rejected_canary_is_rejected() {
    // A cross-class TERMINAL return of a PLACE (`machine -> i32 { self.bool_field }`)
    // is rejected. The shape gate rejects a cross-class literal but blanket-accepts
    // place values, so this was a SILENT MISCOMPILE; the terminal-return path now
    // class-checks place values (gated to avoid double-reporting a literal).
    let canary = fail_canary("arithmetic/terminal_return_class_mismatch_rejected");
    let diagnostics = check_canary(&canary)
        .expect_err("expected terminal-return class-mismatch canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("stores a boolean into a `i32` return value"),
        "expected a clear cross-class terminal-return diagnostic, got:\n{combined}"
    );
}

#[test]
fn wrong_struct_type_argument_rejected_canary_is_rejected() {
    // NOMINAL type confusion at a call argument (`take_foo(&self.bar)` for a `&Foo`
    // parameter) is rejected. This was a SILENT MISCOMPILE -- the shape gate
    // blanket-accepts a place against any Named parameter, comparing only shape not
    // the type name. The user-type complement of the cross-class scalar family.
    let canary = fail_canary("arithmetic/wrong_struct_type_argument_rejected");
    let diagnostics =
        check_canary(&canary).expect_err("expected wrong-struct-type-argument canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("expects the `Foo` data type but got `Bar`"),
        "expected a clear nominal-type-mismatch diagnostic, got:\n{combined}"
    );
}

#[test]
fn wrong_struct_type_assignment_rejected_canary_is_rejected() {
    // NOMINAL type confusion at an assignment (`self.foo = self.bar`, Bar into a
    // Foo place) is rejected -- a distinct position from the call-argument canary,
    // sharing report_data_type_conflict.
    let canary = fail_canary("arithmetic/wrong_struct_type_assignment_rejected");
    let diagnostics =
        check_canary(&canary).expect_err("expected wrong-struct-type-assignment canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("expects the `Foo` data type but got `Bar`"),
        "expected a clear nominal-type-mismatch diagnostic at the assignment, got:\n{combined}"
    );
}

#[test]
fn wrong_struct_type_array_element_rejected_canary_is_rejected() {
    // NOMINAL type confusion at an array-literal element (`[self.bar, ..]` into a
    // `[Foo; 2]`) is rejected -- the array element check now runs the nominal guard
    // for data element types, not only the scalar class/width check.
    let canary = fail_canary("arithmetic/wrong_struct_type_array_element_rejected");
    let diagnostics = check_canary(&canary)
        .expect_err("expected wrong-struct-type-array-element canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("expects the `Foo` data type but got `Bar`"),
        "expected a clear nominal-type-mismatch diagnostic at the array element, got:\n{combined}"
    );
}

#[test]
fn unknown_field_read_rejected_canary_is_rejected() {
    // A READ of a nonexistent field (a typo) in an expression is rejected at
    // type-check ("reads `self.cont`, but data X has no field `cont`"), not silently
    // passed.
    let canary = fail_canary("arithmetic/unknown_field_read_rejected");
    let diagnostics =
        check_canary(&canary).expect_err("expected unknown-field read canary to reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("has no field `cont`"),
        "expected a clear unknown-field diagnostic for the read, got:\n{combined}"
    );
}
