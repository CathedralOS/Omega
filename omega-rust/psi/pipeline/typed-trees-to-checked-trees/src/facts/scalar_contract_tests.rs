use super::*;
use checked_trees::{
    CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedOperatorUseFact,
    ClosedScalarContractValue,
};

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn closed_literal_contracts_require_builtin_equality_meaning() {
    for (scalar, literal) in [("u16", "7u16"), ("bool", "true")] {
        for declared in [false, true] {
            let declaration = if declared {
                format!(
                    "boundary operator == Meaning::equal(left: {scalar}, right: {scalar}) -> bool;"
                )
            } else {
                String::new()
            };
            let program = typed(&format!(
                r#"
                {declaration}
                machine value() -> {scalar}
                requires {literal} == {literal}
                ensures {literal} == {literal}
                {{ {literal} }}
            "#
            ));
            let machine = program.machines().first().unwrap();
            // Contract expressions need not appear among execution-value
            // operator rows. An empty checked roster must not imply builtin.
            let plan = build_closed_scalar_value_contract_plan(
                &program,
                machine,
                &CheckedOperatorFacts::default(),
            );
            assert_eq!(plan.requires()[0].is_some(), !declared);
            assert_eq!(plan.ensures()[0].is_some(), !declared);
        }
    }
}

#[test]
fn integer_comparison_declarations_do_not_replace_boolean_tautologies() {
    let program = typed(
        r#"
        boundary operator == Meaning::equal(left: u16, right: u16) -> bool;
        machine value() -> bool
        requires true == true
        ensures false == false
        { false }
    "#,
    );
    let plan = build_closed_scalar_value_contract_plan(
        &program,
        program.machines().first().unwrap(),
        &CheckedOperatorFacts::default(),
    );
    assert_eq!(
        plan.requires(),
        &[Some(ClosedScalarContractValue::Boolean(true))]
    );
    assert_eq!(
        plan.ensures(),
        &[Some(ClosedScalarContractValue::Boolean(false))]
    );
}

#[test]
fn result_predicates_and_literal_requirements_gate_their_own_meanings() {
    for (spelling, require_builtin, ensure_builtin) in [("==", false, true), ("<", true, false)] {
        let program = typed(&format!(
            r#"
            boundary operator {spelling} Meaning::compare(left: u16, right: u16) -> bool;
            machine value() -> u16
            requires 7u16 == 7u16
            ensures result < 256u16
            {{ 7u16 }}
        "#
        ));
        let plan = build_closed_scalar_value_contract_plan(
            &program,
            program.machines().first().unwrap(),
            &CheckedOperatorFacts::default(),
        );
        assert_eq!(plan.requires()[0].is_some(), require_builtin, "{spelling}");
        assert_eq!(
            matches!(
                plan.ensures()[0],
                Some(ClosedScalarContractValue::ResultPredicate(_))
            ),
            ensure_builtin,
            "{spelling}"
        );
    }
}

#[test]
fn unknown_literal_carriers_do_not_hide_heterogeneous_comparators() {
    let program = typed(
        r#"
        boundary operator == Meaning::compare(left: u16, right: bool) -> bool;
        machine value() -> u16
        requires 7u16 == 7u16
        ensures result == 7u16
        { 7u16 }
    "#,
    );
    let plan = build_closed_scalar_value_contract_plan(
        &program,
        program.machines().first().unwrap(),
        &CheckedOperatorFacts::default(),
    );
    assert_eq!(plan.requires(), &[None]);
    assert_eq!(plan.ensures(), &[None]);
}

#[test]
fn retained_nonbuiltin_operator_status_cannot_be_replaced_by_literal_shape() {
    let program = typed(
        r#"
        machine value() -> u16
        requires 7u16 == 7u16
        ensures result == 7u16
        { 7u16 }
    "#,
    );
    let machine = program.machines().first().unwrap();
    for status in [
        CheckedOperatorResolutionStatus::Missing,
        CheckedOperatorResolutionStatus::Resolved,
        CheckedOperatorResolutionStatus::Ambiguous,
        CheckedOperatorResolutionStatus::DomainPending,
        CheckedOperatorResolutionStatus::Inadmissible,
        CheckedOperatorResolutionStatus::BuiltinFallback,
    ] {
        let mut operators = CheckedOperatorFacts::default();
        for contract in program.machine_contracts(machine) {
            let [typed_trees::domain::ProofFact::Expression(expression)] =
                program.proof_facts.span_or_empty(contract.facts)
            else {
                panic!("one contract expression");
            };
            operators.uses.append(CheckedOperatorUseFact {
                expression: *expression,
                status,
                ..Default::default()
            });
        }
        let plan = build_closed_scalar_value_contract_plan(&program, machine, &operators);
        let builtin = status == CheckedOperatorResolutionStatus::BuiltinFallback;
        assert_eq!(plan.requires()[0].is_some(), builtin, "{status:?}");
        assert_eq!(plan.ensures()[0].is_some(), builtin, "{status:?}");
    }
}
