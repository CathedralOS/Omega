use super::{
    ArithmeticDomain, Diagnostic, ExpressionNode, IntegerLanding, IntegerLiteral, IntegerRadix,
    LandedIntegerType, TypeConstraintNode, TypedTrees, arguments, evaluate_const_range_endpoints,
    pending_endpoints,
};
use crate::SelectedBuildTimeOperators;
fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(0), &tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
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
        evaluate_const_range_endpoints(&mut program, None)
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
        let errors = evaluate_const_range_endpoints(&mut program, None)
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
        let admission = crate::BuildTimeAdmissionPlan::infer(&program, None);
        let (values, warnings) = arguments::evaluate(
            &program,
            &program,
            &admission,
            pending[0].expression,
            super::EndpointCallee::plain(pending[0].machine),
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
        evaluate_const_range_endpoints(&mut program, None)
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
        evaluate_const_range_endpoints(&mut program, None)
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
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: &syntax,
                sources: Some(Arc::new(sources)),
                top_level_bindings: Vec::new(),
            },
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
        let admission = crate::BuildTimeAdmissionPlan::infer(&program, None);
        let (values, _) = arguments::evaluate(
            &evaluated,
            &program,
            &admission,
            endpoint.expression,
            super::EndpointCallee::plain(endpoint.machine),
            Some(&Selection(true)),
        )
        .expect("admitted constant argument");
        assert_eq!(values, vec![crate::BuildTimeValue::Int(256)]);
        let error = arguments::evaluate(
            &evaluated,
            &program,
            &admission,
            endpoint.expression,
            super::EndpointCallee::plain(endpoint.machine),
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
    evaluate_const_range_endpoints(&mut program, None)
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
        let errors = evaluate_const_range_endpoints(&mut program, None)
            .expect_err("a signature bound cannot depend on its own invocation");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("cyclic range endpoint")),
            "{errors:?}"
        );
    }
}

#[test]
fn generic_record_arguments_fold_endpoint_calls_before_synthesis() {
    // A declared range endpoint inside a generic argument is a const position:
    // the instance's canonical interval must exist before resolution. The
    // pre-resolution bridge folds each authored call leaf through the shared
    // typed endpoint gate, so downstream const-parameter inference sees the
    // same bound a literal spelling would produce.
    for (argument, instance, bound) in [
        ("u64[0..=limit()]", "RangeValue<u64 in [0..=256]>", 256),
        ("u64[0..limit()]", "RangeValue<u64 in [0..=255]>", 255),
        (
            "u64[0..=endpoint(256)]",
            "RangeValue<u64 in [0..=256]>",
            256,
        ),
        (
            "u64[0..=endpoint(limit())]",
            "RangeValue<u64 in [0..=256]>",
            256,
        ),
        (
            "u64[0..=Limits::capacity()]",
            "RangeValue<u64 in [0..=256]>",
            256,
        ),
        ("u64[0..=limit() + 0]", "RangeValue<u64 in [0..=256]>", 256),
    ] {
        let source = format!(
            "machine limit() -> u64 {{ 256 }}
             machine endpoint(value: u64) -> u64 {{ value }}
             data Limits {{}} machine Limits::capacity() -> u64 {{ 256 }}
             machine upper_bound<const N: u64>(value: u64[0..=N]) -> u64 {{ N }}
             data RangeValue<T [copy]> [copy] {{ value: T; }}
             machine keep() -> u64 {{
                 let bounded: RangeValue<{argument}> = RangeValue {{ value: 0 }};
                 upper_bound(bounded.value)
             }}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(0), &tokens)
                .unwrap();
        let evaluated = crate::evaluate_pre_resolution(crate::BuildTimeEvaluationRequest {
            syntax_trees: syntax,
            source_context: None,
        })
        .unwrap_or_else(|errors| panic!("{argument}: pre-resolution: {errors:?}"));
        let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap_or_else(|errors| panic!("{argument}: resolution: {errors:?}"));
        let mut program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .unwrap_or_else(|error| panic!("{argument}: typed lowering: {error:?}"));
        let instance = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == instance)
            .unwrap_or_else(|| panic!("{argument}: no synthesized instance `{instance}`"));
        assert!(instance.generic_instance.is_some(), "{argument}");
        // The endpoint call folded before resolution, so the typed program has
        // no pending endpoint left at the generic argument.
        assert!(
            pending_endpoints(&program).unwrap().is_empty(),
            "{argument}"
        );
        pre_check
            .evaluate(&mut program)
            .unwrap_or_else(|errors| panic!("{argument}: pre-check: {errors:?}"));
        let checked = typed_trees_to_checked_trees::lower_typed_trees(program)
            .unwrap_or_else(|errors| panic!("{argument}: checked lowering: {errors:?}"));
        let keep = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "keep")
            .expect("consumer machine");
        let admission = crate::BuildTimeAdmissionPlan::infer(&checked.typed, None);
        let execution = admission
            .evaluate_machine_symbol_for_invocation_measured(
                &checked.typed,
                keep.symbol,
                Vec::new(),
                crate::BuildTimeInvocationCustody::Symbol(keep.symbol),
            )
            .unwrap_or_else(|reason| panic!("{argument}: {reason}"));
        assert_eq!(
            *execution.value(),
            crate::BuildTimeValue::Int(bound),
            "{argument}"
        );
    }
}

#[test]
fn domain_qualified_callee_positions_fold_members_and_reject_nonmembers() {
    // `bounded` keeps its declared domain on both the parameter and the
    // result. The endpoint route proves the concrete value's membership
    // through the shared domain-fact evaluator instead of stripping `u64 in
    // Positive` to `u64`; a non-member argument rejects before the body runs.
    let declarations = "domain u64::Positive requires self > 0;
         machine bounded(value: u64 in Positive) -> u64 in Positive { value }
         machine identity(value: u64) -> u64 { value }";
    for (endpoint, expected) in [
        ("bounded(256)", Ok("256")),
        ("bounded(identity(1))", Ok("1")),
        (
            "bounded(0)",
            Err("range endpoint value `0` is outside domain `Positive`"),
        ),
        (
            "bounded(identity(0))",
            Err("range endpoint value `0` is outside domain `Positive`"),
        ),
    ] {
        let mut program = typed(&format!(
            "{declarations}
             machine keep(value: u64[0..={endpoint}]) {{}}"
        ));
        let result = evaluate_const_range_endpoints(&mut program, None);
        match expected {
            Ok(bound) => {
                result.unwrap_or_else(|errors| panic!("{endpoint}: {errors:?}"));
                assert_eq!(
                    folded_maximum(&program).as_deref(),
                    Some(bound),
                    "{endpoint}"
                );
            }
            Err(fragment) => {
                let errors = result.expect_err(endpoint);
                assert_eq!(errors.len(), 1, "{endpoint}: {errors:?}");
                assert!(
                    errors[0].message.contains(fragment),
                    "{endpoint}: {}",
                    errors[0].message
                );
            }
        }
    }
}

#[test]
fn domain_qualified_result_positions_check_the_returned_value() {
    // A result domain is checked on the value the callee actually returned:
    // `zero()` returns 0 into `u64 in Positive` and must not fold.
    let mut program = typed(
        "domain u64::Positive requires self > 0;
         machine zero() -> u64 in Positive { 0 }
         machine keep(value: u64[0..=zero()]) {}",
    );
    let errors = evaluate_const_range_endpoints(&mut program, None).expect_err("non-member result");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0]
            .message
            .contains("range endpoint value `0` is outside domain `Positive`"),
        "{}",
        errors[0].message
    );
}

#[test]
fn proved_parameter_domains_do_not_stand_down_the_fence_for_other_premises() {
    // The membership proof discharges exactly the root's parameter-domain
    // premises. An authored `requires` clause on the callee, or a domain
    // premise on a nested callee the endpoint never proved, must keep the
    // conservative closure fence rather than ride the concrete-premise route.
    for source in [
        "domain u64::Positive requires self > 0;
         machine bounded(value: u64 in Positive) -> u64
         requires
             value <= 512;
         { value }
         machine keep(value: u64[0..=bounded(256)]) {}",
        "domain u64::Positive requires self > 0;
         machine inner(value: u64 in Positive) -> u64 { value }
         machine outer(value: u64) -> u64 { inner(value) }
         machine keep(value: u64[0..=outer(256)]) {}",
    ] {
        let mut program = typed(source);
        let errors = evaluate_const_range_endpoints(&mut program, None)
            .expect_err("an unproved premise keeps the closure fence");
        assert_eq!(errors.len(), 1, "{source}: {errors:?}");
        assert!(
            errors[0]
                .message
                .contains("has an authored `requires` premise"),
            "{source}: {}",
            errors[0].message
        );
    }
}

#[test]
fn boolean_arguments_and_boolean_helpers_fold_into_integer_endpoints() {
    // A Boolean literal, Boolean logic, and a Boolean-returning helper are
    // closed arguments of an integer-returning endpoint callee. Each helper
    // folds to a Boolean literal in argument position; the bound itself is
    // still the integer the callee returned.
    let declarations = "machine pick(wide: bool) -> u64 {
             transition wide {
                 true -> 512
                 false -> 256
             }
         }
         machine is_wide() -> bool { false }
         machine both(first: bool, second: bool) -> bool { first && second }";
    for (endpoint, bound) in [
        ("pick(false)", "256"),
        ("pick(true)", "512"),
        ("pick(is_wide())", "256"),
        ("pick(both(true, is_wide()))", "256"),
        ("pick(true && is_wide())", "256"),
        ("pick(false || true)", "512"),
        ("pick(is_wide()) + 1", "257"),
    ] {
        let mut program = typed(&format!(
            "{declarations}
             machine keep(value: u64[0..={endpoint}]) {{}}"
        ));
        evaluate_const_range_endpoints(&mut program, None)
            .unwrap_or_else(|errors| panic!("{endpoint}: {errors:?}"));
        assert!(
            pending_endpoints(&program).unwrap().is_empty(),
            "{endpoint}"
        );
        // Surrounding bound arithmetic stays authored around the folded
        // literal, so read the bound through the shared closed query.
        let (_, _, constraints) = program
            .type_reference_table
            .constrained_type_reference_sites()[0];
        let TypeConstraintNode::Range { maximum, .. } =
            program.type_reference_table.constraints(constraints)[0]
        else {
            panic!("{endpoint}: authored range");
        };
        assert_eq!(
            validation::closed_integer_range_bound(&program, maximum)
                .map(|value| value.to_string())
                .as_deref(),
            Some(bound),
            "{endpoint}"
        );
    }
}

#[test]
fn boolean_results_never_become_range_bounds_and_mismatched_arguments_reject() {
    // The bound position keeps its integer-carrier requirement even when the
    // Boolean call sits under bound arithmetic; an integer into a Boolean
    // parameter, a Boolean into an integer parameter, and a comparison in a
    // Boolean argument all stay outside this route.
    let declarations = "machine pick(wide: bool) -> u64 {
             transition wide {
                 true -> 512
                 false -> 256
             }
         }
         machine is_wide() -> bool { false }
         machine endpoint(value: u64) -> u64 { value }";
    for (endpoint, fragment) in [
        ("is_wide()", "requires an exact builtin integer carrier"),
        ("is_wide() + 1", "requires an exact builtin integer carrier"),
        ("pick(256)", "admits only Boolean literals"),
        ("endpoint(false)", "closed integer expression"),
        ("pick(1 < 2)", "admits only Boolean literals"),
    ] {
        let mut program = typed(&format!(
            "{declarations}
             machine keep(value: u64[0..={endpoint}]) {{}}"
        ));
        let errors = evaluate_const_range_endpoints(&mut program, None).expect_err(endpoint);
        assert!(
            errors.iter().any(|error| error.message.contains(fragment)),
            "{endpoint}: {errors:?}"
        );
    }
}

#[test]
fn fully_supplied_static_applications_fold_through_their_specialized_instance() {
    // Every binder is supplied by a closed const spelling, so the prepared
    // program's ordinary specialization produces a concrete instance and the
    // endpoint resolves it there. The instance's substituted signature is
    // what the argument and result positions read: `bounded<256>(0)` checks
    // `0` against `u64[0..=256]`, not the template's symbolic `N`.
    let declarations = "machine identity<const N: u64>() -> u64 { N }
         machine choose<const N: u64>(flag: bool) -> u64 {
             transition flag {
                 true -> N
                 false -> 0
             }
         }
         machine flag() -> bool { true }
         machine bounded<const N: u64>(value: u64[0..=N]) -> u64 { N }
         machine limit() -> u64 { 128 }
         const RANGE: u64 = 256;";
    for (endpoint, bound) in [
        ("identity<256>()", "256"),
        ("identity<RANGE>()", "256"),
        ("choose<256>(true)", "256"),
        ("choose<256>(flag())", "256"),
        ("choose<256>(false)", "0"),
        ("bounded<256>(0)", "256"),
        ("identity<256>() + identity<1>()", "257"),
    ] {
        let mut program = typed(&format!(
            "{declarations}
             machine keep(value: u64[0..={endpoint}]) {{}}"
        ));
        assert!(
            !pending_endpoints(&program).unwrap().is_empty(),
            "{endpoint}: static application must be pending"
        );
        evaluate_const_range_endpoints(&mut program, None)
            .unwrap_or_else(|errors| panic!("{endpoint}: {errors:?}"));
        assert!(
            pending_endpoints(&program).unwrap().is_empty(),
            "{endpoint}"
        );
        // `keep` is declared last; `bounded`'s own `u64[0..=N]` precedes it.
        let (_, _, constraints) = *program
            .type_reference_table
            .constrained_type_reference_sites()
            .last()
            .expect("keep's authored range");
        let TypeConstraintNode::Range { maximum, .. } =
            program.type_reference_table.constraints(constraints)[0]
        else {
            panic!("{endpoint}: authored range");
        };
        assert_eq!(
            validation::closed_integer_range_bound(&program, maximum)
                .map(|value| value.to_string())
                .as_deref(),
            Some(bound),
            "{endpoint}"
        );
    }
}

#[test]
fn inference_needing_and_partial_static_applications_stay_rejected() {
    // An application with no static arguments needs inference and is not an
    // endpoint call at all; a partially supplied one is pending so the
    // missing argument is named instead of silently skipped. An instance's
    // substituted range still rejects an out-of-range concrete argument.
    let declarations = "machine identity<const N: u64>() -> u64 { N }
         machine two<const A: u64, const B: u64>() -> u64 { A + B }
         machine bounded<const N: u64>(value: u64[0..=N]) -> u64 { N }";
    let program = typed(&format!(
        "{declarations}
         machine keep(value: u64[0..=bounded(0)]) {{}}"
    ));
    assert!(
        pending_endpoints(&program).unwrap().is_empty(),
        "an inference-needing application is not pending"
    );
    for (endpoint, fragment) in [
        ("two<1>()", "cannot be derived"),
        ("bounded<256>(300)", "outside declared range `0..=256`"),
    ] {
        let mut program = typed(&format!(
            "{declarations}
             machine keep(value: u64[0..={endpoint}]) {{}}"
        ));
        let errors = evaluate_const_range_endpoints(&mut program, None).expect_err(endpoint);
        assert!(
            errors.iter().any(|error| error.message.contains(fragment)),
            "{endpoint}: {errors:?}"
        );
    }
}

fn checked_pipeline(source: &str) -> Result<(), Vec<Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(0), &tokens).unwrap();
    let evaluated = crate::evaluate_pre_resolution(crate::BuildTimeEvaluationRequest {
        syntax_trees: syntax,
        source_context: None,
    })?;
    let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )?;
    let mut program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|error| vec![error])?;
    pre_check.evaluate(&mut program)?;
    typed_trees_to_checked_trees::lower_typed_trees(program).map(|_| ())
}

#[test]
fn generic_record_arguments_still_reject_unclosable_endpoint_calls() {
    // An endpoint call that cannot resolve, and a generic callee whose binder
    // would need inference from an ordinary argument, must both stay rejected
    // rather than weakening admission. (A fully supplied static application
    // is closable and folds through its instance.)
    for source in [
        "machine upper_bound<const N: u64>(value: u64[0..=N]) -> u64 { N }
         data RangeValue<T [copy]> [copy] { value: T; }
         machine keep() -> u64 {
             let bounded: RangeValue<u64[0..=missing()]> = RangeValue { value: 0 };
             upper_bound(bounded.value)
         }",
        "machine upper_bound<const N: u64>(value: u64[0..=N]) -> u64 { N }
         data RangeValue<T [copy]> [copy] { value: T; }
         machine keep() -> u64 {
             let bounded: RangeValue<u64[0..=upper_bound(0)]> = RangeValue { value: 0 };
             upper_bound(bounded.value)
         }",
    ] {
        assert!(
            checked_pipeline(source).is_err(),
            "unclosable endpoint must reject: {source}"
        );
    }
}

/// A selected boundary-operator use inside an endpoint callee: the provider's
/// ordinary checked body computes `left | right` = 7 where builtin `%` would
/// fold 1, so a folded bound of 7 is the positive witness that the provider
/// machine -- not host arithmetic -- ran.
fn provider_endpoint_fixture() -> (TypedTrees, Vec<crate::SelectedBuildTimeProviderBody>) {
    let program = typed(
        "data Math {}
         boundary operator % Math::remainder(left: u64, right: u64) -> u64;
         data Provider {}
         machine Provider::remainder(left: u64, right: u64) -> u64 satisfies Math::remainder { left | right }
         machine limit() -> u64 { let left:u64 = 7; let right:u64 = 2; transition { _ -> (left % right) } }
         machine take_bounded(value: u64[0..=limit()]) -> u64 { value }",
    );
    let rows = provider_rows(&program);
    (program, rows)
}

/// The fixture's selected provider-body rows: every resolved boundary
/// operator use binds `Provider::remainder`'s exact entry under a fixed test
/// plan commitment.
fn provider_rows(program: &TypedTrees) -> Vec<crate::SelectedBuildTimeProviderBody> {
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(program);
    facts
        .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::Resolved)
        .filter(|fact| {
            program.operators().iter().any(|operator| {
                operator.symbol == fact.selected_operator_symbol && operator.is_boundary
            })
        })
        .map(|fact| {
            let provider = program
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Provider::remainder")
                .unwrap();
            let entry = program.machine_states(provider).first().unwrap();
            crate::SelectedBuildTimeProviderBody {
                expression: fact.expression,
                origin: fact.origin,
                requirement: fact.selected_operator_symbol,
                operands: fact.operands(program).unwrap(),
                provider_machine: provider.symbol,
                provider_state: entry.symbol,
                provider_type: provider.attached_data.as_ref().unwrap().as_str().to_owned(),
                provider: checked_trees::CheckedProviderPlanCommitment::from_digest([7; 32]),
            }
        })
        .collect()
}

/// The folded integer bound of the fixture's single range constraint.
fn folded_maximum(program: &TypedTrees) -> Option<String> {
    let mut bounds = Vec::new();
    for (_, _, constraints) in program
        .type_reference_table
        .constrained_type_reference_sites()
    {
        for constraint in program.type_reference_table.constraints(constraints) {
            let TypeConstraintNode::Range { maximum, .. } = constraint else {
                continue;
            };
            if let ExpressionNode::Integer(literal) = program.expression_table.expression(*maximum)
            {
                bounds.push(literal.value_bignum().unwrap().to_string());
            }
        }
    }
    (bounds.len() == 1).then(|| bounds.pop().unwrap())
}

#[test]
fn provider_boundary_endpoint_waits_for_selected_execution() {
    let (program, rows) = provider_endpoint_fixture();
    assert_eq!(rows.len(), 1);
    assert!(
        super::pending_endpoint_calls_need_operator_selection(&program, None).unwrap(),
        "a boundary-operator callee must defer its endpoint until selected rows exist"
    );
    let independent = typed(
        "machine limit() -> u64 { 7 }
         machine take_bounded(value: u64[0..=limit()]) -> u64 { value }",
    );
    assert!(
        !super::pending_endpoint_calls_need_operator_selection(&independent, None).unwrap(),
        "an independent endpoint keeps its early route"
    );
}

#[test]
fn provider_boundary_endpoint_executes_the_selected_body() {
    let (mut program, rows) = provider_endpoint_fixture();
    crate::validate_selected_provider_bodies(&program, &rows).unwrap();
    super::evaluate_selected_range_endpoints(
        &mut program,
        None,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &rows,
        },
    )
    .unwrap_or_else(|errors| panic!("provider-boundary endpoint: {errors:?}"));
    assert_eq!(
        folded_maximum(&program).as_deref(),
        Some("7"),
        "the provider's `left | right` body must run; builtin `%` would fold 1"
    );
}

#[test]
fn unselected_boundary_endpoint_never_falls_back_to_host_semantics() {
    let (mut program, _rows) = provider_endpoint_fixture();
    let errors = super::evaluate_selected_range_endpoints(
        &mut program,
        None,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &[],
        },
    )
    .expect_err("an unselected boundary use must not execute");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("requires exact authored selection")),
        "{errors:?}"
    );
    assert!(folded_maximum(&program).is_none(), "no fold may survive");
}

#[test]
fn deferred_continuation_folds_provider_boundary_endpoint() {
    let source = r#"
data Math {}
boundary operator % Math::remainder(left: u64, right: u64) -> u64;
data Provider {}
machine Provider::remainder(left: u64, right: u64) -> u64 satisfies Math::remainder { left | right }
machine limit() -> u64 { let left:u64 = 7; let right:u64 = 2; transition { _ -> (left % right) } }
machine take_bounded(value: u64[0..=limit()]) -> u64 { value }
"#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(0), &tokens).unwrap();
    let evaluated = crate::evaluate_pre_resolution(crate::BuildTimeEvaluationRequest {
        syntax_trees: syntax,
        source_context: None,
    })
    .unwrap();
    let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let mut program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let Some(pending) = pre_check.evaluate_or_defer(&mut program).unwrap() else {
        panic!("a provider-dependent endpoint must defer its pre-check continuation");
    };
    assert_eq!(
        program.pending_const_range_endpoints.len(),
        1,
        "the deferred endpoint must keep interim checking from reading it as a non-constant bound"
    );
    let rows = provider_rows(&program);
    pending
        .evaluate_selected_operators(
            &mut program,
            SelectedBuildTimeOperators {
                operators: &[],
                provider_bodies: &rows,
            },
        )
        .unwrap_or_else(|errors| panic!("deferred endpoint: {errors:?}"));
    assert_eq!(
        folded_maximum(&program).as_deref(),
        Some("7"),
        "the deferred endpoint must fold under the selected provider body"
    );
    assert!(
        program.pending_const_range_endpoints.is_empty(),
        "the landed endpoint must release its deferred-evaluation mark"
    );
}

/// The deferred mark keeps interim package checking from misreading the
/// pending endpoint as a non-constant bound, while every final checking mode
/// still refuses a program whose continuation never resumed. After the
/// selected fold lands its integer, ordinary checked lowering accepts the
/// same tree.
#[test]
fn deferred_endpoint_mark_admits_interim_checking_only() {
    let source = r#"
data Math {}
boundary operator % Math::remainder(left: u64, right: u64) -> u64;
data Provider {}
machine Provider::remainder(left: u64, right: u64) -> u64 satisfies Math::remainder { left | right }
machine limit() -> u64 { let left:u64 = 7; let right:u64 = 2; transition { _ -> (left % right) } }
machine take_bounded(value: u64[0..=limit()]) -> u64 { value }
"#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(0), &tokens).unwrap();
    let evaluated = crate::evaluate_pre_resolution(crate::BuildTimeEvaluationRequest {
        syntax_trees: syntax,
        source_context: None,
    })
    .unwrap();
    let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let mut program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let Some(pending) = pre_check.evaluate_or_defer(&mut program).unwrap() else {
        panic!("a provider-dependent endpoint must defer its pre-check continuation");
    };
    typed_trees_to_checked_trees::lower_preliminary_typed_trees(program.clone()).unwrap_or_else(
        |errors| panic!("the marked endpoint must survive interim checking: {errors:?}"),
    );
    let errors = typed_trees_to_checked_trees::lower_typed_trees(program.clone())
        .expect_err("final checking must refuse a lost deferred continuation");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("still unevaluated")),
        "{errors:?}"
    );
    let rows = provider_rows(&program);
    pending
        .evaluate_selected_operators(
            &mut program,
            SelectedBuildTimeOperators {
                operators: &[],
                provider_bodies: &rows,
            },
        )
        .unwrap_or_else(|errors| panic!("deferred endpoint: {errors:?}"));
    typed_trees_to_checked_trees::lower_typed_trees(program)
        .unwrap_or_else(|errors| panic!("the folded endpoint must check: {errors:?}"));
}

#[test]
fn template_signature_bounds_with_endpoint_calls_reject_under_explicit_application() {
    // The template's own bound `u64[0..=limit()]` folds in the working tree,
    // but the instance's cloned bound lives only in the prepared tree, which
    // is prepared once before any fold. Positions of a static application
    // read that tree, so the bound is not closed there and the application
    // rejects as an unclosed signature bound rather than folding a wrong
    // value or reading the template's symbolic bound. Closing this needs a
    // second preparation after the template bound folds; it is not admitted
    // by reading the working tree, whose handles the clone does not share.
    for endpoint in [
        "bounded<256>(0)",
        "bounded<256>(300)",
        "result_bounded<256>()",
    ] {
        let mut program = typed(&format!(
            "machine limit() -> u64 {{ 256 }}
             machine bounded<const N: u64>(value: u64[0..=limit()]) -> u64 {{ N }}
             machine result_bounded<const N: u64>() -> u64[0..=limit()] {{ N }}
             machine keep(value: u64[0..={endpoint}]) {{}}"
        ));
        let errors = evaluate_const_range_endpoints(&mut program, None).expect_err(endpoint);
        assert_eq!(errors.len(), 1, "{endpoint}: {errors:?}");
        assert!(
            errors[0]
                .message
                .contains("range endpoint signature bound is not closed"),
            "{endpoint}: {}",
            errors[0].message
        );
        // The failed application must not leave a partial fold behind.
        assert!(folded_maximum(&program).is_none(), "{endpoint}");
    }
}
