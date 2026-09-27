use super::{array_leaves, constant, evaluate, literal_encoding, parse};
use language_semantics::const_value::CanonicalConstIdentity;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::Item;

#[test]
fn computed_nominal_leaves_preserve_carriers_fields_and_exact_arithmetic() {
    let declarations = "data Leaf [copy] { count: u64; }
        data Config [copy] { leaf: Leaf; flags: [bool; 2]; }";
    let evaluated = evaluate(&format!("{declarations}
        const SIZE: u64 = 7 / 2 * 2;
        const CONFIG: Config = Config {{ flags: [SIZE == 7, false], leaf: Leaf {{ count: SIZE * 2 }} }};"))
        .expect("selected nominal scalar leaves");
    let config = constant(&evaluated, "CONFIG");
    let receipt = config
        .normalization
        .as_ref()
        .expect("nominal initializer receipt");
    assert_eq!(
        receipt.canonical_result_encoding,
        literal_encoding(
            &format!(
                "{declarations}
        const CONFIG: Config = Config {{ leaf: Leaf {{ count: 14 }}, flags: [true, false] }};"
            ),
            "CONFIG"
        )
    );
    assert!(matches!(
        evaluated
            .expressions
            .expression(receipt.authored_expression),
        ExpressionNode::StructLiteral(_)
    ));
    assert_ne!(receipt.authored_expression, config.value);
    assert!(
        receipt
            .selections
            .iter()
            .any(|origin| origin.declaration == constant(&evaluated, "SIZE").name.source_span())
    );
    syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&evaluated),
    )
    .expect("nominal initializer receipt independently rejoins authored selections");
}

#[test]
fn computed_nominal_leaves_reject_invalid_unused_fields_and_constructor_shapes() {
    for initializer in [
        "Config { count: 200 + 100, enabled: true }",
        "Config { count: 7 / 2, enabled: true }",
        "Config { count: 2 + 1, enabled: 2 }",
        "Config { count: 2 + 1 }",
        "Config { count: 2 + 1, count: 2, enabled: true }",
        "Other { count: 2 + 1, enabled: true }",
    ] {
        assert!(
            evaluate(&format!(
                "data Config [copy] {{ count: u8; enabled: bool; }}
            data Other [copy] {{ count: u8; enabled: bool; }}
            const UNUSED: Config = {initializer};"
            ))
            .is_err(),
            "{initializer}"
        );
    }
}

#[test]
fn computed_case_payloads_and_record_arrays_share_scalar_leaf_evaluation() {
    let declarations = "data Value [copy] { case Empty; case Number(count: u64); }
        data Row [copy] { count: u64; enabled: bool; }";
    let evaluated = evaluate(&format!(
        "{declarations}
        const VALUE: Value = Value::Number {{ count: 7 / 2 * 2 }};
        const ROWS: [Row; 2] = [Row {{ count: 1 + 1, enabled: true }},
            Row {{ count: 7 / 2 * 2, enabled: false && (1u8 / 0 == 0) }}];"
    ))
    .expect("case and record array leaves");
    for (name, value) in [
        ("VALUE", "Value::Number { count: 7 }"),
        (
            "ROWS",
            "[Row { count: 2, enabled: true }, Row { count: 7, enabled: false }]",
        ),
    ] {
        let carrier = if name == "VALUE" { "Value" } else { "[Row; 2]" };
        assert_eq!(
            constant(&evaluated, name)
                .normalization
                .as_ref()
                .unwrap()
                .canonical_result_encoding,
            literal_encoding(
                &format!("{declarations} const {name}: {carrier} = {value};"),
                name
            )
        );
    }
    syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&evaluated),
    )
    .expect("case constructor receipt replay");
}

#[test]
fn computed_record_leaves_preserve_payloadless_case_siblings() {
    let declarations = "data Mode [copy] { case On; case Off; }
        data Config [copy] { mode: Mode; count: u64; }";
    let evaluated = evaluate(&format!(
        "{declarations}
        const UNUSED: Mode = Mode::Off;
        const CONFIG: Config = Config {{ mode: Mode::On, count: 7 / 2 * 2 }};"
    ))
    .expect("case literal is not a constant dependency");
    assert_eq!(
        constant(&evaluated, "CONFIG")
            .normalization
            .as_ref()
            .unwrap()
            .canonical_result_encoding,
        literal_encoding(
            &format!(
                "{declarations} const CONFIG: Config = Config {{ mode: Mode::On, count: 7 }};"
            ),
            "CONFIG"
        )
    );
    syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&evaluated),
    )
    .expect("case identity survives evaluation and receipt replay");
}

#[test]
fn computed_nominal_leaves_reject_changed_result_and_dependency_receipts() {
    let evaluated = evaluate(
        "data Config [copy] { count: u64; }
        const SIZE: u64 = 7 / 2 * 2;
        const CONFIG: Config = Config { count: SIZE * 2 };",
    )
    .expect("nominal receipt baseline");
    for mutation in ["result", "dependency", "operator"] {
        let mut invalid = evaluated.clone();
        let item = *invalid.root_item_handles().iter().find(|item| {
            matches!(invalid.root_item(**item), Item::Const(definition) if definition.name.as_str() == "CONFIG")
        }).unwrap();
        let Item::Const(mut definition) = invalid.root_item(item).clone() else {
            panic!("constant");
        };
        let receipt = definition.normalization.as_mut().unwrap();
        match mutation {
            "result" => receipt.canonical_result_encoding.push('0'),
            "dependency" => receipt.selections.clear(),
            "operator" => receipt.builtin_operators.clear(),
            _ => unreachable!(),
        }
        invalid.items.replace_item(item, Item::Const(definition));
        assert!(
            syntax_trees_to_symbol_resolved_trees::resolve(
                syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&invalid)
            )
            .is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn computed_array_leaves_land_independently_and_retain_the_authored_array() {
    let (syntax, sources) = parse(
        "const SIZE: u64 = 7 / 2 * 2;
         const SIZES: [u64; 2] = [SIZE * 2, 3];
         const FLAGS: [bool; 2] = [SIZE == 6, false];
         const GRID: [[u8; 2]; 1] = [[1 + 1, 2]];",
    );
    let authored_sizes = constant(&syntax, "SIZES").value;
    let authored_span = syntax.expressions.source_span(authored_sizes);
    let evaluated = super::super::evaluate(syntax, Some(sources), &[], None).expect("array leaves");

    assert_eq!(
        array_leaves(&evaluated, constant(&evaluated, "SIZES").value),
        vec![
            ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(14)),
            ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(3)),
        ]
    );
    assert_eq!(
        array_leaves(&evaluated, constant(&evaluated, "FLAGS").value),
        vec![
            ExpressionNode::Boolean(false),
            ExpressionNode::Boolean(false)
        ]
    );
    assert_eq!(
        array_leaves(&evaluated, constant(&evaluated, "GRID").value),
        vec![
            ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(2)),
            ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(2)),
        ]
    );
    let sizes = constant(&evaluated, "SIZES");
    let ExpressionNode::ArrayLiteral(elements) = evaluated
        .expressions
        .expression(sizes.normalization.as_ref().unwrap().authored_expression)
    else {
        panic!("authored array receipt");
    };
    assert!(matches!(
        evaluated
            .expressions
            .expression(evaluated.expressions.expression_handles(*elements)[0]),
        ExpressionNode::Binary(_)
    ));
    assert_eq!(
        evaluated.expressions.source_span(sizes.value),
        authored_span
    );
    assert!(
        sizes
            .normalization
            .as_ref()
            .unwrap()
            .selections
            .iter()
            .any(|origin| origin.declaration == constant(&evaluated, "SIZE").name.source_span())
    );
    assert!(
        constant(&evaluated, "FLAGS")
            .normalization
            .as_ref()
            .unwrap()
            .selections
            .iter()
            .any(|origin| origin.declaration == constant(&evaluated, "SIZE").name.source_span())
    );
    assert!(
        constant(&evaluated, "GRID")
            .normalization
            .as_ref()
            .unwrap()
            .selections
            .is_empty()
    );
    for (name, literal) in [
        ("SIZES", "const SIZES: [u64; 2] = [14, 3];"),
        ("FLAGS", "const FLAGS: [bool; 2] = [false, false];"),
        ("GRID", "const GRID: [[u8; 2]; 1] = [[2, 2]];"),
    ] {
        assert_eq!(
            constant(&evaluated, name)
                .normalization
                .as_ref()
                .unwrap()
                .canonical_result_encoding,
            literal_encoding(literal, name)
        );
    }
}

#[test]
fn computed_array_leaves_owe_their_declared_landing_even_when_private_and_unused() {
    let errors = evaluate("const BAD: [u8; 1] = [200 + 100]; machine run() -> u64 { 0 }")
        .expect_err("private array landing");
    assert!(
        errors
            .iter()
            .any(|error| { error.message.contains("land") || error.message.contains("u8") })
    );
}

#[test]
fn computed_array_leaves_admit_checked_machine_calls() {
    let evaluated = evaluate(
        "const CALL: [u64; 1] = [read()];
         machine read() -> u64 { 1 }",
    )
    .expect("array leaves use ordinary invocation admission");
    let declaration = constant(&evaluated, "CALL");
    assert_eq!(
        array_leaves(&evaluated, declaration.value),
        vec![ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(1)
        )]
    );
}

#[test]
fn retained_invocation_replay_rechecks_concrete_crash_discharge() {
    // Both actuals would return 7 if interpretation were used as admission.
    // Only false discharges the published ceiling; keep the folded value fixed.
    let (syntax, sources) = parse(
        "machine gate(flag: bool) -> u64 crashes Trap flag { 7 }
        const SIZE: u64 = gate(false);",
    );
    let evaluated = super::super::evaluate(syntax, Some(sources.clone()), &[], None)
        .expect("concrete safe invocation");
    let mut typed =
        crate::front_end::typed_program_from_evaluated_syntax(&evaluated, Some(sources));
    super::super::validate_retained_invocations(&typed, None).expect("unchanged discharge");
    let original = typed.const_declarations()[0].authored_initializer;
    let typed_trees::expression::ExpressionNode::Call(call) =
        typed.expression_table.expression(original)
    else {
        panic!("retained call");
    };
    let argument = typed.expression_table.expression_handles(call.arguments)[0];
    *typed.expression_table.expression_mut(argument) =
        typed_trees::expression::ExpressionNode::Boolean(true);
    let diagnostics = super::super::validate_retained_invocations(&typed, None)
        .expect_err("same result cannot preserve changed invocation admission");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("retains unhandled")),
        "{diagnostics:?}"
    );
}

#[test]
fn retained_invocation_replay_rejects_changed_or_erased_computation_and_results() {
    let (syntax, sources) = parse(
        "machine size() -> u64 { 7 }
        machine unrelated() -> u64 { 7 }
        const SIZE: u64 = size();",
    );
    let evaluated =
        super::super::evaluate(syntax, Some(sources.clone()), &[], None).expect("evaluated call");
    let typed = crate::front_end::typed_program_from_evaluated_syntax(&evaluated, Some(sources));
    super::super::validate_retained_invocations(&typed, None).expect("unchanged invocation replay");
    let declaration = typed.const_declarations()[0].clone();
    for mutation in 0..6 {
        let mut changed = typed.clone();
        match mutation {
            0 => {
                *changed
                    .expression_table
                    .expression_mut(declaration.authored_initializer) =
                    typed_trees::expression::ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(7),
                    )
            }
            1 => {
                *changed
                    .expression_table
                    .expression_mut(declaration.materialized_initializer) =
                    typed_trees::expression::ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(8),
                    )
            }
            2 => {
                let other = changed
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == "unrelated")
                    .unwrap();
                let target = changed.machine_states(other)[0].symbol;
                let typed_trees::expression::ExpressionNode::Call(call) = changed
                    .expression_table
                    .expression_mut(declaration.authored_initializer)
                else {
                    panic!("retained original call")
                };
                call.target_symbol = target;
            }
            3 => {
                let span = changed.roots.const_declarations;
                let retained = &mut changed.tables.const_declarations.span_mut_or_empty(span)[0];
                retained.authored_initializer =
                    typed_trees::expression::ExpressionHandle::invalid();
                retained.materialized_initializer =
                    typed_trees::expression::ExpressionHandle::invalid();
            }
            4 => {
                let machine = changed
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == "size")
                    .unwrap();
                let state = &changed.machine_states(machine)[0];
                let value = match &changed.statement_table.statements(state.statement_nodes)[0] {
                    typed_trees::statement::StatementNode::Expression(value) => *value,
                    typed_trees::statement::StatementNode::Transition(transition) => {
                        let typed_trees::statement::TransitionTargetNode::Value(value) =
                            changed.statement_table.transition_target(transition.target)
                        else {
                            panic!("value return")
                        };
                        *value
                    }
                    _ => panic!("value body"),
                };
                *changed.expression_table.expression_mut(value) =
                    typed_trees::expression::ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(8),
                    );
            }
            5 => {
                let value = changed.expression_table.insert(
                    typed_trees::expression::ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(7),
                    ),
                );
                changed
                    .expression_table
                    .set_source_span(value, declaration.initializer_source_span);
                let span = changed.roots.const_declarations;
                changed.tables.const_declarations.span_mut_or_empty(span)[0]
                    .materialized_initializer = value;
            }
            _ => unreachable!(),
        }
        assert!(
            super::super::validate_retained_invocations(&changed, None).is_err(),
            "mutation {mutation} bypassed replay"
        );
    }
}

#[test]
fn retained_invocation_replay_rejoins_helper_constant_values() {
    let (syntax, sources) = parse(
        "const BASE: u64 = 7;
        machine size() -> u64 { BASE }
        const SIZE: u64 = size();",
    );
    let evaluated = super::super::evaluate(syntax, Some(sources.clone()), &[], None)
        .expect("evaluated dependency");
    let typed = crate::front_end::typed_program_from_evaluated_syntax(&evaluated, Some(sources));
    super::super::validate_retained_invocations(&typed, None).expect("unchanged dependency");
    for mutate_value in [false, true] {
        let mut changed = typed.clone();
        let span = changed.roots.const_declarations;
        let dependency = &mut changed.tables.const_declarations.span_mut_or_empty(span)[0];
        dependency.canonical_value_encoding =
            Some(CanonicalConstIdentity::integer("u64", 8).encoding);
        if mutate_value {
            let root = dependency.materialized_initializer;
            *changed.expression_table.expression_mut(root) =
                typed_trees::expression::ExpressionNode::Integer(
                    numerics::literals::IntegerLiteral::from_value(8),
                );
        }
        assert!(
            super::super::validate_retained_invocations(&changed, None).is_err(),
            "helper constant drift bypassed receiving replay (value changed: {mutate_value})"
        );
    }
}

#[test]
fn retained_invocation_replay_checks_payloadless_constructor_siblings() {
    let (syntax, sources) = parse(
        "data Mode [copy] { case On; case Off; }
        data Config [copy] { mode: Mode; count: u64; }
        machine size() -> u64 { 7 }
        const CONFIG: Config = Config { mode: Mode::On, count: size() };",
    );
    let evaluated =
        super::super::evaluate(syntax, Some(sources.clone()), &[], None).expect("evaluated record");
    let typed = crate::front_end::typed_program_from_evaluated_syntax(&evaluated, Some(sources));
    let declaration = &typed.const_declarations()[0];
    assert!(typed.expression_table.authored_selection_occurrences(declaration.materialized_initializer)
        .filter_map(|occurrence| typed.authored_declaration_selections().get(occurrence))
        .any(|selection| selection.kind() == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call), "materialized constructor retains its complete call roster");
    super::super::validate_retained_invocations(&typed, None).expect("payloadless sibling replay");
}

#[test]
fn retained_composed_invocation_rejects_erased_roots() {
    let (syntax, sources) = parse("machine size() -> u64 { 7 } const SIZE: u64 = size() * 2;");
    let evaluated = super::super::evaluate(syntax, Some(sources.clone()), &[], None)
        .expect("evaluated composition");
    let mut typed =
        crate::front_end::typed_program_from_evaluated_syntax(&evaluated, Some(sources));
    super::super::validate_retained_invocations(&typed, None).expect("unchanged composition");
    let span = typed.roots.const_declarations;
    let declaration = &mut typed.tables.const_declarations.span_mut_or_empty(span)[0];
    declaration.authored_initializer = typed_trees::expression::ExpressionHandle::invalid();
    declaration.materialized_initializer = typed_trees::expression::ExpressionHandle::invalid();
    assert!(super::super::validate_retained_invocations(&typed, None).is_err());
}
