use super::*;

#[test]
fn scalar_array_destinations_report_fractional_origins() {
    for source in [
        "machine make() { let values: [i32; 2] = [0.1 * 70, 7i32 / 2 * 2]; }",
        "machine make() { let values: [[i32; 1]; 1] = [[0.1 * 70]]; }",
        "machine make() { let mut values: [i32; 1] = [0]; values = [0.1 * 70]; }",
        "data Packet { values: [i32; 1]; } machine make() { let packet: Packet = Packet { values: [0.1 * 70] }; }",
        "machine accept(values: [i32; 1]) {} machine make() { accept([0.1 * 70]); }",
        "machine make() -> [i32; 1] { [0.1 * 70] }",
        "machine make() -> [i32; 1] { transition { _ -> [0.1 * 70] } }",
    ] {
        let warnings = anonymous_integer_landing_warnings(&typed(source));
        let [warning] = warnings.as_slice() else {
            panic!("{source}: {warnings:?}")
        };
        assert!(warning.message.contains("fractional intermediate `1/10`"));
        assert!(warning.message.contains("integer `7`"));
        let offset = source.find("0.1").expect("fractional element");
        assert_eq!(
            warning.source_span.expect("authored origin").span,
            source::Span::new(offset, offset + 3)
        );
    }
}

#[test]
fn array_width_grants_do_not_escape_to_shared_float_destinations() {
    let mut program = typed(&format!(
        "machine make() {{
        let integers: [i32; 2] = [{LARGE_ARGUMENT}, 7i32 / 2 * 2];
        let floats: [f64; 2] = [0.0, 0.0];
    }}"
    ));
    assert_eq!(width_grants(&program).len(), 2);
    let arrays: Vec<_> = program
        .expression_table
        .expression_entries()
        .filter_map(|(handle, node)| match node {
            ExpressionNode::ArrayLiteral(elements) => Some((handle, *elements)),
            _ => None,
        })
        .collect();
    assert_eq!(arrays.len(), 2);
    *program.expression_table.expression_mut(arrays[1].0) =
        ExpressionNode::ArrayLiteral(arrays[0].1);
    assert!(
        width_grants(&program).is_empty(),
        "the shared float elements retain their width obligations"
    );
}

#[test]
fn an_unsupported_array_parent_retains_its_element_width_obligations() {
    let mut program = typed(&format!(
        "machine make() {{ let integers: [i32; 1] = [{LARGE_ARGUMENT}]; }}"
    ));
    assert_eq!(width_grants(&program).len(), 2);
    let elements = program
        .expression_table
        .expression_entries()
        .find_map(|(_, node)| match node {
            ExpressionNode::ArrayLiteral(elements) => Some(*elements),
            _ => None,
        })
        .expect("typed array literal");
    program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(elements));
    assert!(
        width_grants(&program).is_empty(),
        "an untyped second parent cannot borrow another array's destination"
    );
}
