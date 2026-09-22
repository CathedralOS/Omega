use super::{Sources, compile, root_inputs};
use semantic_vocabulary::IeeeFloatValue;
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
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
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name(machine),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("machine-computed constant reaches Terminal")
    .into_artifact();
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
fn indexed_constant_helper_discharge_reaches_source_free_execution() {
    for body in [
        "let values: [u64; 2] = [0, value]; divide(values[1])",
        "let values: [[u64; 2]; 1] = [[0, value]];
         let alias: [[u64; 2]; 1] = values;
         let unrelated: u64 = 7;
         divide(alias[0][1])",
    ] {
        let tree = Sources::new();
        let root = tree.package("root");
        Sources::write(
            root.join("main.omg"),
            &format!(
                "machine divide(value: u64) -> u64
             crashes Trap value == 0
             {{ transition {{ value != 0 -> 10 / value }} crash Trap; }}
             machine forward(value: u64) -> u64 {{ {body} }}
             const SIZE: u64 = forward(2);
             machine read() -> u64 {{ SIZE }}"
            ),
        );
        assert_native_result_after_source_removal(tree, root, &[target::NativeTarget::host()], &[]);
    }
}

#[test]
fn computed_table_selector_discharge_reaches_source_free_native_execution() {
    // The full u8 coordinate range fits this table, keeping index-bounds
    // admission independent of whether the evaluator preserves wrapping.
    let table_elements = (0..256)
        .map(|element_index| if element_index == 1 { "value" } else { "0" })
        .collect::<Vec<_>>()
        .join(", ");
    for body in [
        "let values: [u64; 2] = [0, value]; divide(values[1u64 + 0u64])",
        "let values: [u64; 2] = [0, value]; let selector: u64 = 1; divide(values[selector])",
        "let values: [u64; 256] = [TABLE];
         let selector: u8 in Wrapping = (255 as u8 in Wrapping) + 2u8;
         divide(values[selector])",
        "let values: [[u64; 2]; 1] = [[0, value]];
         let selector: u64 = 0;
         let row: [u64; 2] = values[selector];
         divide(row[1u64 + 0u64])",
        "let values: [[u64; 2]; 2] = [[0, value], [value, 0]];
         let mut selector: u64 = 0;
         let row: [u64; 2] = values[selector];
         selector = 1;
         divide(row[1u64 + 0u64])",
    ] {
        let body = body.replace("TABLE", &table_elements);
        let tree = Sources::new();
        let root = tree.package("root");
        Sources::write(
            root.join("main.omg"),
            &format!(
                "machine divide(value: u64) -> u64
                 crashes Trap value == 0
                 {{ transition {{ value != 0 -> 10 / value }} crash Trap; }}
                 machine forward(value: u64) -> u64 {{ {body} }}
                 const SIZE: u64 = forward(2);
                 machine read() -> u64 {{ SIZE }}"
            ),
        );
        assert_native_result_after_source_removal(tree, root, &[target::NativeTarget::host()], &[]);
    }
}

#[test]
fn computed_table_selectors_preserve_failures_and_saved_index_values() {
    let table_elements = (0..256)
        .map(|element_index| if element_index == 1 { "value" } else { "0" })
        .collect::<Vec<_>>()
        .join(", ");
    for body in [
        "let values: [u64; 2] = [0, value]; divide(values[0u64 + 0u64])",
        "let values: [u64; 2] = [0, value]; let mut selector: u64 = 1;
         selector = 0; divide(values[selector])",
        "let mut values: [u64; 2] = [0, value]; values[1] = 0;
         divide(values[1u64 + 0u64])",
        "let values: [u64; 256] = [TABLE];
         let selector: u8 in Wrapping = (255 as u8 in Wrapping) + 1u8;
         divide(values[selector])",
        "let values: [[u64; 2]; 2] = [[0, 0], [0, value]];
         let mut selector: u64 = 0;
         let row: [u64; 2] = values[selector];
         selector = 1;
         divide(row[1u64 + 0u64])",
        "let values: [u64; 2] = [divide(0), value]; divide(values[1u64 + 0u64])",
    ] {
        let body = body.replace("TABLE", &table_elements);
        let tree = Sources::new();
        let root = tree.package("root");
        Sources::write(
            root.join("main.omg"),
            &format!(
                "machine divide(value: u64) -> u64
                 crashes Trap value == 0
                 {{ transition {{ value != 0 -> 10 / value }} crash Trap; }}
                 machine forward(value: u64) -> u64 {{ {body} }}
                 const UNUSED: u64 = forward(2);
                 machine read() -> u64 {{ 7 }}"
            ),
        );
        let diagnostics = compiler::compile_to_checked(compiler::CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..compiler::CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err("a selected zero or failed sibling still prevents constant evaluation");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("constant invocation")
                    && diagnostic.message.contains("unhandled [Trap]")
            }),
            "{body}: {diagnostics:#?}"
        );
    }
}

#[test]
fn indexed_constant_helper_keeps_zero_and_mutated_values_rejected() {
    for (body, actual) in [
        ("let values: [u64; 2] = [0, value]; divide(values[1])", "0"),
        ("let values: [u64; 2] = [0, value]; divide(values[0])", "2"),
        (
            "let mut values: [u64; 2] = [0, value]; values[1] = 0; divide(values[1])",
            "2",
        ),
        (
            "let values: [u64; 2] = [divide(0), value]; divide(values[1])",
            "2",
        ),
    ] {
        let tree = Sources::new();
        let root = tree.package("root");
        Sources::write(
            root.join("main.omg"),
            &format!(
                "machine divide(value: u64) -> u64
             crashes Trap value == 0
             {{ transition {{ value != 0 -> 10 / value }} crash Trap; }}
             machine forward(value: u64) -> u64 {{ {body} }}
             const SIZE: u64 = forward({actual});
             machine read() -> u64 {{ SIZE }}"
            ),
        );
        let diagnostics = compiler::compile_to_checked(compiler::CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..compiler::CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err("projection cannot discharge a real or unproven crash");
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
    assert_native_result_after_source_removal(tree, root, &[target::NativeTarget::host()], &[]);
}

#[test]
fn constant_helper_preconditions_reach_native_execution_after_source_removal() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings;
         machine divide(value: u64) -> u64 requires value != 0 { 10 / value }
         machine forward(value: u64) -> u64 requires value != 0 {
             divide(value)
         }
         pub const VALUE: u64 = forward(2);",
    );
    Sources::write(
        root.join("main.omg"),
        "use settings; machine read() -> u64 { settings::VALUE }",
    );
    assert_native_result_after_source_removal(tree, root, &[target::NativeTarget::host()], &[]);
}

#[test]
fn widened_helper_preconditions_reach_native_execution_after_source_removal() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings;
         machine divide(value: u64) -> u64 requires value != 0 { 10 / value }
         machine forward(value: u8) -> u64 requires value != 0 {
             divide(value as u64)
         }
         pub const VALUE: u64 = forward(2u8);",
    );
    Sources::write(
        root.join("main.omg"),
        "use settings; machine read() -> u64 { settings::VALUE }",
    );
    assert_native_result_after_source_removal(tree, root, &[target::NativeTarget::host()], &[]);
}

#[test]
fn widened_runtime_call_preconditions_execute_after_source_removal() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "machine divide(value: u64) -> u64 requires value != 0 { 10 / value }
         machine forward(value: u8) -> u64 requires value != 0 {
             divide(value as u64)
         }
         machine read() -> u64 { forward(2u8) }",
    );
    assert_native_result_after_source_removal(tree, root, &[target::NativeTarget::host()], &[]);
}

#[test]
fn signed_nonzero_call_preconditions_execute_after_source_removal() {
    for (source_type, argument, quotient) in [
        ("i8", "-2i8", "-5"),
        ("i8", "2i8", "5"),
        ("i16", "-2i16", "-5"),
        ("i16", "2i16", "5"),
        ("i32", "-2i32", "-5"),
        ("i32", "2i32", "5"),
    ] {
        let tree = Sources::new();
        let root = tree.package("root");
        Sources::write(
            root.join("main.omg"),
            &format!(
                "machine divide(value: i64) -> i64 requires value != 0 {{ 10 / value }}
                 machine forward(value: {source_type}) -> i64 requires value != 0 {{
                     divide(value as i64)
                 }}
                 machine read() -> u64 {{
                     transition {{
                         forward({argument}) == {quotient} -> 5
                         _ -> 0
                     }}
                 }}"
            ),
        );
        assert_native_result_after_source_removal(tree, root, &native_arithmetic_targets(), &[]);
    }
}

fn native_arithmetic_targets() -> [target::NativeTarget; 4] {
    [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ]
}

#[test]
fn exact_native_division_and_remainder_execute_after_source_removal() {
    for bits in [8, 16, 32, 64] {
        for (sign, argument, divisor, quotient, remainder) in [
            ("i", "-7", "2", "-3", "-1"),
            ("i", "7", "-2", "-3", "1"),
            ("u", "7", "2", "3", "1"),
        ] {
            for (operator, expected) in [("/", quotient), ("%", remainder)] {
                let tree = Sources::new();
                let root = tree.package("root");
                Sources::write(
                    root.join("main.omg"),
                    &format!(
                        "machine calculate(value: {sign}{bits}) -> {sign}{bits} {{ value {operator} {divisor} }}
                         machine read() -> u64 {{ transition {{
                             calculate({argument}{sign}{bits}) == {expected} -> 5
                             _ -> 0
                         }} }}"
                    ),
                );
                assert_native_result_after_source_removal(
                    tree,
                    root,
                    &native_arithmetic_targets(),
                    &[],
                );
            }
        }
    }
}

#[test]
fn exact_native_division_rejects_zero_and_unrepresentable_quotients() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (bits, minimum) in [
        (8, "-128"),
        (16, "-32768"),
        (32, "-2147483648"),
        (64, "-9223372036854775808"),
    ] {
        for operator in ["/", "%"] {
            for expression in [
                format!("({minimum}i{bits}) {operator} (-1i{bits})"),
                format!("(7i{bits}) {operator} (0i{bits})"),
            ] {
                Sources::write(
                    root.join("main.omg"),
                    &format!("machine read() -> i{bits} {{ {expression} }}"),
                );
                assert_exact_arithmetic_rejected(&root, &expression);
            }
            // A nonzero divisor does not exclude the unrepresentable MIN/-1 quotient.
            Sources::write(
                root.join("main.omg"),
                &format!(
                    "machine calculate(value: i{bits}) -> i{bits} requires value != 0 {{ ({minimum}i{bits}) {operator} value }}
                     machine read() -> i{bits} {{ calculate(2i{bits}) }}"
                ),
            );
            assert_exact_arithmetic_rejected(
                &root,
                &format!("i{bits} {operator}: nonzero is not quotient representability"),
            );
        }
    }
}

fn assert_exact_arithmetic_rejected(root: &std::path::Path, context: &str) {
    // Runtime divide/remainder formation is discharged during Terminal
    // production, not necessarily by the checked-tree API. Neither stage may
    // publish executable custody for zero or an unrepresentable quotient.
    match compiler::compile_to_checked(compiler::CheckedCompileRequest::new(
        &root.join("main.omg"),
        None,
    )) {
        Err(diagnostics) => assert!(!diagnostics.is_empty(), "{context}"),
        Ok(checked) => {
            let result = terminal_production::TerminalProductionRequest::new(
                &checked,
                TerminalMachineSelection::Name("read"),
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ))
            .map(|produced| produced.into_artifact());
            assert!(
                result.is_err(),
                "{context}: invalid Exact arithmetic published {result:?}"
            );
        }
    }
}

#[test]
fn exact_remainder_minus_one_executes_with_selected_rule_enabled() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "machine remainder(value: i64) -> i64 requires value >= -10 && value <= 10 { value % -1 }
         machine read() -> u64 { transition { remainder(-7i64) == 0 -> 5 _ -> 0 } }",
    );
    assert_native_result_after_source_removal(tree, root, &native_arithmetic_targets(),
        &[optimization_core::Optimization::SelectedIncomingWrappingRemainderMinusOneZeroMaterialization]);
}

#[test]
fn constant_helper_preconditions_reject_false_concrete_invocations() {
    let tree = Sources::new();
    let root = tree.package("root");
    // The body itself returns successfully even at zero. Only checking the
    // invocation's authored premise can reject this unused initializer.
    Sources::write(
        root.join("main.omg"),
        "machine five(value: u64) -> u64 requires value != 0 { 5 }
         const UNUSED: u64 = five(0);
         machine read() -> u64 { 5 }",
    );
    let diagnostics = compiler::compile_to_checked(compiler::CheckedCompileRequest::new(
        &root.join("main.omg"),
        None,
    ))
    .expect_err("successful evaluation cannot discharge a false authored premise");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("violates required fact `value != 0`")
                && diagnostic.message.contains("structurally false")
        }),
        "{diagnostics:?}"
    );
}

fn assert_native_result_after_source_removal(
    tree: Sources,
    root: std::path::PathBuf,
    targets: &[target::NativeTarget],
    optimizations: &[optimization_core::Optimization],
) {
    let checked = compile(&root, root_inputs(&root));
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("read"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("checked constant reaches Terminal")
    .into_artifact();
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
        .expect("reload independent Terminal artifact");
    drop(checked);
    drop(tree);
    assert!(!root.exists(), "native publication cannot reread source");

    assert_eq!(
        interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .expect("verified artifact executes without source"),
        TerminalExecutionResult::Scalar(super::array_construction::integer(5, 64)),
    );

    let selections =
        optimization_core::OptimizationSelections::new(optimizations.iter().copied()).unwrap();
    for native in targets {
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
            *native,
            &[],
        )
        .unwrap();
        let emission_source = physical.into_function_fragment_emission_source();
        let fragments =
            machine_emission::stage_optimized_function_fragment_emission(emission_source).unwrap();
        let framed =
            machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
        let text = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
        let source = std::sync::Arc::new(
            object_file::stage_optimized_relocation_free_object_container(text).unwrap(),
        );
        let object =
            image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
        image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
        let image = image_emission::emit_direct_executable_image(&object, 0).unwrap();
        image_emission::validate_direct_executable_image(&object, &image).unwrap();
        #[cfg(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        ))]
        if *native == target::NativeTarget::host() {
            native_function::assert_c_text(
                &image.output().final_text_bytes,
                object.entry_function().text_offset,
                "#include <stdint.h>\nextern uint64_t omega_entry(void);\nint main(void) { return omega_entry() == 5 ? 0 : 1; }",
            );
        }
        #[cfg(not(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(target_os = "macos", target_arch = "aarch64")
        )))]
        eprintln!("SKIP: native constant execution requires Linux x64/ARM64 or macOS ARM64");
    }
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
