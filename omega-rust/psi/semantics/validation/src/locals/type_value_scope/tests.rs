use typed_trees::TypedTrees;

fn parse(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
}

fn check(source: &str, accepted: bool) {
    let outcome = crate::validate_program(&parse(source));
    if accepted {
        assert!(outcome.is_ok(), "{outcome:?}\n{source}");
    } else {
        let diagnostics = outcome.expect_err("foreign type-bound value must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("explicit parameter scope")),
            "{diagnostics:?}\n{source}"
        );
    }
}

#[test]
fn type_bound_values_require_current_state_parameters() {
    for (parameters, arguments, bound, accepted) in [
        ("", "", "hidden", false),
        ("limit: f64, ", "hidden, ", "limit", true),
    ] {
        check(
            &format!(
                "machine run(hidden: f64) {{ transition {{ _ -> next({arguments}0.0) }} state next({parameters}value: f64 [0.0..={bound}]) {{}} }}"
            ),
            accepted,
        );
    }
    check(
        "machine run() { transition { _ -> first(1.0) } state first(hidden: f64) { transition { _ -> next(0.0) } } state next(value: f64 [0.0..=hidden]) {} }",
        false,
    );
}

#[test]
fn type_bound_self_requires_a_declared_receiver() {
    for (parameters, accepted) in [("", false), ("&self, ", true)] {
        check(
            &format!(
                "data Owner {{ count: u64 [0..=4]; }} machine Owner::run(&self) {{ transition {{ _ -> next(0) }} state next({parameters}index: u64 [0..=self.count]) {{}} }}"
            ),
            accepted,
        );
    }
}

#[test]
fn local_and_cast_type_bounds_use_the_prior_statement_prefix() {
    for declaration in [
        "let value: f64 [0.0..=limit] = 0.0;",
        "let value: f64 = 0.0 as f64 [0.0..=limit];",
    ] {
        check(
            &format!("machine run() {{ let limit: f64 = 4.0; {declaration} }}"),
            true,
        );
        check(
            &format!("machine run() {{ {declaration} let limit: f64 = 4.0; }}"),
            false,
        );
        check(
            &format!(
                "machine run(limit: f64) {{ transition {{ _ -> next() }} state next() {{ {declaration} }} }}"
            ),
            false,
        );
    }
}

#[test]
fn return_and_nested_element_bounds_use_the_signature_frontier() {
    for (parameters, arguments, bound, accepted) in [
        ("", "", "hidden", false),
        ("limit: f64", "hidden", "limit", true),
    ] {
        check(
            &format!(
                "machine run(hidden: f64) -> f64 {{ transition {{ _ -> next({arguments}) }} state next({parameters}) -> f64 [0.0..={bound}] {{ 0.0 }} }}"
            ),
            accepted,
        );
        let separator = if parameters.is_empty() { "" } else { ", " };
        check(
            &format!(
                "machine run(hidden: f64) {{ transition {{ _ -> next({arguments}{separator}[0.0]) }} state next({parameters}{separator}values: [f64 [0.0..={bound}]; 1]) {{}} }}"
            ),
            accepted,
        );
    }
}

#[test]
fn same_signature_slice_bounds_and_const_array_extents_remain_valid() {
    check(
        "machine run(items: &[u8], index: u64 [0..items.len]) {}",
        true,
    );
    check(
        "machine run(index: u64 [0..items.len], items: &[u8]) {}",
        true,
    );
    check(
        "machine run<const N: u64>(values: &[u8; N]) { transition { _ -> next(values) } state next(items: &[u8; N]) {} }",
        true,
    );
    let outcome = crate::validate_program(&parse(
        "machine run(hidden: u64) { transition { _ -> next() } state next() { let items: [u8; hidden]; } }",
    ));
    assert!(
        outcome
            .expect_err("runtime fixed extent is unsupported")
            .iter()
            .any(|diagnostic| diagnostic.message.contains("fixed-array length"))
    );
}

#[test]
fn type_bounds_retain_exact_parameter_identity_without_name_repair() {
    use typed_trees::{
        expression::ExpressionNode,
        types::{TypeConstraintNode, TypeReferenceNode},
    };

    let source = "machine run(limit: f64) { transition { _ -> next(limit, 0.0) } state next(limit: f64, value: f64 [0.0..=limit]) {} }";
    let mut program = parse(source);
    let states = program.machine_states(&program.machines()[0]);
    let entry = program.state_parameters(&states[0])[0].symbol;
    let target = program.state_parameters(&states[1])[0].symbol;
    let constrained = program.state_parameters(&states[1])[1].type_reference;
    let TypeReferenceNode::Constrained { constraints, .. } =
        program.type_reference_table.type_reference(constrained)
    else {
        panic!("dependent range");
    };
    let TypeConstraintNode::Range { maximum, .. } =
        program.type_reference_table.constraints(*constraints)[0]
    else {
        panic!("range bound");
    };
    let ExpressionNode::Name(path) = program.expression_table.expression_mut(maximum) else {
        panic!("parameter bound");
    };
    assert_eq!(path.head_symbol, target);
    assert_eq!(path.symbol, target);
    assert_ne!(entry, target);
    path.head_symbol = entry;
    path.symbol = entry;
    let diagnostics = crate::validate_program(&program).expect_err("foreign same-name bound");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("explicit parameter scope")),
        "{diagnostics:?}"
    );
}
