use super::{Sources, compile, compile_to_checked_with_packages, root_inputs};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};
use terminal_psi::StructuralTypeShape;

pub(super) fn integer(value: u128, width: u16) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, width).expect("integer carrier"),
        value: IntegerValue::Unsigned(value),
    }
}

pub(super) fn assert_decoded_array(
    semantics: &[u8],
    proof: &[u8],
    arguments: &[TerminalScalarValue],
    dimensions: &[u64],
    expected: &[TerminalScalarValue],
    leaf: ScalarType,
) {
    let result = terminal_interpreter::interpret_terminal_artifact(
        semantics,
        proof,
        &proof_admission::AdmissionProfile::default(),
        arguments,
    )
    .expect("independently decoded array executes");
    let TerminalExecutionResult::ScalarArray(result) = result else {
        panic!("owned array payload, not structural identity alone: {result:?}");
    };
    assert_eq!(result.value.elements, expected);
    let module = terminal_codec::decode_module(semantics).expect("decode exact result type");
    let mut current = result.value.structural_type;
    for expected_length in dimensions {
        let declaration = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == current)
            .expect("retained dimension");
        let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
            panic!("array dimension");
        };
        assert_eq!(length, *expected_length);
        current = element;
    }
    assert_eq!(
        module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == current)
            .expect("retained primitive even below an empty dimension")
            .shape,
        StructuralTypeShape::PrimitiveScalar(leaf)
    );
}

#[test]
fn array_results_retain_module_selection_and_empty_dimensions_after_decode() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (module, first) in [("first", 7), ("second", 9)] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!(
                "module {module}; data Sizes {{}}
                 const Sizes::ROWS: [[u8; 2]; 1] = [[{first}, 2]];
                 const Sizes::EMPTY: [[u8; 0]; 1] = [[]];"
            ),
        );
    }
    Sources::write(
        root.join("main.omg"),
        "use first; use second;
         machine read() -> [u8; 2] { first::Sizes::ROWS[0] }
         machine other() -> [u8; 2] { second::Sizes::ROWS[0] }
         machine empty() -> [u8; 0] { first::Sizes::EMPTY[0] }
         machine anonymous() -> [u8; 2] { [7, 9] }
         machine nested() -> [[u8; 2]; 2] { [[1u8, 2u8], [3u8, 4u8]] }
         machine nested_empty() -> [[bool; 0]; 2] { [[], []] }
         machine outer_empty() -> [[u8; 3]; 0] { [] }
         machine flags() -> [bool; 2] { [true, false] }",
    );
    let checked = compile(&root, root_inputs(&root));
    let unsigned_byte = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let mut artifacts = Vec::new();
    for (name, dimensions, expected, leaf) in [
        (
            "read",
            vec![2],
            vec![integer(7, 8), integer(2, 8)],
            unsigned_byte,
        ),
        (
            "other",
            vec![2],
            vec![integer(9, 8), integer(2, 8)],
            unsigned_byte,
        ),
        ("empty", vec![0], vec![], unsigned_byte),
        (
            "anonymous",
            vec![2],
            vec![integer(7, 8), integer(9, 8)],
            unsigned_byte,
        ),
        (
            "nested",
            vec![2, 2],
            (1..=4).map(|value| integer(value, 8)).collect(),
            unsigned_byte,
        ),
        ("nested_empty", vec![2, 0], vec![], ScalarType::Boolean),
        ("outer_empty", vec![0, 3], vec![], unsigned_byte),
        (
            "flags",
            vec![2],
            vec![
                TerminalScalarValue::Boolean(true),
                TerminalScalarValue::Boolean(false),
            ],
            ScalarType::Boolean,
        ),
    ] {
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, name)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        artifacts.push((
            terminal_codec::encode_module(&lowered.semantic_module).expect("canonical semantics"),
            terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("canonical proof"),
            dimensions,
            expected,
            leaf,
        ));
    }
    drop(checked);
    for (semantics, proof, dimensions, expected, leaf) in artifacts {
        assert_decoded_array(&semantics, &proof, &[], &dimensions, &expected, leaf);
    }
}

#[test]
fn array_construction_composes_with_local_bindings_and_ordinary_calls() {
    let tree = Sources::new();
    let root = tree.package("root");
    for body in [
        "let values: [u8; 2] = [7, 9]; values",
        "touch(); let values: [u8; 2] = [7, 9]; touch(); values",
        "let mut count: u8 = 0; count = 1; let values: [u8; 2] = [7, 9]; count = 2; values",
        "let ignored: [u8; 2] = [1, 2]; let values: [u8; 2] = [7, 9]; values",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("machine touch() {{}} machine read() -> [u8; 2] {{ {body} }}"),
        );
        let checked = compile(&root, root_inputs(&root));
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "read")
            .unwrap_or_else(|error| panic!("{body}: {error:?}"));
        let semantics = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
        let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
        drop(checked);
        drop(lowered);
        assert_decoded_array(
            &semantics,
            &proof,
            &[],
            &[2],
            &[integer(7, 8), integer(9, 8)],
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        );
    }
}

#[test]
fn existing_module_array_customer_returns_real_arrays_from_portable_bytes() {
    let tree = Sources::new();
    let root = tree.package("root");
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tests/omega/pass/modules/module_array_constant_indices");
    for filename in ["main.omg", "settings.omg"] {
        Sources::write(
            root.join(filename),
            &std::fs::read_to_string(fixture.join(filename)).expect("existing customer source"),
        );
    }
    let checked = compile(&root, root_inputs(&root));
    let mut artifacts = Vec::new();
    for (name, arguments, width, values) in [
        ("selected_row", vec![], 8, vec![7, 9]),
        ("selected_empty_row", vec![], 8, vec![]),
        ("root_array", vec![], 64, vec![1, 2]),
        ("selected_array", vec![], 64, vec![2, 1]),
        ("computed_row", vec![integer(42, 8)], 8, vec![42, 9]),
        ("called_row", vec![integer(42, 8)], 8, vec![42, 9]),
        ("bound_row", vec![integer(42, 8)], 8, vec![42, 9]),
        ("passed_row", vec![integer(42, 8)], 8, vec![42, 9]),
    ] {
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, name)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        artifacts.push((
            terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
            terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap(),
            arguments,
            width,
            values,
        ));
    }
    drop(checked);
    for (semantics, proof, arguments, width, values) in artifacts {
        let elements = values
            .into_iter()
            .map(|value| integer(value, width))
            .collect::<Vec<_>>();
        assert_decoded_array(
            &semantics,
            &proof,
            &arguments,
            &[elements.len() as u64],
            &elements,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, width).unwrap()),
        );
    }
}

#[test]
fn computed_array_leaves_retain_parameters_arithmetic_and_local_snapshots() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "machine read(value: u8) -> [[u8; 2]; 2] {
            let mut current: u8 = value;
            let saved: u8 = current;
            current = 11;
            [[value, 7u8 + 2u8], [saved, current]]
         }",
    );
    let checked = compile(&root, root_inputs(&root));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "read").unwrap();
    let semantics = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    drop(checked);
    drop(lowered);
    for value in [0, 42, 255] {
        assert_decoded_array(
            &semantics,
            &proof,
            &[integer(value, 8)],
            &[2, 2],
            &[
                integer(value, 8),
                integer(9, 8),
                integer(value, 8),
                integer(11, 8),
            ],
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        );
    }
}

#[test]
fn array_leaf_calls_and_storage_reads_execute_in_source_order() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "machine stamp(value: &mut u64, number: u64) -> u64 { value = number; number }
         machine read(before: u64, after: u64) -> [u64; 5] {
            let mut slot: u64 = 201;
            [slot, stamp(&mut slot, before), slot, stamp(&mut slot, after), slot]
         }",
    );
    let checked = compile(&root, root_inputs(&root));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "read").unwrap();
    let semantics = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    drop(checked);
    drop(lowered);
    assert_decoded_array(
        &semantics,
        &proof,
        &[integer(7, 64), integer(9, 64)],
        &[5],
        &[201, 7, 7, 9, 9].map(|value| integer(value, 64)),
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
    );
}

#[test]
fn boolean_array_leaves_preserve_selective_calls_and_effect_order() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "machine stamp(value: &mut u64, number: u64) -> bool { value = number; true }
         machine read(enabled: bool) -> [bool; 4] {
            let mut slot: u64 = 201;
            [enabled && stamp(&mut slot, 7), slot == 7u64,
             enabled || stamp(&mut slot, 9), slot == 9u64]
         }",
    );
    let checked = compile(&root, root_inputs(&root));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "read").unwrap();
    let semantics = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    drop(checked);
    drop(lowered);
    for (enabled, expected) in [
        (false, [false, false, true, true]),
        (true, [true, true, true, false]),
    ] {
        assert_decoded_array(
            &semantics,
            &proof,
            &[TerminalScalarValue::Boolean(enabled)],
            &[4],
            &expected.map(TerminalScalarValue::Boolean),
            ScalarType::Boolean,
        );
    }
}

#[test]
fn computed_array_leaves_preserve_widening_and_exact_casts() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "machine identity(input: u8) -> u8 { input }
         machine read(input: u8) -> [u16; 4] {
            [input as u16, (input as u16) + 1u16, (7u8 + 2u8) as u16,
             (identity(input) as u16) + 1u16]
         }",
    );
    let checked = compile(&root, root_inputs(&root));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "read").unwrap();
    let semantics = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    drop(checked);
    drop(lowered);
    for input in [0, 42, 255] {
        assert_decoded_array(
            &semantics,
            &proof,
            &[integer(input, 8)],
            &[4],
            &[
                integer(input, 16),
                integer(input + 1, 16),
                integer(9, 16),
                integer(input + 1, 16),
            ],
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap()),
        );
    }
}

#[test]
fn computed_array_leaves_reject_implicit_integer_widening() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "machine read(input: u8) -> [u16; 1] { [input] }",
    );
    let Err(diagnostics) =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
    else {
        panic!("array destination cannot implicitly widen a typed scalar leaf");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "array literal element has type `u8`, expected `u16`; use an explicit conversion"
            )),
        "carrier mismatch is diagnosed: {diagnostics:?}"
    );
}

#[test]
fn computed_array_exact_narrowing_retains_its_proof_obligation() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "machine read(input: u16 [0..=255]) -> [u8; 1] { [input as u8] }",
    );
    let checked = compile(&root, root_inputs(&root));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "read").unwrap();
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::IntegerExactCast { .. }
            )),
        "partial conversion must retain an exact-cast operation"
    );
    let semantics = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    drop(checked);
    drop(lowered);
    for input in [0, 42, 255] {
        assert_decoded_array(
            &semantics,
            &proof,
            &[integer(input, 16)],
            &[1],
            &[integer(input, 8)],
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        );
    }
    assert!(
        terminal_interpreter::interpret_terminal_artifact(
            &semantics,
            &proof,
            &proof_admission::AdmissionProfile::default(),
            &[integer(256, 16)],
        )
        .is_err(),
        "input outside the exact-cast precondition must reject"
    );
}

#[test]
fn boolean_array_leaves_compose_nested_selection_and_constant_projection() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "data Sizes {}
         const Sizes::FLAGS: [bool; 2] = [true, false];
         machine read(left: bool, right: bool) -> [bool; 4] {
            [(left && right) || !left, left && (right || !left),
             Sizes::FLAGS[0] && left, Sizes::FLAGS[1] || right]
         }",
    );
    let checked = compile(&root, root_inputs(&root));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "read").unwrap();
    let semantics = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    drop(checked);
    drop(lowered);
    for (left, right, expected) in [
        (false, false, [true, false, false, false]),
        (false, true, [true, false, false, true]),
        (true, false, [false, false, true, false]),
        (true, true, [true, true, true, true]),
    ] {
        assert_decoded_array(
            &semantics,
            &proof,
            &[left, right].map(TerminalScalarValue::Boolean),
            &[4],
            &expected.map(TerminalScalarValue::Boolean),
            ScalarType::Boolean,
        );
    }
}
