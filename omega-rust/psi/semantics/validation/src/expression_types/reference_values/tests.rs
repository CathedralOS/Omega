use super::*;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolution");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("typing")
}

fn member(program: &TypedTrees, name: &str) -> ExpressionHandle {
    program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            (matches!(node, ExpressionNode::Member(_))
                && program.expression_table.display_name(handle) == name)
                .then_some(handle)
        })
        .expect("member expression")
}

fn expected(program: &TypedTrees) -> TypeReferenceHandle {
    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .flat_map(|state| program.state_parameters(state))
        .find(|parameter| parameter.name.as_str() == "expected")
        .expect("expected parameter")
        .type_reference
}

#[test]
fn generic_array_views_compare_selected_elements_not_the_data_telescope() {
    let mut program = typed(
        "data Box<Element> { values: [Element; 2]; } machine inspect<Value>(value: Box<Value>, expected: &[Value]) { value.values; }",
    );
    let expression = member(&program, "value.values");
    assert!(member_matches_reference(
        &program,
        expression,
        expected(&program)
    ));
    let mut substitutions = Vec::new();
    let actual =
        declared_value_type(&program, expression, &mut substitutions).expect("declared array");
    let TypeReferenceNode::FixedArray { element_type, .. } =
        *program.type_reference_table.type_reference(actual)
    else {
        panic!("array");
    };
    let slice = program
        .type_reference_table
        .insert(TypeReferenceNode::Slice { element_type });
    let wrong = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: slice,
            access: ReferenceAccess::Shared,
            lifetime: None,
        });
    assert!(
        !member_matches_reference(&program, expression, wrong),
        "the data declaration's Element is not the caller's Value"
    );
}

#[test]
fn substituted_reference_field_attenuates_mutable_only_to_shared() {
    for (actual_access, required, accepted) in [
        (ReferenceAccess::Mutable, "&Value", true),
        (ReferenceAccess::Shared, "&mut Value", false),
        (ReferenceAccess::WriteOnly, "&Value", false),
    ] {
        let mut program = typed(&format!(
            "data Box<Element> {{ value: Element; }} machine inspect<Value>(value: Box<Value>, expected: {required}) {{ value.value; }}"
        ));
        let receiver = program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine))
            .flat_map(|state| program.state_parameters(state))
            .find(|parameter| parameter.name.as_str() == "value")
            .expect("receiver")
            .type_reference;
        let mut receiver_node = program
            .type_reference_table
            .type_reference(receiver)
            .clone();
        let TypeReferenceNode::Generic { arguments, .. } = &mut receiver_node else {
            panic!("generic receiver");
        };
        let [referee] = program
            .type_reference_table
            .type_reference_handles(*arguments)
        else {
            panic!("one type argument");
        };
        let referee = *referee;
        // Reference arguments are representable typed inputs even though the
        // parser currently rejects an authored `Box<&Value>` application.
        let reference = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee,
                access: actual_access,
                lifetime: None,
            });
        *arguments = program
            .type_reference_table
            .insert_type_reference_handles([reference]);
        program
            .type_reference_table
            .substitute_node(receiver, receiver_node);
        assert_eq!(
            member_matches_reference(
                &program,
                member(&program, "value.value"),
                expected(&program)
            ),
            accepted
        );
    }
}

#[test]
fn repeated_generic_telescope_projection_remains_explicitly_unsupported() {
    let program = typed(
        "data Box<Element> { value: Element; } machine inspect<Value>(value: Box<Box<Value>>, expected: &Value) { value.value.value; }",
    );
    assert!(
        !member_matches_reference(
            &program,
            member(&program, "value.value.value"),
            expected(&program)
        ),
        "re-entry needs scoped argument views, not an overwritten flat binding"
    );
}
