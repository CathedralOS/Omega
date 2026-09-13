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
fn array_reference_correspondence_preserves_elements_and_access() {
    for (actual, required, accepted) in [
        ("&mut [u8; 3]", "&mut [u8]", true),
        ("&mut [u8; 0]", "&mut [u8]", true),
        ("&[u8; 3]", "&[u8]", true),
        ("&mut [u8; 3]", "&[u8]", true),
        ("&mut [[u8; 2]; 3]", "&mut [[u8; 2]]", true),
        ("&mut [u8; 3]", "&mut [u8; 3]", true),
        ("&write [u8; 3]", "&write [u8; 3]", true),
        ("&mut [u8]", "&[u8]", true),
        ("&mut u32", "&u32", true),
        ("&[u8; 3]", "&mut [u8]", false),
        ("&mut [u16; 3]", "&mut [u8]", false),
        ("&mut [u16; 3]", "&[u8]", false),
        ("&mut [u8 [0..=127]; 3]", "&mut [u8]", false),
        ("&[u16; 3]", "&[u8]", false),
        ("&mut [[u8; 2]; 3]", "&mut [[u8; 3]]", false),
        ("&mut [u8; 3]", "&mut [u8; 2]", false),
        ("&mut u32", "&u64", false),
        ("&mut [u8; 3]", "&write [u8]", false),
        ("&write [u8; 3]", "&mut [u8]", false),
        ("&write [u8; 3]", "&[u8]", false),
        ("&write [u8; 3]", "&write [u8]", false),
    ] {
        for origin in ["named", "projected", "returned"] {
            let declarations = match origin {
                "projected" => format!(
                    "data Holder {{ body: {actual}; }}
                     machine inspect(value: Holder, expected: {required}) {{ value.body; }}"
                ),
                "returned" => format!(
                    "machine retain(input: {actual}) -> {actual} {{ input }}
                     machine inspect(value: {actual}, expected: {required}) {{
                         let returned: {actual} = retain(value); returned;
                     }}"
                ),
                _ => format!("machine inspect(value: {actual}, expected: {required}) {{ value; }}"),
            };
            let program = typed(&declarations);
            let expression = program
                .expression_table
                .iter_expressions()
                .find_map(|(handle, node)| {
                    let selected = match origin {
                        "projected" => matches!(node, ExpressionNode::Member(_)),
                        "returned" => matches!(node, ExpressionNode::Call(_)),
                        _ => {
                            matches!(node, ExpressionNode::Name(_))
                                && program.expression_table.display_name(handle) == "value"
                        }
                    };
                    selected.then_some(handle)
                })
                .unwrap_or_else(|| panic!("{origin}: authored {actual} reference expression"));
            assert_eq!(
                argument_matches_type_reference_handle(&program, expression, expected(&program)),
                accepted,
                "{origin}: {actual} to {required}"
            );
        }
    }
}

#[test]
fn named_shared_reference_inference_does_not_match_closed_nominals() {
    for access in ["&", "&mut ", "&write "] {
        let program = typed(&format!(
            "data Card {{}} data Other {{}}
             machine inspect<T>(value: {access}Card, expected: &T, closed: &Other) {{ value; }}"
        ));
        let parameters =
            program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
        let expression = program
            .expression_table
            .iter_expressions()
            .find_map(|(handle, node)| {
                (matches!(node, ExpressionNode::Name(_))
                    && program.expression_table.display_name(handle) == "value")
                    .then_some(handle)
            })
            .expect("named reference");
        assert_eq!(
            argument_matches_type_reference_handle(&program, expression, expected(&program)),
            access != "&write ",
        );
        assert!(!argument_matches_type_reference_handle(
            &program,
            expression,
            parameters[2].type_reference,
        ));
    }
}

#[test]
fn array_view_matching_cannot_grant_mutation_through_shared_storage() {
    let program = typed(
        "data Holder { bytes: &mut [u8; 3]; }
         machine inspect(value: &Holder, expected: &mut [u8]) { value.bytes; }",
    );
    let expression = member(&program, "value.bytes");
    let root = &program.state_parameters(&program.machine_states(&program.machines()[0])[0])[0];
    assert!(projected_matches_reference(
        &program,
        expression,
        expected(&program)
    ));
    assert!(!place_forwards_mutable_reference(
        &program,
        expression,
        root.symbol,
        root.type_reference,
    ));
}

#[test]
fn mutable_array_views_do_not_erase_carrier_constraints() {
    let mut program =
        typed("machine inspect(value: &mut [u8; 3], expected: &mut [u8], read: &[u8]) { value; }");
    let parameters = program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
    let actual = parameters[0].type_reference;
    let mutable_view = parameters[1].type_reference;
    let shared_view = parameters[2].type_reference;
    assert!(reference_type_matches(&program, actual, mutable_view, &[]));
    let mut reference = program.type_reference_table.type_reference(actual).clone();
    let TypeReferenceNode::Reference { referee, .. } = &mut reference else {
        panic!("mutable reference");
    };
    let constraints = program
        .type_reference_table
        .insert_constraints([TypeConstraintNode::Named("qualified".into())]);
    *referee = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: *referee,
            constraints,
        });
    program
        .type_reference_table
        .substitute_node(actual, reference);
    assert!(
        !reference_type_matches(&program, actual, mutable_view, &[]),
        "a shape match cannot authorize writes that break a carrier predicate"
    );
    assert!(
        reference_type_matches(&program, actual, shared_view, &[]),
        "a plain shared view does not alter the qualified carrier"
    );
}

#[test]
fn generic_array_views_compare_selected_elements_not_the_data_telescope() {
    let mut program = typed(
        "data Box<Element> { values: [Element; 2]; } machine inspect<Value>(value: Box<Value>, expected: &[Value]) { value.values; }",
    );
    let expression = member(&program, "value.values");
    assert!(projected_matches_reference(
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
        !projected_matches_reference(&program, expression, wrong),
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
            projected_matches_reference(
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
        !projected_matches_reference(
            &program,
            member(&program, "value.value.value"),
            expected(&program)
        ),
        "re-entry needs scoped argument views, not an overwritten flat binding"
    );
}

#[test]
fn indexed_reference_leaf_requires_builtin_meaning_and_exact_referee() {
    for (operator, accepted) in [
        ("", true),
        (
            "operator [] Indexing::index(values: &[u8; 2], index: u64) -> u8;",
            true,
        ),
        (
            "boundary operator [] Indexing::index(values: &[View], index: u64) -> View;",
            false,
        ),
    ] {
        let program = typed(&format!(
            "data View {{ body: &mut i32; }} data Indexing {{}} {operator}
             machine inspect(values: [View; 1], index: u64, expected: &mut i32) {{ values[index].body; }}"
        ));
        let expression = program
            .expression_table
            .iter_expressions()
            .find_map(|(handle, node)| matches!(node, ExpressionNode::Member(_)).then_some(handle))
            .expect("indexed member");
        assert_eq!(
            projected_matches_reference(&program, expression, expected(&program)),
            accepted,
            "{operator}"
        );
    }
    let mut program = typed(
        "machine inspect(values: [u32; 1], index: u64, expected: &mut i32) { values[index]; }",
    );
    let array = program.state_parameters(&program.machine_states(&program.machines()[0])[0])[0]
        .type_reference;
    let mut array_node = program.type_reference_table.type_reference(array).clone();
    let TypeReferenceNode::FixedArray { element_type, .. } = &mut array_node else {
        panic!("array");
    };
    *element_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: *element_type,
            access: ReferenceAccess::Mutable,
            lifetime: None,
        });
    program
        .type_reference_table
        .substitute_node(array, array_node);
    let expression = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| matches!(node, ExpressionNode::Indexed(_)).then_some(handle))
        .expect("index");
    assert!(!projected_matches_reference(
        &program,
        expression,
        expected(&program)
    ));
}

#[test]
fn forwarding_checks_prefix_permissions_separately_from_leaf_type() {
    for (prefix, accepted) in [
        ("View", true),
        ("&mut View", true),
        ("&View", false),
        ("&write View", false),
    ] {
        let program = typed(&format!(
            "data View {{ body: &mut i32; }} machine inspect(value: {prefix}, expected: &mut i32) {{ value.body; }}"
        ));
        let expression = member(&program, "value.body");
        let parameters =
            program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
        let root = parameters
            .iter()
            .find(|parameter| parameter.name.as_str() == "value")
            .expect("root");
        assert!(
            projected_matches_reference(&program, expression, expected(&program)),
            "type alone does not authorize forwarding"
        );
        assert_eq!(
            place_forwards_mutable_reference(
                &program,
                expression,
                root.symbol,
                root.type_reference
            ),
            accepted,
            "{prefix}"
        );
        let foreign_root = parameters
            .iter()
            .find(|parameter| parameter.name.as_str() == "expected")
            .expect("foreign root");
        assert!(!place_forwards_mutable_reference(
            &program,
            expression,
            foreign_root.symbol,
            root.type_reference
        ));
    }
}

#[test]
fn selected_field_rejects_foreign_nominal_member_identity() {
    let mut program = typed(
        "data View { body: &mut i32; } data Other { body: &mut i32; }
         machine inspect(value: View, expected: &mut i32) { value.body; }",
    );
    let expression = member(&program, "value.body");
    let foreign_symbol = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Other")
        .and_then(|data| {
            program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Field(field) => Some(field.symbol),
                    _ => None,
                })
        })
        .expect("foreign field");
    let mut forged = program.expression_table.expression(expression).clone();
    let ExpressionNode::Member(member) = &mut forged else {
        panic!("member");
    };
    member.member_symbol = foreign_symbol;
    let forged = program.expression_table.insert(forged);
    assert!(!projected_matches_reference(
        &program,
        forged,
        expected(&program)
    ));
}

#[test]
fn generic_index_cannot_select_builtin_meaning_from_unsubstituted_elements() {
    let program = typed(
        "data View { body: &mut i32; } data Box<Element> { values: [Element; 1]; }
         operator [] index(values: &[View], index: u64) -> View;
         machine inspect(boxed: Box<View>, index: u64, expected: &mut i32) { boxed.values[index].body; }",
    );
    let expression = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            matches!(node, ExpressionNode::Member(member) if member.member.as_str() == "body")
                .then_some(handle)
        })
        .expect("indexed body");
    assert!(!projected_matches_reference(
        &program,
        expression,
        expected(&program)
    ));
}
