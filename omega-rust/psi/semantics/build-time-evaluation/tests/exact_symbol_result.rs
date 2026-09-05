use build_time_evaluation::{BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeValue};

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize exact-symbol fixture");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse exact-symbol fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve exact-symbol fixture");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type exact-symbol fixture")
}

#[test]
fn exact_symbol_structured_evaluation_never_reselects_a_sibling_by_name() {
    let typed = typed(
        r#"
        data Left {}
        data Right {}

        machine Left::binding() -> i64 {
            11
        }

        machine Right::binding() -> i64 {
            22
        }
        "#,
    );
    let left = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Left::binding")
        .expect("left producer");
    let right = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Right::binding")
        .expect("right producer");
    assert_ne!(left.symbol, right.symbol);

    let admission = BuildTimeAdmissionPlan::infer(&typed);
    let evaluated = admission
        .evaluate_machine_symbol_for_invocation_measured(
            &typed,
            right.symbol,
            Vec::new(),
            BuildTimeInvocationCustody::Symbol(right.symbol),
        )
        .expect("evaluate the exact right-hand producer");

    assert_eq!(evaluated.value(), &BuildTimeValue::Int(22));
    assert_eq!(evaluated.usage().result_cells(), 1);
}

#[test]
fn exact_symbol_structured_evaluation_rejects_a_non_machine_symbol() {
    let typed = typed(
        r#"
        data BindingLookalike {}

        machine binding() -> i64 {
            7
        }
        "#,
    );
    let data_symbol = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "BindingLookalike")
        .expect("data declaration")
        .symbol;
    let admission = BuildTimeAdmissionPlan::infer(&typed);
    let error = admission
        .evaluate_machine_symbol_for_invocation_measured(
            &typed,
            data_symbol,
            Vec::new(),
            BuildTimeInvocationCustody::Symbol(data_symbol),
        )
        .expect_err("an exact data symbol must not fall back to the machine spelling");
    assert!(error.contains("no machine with exact symbol"));
}
