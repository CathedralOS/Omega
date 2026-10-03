use super::super::{
    TypeConstraintNode, argument_matches_carrier, argument_matches_carrier_with_substitutions,
    argument_matches_type_reference_handle,
};
use super::{
    ExpressionHandle, ExpressionNode, ReferenceAccess, TypeReferenceHandle, TypeReferenceNode,
    TypedTrees, declared_value_matches_reference, place_forwards_mutable_reference,
    projected_matches_reference, reference_type_matches, value_carrier_matches_reference,
};
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
            let program = crate::front_end::typed_program(&declarations);
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
            assert_eq!(
                argument_matches_carrier(&program, expression, expected(&program)),
                accepted,
                "call carrier {origin}: {actual} to {required}"
            );
        }
    }
}

#[test]
fn named_shared_reference_requires_exact_call_type_correspondence() {
    for access in ["&", "&mut ", "&write "] {
        let program = crate::front_end::typed_program(&format!(
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
        assert!(!argument_matches_type_reference_handle(
            &program,
            expression,
            expected(&program)
        ));
        assert!(!argument_matches_carrier(
            &program,
            expression,
            expected(&program)
        ));
        let formal = program.machine_type_parameters(&program.machines()[0])[0].symbol;
        let TypeReferenceNode::Reference { referee: card, .. } = program
            .type_reference_table
            .type_reference(parameters[0].type_reference)
        else {
            panic!("reference source")
        };
        let bindings = [(formal, *card)];
        assert_eq!(
            argument_matches_carrier_with_substitutions(
                &program,
                expression,
                expected(&program),
                &bindings
            ),
            access != "&write ",
        );
        assert!(!argument_matches_carrier_with_substitutions(
            &program,
            expression,
            parameters[2].type_reference,
            &bindings
        ));
        assert!(!argument_matches_type_reference_handle(
            &program,
            expression,
            parameters[2].type_reference,
        ));
    }
}

#[test]
fn array_view_matching_cannot_grant_mutation_through_shared_storage() {
    let program = crate::front_end::typed_program(
        "data Holder { bytes: &mut [u8; 3]; }
         machine inspect(value: &Holder, expected: &mut [u8]) { value.bytes; }",
    );
    let expression = member(&program, "value.bytes");
    let root = &program.state_parameters(&program.machine_states(&program.machines()[0])[0])[0];
    assert!(projected_matches_reference(
        &program,
        expression,
        expected(&program),
        declared_value_matches_reference,
        &[]
    ));
    assert!(!place_forwards_mutable_reference(
        &program,
        expression,
        root.symbol,
        root.type_reference,
    ));
}

#[test]
fn shared_slice_views_forget_constrained_referee_predicates() {
    // `&[u8] in D` spells `Reference { referee: Constrained { Slice } }`: the
    // carrier predicate wraps the slice inside the reference, not the loan.
    // Handing that member to a plain `&[u8]` parameter carries identical
    // bytes while the callee simply cannot rely on the dropped
    // qualification — the documented shared-view forget extended from
    // fixed-array carriers to a constrained slice referee.
    let mut program = crate::front_end::typed_program(
        "machine inspect(constrained: &[u8], plain: &[u8], expected: &[u8], mutable: &mut [u8], narrow: &[u16]) { plain; }",
    );
    let parameters = program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
    let constrained_actual = parameters[0].type_reference;
    let plain_actual = parameters[1].type_reference;
    let shared_view = parameters[2].type_reference;
    let mutable_view = parameters[3].type_reference;
    let narrow_view = parameters[4].type_reference;
    assert!(reference_type_matches(
        &program,
        constrained_actual,
        shared_view,
        &[],
        &[]
    ));
    let constraints = program
        .type_reference_table
        .insert_constraints([TypeConstraintNode::Named("qualified".into())]);
    let mut reference = program
        .type_reference_table
        .type_reference(constrained_actual)
        .clone();
    let TypeReferenceNode::Reference { referee, .. } = &mut reference else {
        panic!("shared slice reference");
    };
    *referee = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: *referee,
            constraints,
        });
    program
        .type_reference_table
        .substitute_node(constrained_actual, reference);
    assert!(
        reference_type_matches(&program, constrained_actual, shared_view, &[], &[]),
        "a shared view forgets the constrained slice referee's predicate"
    );
    assert!(
        !reference_type_matches(&program, constrained_actual, mutable_view, &[], &[]),
        "a mutable view could write past the forgotten predicate"
    );
    assert!(
        !reference_type_matches(&program, constrained_actual, narrow_view, &[], &[]),
        "forgetting the predicate does not change the element type"
    );
    // The required side's qualification is the callee's claim, not the
    // caller's to drop: a plain slice cannot satisfy a constrained referee.
    let mut constrained_required = program
        .type_reference_table
        .type_reference(shared_view)
        .clone();
    let TypeReferenceNode::Reference { referee, .. } = &mut constrained_required else {
        panic!("required reference");
    };
    *referee = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: *referee,
            constraints,
        });
    program
        .type_reference_table
        .substitute_node(shared_view, constrained_required);
    assert!(
        !reference_type_matches(&program, plain_actual, shared_view, &[], &[]),
        "an unqualified slice cannot satisfy the callee's constrained referee"
    );
    assert!(
        !value_carrier_matches_reference(&program, plain_actual, shared_view, &[], &[]),
        "call carrier matching cannot introduce a non-domain constraint"
    );
    assert!(
        value_carrier_matches_reference(&program, constrained_actual, shared_view, &[], &[]),
        "identical non-domain constraints retain their exact declared identity"
    );
}

#[test]
fn owned_constrained_array_satisfies_only_the_view_domain_it_proves() {
    // `[u8; N] in D` to `&[u8] in D`: the owned carrier's declared domain
    // implies the view's identical constraint, so the correspondence carries
    // the callee's claim across the fixed-to-view projection. An
    // unconstrained array still cannot invent that domain.
    let program = crate::front_end::typed_program(
        "domain [u8]::Utf8 requires valid_utf8(self);
         domain [u8; 4]::Utf8 requires valid_utf8(self);
         data Holder { text: [u8; 4] in Utf8; plain: [u8; 4]; }
         machine inspect(value: Holder, expected: &[u8] in Utf8, narrow: &[u16]) {
             value.text; value.plain;
         }",
    );
    assert!(
        projected_matches_reference(
            &program,
            member(&program, "value.text"),
            expected(&program),
            declared_value_matches_reference,
            &[]
        ),
        "an owned `[u8; 4] in Utf8` view satisfies `&[u8] in Utf8`"
    );
    assert!(
        !projected_matches_reference(
            &program,
            member(&program, "value.plain"),
            expected(&program),
            declared_value_matches_reference,
            &[]
        ),
        "a bare `[u8; 4]` cannot satisfy the callee's constrained referee"
    );
    let parameters = program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
    assert!(
        !projected_matches_reference(
            &program,
            member(&program, "value.text"),
            parameters[2].type_reference,
            declared_value_matches_reference,
            &[]
        ),
        "the byte-carrier bridge does not change the element type"
    );
    // Through an actual reference the same implication admits the matching
    // constrained view and still refuses an unproven one.
    let program = crate::front_end::typed_program(
        "domain [u8]::Utf8 requires valid_utf8(self);
         domain [u8; 4]::Utf8 requires valid_utf8(self);
         machine inspect(value: &[u8; 4] in Utf8, expected: &[u8] in Utf8) { value; }",
    );
    assert!(
        argument_matches_type_reference_handle(
            &program,
            program
                .expression_table
                .iter_expressions()
                .find_map(|(handle, node)| {
                    (matches!(node, ExpressionNode::Name(_))
                        && program.expression_table.display_name(handle) == "value")
                        .then_some(handle)
                })
                .expect("named reference"),
            expected(&program),
        ),
        "a `&[u8; 4] in Utf8` actual satisfies `&[u8] in Utf8`"
    );
    let program = crate::front_end::typed_program(
        "domain [u8]::Utf8 requires valid_utf8(self);
         domain [u8; 4]::Plain requires valid_utf8(self);
         machine inspect(value: &[u8; 4] in Plain, expected: &[u8] in Utf8) { value; }",
    );
    let parameters = program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
    assert!(
        !reference_type_matches(
            &program,
            parameters[0].type_reference,
            parameters[1].type_reference,
            &[],
            &[]
        ),
        "an unrelated `[u8; 4]` domain cannot satisfy `&[u8] in Utf8`"
    );
}

#[test]
fn repeated_generic_telescope_projection_remains_explicitly_unsupported() {
    let program = crate::front_end::typed_program(
        "data Box<Element> { value: Element; } machine inspect<Value>(value: Box<Box<Value>>, expected: &Value) { value.value.value; }",
    );
    assert!(
        !projected_matches_reference(
            &program,
            member(&program, "value.value.value"),
            expected(&program),
            declared_value_matches_reference,
            &[]
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
        let program = crate::front_end::typed_program(&format!(
            "data View {{ body: &mut i32; }} data Indexing {{}} {operator}
             machine inspect(values: [View; 1], index: u64, expected: &mut i32) {{ values[index].body; }}"
        ));
        let expression = program
            .expression_table
            .iter_expressions()
            .find_map(|(handle, node)| matches!(node, ExpressionNode::Member(_)).then_some(handle))
            .expect("indexed member");
        assert_eq!(
            projected_matches_reference(
                &program,
                expression,
                expected(&program),
                declared_value_matches_reference,
                &[]
            ),
            accepted,
            "{operator}"
        );
    }
    let mut program = crate::front_end::typed_program(
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
        expected(&program),
        declared_value_matches_reference,
        &[]
    ));
}

#[test]
fn generic_index_cannot_select_builtin_meaning_from_unsubstituted_elements() {
    let program = crate::front_end::typed_program(
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
        expected(&program),
        declared_value_matches_reference,
        &[]
    ));
}

#[test]
fn shared_call_carriers_defer_domains_for_every_reference_producer() {
    for (declarations, selected) in [
        (
            "machine inspect(value: &[u8], expected: &[u8] in Ascii) { value; }",
            "value",
        ),
        (
            "data Holder { value: &[u8]; } machine inspect(holder: &Holder, expected: &[u8] in Ascii) { holder.value; }",
            "holder.value",
        ),
        (
            "machine retain(value: &[u8]) -> &[u8] { value } machine inspect(value: &[u8], expected: &[u8] in Ascii) { let returned: &[u8] = retain(value); returned; }",
            "retain(value)",
        ),
    ] {
        let program = crate::front_end::typed_program(&format!(
            "domain [u8]::Ascii(bytes) requires ascii_only(bytes); {declarations}"
        ));
        let expression = program
            .expression_table
            .iter_expressions()
            .find_map(|(handle, expression)| {
                (if selected == "retain(value)" {
                    matches!(expression, ExpressionNode::Call(call) if call.target.as_str() == "retain")
                } else {
                    program.expression_table.display_name(handle) == selected
                })
                .then_some(handle)
            })
            .unwrap_or_else(|| panic!("selected reference producer: {selected}"));
        assert!(
            argument_matches_carrier(&program, expression, expected(&program)),
            "{selected}"
        );
        assert!(
            !argument_matches_type_reference_handle(&program, expression, expected(&program)),
            "without call facts the declaration cannot prove the domain: {selected}"
        );
    }
}

fn inline_recast_argument(program: &TypedTrees) -> ExpressionHandle {
    program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            let ExpressionNode::Borrow(borrow) = node else {
                return None;
            };
            matches!(program.expression_table.expression(borrow.target),
                ExpressionNode::Cast(cast) if cast.form.is_recast())
            .then_some(handle)
        })
        .expect("the normalized inline recast retains its borrow shell")
}

#[test]
fn inline_recast_arguments_preserve_target_carrier_qualification_and_access() {
    // This is argument correspondence, not recast admission: the separate
    // representation judgment still proves the source can supply the target.
    for (expression, required, declared, runtime) in [
        ("&mut storage[0] as &mut [u8]", "&mut [u8]", true, true),
        ("&storage[0] as &[u8]", "&[u8]", true, true),
        ("&mut storage[0] as &mut [u8]", "&[u8]", false, true),
        ("&storage[0] as &[u8]", "&mut [u8]", false, false),
        ("&mut storage[0] as &mut [u8]", "&write [u8]", false, false),
        ("&mut storage[0] as &mut [u8]", "&mut [u16]", false, false),
        ("&storage[0] as &[u8]", "&[u16]", false, false),
        (
            "&mut storage[0] as &mut [u8]",
            "&mut [u8] in Ascii",
            false,
            false,
        ),
        ("&storage[0] as &[u8]", "&[u8] in Ascii", false, false),
    ] {
        let program = crate::front_end::typed_program(&format!(
            "domain [u8]::Ascii(bytes) requires ascii_only(bytes);
             machine inspect(storage: &mut [u8; 8], expected: {required}) {{ {expression}; }}"
        ));
        let argument = inline_recast_argument(&program);
        assert_eq!(
            argument_matches_type_reference_handle(&program, argument, expected(&program)),
            declared,
            "declaration correspondence: {expression} to {required}"
        );
        assert_eq!(
            argument_matches_carrier(&program, argument, expected(&program)),
            runtime,
            "runtime correspondence: {expression} to {required}"
        );
    }
}

#[test]
fn inline_recast_arguments_match_exact_selected_generic_elements() {
    let program = crate::front_end::typed_program(
        "machine inspect<Element>(storage: &mut [u8; 8], expected: &mut [Element],
                                  selected: &u8, wrong: &u16) {
             &mut storage[0] as &mut [u8];
         }",
    );
    let machine = &program.machines()[0];
    let parameters = program.state_parameters(&program.machine_states(machine)[0]);
    let formal = program.machine_type_parameters(machine)[0].symbol;
    let argument = inline_recast_argument(&program);
    assert!(!argument_matches_carrier(
        &program,
        argument,
        expected(&program)
    ));
    for (position, accepted) in [(2, true), (3, false)] {
        let TypeReferenceNode::Reference { referee, .. } = program
            .type_reference_table
            .type_reference(parameters[position].type_reference)
        else {
            panic!("selected carrier")
        };
        assert_eq!(
            argument_matches_carrier_with_substitutions(
                &program,
                argument,
                expected(&program),
                &[(formal, *referee)]
            ),
            accepted
        );
    }
}

#[test]
fn inline_recast_arguments_do_not_rebind_inserted_caller_types() {
    let program = crate::front_end::typed_program(
        "machine inspect<Required, Caller>(storage: &mut [u8; 8], expected: &mut Required,
                                           selected: &mut [Caller], wrong: &u16) {
             &mut storage[0] as &mut [Caller];
         }",
    );
    let machine = &program.machines()[0];
    let parameters = program.state_parameters(&program.machine_states(machine)[0]);
    let formals = program.machine_type_parameters(machine);
    let referee = |position: usize| {
        let TypeReferenceNode::Reference { referee, .. } = program
            .type_reference_table
            .type_reference(parameters[position].type_reference)
        else {
            panic!("reference carrier")
        };
        *referee
    };
    let argument = inline_recast_argument(&program);
    let bindings = [
        (formals[0].symbol, referee(2)),
        (formals[1].symbol, referee(3)),
    ];
    assert!(
        argument_matches_carrier_with_substitutions(
            &program,
            argument,
            expected(&program),
            &bindings
        ),
        "Required selects the caller's [Caller]; Caller must not then become u16"
    );
    assert!(
        !argument_matches_carrier_with_substitutions(
            &program,
            argument,
            expected(&program),
            &[(formals[0].symbol, referee(3))]
        ),
        "a different selected carrier remains incompatible"
    );
}
