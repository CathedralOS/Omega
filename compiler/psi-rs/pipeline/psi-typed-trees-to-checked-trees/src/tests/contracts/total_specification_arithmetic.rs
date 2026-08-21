use super::*;

fn checked(
    source: &str,
) -> Result<psi_checked_trees::CheckedTrees, Vec<psi_diagnostics::Diagnostic>> {
    lower_typed_trees(parse_typed_trees(source))
}

#[test]
fn direct_trapping_arithmetic_is_illegal_in_machine_contracts() {
    let source = r#"
        machine add(left: i32 in Trapping, right: i32 in Trapping) -> bool
        requires
            left + right > 0
        crashes Trap
            left - right < 0
        {
            true
        }
    "#;

    let diagnostics = checked(source).expect_err("Trapping arithmetic cannot form a proposition");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("direct Trapping arithmetic `+`")
            && diagnostic.message.contains("requires contract")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("direct Trapping arithmetic `-`")
            && diagnostic.message.contains("crashes contract")
    }));
}

#[test]
fn trapping_policy_conversion_is_illegal_in_a_contract() {
    let source = r#"
        machine convert(value: i32) -> bool
        requires
            (value as i32 in Trapping) == value
        {
            true
        }
    "#;

    let diagnostics = checked(source).expect_err("Trapping conversion cannot form a proposition");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("direct Trapping conversion is illegal")
    }));
}

#[test]
fn total_operations_on_trapping_values_remain_legal_in_contracts() {
    let source = r#"
        machine classify(left: i32 in Trapping, right: i32 in Trapping) -> bool
        requires
            left == right
        requires
            (left & right) == 0
        {
            true
        }
    "#;

    checked(source).expect("equality and bitwise inspection are total");
}

#[test]
fn wrapping_and_saturating_arithmetic_remain_total_contract_terms() {
    let source = r#"
        machine wrapped(left: i32 in Wrapping, right: i32 in Wrapping) -> bool
        requires
            left + right == left
        {
            true
        }

        machine saturated(left: i32 in Saturating, right: i32 in Saturating) -> bool
        requires
            left * right == right
        {
            true
        }
    "#;

    checked(source).expect("Wrapping and Saturating arithmetic are total");
}

#[test]
fn direct_trapping_arithmetic_is_illegal_in_state_arrival_contracts() {
    let source = r#"
        machine staged(value: i32 in Trapping) -> bool {
            transition { _ -> next(value) }

            state next(value: i32 in Trapping) -> bool
            requires
                value + 1 > 0
            {
                true
            }
        }
    "#;

    let diagnostics = checked(source).expect_err("state contracts are total propositions");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("direct Trapping arithmetic `+`")
            && diagnostic
                .message
                .contains("state `next` requires contract")
    }));
}

#[test]
fn named_float_trapping_arithmetic_rejects_but_classification_remains_total() {
    let rejected = r#"
        data F64 {}
        boundary operator F64::negate(value: f64 in Trapping) -> f64;

        machine classify(value: f64 in Trapping) -> bool
        requires
            F64::negate(value) == value
        {
            true
        }
    "#;
    let diagnostics = checked(rejected).expect_err("named Trapping float arithmetic rejects");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("direct Trapping named float operation `negate`")
    }));

    let accepted = r#"
        data F64 {}
        boundary operator F64::is_finite(value: f64) -> bool;

        machine classify(value: f64 in Trapping) -> bool
        requires
            F64::is_finite(value)
        {
            true
        }
    "#;
    checked(accepted).expect("float classification is total on Trapping values");
}

#[test]
fn custom_float_returning_operator_is_not_mistaken_for_float_arithmetic() {
    let source = r#"
        data Utility {}
        boundary operator Utility::identity(value: f64 in Trapping) -> f64;

        machine classify(value: f64 in Trapping) -> bool
        requires
            Utility::identity(value) == value
        {
            true
        }
    "#;

    checked(source).expect("a custom float-returning operator is not a primitive arithmetic row");
}

#[test]
fn trapping_arithmetic_is_illegal_in_trait_and_machine_parameter_signatures() {
    let source = r#"
        trait ArithmeticRule {
            machine accepts(value: i32 in Trapping) -> bool
            requires
                value + 1 > 0;
        }

        machine apply<machine Selected>()
        where machine Selected(value: i32 in Trapping) -> bool
            requires value - 1 > 0;
        {}
    "#;

    let diagnostics = checked(source).expect_err("abstract callable contracts inhabit Prop");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("trait `ArithmeticRule` state `accepts` requires contract")
            && diagnostic
                .message
                .contains("direct Trapping arithmetic `+`")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("machine-parameter requirement `Selected` requires contract")
            && diagnostic
                .message
                .contains("direct Trapping arithmetic `-`")
    }));
}

#[test]
fn trapping_arithmetic_is_illegal_in_operator_contracts() {
    let source = r#"
        data Arithmetic {}
        boundary operator Arithmetic::increment(value: i32 in Trapping) -> i32
        ensures
            result == value + 1;
    "#;

    let diagnostics = checked(source).expect_err("operator contracts inhabit Prop");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator `Arithmetic::increment` ensures contract")
            && diagnostic
                .message
                .contains("direct Trapping arithmetic `+`")
    }));
}

#[test]
fn trapping_arithmetic_is_illegal_in_domain_data_and_trait_predicates() {
    let source = r#"
        domain i32::Risky
        requires
            (self as i32 in Trapping) + 1 > 0;

        data Ledger
        where
            count + 1 > 0,
        {
            count: i32 in Trapping;
        }

        trait InvariantRule {
            invariant (1 as i32 in Trapping) + 1 > 0;
        }
    "#;

    let diagnostics = checked(source).expect_err("predicate facts inhabit Prop");
    for owner in [
        "domain `i32::Risky` predicate",
        "data `Ledger` default-domain predicate",
        "trait `InvariantRule` invariant",
    ] {
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(owner)
                    && diagnostic
                        .message
                        .contains("direct Trapping arithmetic `+`")
            }),
            "missing totality diagnostic for {owner}: {diagnostics:?}"
        );
    }
}

#[test]
fn total_abstract_contract_operations_remain_legal() {
    let source = r#"
        trait ClassificationRule {
            machine accepts(left: i32 in Trapping, right: i32 in Trapping) -> bool
            requires
                left == right
            requires
                (left & right) == 0;
        }

        data Arithmetic {}
        boundary operator Arithmetic::wrapped(value: i32 in Wrapping) -> i32
        ensures
            result == value + 1;

        machine apply<machine Selected>()
        where machine Selected(value: i32 in Saturating) -> bool
            requires value * 2 == value;
        {}
    "#;

    checked(source).expect("abstract contracts retain their total selected operations");
}
