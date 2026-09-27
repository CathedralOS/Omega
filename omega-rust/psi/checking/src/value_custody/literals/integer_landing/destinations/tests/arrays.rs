use super::{ExpressionNode, LARGE_ARGUMENT, typed, width_grants};

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
