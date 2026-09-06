use super::*;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add("normal_return_calls.omg".into(), source.into())
        .source_id;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        std::sync::Arc::new(sources),
    )
    .unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn selected_call(program: &TypedTrees, target: &str) -> TableCallExpression {
    program
        .expression_table
        .expression_nodes()
        .find_map(|expression| {
            let ExpressionNode::Call(call) = expression else {
                return None;
            };
            (call.target.as_str() == target).then(|| call.clone())
        })
        .expect("the source call")
}

fn eligible(program: &TypedTrees, call: &TableCallExpression) -> bool {
    let operational = crate::infer_operational_may(program);
    let reaches = crate::infer_service_reaches(program, &operational);
    normal_return_call_candidate(program, call, &operational, &reaches).is_ok()
}

const COST: &str = "
    machine cost(level: u32[1..=10]) -> u32[15..=60] { 10 + level * 5 }
    machine run(level: u32[1..=10]) -> u32 { cost(level) }
";

#[test]
fn normal_return_equality_does_not_require_premature_checked_termination() {
    let program = typed(COST);
    let call = selected_call(&program, "cost");
    let (machine, _) = plain_value_call_target(&program, &call).unwrap();
    assert!(!crate::denotational_calls::unconditionally_terminates(
        &program,
        machine.symbol
    ));
    assert!(eligible(&program, &call));
}

#[test]
fn requires_remain_caller_obligations_not_an_eligibility_theorem() {
    let program = typed(
        "
        machine cost(level: u32) -> u32 requires level > 0 { level }
        machine run(level: u32) -> u32 { cost(level) }
    ",
    );
    let call = selected_call(&program, "cost");
    assert!(eligible(&program, &call));
    let operational = crate::infer_operational_may(&program);
    let reaches = crate::infer_service_reaches(&program, &operational);
    let mut diagnostics = Vec::new();
    assert!(
        crate::fact_call_projections::validate_checked_call_candidate(
            &program,
            &call,
            &operational,
            &reaches,
            &|reason, diagnostics| diagnostics.push(diagnostics::Diagnostic::error(reason)),
            &mut diagnostics,
        )
        .is_none()
    );
    assert!(!diagnostics.is_empty());
}

#[test]
fn target_and_runtime_arity_cannot_be_recovered_from_matching_text() {
    let program = typed(COST);
    let original = selected_call(&program, "cost");
    let mut call = original.clone();
    call.target_symbol = SymbolHandle::invalid();
    assert!(!eligible(&program, &call));
    let mut call = original.clone();
    call.arguments = arena::HandleSpan::empty();
    assert!(!eligible(&program, &call));
    let mut call = original;
    call.receiver = program.expression_table.expression_handles(call.arguments)[0];
    assert!(!eligible(&program, &call));
}

#[test]
fn reachable_checked_value_helpers_share_the_existing_effect_closure() {
    let mut program = typed(
        "
        machine identity(level: u32) -> u32 { level }
        machine cost(level: u32) -> u32 { identity(level) }
        machine run(level: u32) -> u32 { cost(level) }
    ",
    );
    let call = selected_call(&program, "cost");
    assert!(eligible(&program, &call));
    program.machines_mut()[0].body_is_present = false;
    assert!(!eligible(&program, &call));
}

#[test]
fn mutable_or_reference_inputs_do_not_become_value_only_reads() {
    for source in [
        "machine cost(mut level: u32) -> u32 { level = 1; level }
         machine run(level: u32) -> u32 { cost(level) }",
        "machine cost(level: &u32) -> u32 { 1 }
         machine run(level: &u32) -> u32 { cost(level) }",
    ] {
        let program = typed(source);
        assert!(!eligible(&program, &selected_call(&program, "cost")));
    }
}

#[test]
fn custom_operator_bodies_do_not_inherit_empty_machine_call_effects() {
    for (declaration, expected) in [
        (
            "operator + u32::custom(left: u32, right: u32) -> u32;",
            false,
        ),
        (
            "operator + f64::unrelated(left: f64, right: f64) -> f64;",
            true,
        ),
    ] {
        // One exact u32 operand lets the existing meaning owner rule out the
        // unrelated f64 declaration. Computed operand types stay conservative.
        let program = typed(&format!(
            "{declaration}
             machine cost(level: u32 [1..=10]) -> u32 {{ level + 1 }}
             machine run(level: u32 [1..=10]) -> u32 {{ cost(level) }}"
        ));
        assert_eq!(
            eligible(&program, &selected_call(&program, "cost")),
            expected
        );
    }
}

#[test]
fn unresolved_body_reads_do_not_inherit_parameter_spelling() {
    let mut program = typed(COST);
    let call = selected_call(&program, "cost");
    let (machine, state) = plain_value_call_target(&program, &call).unwrap();
    let parameter = program.state_parameters(state)[0].symbol;
    assert!(machine.symbol.is_valid());
    let expression = program
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| {
            matches!(node, ExpressionNode::Name(path) if path.symbol == parameter).then_some(handle)
        })
        .unwrap();
    let ExpressionNode::Name(path) = program.expression_table.expression_mut(expression) else {
        unreachable!()
    };
    path.symbol = SymbolHandle::invalid();
    assert!(!eligible(&program, &call));
}

#[test]
fn absent_effect_summaries_are_not_evidence_of_purity() {
    let program = typed(COST);
    let call = selected_call(&program, "cost");
    let operational = crate::infer_operational_may(&program);
    let reaches = crate::infer_service_reaches(&program, &operational);
    assert!(
        normal_return_call_candidate(
            &program,
            &call,
            &flow_effects::OperationalPlan::default(),
            &reaches
        )
        .is_err()
    );
}
