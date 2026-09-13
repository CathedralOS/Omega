use super::*;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(0), &tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn runtime_receivers_and_open_generic_callees_are_not_pending_endpoints() {
    for source in [
        "data Limits {} machine Limits::capacity(&self) -> u64 { 256 }
         machine bounded(limits: Limits, value: u64[0..=limits.capacity()]) {}",
        "data Limits {} machine Limits::capacity<const N: u64>() -> u64 { 256 }
         machine bounded(value: u64[0..=Limits::capacity()]) {}",
    ] {
        let program = typed(source);
        assert!(pending_endpoints(&program).unwrap().is_empty(), "{source}");
    }
}

#[test]
fn substituted_target_cannot_borrow_another_type_qualifier() {
    let mut program = typed(
        "data Limits {} data Other {}
         machine Limits::capacity() -> u64 { 256 }
         machine Other::capacity() -> u64 { 512 }
         machine bounded(value: u64[0..=Limits::capacity()]) {}",
    );
    let pending = pending_endpoints(&program).unwrap();
    assert_eq!(pending.len(), 1);
    let expression = pending[0].expression;
    let other = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Other::capacity")
        .expect("other attached machine");
    let other_entry = program.machine_states(other)[0].symbol;
    let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
        panic!("authored endpoint call");
    };
    call.target_symbol = other_entry;
    assert!(
        pending_endpoints(&program).unwrap().is_empty(),
        "resolved owner and qualifier must agree"
    );
}

#[test]
fn closed_integer_arguments_keep_carriers_and_exact_landings() {
    for (parameter, argument, expected) in [
        ("u64", "256", "256"),
        ("u64", "1 / 2 * 512", "256"),
        ("u64", "18446744073709551615u64", "18446744073709551615"),
        ("i64", "-9223372036854775808i64", "-9223372036854775808"),
        ("u8", "255", "255"),
        ("u64", "7u64 / 2 * 2", "6"),
    ] {
        let mut program = typed(&format!(
            "machine endpoint(value: {parameter}) -> {parameter} {{ value }}
             machine bounded(value: {parameter}[0..=endpoint({argument})]) {{}}"
        ));
        let pending = pending_endpoints(&program).unwrap();
        assert_eq!(pending.len(), 1);
        evaluate_const_range_endpoints_with_authority(&mut program, None)
            .unwrap_or_else(|errors| panic!("{argument}: {errors:?}"));
        let ExpressionNode::Integer(literal) =
            program.expression_table.expression(pending[0].expression)
        else {
            panic!("evaluated endpoint must be a literal");
        };
        assert_eq!(literal.value_bignum().unwrap().to_string(), expected);
    }
}

#[test]
fn ignored_arguments_cannot_hide_invalid_or_unsupported_inputs() {
    for (parameter, arguments, diagnostic) in [
        ("u64", "true", "closed integer expression"),
        ("u8", "255u16", "range endpoint argument"),
        ("u8", "255u64", "landed range endpoint argument"),
        ("u8", "256", "cannot land exactly"),
        ("u64", "-1", "cannot land exactly"),
        ("u64", "1 / 2", "closed integer expression"),
        ("u64", "7u64 / 0", "closed integer expression"),
        (
            "u64",
            "18446744073709551615u64 + 1",
            "closed integer expression",
        ),
        ("u64[0..=8]", "9", "outside declared range"),
        ("u64", "", "argument count"),
        ("u64", "1, 2", "argument count"),
        ("u64", "input", "closed integer expression"),
    ] {
        let mut program = typed(&format!(
            "machine endpoint(ignored: {parameter}) -> u64 {{ 256 }}
             machine bounded(input: u64, value: u64[0..=endpoint({arguments})]) {{}}"
        ));
        let pending = pending_endpoints(&program).unwrap();
        assert_eq!(pending.len(), 1);
        let errors = evaluate_const_range_endpoints_with_authority(&mut program, None)
            .expect_err("an ignored argument still needs source admission");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains(diagnostic)),
            "{parameter} <- {arguments}: {errors:?}"
        );
        assert!(
            matches!(
                program.expression_table.expression(pending[0].expression),
                ExpressionNode::Call(_)
            ),
            "rejection must not erase the authored call"
        );
    }
}

#[test]
fn argument_landing_retains_nested_fractional_warnings() {
    for argument in ["1 / 2 * 512", "1u64 + (1 / 2 * 510)"] {
        let program = typed(&format!(
            "machine endpoint(value: u64) -> u64 {{ value }}
             machine bounded(value: u64[0..=endpoint({argument})]) {{}}"
        ));
        let pending = pending_endpoints(&program).unwrap();
        let (values, warnings) = arguments::evaluate(
            &program,
            &program,
            pending[0].expression,
            pending[0].machine,
            None,
        )
        .expect("closed integer arguments land");
        assert_eq!(values, vec![crate::BuildTimeValue::Int(256)]);
        assert_eq!(warnings.len(), 1, "{argument}: {warnings:?}");
        assert!(warnings[0].source_span.is_some());
    }
}

#[test]
fn nested_calls_compose_without_erasing_their_integer_carriers() {
    for (parameter, argument, expected) in [
        ("u64", "endpoint(endpoint(256))", "256"),
        ("u64", "endpoint(128) + endpoint(128)", "256"),
        ("u64", "endpoint(1 / 2 * 512)", "256"),
        (
            "u64",
            "endpoint(18446744073709551615u64)",
            "18446744073709551615",
        ),
        (
            "i64",
            "endpoint(-9223372036854775808i64)",
            "-9223372036854775808",
        ),
        ("u8", "endpoint(127) + endpoint(128)", "255"),
    ] {
        let mut program = typed(&format!(
            "machine endpoint(value: {parameter}) -> {parameter} {{ value }}
             machine bounded(value: {parameter}[0..=endpoint({argument})]) {{}}"
        ));
        let calls = pending_endpoints(&program).unwrap();
        let expression = calls.last().unwrap().expression;
        evaluate_const_range_endpoints_with_authority(&mut program, None)
            .unwrap_or_else(|errors| panic!("{parameter}: {argument}: {errors:?}"));
        let value = program
            .closed_integer_value_in(expression, symbols::SymbolHandle::invalid())
            .expect("closed endpoint preserves value and carrier");
        assert_eq!(value.value.to_string(), expected);
        assert_eq!(value.primitive.unwrap().name(), parameter);
    }
}

#[test]
fn failed_outer_calls_restore_successful_inner_calls() {
    for (result_type, result, parameter, argument) in [
        ("u8", "255", "u64", "inner()"),
        ("u8", "255", "u8", "inner() + 1"),
        ("u64", "256", "u64", "inner() / 0"),
        ("u64", "256", "u64", "inner() + input"),
        ("u64[0..=256]", "257", "u64", "inner()"),
        ("bool", "true", "u64", "inner()"),
        ("u64", "256", "u64", "inner(1)"),
    ] {
        let mut program = typed(&format!(
            "machine inner() -> {result_type} {{ {result} }}
             machine endpoint(ignored: {parameter}) -> u64 {{ 256 }}
             machine bounded(input: u64, value: u64[0..=endpoint({argument})]) {{}}"
        ));
        let calls = pending_endpoints(&program).unwrap();
        assert!(calls.len() >= 2);
        evaluate_const_range_endpoints_with_authority(&mut program, None)
            .expect_err("ignored arguments retain typing and evaluation obligations");
        for call in calls {
            assert!(
                matches!(
                    program.expression_table.expression(call.expression),
                    ExpressionNode::Call(_)
                ),
                "failed {parameter} <- {argument} must leave every authored call intact"
            );
        }
    }
}

#[test]
fn endpoint_discovery_rejects_cyclic_expression_graphs() {
    let mut program = typed(
        "machine limit() -> u64 { 256 }
         machine bounded(value: u64[0..=limit() + 1]) {}",
    );
    let (_, _, constraints) = program
        .type_reference_table
        .constrained_type_reference_sites()[0];
    let TypeConstraintNode::Range { maximum, .. } =
        program.type_reference_table.constraints(constraints)[0]
    else {
        panic!("range constraint");
    };
    let ExpressionNode::Binary(binary) = program.expression_table.expression_mut(maximum) else {
        panic!("endpoint arithmetic");
    };
    binary.right = maximum;
    assert!(pending_endpoints(&program).is_err());
}

#[test]
fn folded_arguments_still_require_their_original_selection_authority() {
    use semantic_vocabulary::PackageKeyIdentity;
    use std::{path::PathBuf, sync::Arc};
    struct Selection(bool);
    impl crate::BuildTimeSelectionAuthority for Selection {
        fn allows_declaration_selection(
            &self,
            _: PackageKeyIdentity,
            _: PackageKeyIdentity,
        ) -> bool {
            self.0
        }
        fn package_label(&self, _: PackageKeyIdentity) -> String {
            "argument-package".to_owned()
        }
    }
    for text in [
        "const CAPACITY: u64 = 256;
                machine endpoint(value: u64) -> u64 { value }
                machine bounded(value: u64[0..=endpoint(CAPACITY)]) {}",
        "machine inner() -> u64 { 256 }
                machine endpoint(value: u64) -> u64 { value }
                machine bounded(value: u64[0..=endpoint(inner())]) {}",
    ] {
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add_with_metadata(
                PathBuf::from("main.omg"),
                text.to_owned(),
                PathBuf::from("."),
                Some(PackageKeyIdentity::from_digest([0x63; 32]).unwrap()),
                source::SourceOrigin::User,
            )
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
            &syntax,
            Arc::new(sources),
        )
        .unwrap();
        let program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let pending = pending_endpoints(&program).unwrap();
        let endpoint = pending.last().unwrap();
        let mut evaluated = program.clone();
        for inner in &pending[..pending.len() - 1] {
            *evaluated.expression_table.expression_mut(inner.expression) = ExpressionNode::Integer(
                IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "256")
                    .unwrap()
                    .with_landing(IntegerLanding {
                        landed_type: LandedIntegerType::U64,
                        domain: ArithmeticDomain::Exact,
                    }),
            );
        }
        // Test argument admission directly so the callee's separate gate cannot
        // mask failure to consult the original constant/call occurrence's
        // authority. Only the working graph contains the simulated call result.
        let (values, _) = arguments::evaluate(
            &evaluated,
            &program,
            endpoint.expression,
            endpoint.machine,
            Some(&Selection(true)),
        )
        .expect("admitted constant argument");
        assert_eq!(values, vec![crate::BuildTimeValue::Int(256)]);
        let error = arguments::evaluate(
            &evaluated,
            &program,
            endpoint.expression,
            endpoint.machine,
            Some(&Selection(false)),
        )
        .expect_err("the callee cannot grant selection authority to its arguments");
        assert!(
            error.contains("without direct dependency authority"),
            "{error}"
        );
    }
}

#[test]
fn signature_bounds_are_dependencies_even_when_declared_after_the_consumer() {
    let mut program = typed(
        "machine bounded(value: u64[0..=endpoint(256)]) {}
         machine endpoint(value: u64[1..=limit()]) -> u64[0..=limit()] {value}
         machine limit() -> u64 {256}",
    );
    let calls = pending_endpoints(&program).unwrap();
    assert_eq!(calls.len(), 3);
    assert!(
        calls[..2]
            .iter()
            .all(|call| program.symbols.display_path(call.machine, "::") == "limit")
    );
    assert_eq!(
        program.symbols.display_path(calls[2].machine, "::"),
        "endpoint"
    );
    evaluate_const_range_endpoints_with_authority(&mut program, None)
        .expect("signature dependencies fold before invocation");
    let value = program
        .closed_integer_expression_value(calls[2].expression)
        .unwrap();
    assert_eq!(value.to_u64(), Some(256));
}

#[test]
fn cyclic_signature_bounds_do_not_depend_on_source_order_or_retries() {
    for source in [
        "machine endpoint(value: u64[0..=endpoint(0)]) -> u64 {256}
         machine bounded(value: u64[0..=endpoint(0)]) {}",
        "machine first(value: u64[0..=second(0)]) -> u64 {256}
         machine second(value: u64[0..=first(0)]) -> u64 {256}
         machine bounded(value: u64[0..=first(0)]) {}",
    ] {
        let mut program = typed(source);
        let errors = evaluate_const_range_endpoints_with_authority(&mut program, None)
            .expect_err("a signature bound cannot depend on its own invocation");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("cyclic range endpoint")),
            "{errors:?}"
        );
    }
}
