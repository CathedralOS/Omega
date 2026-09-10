//! Array transport preserves format and representation, not only floating meaning.
use super::*;
use semantic_vocabulary::IeeeFloatValue;

fn execute(source: &str, arguments: &[TerminalScalarValue]) -> Vec<TerminalScalarValue> {
    let checked = checked_source(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "selected")
        .unwrap_or_else(|error| panic!("floating array must lower: {error:?}\n{source}"));
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let TerminalExecutionResult::ScalarArray(result) =
        interpret_terminal_artifact(&semantic, &proof, &AdmissionProfile::default(), arguments)
            .expect("decoded floating array execution")
    else {
        panic!("actual array payload required");
    };
    result.value.elements
}

#[test]
fn floating_array_literals_calls_and_projections_execute_exact_bits() {
    for (carrier, value, zero) in [
        (
            "f32",
            IeeeFloatValue::Binary32(0x7fc01234),
            IeeeFloatValue::Binary32(0x80000000),
        ),
        (
            "f64",
            IeeeFloatValue::Binary64(0x7ff8000000001234),
            IeeeFloatValue::Binary64(0x8000000000000000),
        ),
    ] {
        let expected = [
            TerminalScalarValue::IeeeFloat(value),
            TerminalScalarValue::IeeeFloat(zero),
        ];
        for body in [
            format!("[value, -0.0{carrier}]"),
            format!("keep([identity(value), -0.0{carrier}])"),
            format!("let row: [{carrier}; 2] = keep([value, -0.0{carrier}]); keep(row)"),
        ] {
            let source = format!(
                "machine identity(value: {carrier}) -> {carrier} {{ value }}
                 machine keep(row: [{carrier}; 2]) -> [{carrier}; 2] {{ row }}
                 machine selected(value: {carrier}) -> [{carrier}; 2] {{ {body} }}"
            );
            assert_eq!(execute(&source, &expected[..1]), expected, "{source}");
        }
    }
    assert_eq!(
        execute(
            "data Rows {} const Rows::VALUES: [[f32; 2]; 2] = [[1.5f32, -2.25f32], [0.0f32, -0.0f32]];
             machine selected() -> [f32; 2] { Rows::VALUES[0] }", &[],
        ),
        [TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0x3fc00000)),
         TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(0xc0100000))]
    );
}

#[test]
fn floating_array_source_bits_and_format_cannot_be_substituted() {
    let original = checked_source("machine selected() -> [f32; 2] { [1.5f32, -0.0f32] }");
    checked_trees_to_lowered_psi::lower_machine(&original, "selected").unwrap();
    let (_, expressions) = selected_source(&original);
    let ExpressionNode::ArrayLiteral(elements) =
        original.expression_table.expression(expressions[0])
    else {
        panic!("authored array");
    };
    let leaves = original
        .expression_table
        .expression_handles(*elements)
        .to_vec();
    for (ordinal, literal) in [(0, "2.5f32"), (0, "1.5f64"), (1, "0.0f32")] {
        let carrier = if literal.ends_with("f32") {
            "f32"
        } else {
            "f64"
        };
        let replacement = checked_source(&format!(
            "machine selected() -> [{carrier}; 1] {{ [{literal}] }}"
        ));
        let replacement = replacement
            .expression_table
            .expression_entries()
            .find_map(|(_, node)| matches!(node, ExpressionNode::Float(_)).then_some(node.clone()))
            .unwrap();
        let mut changed = original.clone();
        *changed
            .typed
            .expression_table
            .expression_mut(leaves[ordinal]) = replacement;
        reject(&changed, literal);
    }
}

#[test]
fn projected_floating_array_cannot_erase_an_invalid_sibling_format() {
    let original = checked_source(
        "data Rows {} const Rows::VALUES: [[f32; 1]; 2] = [[1.5f32], [2.5f32]];
         machine selected() -> [f32; 1] { Rows::VALUES[0] }",
    );
    checked_trees_to_lowered_psi::lower_machine(&original, "selected").unwrap();
    let (_, expressions) = selected_source(&original);
    let sibling = row_elements(&original, expressions[0], 1)[0];
    let replacement = checked_source("machine selected() -> [f64; 1] { [2.5f64] }");
    let replacement = replacement
        .expression_table
        .expression_entries()
        .find_map(|(_, node)| matches!(node, ExpressionNode::Float(_)).then_some(node.clone()))
        .unwrap();
    let mut changed = original;
    *changed.typed.expression_table.expression_mut(sibling) = replacement;
    reject(&changed, "projected-away sibling format");
}

#[test]
fn floating_array_arithmetic_still_requires_selected_execution() {
    let checked = checked_source("machine selected(value: f32) -> [f32; 1] { [value + 1.0f32] }");
    reject(&checked, "unselected floating arithmetic");
}
