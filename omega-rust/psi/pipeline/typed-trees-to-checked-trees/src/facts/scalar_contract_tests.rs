use super::*;
use checked_trees::{
    CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedOperatorUseFact,
    ClosedScalarContractValue,
};

mod parameter_predicates;

#[test]
fn canonical_membership_contract_bytes_retain_normalized_indices() {
    let encode = |index: &str| {
        let program = typed(&format!(
            "domain<const I: u64> i64::Coordinate<I>; machine run(value: i64 in Coordinate<{index}>) {{ }}"
        ));
        let fact = program
            .proof_facts
            .iter()
            .find_map(|(_, fact)| {
                matches!(fact, typed_trees::domain::ProofFact::Membership(_)).then_some(fact)
            })
            .expect("membership");
        let mut bytes = Vec::new();
        encode_contract_fact_canonical(
            &program,
            fact,
            &["value".to_owned()],
            &[],
            true,
            &mut bytes,
        );
        bytes
    };
    assert_ne!(encode("7"), encode("9"));
    assert_eq!(encode("7"), encode("(7 + 0)"));
}

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
fn nested_literal_comparisons_retain_exact_carriers_and_operator_meaning() {
    for comparison in ["1u8 < 2u8", "1u8 < 2u16"] {
        for declaration in [
            "",
            "boundary operator < Meaning::compare(left: u8, right: u8) -> bool;",
        ] {
            let program = typed(&format!(
                "{declaration} machine value() -> bool ensures result == ({comparison}) {{ true }}"
            ));
            let plan = build_closed_scalar_value_contract_plan(
                &program,
                program.machines().first().unwrap(),
                &CheckedOperatorFacts::default(),
            );
            let expected = declaration.is_empty() && comparison == "1u8 < 2u8";
            assert_eq!(
                plan.ensures()[0].is_some(),
                expected,
                "{declaration} {comparison}"
            );
            if expected {
                let Some(ClosedScalarContractValue::Predicate(
                    checked_trees::CheckedBooleanExpression::Equal { right, .. },
                )) = &plan.ensures()[0]
                else {
                    panic!("retained Boolean equality");
                };
                let checked_trees::CheckedBooleanExpression::IntegerComparison {
                    left, right, ..
                } = right.as_ref()
                else {
                    panic!("literal comparison must not be folded out of source custody");
                };
                for operand in [left, right] {
                    let checked_trees::CheckedScalarExpression::IntegerLiteral { literal } =
                        operand.as_ref()
                    else {
                        panic!("literal operand");
                    };
                    assert_eq!(
                        literal.landing().unwrap().landed_type,
                        numerics::literals::LandedIntegerType::U8
                    );
                }
            }
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
                Some(ClosedScalarContractValue::Predicate(_))
            ),
            ensure_builtin,
            "{spelling}"
        );
    }
}

#[test]
fn literal_carrier_identity_decides_heterogeneous_comparator_overlap() {
    for (literal, admitted) in [("7u16", true), ("7", false)] {
        let program = typed(&format!(
            r#"
        boundary operator == Meaning::compare(left: u16, right: bool) -> bool;
        machine value() -> u16
        requires {literal} == {literal}
        ensures result == {literal}
        {{ 7u16 }}
    "#,
        ));
        let plan = build_closed_scalar_value_contract_plan(
            &program,
            program.machines().first().unwrap(),
            &CheckedOperatorFacts::default(),
        );
        // A source-owned u16 landing excludes a bool operand, but contextual
        // landing cannot choose builtin meaning before operator selection.
        assert_eq!(plan.requires()[0].is_some(), admitted, "{literal}");
        assert_eq!(plan.ensures()[0].is_some(), admitted, "{literal}");
    }
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
