use crate::lowerer::seeded_continuation::retained_typed_base_is_exact_prefix;

#[test]
fn numeric_result_policies_are_retained_without_result_annotations_or_input_ranges() {
    use numerics::arithmetic::ArithmeticDomain;
    use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};
    for source in [
        "machine run(value: u64 [0..10] in Wrapping) { value; }",
        "machine run() { 1u64 as u64 in Wrapping; }",
    ] {
        let typed = crate::front_end::typed_program(source);
        let carrier = typed
            .symbols
            .child_handles(typed.symbols.root())
            .unwrap()
            .find(|symbol| {
                typed.symbols.builtin_type_atom(*symbol) == Some(symbols::BuiltinTypeAtom::U64)
            })
            .unwrap();
        let reference = typed
            .type_reference_table
            .find_arithmetic_result_type_reference(carrier, ArithmeticDomain::Wrapping)
            .expect("policy-only result retained at producer");
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = typed.type_reference_table.type_reference(reference)
        else {
            panic!("qualified result");
        };
        assert!(
            matches!(typed.type_reference_table.type_reference(*base_type), TypeReferenceNode::Named {symbol, ..} if *symbol == carrier)
        );
        assert!(matches!(
            typed.type_reference_table.constraints(*constraints),
            [TypeConstraintNode::ArithmeticDomain(
                ArithmeticDomain::Wrapping
            )]
        ));
        if let Some(parameter) = typed
            .state_parameters(&typed.machine_states(&typed.machines()[0])[0])
            .first()
        {
            assert_ne!(
                parameter.type_reference, reference,
                "authored range retained separately"
            );
            assert!(
                typed
                    .display_type_reference_with_constraints(parameter.type_reference)
                    .contains("0")
            );
        }
    }
}

#[test]
fn retained_base_rejects_type_identity_changes_hidden_by_display_snapshots() {
    use typed_trees::types::TypeReferenceNode;
    let typed = crate::front_end::typed_program("machine main(value: u64) { value; }");
    let parameter = &typed.state_parameters(&typed.machine_states(&typed.machines()[0])[0])[0];
    let mut changed = typed.clone();
    let TypeReferenceNode::Named { name, .. } = changed
        .type_reference_table
        .type_reference(parameter.type_reference)
        .clone()
    else {
        panic!("named scalar parameter");
    };
    changed.type_reference_table.substitute_node(
        parameter.type_reference,
        TypeReferenceNode::Named {
            name,
            symbol: symbols::SymbolHandle::invalid(),
        },
    );
    assert_eq!(
        typed.snapshot(),
        changed.snapshot(),
        "display is not identity"
    );
    assert!(!retained_typed_base_is_exact_prefix(&typed, &changed));
}

#[test]
fn retained_base_rejects_a_changed_local_inference_origin() {
    let typed =
        crate::front_end::typed_program("machine main() -> u64 { let value: u64 = 7; value }");
    let mut changed = typed.clone();
    let body = changed.machine_states(&changed.machines()[0])[0].statement_nodes;
    let typed_trees::statement::StatementNode::LocalData(local) =
        &mut changed.statement_table.statements_mut(body)[0]
    else {
        panic!("authored local");
    };
    assert!(!local.type_is_inferred);
    local.type_is_inferred = true;
    assert!(!retained_typed_base_is_exact_prefix(&typed, &changed));
}

#[test]
fn inferred_types_do_not_depend_on_generated_binding_names() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        data Record { value: u64; }
        machine produce() -> u64
        ensures outgoing: ready()
        { outgoing = ConcreteEvidence; 7 }
        machine caller(record: &Record) -> u64 {
            let { value as selected } = record;
            let (runtime; outgoing: witness) = produce();
            runtime
        }
    "#;
    let typed = crate::front_end::typed_program(source);
    for name in ["selected", "runtime"] {
        let local = typed
            .machines()
            .iter()
            .flat_map(|machine| typed.machine_states(machine))
            .flat_map(|state| typed.statement_table.statements(state.statement_nodes))
            .find_map(|statement| match statement {
                typed_trees::statement::StatementNode::LocalData(local)
                    if local.name.as_str() == name =>
                {
                    Some(local)
                }
                _ => None,
            })
            .expect("authored binding without an annotation");
        assert!(local.type_is_inferred, "{name} has an inferred type");
        assert_eq!(
            typed
                .type_reference_table
                .display_name(local.type_reference),
            "u64"
        );
    }
}

#[test]
fn proof_output_runtime_calls_copy_arguments_into_the_statement_arena() {
    use typed_trees::{expression::ExpressionNode, statement::StatementNode};

    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        machine touch(first: u64, second: u64, third: u64) {}
        machine effect(output: &mut u64, value: u64)
        requires incoming: ready()
        ensures outgoing: ready()
        {
            output = value;
            outgoing = incoming;
        }
        machine caller(destination: &mut u64, selected: u64)
        requires incoming: ready()
        {
            touch(99, 88, 77);
            let (; outgoing: witness) = effect(destination, selected; incoming);
        }
    "#;
    let typed = crate::front_end::typed_program(source);
    let caller = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "caller")
        .expect("caller machine");
    let state = &typed.machine_states(caller)[0];
    let parameters = typed.state_parameters(state);
    let [package] = typed.proof_output_calls.as_slice() else {
        panic!("one proof-output call");
    };
    assert_eq!(package.state_symbol, state.symbol);
    let runtime_index = package
        .runtime_call_statement_index
        .expect("effect executes");
    assert_eq!(
        runtime_index, 1,
        "ordinary call precedes the proof-output call"
    );
    let StatementNode::Call(runtime) =
        &typed.statement_table.statements(state.statement_nodes)[runtime_index]
    else {
        panic!("proof-output runtime statement");
    };
    let ExpressionNode::Call(expression) = typed.expression_table.expression(package.call) else {
        panic!("retained proof-output expression");
    };
    // The preceding three-argument statement deliberately offsets this arena
    // from expression-call argument storage. A raw span copy reads its literals.
    assert_ne!(runtime.arguments, expression.arguments);
    let runtime_arguments = typed.statement_table.expression_handles(runtime.arguments);
    assert_eq!(
        runtime_arguments,
        typed
            .expression_table
            .expression_handles(expression.arguments)
    );
    assert_eq!(runtime_arguments.len(), parameters.len());
    for (argument, parameter) in runtime_arguments.iter().zip(parameters) {
        let ExpressionNode::Name(path) = typed.expression_table.expression(*argument) else {
            panic!("exact caller parameter argument");
        };
        assert_eq!(path.head_symbol, parameter.symbol);
        assert_eq!(path.symbol, parameter.symbol);
        assert_eq!(typed.symbols.get(path.symbol).parent, state.symbol);
    }
}

#[test]
fn inherited_trait_default_realizations_settle_exact_requirement_symbols() {
    let source = r#"
        trait Resettable {
            machine set(&mut self, value: i32);
            machine reset(&mut self) { self.set(30); }
        }
        trait Counter { requires Resettable; }

        data Left { value: i32; }
        LeftCounter: Left satisfies Counter;
        machine Left::set(&mut self, value: i32) { self.value = value; }

        data Right { value: i32; }
        RightCounter: Right satisfies Counter;
        machine Right::set(&mut self, value: i32) { self.value = value; }
    "#;
    let typed = crate::front_end::typed_program(source);
    let reset_requirement = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Resettable")
        .and_then(|definition| {
            typed
                .trait_machine_signatures(definition)
                .iter()
                .find(|requirement| requirement.name.as_str() == "reset")
        })
        .expect("Resettable::reset requirement");

    let applications = ["Left::reset", "Right::reset"].map(|name| {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("synthesized default realization");
        let [conformance] = typed.machine_trait_conformances(machine) else {
            panic!("one synthesized requirement edge");
        };
        assert_eq!(conformance.requirement_symbol, reset_requirement.symbol);
        let state = typed.machine_states(machine).first().expect("entry state");
        let call = typed
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                typed_trees::statement::StatementNode::Call(call) => Some(call),
                _ => None,
            })
            .expect("default body call");
        call.target_symbol
    });
    assert_ne!(applications[0], applications[1]);
}

#[test]
fn deep_left_associated_boolean_expression_types_on_the_default_test_stack() {
    let expression = std::iter::repeat_n("enabled", 128)
        .collect::<Vec<_>>()
        .join(" && ");
    let source =
        format!("data Root {{}} machine Root::measure(enabled: bool) -> bool {{ {expression} }}");

    crate::front_end::typed_program_result(&source)
        .expect("type deep expression on default test stack");
}

#[test]
fn exact_quoted_bytes_land_as_an_owned_fixed_u8_array() {
    let typed = crate::front_end::typed_program_result(
        r#"
        machine bytes() -> [u8; 2] {
            "\x80A"
        }
        "#,
    )
    .expect("exact-width raw bytes should type");

    let array = typed
        .expression_table
        .expression_entries()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::ArrayLiteral(elements) => Some(*elements),
            _ => None,
        })
        .expect("the contextual string must become an ordinary array literal");
    let values = typed
        .expression_table
        .expression_handles(array)
        .iter()
        .map(
            |element| match typed.expression_table.expression(*element) {
                typed_trees::expression::ExpressionNode::Integer(literal) => {
                    literal.value_i64().expect("byte integer")
                }
                other => panic!("expected byte integer, got {other:?}"),
            },
        )
        .collect::<Vec<_>>();
    assert_eq!(values, [0x80, i64::from(b'A')]);
}

#[test]
fn quoted_bytes_reject_short_and_long_owned_fixed_array_destinations() {
    for (width, literal_length) in [(3, 2), (1, 2)] {
        let diagnostic = crate::front_end::typed_program_result(&format!(
            "machine bytes() -> [u8; {width}] {{ \"ab\" }}"
        ))
        .expect_err("fixed byte arrays neither pad nor truncate literals");
        assert!(
            diagnostic.message.contains(&format!(
                "quoted byte literal has {literal_length} source byte(s)"
            )) && diagnostic
                .message
                .contains(&format!("requires exactly {width}")),
            "{}",
            diagnostic.message,
        );
    }
}

#[test]
fn quoted_bytes_reject_non_byte_fixed_array_destinations() {
    let diagnostic =
        crate::front_end::typed_program_result(r#"machine words() -> [u16; 2] { "ab" }"#)
            .expect_err("quoted bytes must not acquire a non-byte element interpretation");
    assert!(
        diagnostic.message.contains("element type must be `u8`"),
        "{}",
        diagnostic.message,
    );
}

#[test]
fn quoted_bytes_reject_nonliteral_or_undetermined_widths() {
    for source in [
        r#"
            machine bytes<const N: u64>() -> [u8; N] {
                "ab"
            }
        "#,
        r#"
            machine width() -> u64 { 2 }
            machine bytes() -> [u8; width()] {
                "ab"
            }
        "#,
    ] {
        let diagnostic = crate::front_end::typed_program_result(source)
            .expect_err("a parameter/call width is not a resolved literal extent");
        assert!(
            diagnostic
                .message
                .contains("width must be a compile-known resolved integer literal"),
            "{}",
            diagnostic.message,
        );
    }
}

#[test]
fn trait_machine_requirement_identity_reaches_typed_trees() {
    let typed =
        crate::front_end::typed_program("trait PrivateCallbackSlot<machine Requirement> {}");
    let trait_definition = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "PrivateCallbackSlot")
        .expect("PrivateCallbackSlot trait");
    let [parameter] = typed.trait_type_parameters(trait_definition) else {
        panic!("one typed trait machine requirement parameter")
    };
    assert!(parameter.symbol.is_valid());
    assert!(matches!(
        parameter.kind,
        typed_trees::data::TypeParameterKind::Machine {
            contract: typed_trees::data::MachineParameterContract::RequirementIdentity
        }
    ));
}

#[test]
fn exact_trait_requirement_argument_reaches_typed_conformance() {
    let source = r#"
        boundary trait WindowProcedure { machine call(value: u32); }
        trait PrivateCallbackSlot<machine Requirement> {}
        data WndClassLayout {}
        WndClassWindowProcedureSlot:
            WndClassLayout satisfies PrivateCallbackSlot<WindowProcedure::call>;
    "#;
    let typed = crate::front_end::typed_program(source);
    let window_procedure = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "WindowProcedure")
        .expect("WindowProcedure trait");
    let requirement = typed
        .trait_machine_signatures(window_procedure)
        .first()
        .expect("WindowProcedure::call");
    let conformance = typed.conformances().first().expect("slot conformance");
    let [argument] = typed
        .type_reference_table
        .type_reference_handles(conformance.arguments)
    else {
        panic!("one typed slot requirement argument")
    };
    let typed_trees::types::TypeReferenceNode::Named { symbol, name } =
        typed.type_reference_table.type_reference(*argument)
    else {
        panic!("typed requirement argument remains a named identity")
    };
    assert_eq!(name.as_str(), "WindowProcedure::call");
    assert_eq!(*symbol, requirement.symbol);
}
