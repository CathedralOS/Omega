use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, compile, compile_canary_without_output,
    compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host, executable_name,
    fail_canary, fs, interpret, pass_canary, unique_no_output_build_dir,
};
use compiler::CheckedCompileRequest;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use typed_trees::types::PrimitiveType;

#[test]
fn declared_range_inference_returns_the_selected_endpoint() {
    use build_time_evaluation::{
        BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeValue,
    };

    let canary = pass_canary("generics/declared_range_endpoint_inference");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("computed declared endpoints select closed calls");
    let admission = BuildTimeAdmissionPlan::infer(&checked.typed, None);
    for (name, arguments, expected) in [
        ("inferred", vec![BuildTimeValue::Int(0)], 256),
        ("computed", vec![BuildTimeValue::Int(0)], 256),
        ("full_width", vec![BuildTimeValue::Int(0)], 511),
        ("wide_intermediate", vec![BuildTimeValue::Int(0)], 256),
        ("computed_exclusive", vec![BuildTimeValue::Int(0)], 256),
        ("explicit", vec![BuildTimeValue::Int(0)], 512),
        ("fractional", vec![BuildTimeValue::Int(0)], 256),
        ("call_fractional", vec![], 256),
        ("named", vec![BuildTimeValue::Int(0)], 256),
        ("call_named", vec![], 256),
        ("named_exclusive", vec![BuildTimeValue::Int(0)], 255),
        ("named_full_width", vec![BuildTimeValue::Int(0)], -1),
        (
            "named_full_width_exclusive",
            vec![BuildTimeValue::Int(0)],
            -2,
        ),
        ("named_signed", vec![BuildTimeValue::Int(-2)], -2),
        ("scoped_named", vec![BuildTimeValue::Int(0)], 256),
        ("scoped_other", vec![BuildTimeValue::Int(0)], 511),
        ("free_capacity", vec![BuildTimeValue::Int(0)], 17),
        ("exclusive_full_u64", vec![BuildTimeValue::Int(0)], -1),
        ("inclusive_to_exclusive", vec![BuildTimeValue::Int(0)], 257),
        ("exclusive_to_inclusive", vec![BuildTimeValue::Int(0)], 256),
        ("typed_exclusive", vec![BuildTimeValue::Int(0)], 5),
        ("argument_named", vec![BuildTimeValue::Int(0)], 256),
        ("argument_fractional", vec![BuildTimeValue::Int(0)], 256),
        ("argument_full_width", vec![BuildTimeValue::Int(0)], -1),
        ("argument_scoped", vec![BuildTimeValue::Int(0)], 256),
        ("argument_order", vec![BuildTimeValue::Int(0)], 256),
        ("nested_argument", vec![BuildTimeValue::Int(0)], 256),
        ("nested_arithmetic", vec![BuildTimeValue::Int(0)], 256),
        ("surrounding_arithmetic", vec![BuildTimeValue::Int(0)], 256),
        ("nested_fractional", vec![BuildTimeValue::Int(0)], 256),
        ("nested_full_width", vec![BuildTimeValue::Int(0)], -1),
        ("surrounding_full_width", vec![BuildTimeValue::Int(0)], 511),
        ("constrained_argument", vec![BuildTimeValue::Int(0)], 256),
        ("constrained_result", vec![BuildTimeValue::Int(0)], 256),
        ("constrained_composition", vec![BuildTimeValue::Int(0)], 256),
        ("constrained_wide", vec![BuildTimeValue::Int(0)], -1),
        ("field_bound", vec![], 256),
        ("generic_field_bound", vec![], 256),
        ("generic_named_bound", vec![], 256),
        ("generic_call_bound", vec![], 256),
        ("generic_exclusive_call_bound", vec![], 255),
        ("generic_scoped_call_bound", vec![], 256),
        ("generic_argument_call_bound", vec![], 256),
        ("generic_nested_call_bound", vec![], 256),
        ("generic_signed_call_bound", vec![], -2),
        ("generic_free_capacity_bound", vec![], 17),
        ("generic_equivalent_bound", vec![], 256),
        ("generic_wide_bound", vec![], -1),
        ("generic_forwarded_bound", vec![], 256),
        ("field_scoped_exclusive", vec![], 511),
    ] {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| checked.symbols.display_path(machine.symbol, "::") == name)
            .expect("range consumer");
        let execution = admission
            .evaluate_machine_symbol_for_invocation_measured(
                &checked.typed,
                machine.symbol,
                arguments.clone(),
                BuildTimeInvocationCustody::Symbol(machine.symbol),
            )
            .expect("checked generic call executes");
        assert_eq!(execution.value(), &BuildTimeValue::Int(expected), "{name}");
        let state = &checked.machine_states(machine)[0];
        let integer = |bits: i64, reference| {
            let primitive = checked
                .primitive_type_reference(reference)
                .expect("integer carrier");
            // This fixture uses 64-bit carriers. BuildTimeValue retains unsigned
            // results as i64 bits, so the receiver must recover their carrier.
            assert!(matches!(primitive, PrimitiveType::I64 | PrimitiveType::U64));
            let signed = primitive.is_signed_integer();
            terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: semantic_vocabulary::IntegerType::new(
                    if signed {
                        semantic_vocabulary::IntegerSign::Signed
                    } else {
                        semantic_vocabulary::IntegerSign::Unsigned
                    },
                    64,
                )
                .unwrap(),
                value: if signed {
                    semantic_vocabulary::IntegerValue::Signed(i128::from(bits))
                } else {
                    semantic_vocabulary::IntegerValue::Unsigned(u128::from(bits as u64))
                },
            }
        };
        let terminal_arguments = arguments
            .iter()
            .zip(checked.state_parameters(state))
            .map(|(value, parameter)| {
                let BuildTimeValue::Int(bits) = value else {
                    panic!("integer test input");
                };
                integer(*bits, parameter.type_reference)
            })
            .collect::<Vec<_>>();
        let expected = integer(expected, state.return_type);
        let artifact = terminal_production::TerminalProductionRequest::new(&checked, name)
            .produce_artifact()
            .unwrap_or_else(|error| panic!("Terminal range consumer {name}: {error:?}"));
        // Only canonical bytes and fresh input values enter the receiver;
        // compile-time execution cannot stand in for this separate check.
        let execution = terminal_interpreter::interpret_terminal_artifact_measured(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &terminal_arguments,
            TerminalStructuralInputs::default(),
            &mut AcceptTerminalEffects,
        )
        .unwrap_or_else(|error| panic!("Terminal range execution {name}: {error:?}"));
        assert_eq!(
            execution.value(),
            terminal_interpreter::TerminalExecutionResult::Scalar(expected),
            "{name}"
        );
    }
}

#[test]
fn declared_range_inference_local_effects_retain_pending_terminal_boundaries() {
    // Attribute the remaining Terminal boundary without generic machinery.
    // Direct local writes execute below; borrowed calls still need their join.
    let scratch = unique_no_output_build_dir();
    fs::create_dir_all(&scratch).unwrap();
    let path = scratch.join("main.omg");
    fs::write(&path,
        "data Value [copy] { value: u64; } machine change(value: &mut Value) { value.value = 5; } machine borrowed_store() -> u64 { let mut first: Value = Value { value: 256 }; let second: Value = first; change(&mut first); second.value }",
    ).unwrap();
    let plain =
        compile_reviewed_repository_fixture(CheckedCompileRequest::new(&path, None)).unwrap();
    let plain = terminal_production::TerminalProductionRequest::new(&plain, "borrowed_store")
        .produce_artifact();
    assert!(
        matches!(
            plain,
            Err(terminal_production::TerminalArtifactProductionError::Lowering(_))
        ),
        "borrowed_store: {plain:?}"
    );
    assert!(
        format!("{plain:?}").contains("source-independent checked scalar control plan"),
        "borrowed_store: {plain:?}"
    );
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn declared_range_inference_record_copies_and_full_width_fields_execute() {
    let scratch = unique_no_output_build_dir();
    fs::create_dir_all(&scratch).unwrap();
    let path = scratch.join("main.omg");
    for (name, text, expected) in [
        (
            "local_store",
            "data Value [copy] { value: u64; } machine local_store() -> u64 { let mut first: Value = Value { value: 256 }; let second: Value = first; first.value = 5; second.value }",
            256_u128,
        ),
        (
            "moved_transition",
            "data Value { value: u64; } data Inner { value: u64; flag: bool; } data Outer { inner: Inner; } machine moved_transition() -> u64 { let keep: Value = Value { value: 17 }; let first: Outer = Outer { inner: Inner { value: 256, flag: true } }; let second: Outer = first; transition second.inner.flag { true -> yes(second.inner.value ^ keep.value) _ -> no() } state yes(value: u64) { value } state no() { 0 } }",
            273_u128,
        ),
        (
            "copied_transition",
            "data Value [copy] { value: u64; flag: bool; } data Outer [copy] { inner: Value; } machine copied_transition() -> u64 { let first: Outer = Outer { inner: Value { value: 256, flag: true } }; let second: Outer = first; transition second.inner.flag { true -> yes(second.inner.value) _ -> no() } state yes(value: u64) { value } state no() { 17 } }",
            256_u128,
        ),
        (
            "local_transition",
            "data Value [copy] { value: u64; flag: bool; } data Outer [copy] { inner: Value; } machine local_transition() -> u64 { let bounded: Outer = Outer { inner: Value { value: 256, flag: true } }; transition bounded.inner.flag { true -> yes(bounded.inner.value) _ -> no() } state yes(value: u64) { value } state no() { 0 } }",
            256_u128,
        ),
        (
            "local_transition_false",
            "data Value [copy] { value: u64; flag: bool; } data Outer [copy] { inner: Value; } machine local_transition_false() -> u64 { let bounded: Outer = Outer { inner: Value { value: 256, flag: false } }; transition bounded.inner.flag { true -> yes(bounded.inner.value) _ -> no() } state yes(value: u64) { value } state no() { 0 } }",
            0,
        ),
        (
            "copied",
            "data Value [copy] { value: u64; } machine copied() -> u64 { let first: Value = Value { value: 256 }; let second: Value = first; first.value ^ second.value }",
            0_u128,
        ),
        (
            "sequenced",
            "data Value [copy] { value: u64; } machine sequenced() -> u64 { let before: u64 = 7; let first: Value = Value { value: 256 }; let second: Value = first; let third: Value = second; before ^ first.value ^ second.value ^ third.value }",
            263,
        ),
        (
            "wide",
            "data Value [copy] { value: u64[0..18446744073709551616]; } machine wide() -> u64 { let bounded: Value = Value { value: 18446744073709551615 }; bounded.value }",
            u128::from(u64::MAX),
        ),
        (
            "survivor",
            "data Owned { value: u64; } data Copy [copy] { value: u64; } machine survivor() -> u64 { let keep: Owned = Owned { value: 17 }; let first: Copy = Copy { value: 256 }; let second: Copy = first; keep.value ^ first.value ^ second.value }",
            17,
        ),
        (
            "moved",
            "data Owned { value: u64; } machine moved() -> u64 { let keep: Owned = Owned { value: 17 }; let first: Owned = Owned { value: 256 }; let second: Owned = first; keep.value ^ second.value }",
            273,
        ),
        (
            "nested",
            "data Value [copy] { value: u64; } data Outer [copy] { inner: Value; } machine nested() -> u64 { let bounded: Outer = Outer { inner: Value { value: 256 } }; bounded.inner.value }",
            256,
        ),
        (
            "siblings",
            "data Value [copy] { value: u64; } data Outer [copy] { left: Value; right: Value; } machine siblings() -> u64 { let bounded: Outer = Outer { left: Value { value: 17 }, right: Value { value: 256 } }; bounded.left.value ^ bounded.right.value }",
            273,
        ),
        (
            "deep",
            "data Value [copy] { value: u64[0..18446744073709551616]; flag: bool; } data Middle [copy] { inner: Value; } data Outer [copy] { middle: Middle; } machine choose(flag: bool, value: u64) -> u64 { transition flag { true -> yes(value) _ -> no() } state yes(value: u64) { value } state no() { 0 } } machine deep() -> u64 { let bounded: Outer = Outer { middle: Middle { inner: Value { value: 18446744073709551615, flag: true } } }; choose(bounded.middle.inner.flag, bounded.middle.inner.value) }",
            u128::from(u64::MAX),
        ),
        (
            "nested_move",
            "data Value { value: u64; } data Outer { inner: Value; } machine nested_move() -> u64 { let first: Outer = Outer { inner: Value { value: 256 } }; let second: Outer = first; second.inner.value }",
            256,
        ),
    ] {
        fs::write(&path, text).unwrap();
        let checked =
            compile_reviewed_repository_fixture(CheckedCompileRequest::new(&path, None)).unwrap();
        let artifact = terminal_production::TerminalProductionRequest::new(&checked, name)
            .produce_artifact()
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let result = terminal_interpreter::interpret_terminal_artifact_measured(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs::default(),
            &mut AcceptTerminalEffects,
        )
        .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert_eq!(
            result.value(),
            terminal_interpreter::TerminalExecutionResult::Scalar(
                terminal_interpreter::TerminalScalarValue::Integer {
                    scalar_type: semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64
                    )
                    .unwrap(),
                    value: semantic_vocabulary::IntegerValue::Unsigned(expected),
                }
            ),
            "{name}"
        );
    }
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn declared_range_inference_computed_fields_preserve_establishment_checks() {
    let scratch = unique_no_output_build_dir();
    fs::create_dir_all(&scratch).unwrap();
    let main_path = scratch.join("main.omg");
    fs::write(
        &main_path,
        "machine endpoint(value: u64) -> u64 {value}
        data BoundedValue [copy] {value: u64[0..=endpoint(256)];}
        machine invalid() -> BoundedValue {BoundedValue {value: 257}}",
    )
    .unwrap();
    let diagnostics =
        compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
            .expect_err("computed endpoint does not waive field establishment");
    fs::remove_dir_all(&scratch).unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("range")),
        "{diagnostics:?}"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("bound is not a constant")),
        "{diagnostics:?}"
    );
}

#[test]
fn declared_range_inference_generic_arguments_preserve_field_and_identity_errors() {
    for body in [
        "let bounded: RangeValue<u64[0..=256]> = RangeValue { value: 257 }; bounded.value",
        "let first: RangeValue<u64[0..=256]> = RangeValue { value: 0 }; let second: RangeValue<u64[0..=257]> = first; second.value",
    ] {
        let scratch = unique_no_output_build_dir();
        fs::create_dir_all(&scratch).unwrap();
        let path = scratch.join("main.omg");
        fs::write(&path, format!("data RangeValue<T [copy]> [copy] {{ value: T; }} machine invalid() -> u64 {{ {body} }}")).unwrap();
        let diagnostics = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &path, None,
        ))
        .expect_err(
            "range specialization cannot waive field establishment or exact application identity",
        );
        fs::remove_dir_all(scratch).unwrap();
        assert!(!diagnostics.is_empty(), "{body}");
    }
}

#[test]
fn declared_range_inference_nested_results_keep_source_type_errors() {
    for (source, expected) in [
        (
            "machine small() -> u8 {255}
          machine endpoint(ignored: u64) -> u64 {256}
          machine bounded(value: u64[0..=endpoint(small())]) {}",
            "range endpoint argument",
        ),
        (
            "machine small() -> u8 {255}
          machine bounded(value: u64[0..=small() + 1]) {}",
            "range",
        ),
        (
            "machine small() -> u8 {255}
          machine endpoint(ignored: u8) -> u64 {256}
          machine bounded(value: u64[0..=endpoint(small() + 1)]) {}",
            "closed integer expression",
        ),
        (
            "data Limits {} machine Limits::capacity(&self) -> u64 {256}
          machine endpoint(ignored: u64) -> u64 {256}
          machine bounded(limits: Limits, value: u64[0..=endpoint(limits.capacity())]) {}",
            "closed integer expression",
        ),
        (
            "machine open<const N: u64>() -> u64 {256}
          machine endpoint(ignored: u64) -> u64 {256}
          machine bounded(value: u64[0..=endpoint(open())]) {}",
            "closed integer expression",
        ),
        (
            "machine endpoint(ignored: u64[1..=256]) -> u64 {256}
          machine bounded(value: u64[0..=endpoint(257)]) {}",
            "outside declared range",
        ),
        (
            "machine endpoint() -> u64[0..=256] {257}
          machine bounded(value: u64[0..=endpoint()]) {}",
            "outside declared range",
        ),
        (
            "machine endpoint(value: u64[0..0]) -> u64 {256}
          machine bounded(value: u64[0..=endpoint(0)]) {}",
            "outside declared range",
        ),
        (
            "machine endpoint() -> u8 {256}
          machine bounded(value: u64[0..=endpoint()]) {}",
            "integer",
        ),
    ] {
        let scratch = unique_no_output_build_dir();
        fs::create_dir_all(&scratch).unwrap();
        let main_path = scratch.join("main.omg");
        fs::write(&main_path, source).unwrap();
        let result =
            compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None));
        fs::remove_dir_all(&scratch).unwrap();
        let diagnostics = result.expect_err("folding cannot hide a source type error");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn canonical_range_call_endpoints_bind_omitted_binder_and_reject_conflicts() {
    // A declared endpoint call is a canonical range position like a literal:
    // `TinyBytes<u64[0..=limit()]>` binds `Capacity` to 256 on a data field and
    // inside a machine parameter, the exclusive arithmetic spelling selects the
    // same instance, and a nested callee chain folds through the shared
    // evaluator. An explicit argument must satisfy the equation exactly, and
    // an endpoint call that cannot close keeps the authored rejection.
    let scratch = unique_no_output_build_dir();
    fs::create_dir_all(&scratch).unwrap();
    let path = scratch.join("main.omg");
    let shared = "machine limit() -> u64[0..=300] { 256 }
        machine nested() -> u64[0..=400] { limit() + 100 }
        data TinyBytes<Length [copy], const Capacity: u64> [copy]
        where
            Length == u64[0..=Capacity]
        {
            storage: [u8; Capacity];
            length: Length;
        }";
    fs::write(
        &path,
        format!(
            "{shared}
             data Container {{
                 field: TinyBytes<u64[0..=limit()]>;
                 exclusive: TinyBytes<u64[0..limit() + 1]>;
                 chained: TinyBytes<u64[0..=nested()]>;
             }}
             machine measure(value: TinyBytes<u64[0..=limit()]>) -> u64 {{ value.length }}
             machine Main::main(&mut self) {{}}"
        ),
    )
    .unwrap();
    compile_reviewed_repository_fixture(CheckedCompileRequest::new(&path, None))
        .expect("declared call endpoints bind the omitted binder on a data template and a machine application");
    fs::write(
        &path,
        format!(
            "{shared}
             data Container {{ field: TinyBytes<u64[0..=limit()], 512>; }}
             machine Main::main(&mut self) {{}}"
        ),
    )
    .unwrap();
    let diagnostics = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&path, None))
        .expect_err("an explicit conflicting argument must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("Capacity")),
        "{diagnostics:?}"
    );
    fs::write(
        &path,
        "machine open<const N: u64>() -> u64 { 256 }
         data TinyBytes<Length [copy], const Capacity: u64> [copy]
         where
             Length == u64[0..=Capacity]
         {
             storage: [u8; Capacity];
             length: Length;
         }
         data Container { field: TinyBytes<u64[0..=open()]>; }
         machine Main::main(&mut self) {}",
    )
    .unwrap();
    let diagnostics = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&path, None))
        .expect_err("an unclosable endpoint call supplies no canonical endpoint");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("canonical range")),
        "{diagnostics:?}"
    );
    fs::remove_dir_all(&scratch).unwrap();
}

#[test]
fn bounded_integer_field_stores_run_natively() {
    let canary = pass_canary("borrows/bounded_integer_field_store");
    let scratch = unique_no_output_build_dir();
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .unwrap_or_else(|diagnostics| {
            panic!(
                "bounded stores must publish ({}): {diagnostics:?}",
                scratch.display()
            )
        });
    let executable = compilation
        .checked_native_executable_path()
        .expect("retain the bounded-store executable");
    let output = Command::new(executable)
        .output()
        .expect("execute bounded stores");
    assert_eq!(output.status.code(), Some(70), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    fs::remove_dir_all(&scratch).expect("remove successful native observations");
}

#[test]
fn declared_range_inference_hosted_entry_runs_natively() {
    let canary = pass_canary("generics/declared_range_endpoint_inference");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("checked hosted range caller");
    let _artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
        .produce_artifact()
        .expect("hosted caller retains its inferred call through Terminal production");
    let scratch = unique_no_output_build_dir();
    // Execute the authored entry with its real receiver and selected Console;
    // observing N in the evaluator alone does not establish native inference.
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .unwrap_or_else(|diagnostics| {
            panic!(
                "hosted range inference must publish ({}): {diagnostics:?}",
                scratch.display()
            )
        });
    let executable = compilation
        .checked_native_executable_path()
        .expect("retain the published range-inference executable");
    let output = Command::new(executable)
        .output()
        .expect("execute hosted range inference");
    assert_eq!(output.status.code(), Some(70), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    fs::remove_dir_all(&scratch).expect("remove successful native observations");
}

#[test]
fn runtime_decreases_u64_measure_exit_canary_runs() {
    // u64-typed termination measures verify like usize ones (the usize
    // retirement's stage-1 enabler; natural_measure_names_match).
    let canary = pass_canary(fixture_roster::RUNTIME_DECREASES_U64_MEASURE_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-decu64-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("u64 decreases canary should compile (termination must accept u64 measures)");
    let executable = compilation
        .checked_native_executable_path()
        .expect("u64 decreases canary should retain its executable receipt");
    let output = Command::new(executable)
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
    let canary = pass_canary(fixture_roster::RUNTIME_WRAPPING_OPERAND_TRUNCATION_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-wraptrunc-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("wrapping operand truncation canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("wrapping operand truncation canary should retain its executable receipt");
    let output = Command::new(executable)
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
    let canary = pass_canary(fixture_roster::RUNTIME_FLOAT_COMPARE_BOOL_EXIT);
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
    let canary = pass_canary(fixture_roster::AGGREGATE_TRANSITION_ARGS_EXIT);
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
    let canary = pass_canary(fixture_roster::DEEP_NESTED_WRITE_PATHS_EXIT);
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
    let canary = pass_canary(fixture_roster::ZII_DEFAULT_COMPOSITE_EXIT);
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
    let canary = pass_canary(fixture_roster::ZII_STRING_HOST_WRITE_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
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
    let canary = pass_canary(fixture_roster::ZII_DEFAULT_STRING_EQUALITY_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
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
    let canary = pass_canary(fixture_roster::RUNTIME_OWNED_STRING_BYTE_VIEW_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
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
    let canary = pass_canary(fixture_roster::EQUATABLE_SUM_STALE_PAYLOAD_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_TEXT_NOT_EQUALS_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
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
    let canary = pass_canary(fixture_roster::RUNTIME_TEXT_EQUALS_BOOLEAN_OPERAND_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
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
    let canary = pass_canary(fixture_roster::CASE_LITERAL_TEXTEQ_TERMINAL_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
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
    let canary = pass_canary(fixture_roster::CASE_LITERAL_TEXTEQ_FIELD_STORE_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
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
    let canary = pass_canary(fixture_roster::RUNTIME_TEXT_EQUALS_VALUE_POSITIONS_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
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
    let canary = pass_canary(fixture_roster::SUM_PAYLOAD_CAST_OPERAND_FIELD_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-sumcast-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("sum cast-operand payload canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("sum cast-operand payload canary should retain its executable receipt");
    let output = Command::new(executable)
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
    let canary = pass_canary(fixture_roster::RUNTIME_BRANCHING_CALLEE_CHAIN_EXIT);
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
    let canary = pass_canary(fixture_roster::RECURSIVE_RESULT_BIND_FIRST_ARG);
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
    let canary = pass_canary(fixture_roster::RUNTIME_RECURSIVE_RESULT_ROLES_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_TRAPPING_GUARD_OVERFLOW_TRAPS);
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
    let canary = pass_canary(fixture_roster::RUNTIME_TRAPPING_OVERFLOW_TRAPS);
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
    let canary = pass_canary(fixture_roster::RUNTIME_GUARD_PROVEN_COUNTER_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_GUARD_NARROWED_TRANSITION_ARG_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_GUI_WINDOW_LIFECYCLE_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-gui-life-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
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
    let canary = pass_canary(fixture_roster::RUNTIME_GUI_FOREGROUND_WINDOW_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("gui foreground-window canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (virtual foreground call + destroy), got {}",
        outcome.exit_code
    );

    let build_dir = std::env::temp_dir().join(format!("omega-gui-fg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
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
    let canary = pass_canary(fixture_roster::RUNTIME_GUI_WINDOW_BLIT_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-gui-wnd-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
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
    let canary = pass_canary(fixture_roster::RUNTIME_GENERIC_VALUE_CALL_AGREEING_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_GENERIC_VALUE_CALL_EXIT);
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
fn runtime_value_generic_subject_exit_canary_runs() {
    // Runtime-capable `Count: i32`/`K: i32` binders realize as one ordinary
    // trailing parameter on a shared dynamic body. Native execution observes
    // each captured subject: a literal-initialized local and the reassigned
    // source's current value reach the same specialization, `forward` passes
    // its own realized parameter, and `bounded` owes its `requires` obligation
    // on the captured subject, while the literal `prefix_count<4>` keeps its
    // closed specialization. The mutable source lives in `reassigned_source`
    // because native legalization does not yet admit a provider-attachment
    // receiver and a primitive local in one machine. Discriminating:
    // 3 + 5 + 5 + 4 = 17; any subject dropping or retagging its bound value
    // exits with a different code.
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_GENERIC_SUBJECT_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-rvg-subject-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime value generic subject canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime value generic canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime value generic subject canary should run");
    assert_eq!(
        output.status.code(),
        Some(17),
        "expected every captured subject to survive runtime binding (exit 17), got {:?}
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_generic_fail_canaries_reject() {
    // The rejection half of the runtime/value-binder contract: a proof-static
    // `const` binder never admits a runtime subject, a `requires` obligation
    // binds to the subject captured at the call rather than a stale earlier
    // value, and a runtime subject cannot determine a static layout extent.
    for &path in [
        fixture_roster::CONST_GENERIC_RUNTIME_ARGUMENT,
        fixture_roster::VALUE_GENERIC_RUNTIME_REQUIRES_UNPROVEN,
        fixture_roster::VALUE_GENERIC_RUNTIME_STATIC_LENGTH,
    ]
    .iter()
    {
        let canary = fail_canary(path);
        let expected = fs::read_to_string(canary.join("expected.txt"))
            .expect("runtime value generic fail canary should carry expected.txt");
        let diagnostics = compile_canary_without_output(&canary)
            .expect_err("runtime value binder misuse should be rejected");
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            combined.contains(expected.trim()),
            "{} missing expected fragment {:?}:\n{}",
            canary.display(),
            expected.trim(),
            combined
        );
    }
}

#[test]
fn trait_generic_bound_static_dispatch_canary_runs() {
    let canary = pass_canary(fixture_roster::TRAIT_GENERIC_BOUND_STATIC_DISPATCH);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
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
    let canary = pass_canary(fixture_roster::RUNTIME_GENERIC_PARAM_POSITION_INFERENCE_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
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
