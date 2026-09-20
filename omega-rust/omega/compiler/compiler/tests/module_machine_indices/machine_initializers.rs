use super::{Sources, compile, root_inputs};
use semantic_vocabulary::IeeeFloatValue;
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "macos", target_arch = "aarch64")
))]
#[path = "../../../../../../tests/native-differential/tests/common/native_function.rs"]
#[allow(dead_code)]
mod native_function;

fn assert_source_free_result(checked: compiler::CheckedCompilation, machine: &str, expected: u64) {
    assert_source_free_scalar_result(
        checked,
        machine,
        super::array_construction::integer(u128::from(expected), 64),
    );
}

fn assert_source_free_scalar_result(
    checked: compiler::CheckedCompilation,
    machine: &str,
    expected: TerminalScalarValue,
) {
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, machine)
        .produce_artifact()
        .expect("machine-computed constant reaches Terminal");
    drop(checked);
    assert_eq!(
        interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .expect("machine-computed constant executes without checked source"),
        TerminalExecutionResult::Scalar(expected),
    );
}

#[test]
fn closed_generic_helper_constants_reach_source_free_execution() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings;
         machine identity<T>(value: T) -> T { value }
         pub const VALUE: u64 = identity<u64>(7);",
    );
    Sources::write(
        root.join("main.omg"),
        "use settings::VALUE; machine read() -> u64 { VALUE }",
    );
    assert_source_free_result(compile(&root, root_inputs(&root)), "read", 7);
}

#[test]
fn floating_helper_constants_preserve_their_declared_format_through_terminal() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (carrier, expected) in [
        ("f32", IeeeFloatValue::Binary32(0x3fc0_0000)),
        ("f64", IeeeFloatValue::Binary64(0x3ff8_0000_0000_0000)),
    ] {
        Sources::write(
            root.join("settings.omg"),
            &format!(
                "module settings; machine retain(value: {carrier}) -> {carrier} {{ value }}
                 pub const VALUE: {carrier} = retain(retain(1.5{carrier}));"
            ),
        );
        Sources::write(
            root.join("main.omg"),
            &format!("use settings::VALUE; machine read() -> {carrier} {{ VALUE }}"),
        );
        assert_source_free_scalar_result(
            compile(&root, root_inputs(&root)),
            "read",
            TerminalScalarValue::IeeeFloat(expected),
        );
    }
}

#[test]
fn imported_anonymous_float_constants_round_once_before_source_free_execution() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (carrier, initializer, expected) in [
        ("f32", "1.0 + 0.5", IeeeFloatValue::Binary32(0x3fc0_0000)),
        (
            "f64",
            "1.0 + 0.5",
            IeeeFloatValue::Binary64(0x3ff8_0000_0000_0000),
        ),
        (
            "f32",
            "(16777216 + 1) - 16777216",
            IeeeFloatValue::Binary32(0x3f80_0000),
        ),
        (
            "f64",
            "(9007199254740992 + 1) - 9007199254740992",
            IeeeFloatValue::Binary64(0x3ff0_0000_0000_0000),
        ),
        ("f32", "1 / 10", IeeeFloatValue::Binary32(0x3dcc_cccd)),
        ("f32", "1e100 + 0", IeeeFloatValue::Binary32(0x7f80_0000)),
        ("f32", "1e-45 + 0", IeeeFloatValue::Binary32(0x0000_0001)),
        (
            "f64",
            "1e400 + 0",
            IeeeFloatValue::Binary64(0x7ff0_0000_0000_0000),
        ),
        (
            "f32",
            "(match true { true -> 1 / 3, false -> 2 / 3 }) * 3",
            IeeeFloatValue::Binary32(0x3f80_0000),
        ),
        (
            "f64",
            "(match CHOOSE { true -> 1 / 3, false -> 2 / 3 }) * 3",
            IeeeFloatValue::Binary64(0x3ff0_0000_0000_0000),
        ),
        ("f32", "-1e-50 + 0", IeeeFloatValue::Binary32(0x8000_0000)),
        (
            "f32",
            "(match truth() { true -> 1 / 3, false -> 2 / 3 }) * 3",
            IeeeFloatValue::Binary32(0x3f80_0000),
        ),
        (
            "f64",
            "1 / 10",
            IeeeFloatValue::Binary64(0x3fb9_9999_9999_999a),
        ),
        (
            "f32",
            "8388609 + 0.499999999999999",
            IeeeFloatValue::Binary32(0x4b00_0001),
        ),
    ] {
        Sources::write(
            root.join("settings.omg"),
            &format!(
                "module settings; machine truth() -> bool {{ true }} const CHOOSE: bool = 2 > 1; pub const VALUE: {carrier} = {initializer};"
            ),
        );
        Sources::write(
            root.join("main.omg"),
            &format!("use settings::VALUE; machine read() -> {carrier} {{ VALUE }}"),
        );
        assert_source_free_scalar_result(
            compile(&root, root_inputs(&root)),
            "read",
            TerminalScalarValue::IeeeFloat(expected),
        );
    }
}

#[test]
fn imported_float_constant_initializers_reject_wrong_formats_and_anonymous_zero_division() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (carrier, initializer) in [
        ("f32", "1.0f64"),
        ("f64", "1.0f32"),
        ("f32", "1 / 0"),
        ("f64", "0.0 / 0.0"),
        ("f32", "1 / (2 - 2)"),
        ("f32", "match true { true -> 1, false -> 1 / 0 }"),
    ] {
        Sources::write(
            root.join("settings.omg"),
            &format!("module settings; pub const VALUE: {carrier} = {initializer};"),
        );
        Sources::write(
            root.join("main.omg"),
            &format!("use settings::VALUE; machine read() -> {carrier} {{ VALUE }}"),
        );
        let result = compiler::compile_to_checked(compiler::CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..compiler::CheckedCompileRequest::new(&root.join("main.omg"), None)
        });
        assert!(
            result.is_err(),
            "invalid floating initializer was accepted: {carrier} = {initializer}"
        );
    }
}

#[test]
fn unused_private_float_initializers_still_evaluate_and_validate() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "const UNUSED: f32 = 1 / 3; machine read() -> u64 { 7 }",
    );
    assert_source_free_result(compile(&root, root_inputs(&root)), "read", 7);
    Sources::write(
        root.join("main.omg"),
        "const UNUSED: f32 = 1 / 0; machine read() -> u64 { 7 }",
    );
    assert!(
        compiler::compile_to_checked(compiler::CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..compiler::CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .is_err()
    );
}

#[test]
fn machine_constant_customer_publishes_and_executes_without_source() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../tests/omega/pass/modules/machine_constant_initializers/main.omg"
        )),
    );
    assert_source_free_result(compile(&root, root_inputs(&root)), "read", 7);
}

#[test]
fn machine_constant_closure_preserves_module_selection_and_index_identity() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings;
        machine size() -> u64 { BASE }
        machine identity(value: u64) -> u64 { value }
        pub const SIZE: u64 = identity(size()) * 2;
        const BASE: u64 = 7 / 2 * 2;",
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use settings; {}
        machine size() -> u64 {{ 99 }}
        const BASE: u64 = 99;
        machine read() -> u64 {{ settings::SIZE }} {} {}",
            super::BUFFER,
            super::keep("keep", "settings::SIZE"),
            super::keep("oracle", "14")
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    super::assert_same_machine_types(&checked, "keep", "oracle");
    assert_source_free_result(checked, "read", 14);
}

#[test]
fn machine_calls_compose_with_nominal_projection_and_boolean_selection() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "data Config [copy] { size: u64; enabled: bool; }
        machine size() -> u64 { 7 }
        machine truth(value: bool) -> bool { value }
        const CONFIG: Config = Config { size: size() * 2, enabled: truth(size() == 7) };
        const SELECTED: u64 = match truth(true) { true -> size(), false -> size() * 2 };
        machine read() -> u64 { CONFIG.size + SELECTED }",
    );
    assert_source_free_result(compile(&root, root_inputs(&root)), "read", 21);
}

#[test]
fn machine_constant_record_retains_payloadless_case_siblings() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "data Mode [copy] { case On; case Off; }
        data Config [copy] { mode: Mode; count: u64; }
        machine size() -> u64 { 7 }
        const CONFIG: Config = Config { mode: Mode::On, count: size() };
        machine read() -> u64 { 7 }",
    );
    assert_source_free_result(compile(&root, root_inputs(&root)), "read", 7);
}

#[test]
fn concrete_machine_initializers_discharge_failure_routes_before_execution() {
    let tree = Sources::new();
    let root = tree.package("root");
    for argument in ["2", "1 + 1", "identity(1 + 1)"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "machine divide(value: u64) -> u64
                crashes Trap value == 0
                {{ transition {{ value != 0 -> 10 / value }} crash Trap; }}
                machine forward(value: u64) -> u64 {{ divide(value) }}
                machine identity(value: u64) -> u64 {{ value }}
                const SIZE: u64 = forward({argument});
                machine read() -> u64 {{ SIZE }} {} {} {}",
                super::BUFFER,
                super::keep("keep", "SIZE"),
                super::keep("oracle", "5"),
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        super::assert_same_machine_types(&checked, "keep", "oracle");
        assert_source_free_result(checked, "read", 5);
    }
}

#[test]
fn constant_helper_failure_discharge_preserves_widened_arguments() {
    let tree = Sources::new();
    let root = tree.package("root");
    for argument in ["value as u64", "(value as u16) as u64"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "machine divide(value: u64) -> u64
                 crashes Trap value == 0
                 {{ transition {{ value != 0 -> 10 / value }} crash Trap; }}
                 machine forward(value: u8) -> u64 {{ divide({argument}) }}
                 const SIZE: u64 = forward(2u8);
                 machine read() -> u64 {{ SIZE }}"
            ),
        );
        assert_source_free_result(compile(&root, root_inputs(&root)), "read", 5);
    }
}

#[test]
fn constant_helper_conversion_does_not_hide_a_reachable_trap() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (carrier, body, actual) in [
        ("u8", "divide(value as u64)", "0u8"),
        ("u64", "divide((value as u8 in Wrapping) as u64)", "256u64"),
        (
            "u8",
            "divide(((value as u16 in Wrapping) + 65535) as u64)",
            "1u8",
        ),
        (
            "u8",
            "_ = divide((value as u64 in Wrapping) + 65535);
             divide(((value as u16 in Wrapping) + 65535) as u64)",
            "1u8",
        ),
        (
            "u8",
            "_ = divide(((value as u16 in Wrapping) + 65535) as u64);
             divide((value as u64 in Wrapping) + 65535)",
            "1u8",
        ),
        (
            "u8",
            "_ = divide(((value as u16 in Saturating) + 65535) as u64);
             divide(((value as u16 in Wrapping) + 65535) as u64)",
            "1u8",
        ),
        (
            "u8",
            "_ = divide(((value as u16 in Wrapping) + 65535) as u64);
             divide(((value as u16 in Saturating) + 65535) as u64)",
            "1u8",
        ),
        (
            "u8",
            "let mut divisor: u8 = value; divisor = 0; divide(divisor as u64)",
            "2u8",
        ),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "machine divide(value: u64) -> u64
                 crashes Trap value == 0
                 {{ transition {{ value != 0 -> 10 / value }} crash Trap; }}
                 machine forward(value: {carrier}) -> u64 {{ {body} }}
                 const UNUSED: u64 = forward({actual});
                 machine read() -> u64 {{ 7 }}"
            ),
        );
        let diagnostics = compiler::compile_to_checked(compiler::CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..compiler::CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err("even an unused initializer must discharge the actual conversion's trap");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("constant invocation")
                    && diagnostic.message.contains("unhandled [Trap]")
            }),
            "{body} at {actual}: {diagnostics:#?}"
        );
    }
}

#[test]
fn widened_constant_helper_keeps_the_operands_arithmetic_width() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "machine divide(value: u64) -> u64
         crashes Trap value == 0
         { transition { value != 0 -> 10 / value } crash Trap; }
         machine forward(value: u8) -> u64 {
             divide(((value as u16 in Wrapping) + 65535) as u64)
         }
         const SIZE: u64 = forward(2u8);
         machine read() -> u64 { SIZE }",
    );
    assert_source_free_result(compile(&root, root_inputs(&root)), "read", 10);
}

#[test]
fn widened_constant_helper_executes_natively_after_source_removal() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings;
         machine divide(value: u64) -> u64
         crashes Trap value == 0
         { transition { value != 0 -> 10 / value } crash Trap; }
         machine forward(value: u8) -> u64 {
             let divisor: u16 = value as u16;
             divide(divisor as u64)
         }
         pub const VALUE: u64 = forward(2u8);",
    );
    Sources::write(
        root.join("main.omg"),
        "use settings; machine read() -> u64 { settings::VALUE }",
    );
    let checked = compile(&root, root_inputs(&root));
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read")
        .produce_artifact()
        .expect("checked constant reaches Terminal");
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
        .expect("reload independent Terminal artifact");
    drop(checked);
    drop(tree);
    assert!(!root.exists(), "native publication cannot reread source");

    let selections = optimization_core::OptimizationSelections::new([]).unwrap();
    let optimized = native_realization::optimize_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        native_realization::compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    let physical =
        native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target::NativeTarget::host(),
            &[],
        )
        .unwrap();
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap();
    let framed = machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
    let text = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    let image = image_emission::emit_executable_image(&object, 0).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    native_function::assert_c_text(
        &image.output().final_text_bytes,
        object.entry_function().text_offset,
        "#include <stdint.h>\nextern uint64_t omega_entry(void);\nint main(void) { return omega_entry() == 5 ? 0 : 1; }",
    );
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!("SKIP: native constant execution requires Linux x64/ARM64 or macOS ARM64");
}

#[test]
fn invalid_unused_machine_constant_initializers_reject() {
    let tree = Sources::new();
    let root = tree.package("root");
    for source in [
        "machine narrow() -> u8 { 7 } const SIZE: u64 = narrow();",
        "machine wide(value: u64) -> u64 { value } const SIZE: u64 = wide(7u8);",
        "machine size() -> u64 { SIZE } const SIZE: u64 = size();",
        "machine size() -> u64 { size() } const SIZE: u64 = size();",
        "machine divide(value: u64) -> u64 { 10 / value } const SIZE: u64 = divide(0);",
    ] {
        Sources::write(root.join("main.omg"), source);
        let result = compiler::compile_to_checked(compiler::CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..compiler::CheckedCompileRequest::new(&root.join("main.omg"), None)
        });
        assert!(
            result.is_err(),
            "invalid unused initializer was accepted: {source}"
        );
    }
}
