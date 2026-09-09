use super::{Sources, compile, root_inputs};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};
use terminal_psi::StructuralTypeShape;

fn integer(value: u128, width: u16) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, width).expect("integer carrier"),
        value: IntegerValue::Unsigned(value),
    }
}

fn assert_decoded_array(
    semantics: &[u8],
    proof: &[u8],
    dimensions: &[u64],
    expected: &[TerminalScalarValue],
    leaf: ScalarType,
) {
    let result = terminal_interpreter::interpret_terminal_artifact(
        semantics,
        proof,
        &proof_admission::AdmissionProfile::default(),
        &[],
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
        assert_decoded_array(&semantics, &proof, &dimensions, &expected, leaf);
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
    for (name, width, values) in [
        ("selected_row", 8, vec![7, 9]),
        ("selected_empty_row", 8, vec![]),
        ("root_array", 64, vec![1, 2]),
        ("selected_array", 64, vec![2, 1]),
    ] {
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, name)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        artifacts.push((
            terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
            terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap(),
            width,
            values,
        ));
    }
    drop(checked);
    for (semantics, proof, width, values) in artifacts {
        let elements = values
            .into_iter()
            .map(|value| integer(value, width))
            .collect::<Vec<_>>();
        assert_decoded_array(
            &semantics,
            &proof,
            &[elements.len() as u64],
            &elements,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, width).unwrap()),
        );
    }
}
