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
fn wrapping_and_saturating_division_require_an_independent_nonzero_fact() {
    let accepted = r#"
        machine divide(left: i32 in Wrapping, divisor: i32 in Wrapping) -> bool
        requires divisor >= 1
        requires
            left / divisor == left
        {
            true
        }

        machine remainder(left: i32 in Saturating, divisor: i32 in Saturating) -> bool
        requires divisor <= -1
        requires
            left % divisor == left
        {
            true
        }
    "#;
    checked(accepted).expect("a prior nonzero interval makes division and remainder total");

    let rejected = r#"
        machine divide(left: i32 in Wrapping, divisor: i32 in Wrapping) -> bool
        requires
            left / divisor == left
        {
            true
        }

        machine remainder(left: i32 in Saturating, divisor: i32 in Saturating) -> bool
        requires
            left % divisor == left
        {
            true
        }
    "#;
    let diagnostics = checked(rejected)
        .expect_err("overflow policy does not define a zero divisor in a proposition");
    for operation in ["division", "remainder"] {
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(operation)
                    && diagnostic.message.contains("must be proven nonzero")
            }),
            "missing {operation} definedness diagnostic: {diagnostics:?}",
        );
    }
}

#[test]
fn wrapping_division_cannot_use_its_containing_fact_as_nonzero_evidence() {
    let source = r#"
        machine divide(left: i32 in Wrapping, divisor: i32 in Wrapping) -> bool
        requires
            divisor >= 1 && left / divisor == left
        {
            true
        }
    "#;

    let diagnostics = checked(source).expect_err("a partial term cannot prove its own formation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("division")
            && diagnostic.message.contains("must be proven nonzero")
    }));
}

#[test]
fn abstract_division_definedness_uses_only_prior_contract_facts() {
    let accepted = r#"
        trait ArithmeticRule {
            machine divide(left: i32 in Wrapping, divisor: i32 in Wrapping) -> bool
            requires divisor >= 1
            requires
                left / divisor == left;

            machine remainder(left: i32 in Saturating, divisor: i32 in Saturating) -> bool
            requires divisor <= -1
            requires
                left % divisor == left;
        }
    "#;
    checked(accepted).expect("abstract prior facts make the direct terms total");

    let rejected = r#"
        trait ArithmeticRule {
            machine divide(left: i32 in Wrapping, divisor: i32 in Wrapping) -> bool
            requires
                divisor >= 1 && left / divisor == left;
        }
    "#;
    let diagnostics =
        checked(rejected).expect_err("an abstract fact cannot justify its own partial term");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("trait `ArithmeticRule` state `divide` requires contract")
            && diagnostic.message.contains("division")
            && diagnostic.message.contains("must be proven nonzero")
    }));
}

#[test]
fn abstract_exact_and_saturating_shifts_require_a_prior_valid_count() {
    let accepted = r#"
        trait ShiftRule {
            machine exact(value: u8, count: u8) -> bool
            requires count < 8
            requires
                value >> count == value;

            machine saturating(value: u8 in Saturating, count: u8) -> bool
            requires count < 8
            requires
                value << count == value;

            machine wrapping(value: u8 in Wrapping, count: u8) -> bool
            requires
                value << count == value;
        }
    "#;
    checked(accepted)
        .expect("prior bounds form Exact/Saturating shifts while Wrapping defines every count");

    let rejected = r#"
        trait ShiftRule {
            machine exact(value: u8, count: u8) -> bool
            requires
                value >> count == value;

            machine saturating(value: u8 in Saturating, count: u8) -> bool
            requires
                value << count == value;
        }
    "#;
    let diagnostics =
        checked(rejected).expect_err("Exact and Saturating abstract shifts retain count validity");
    for state in ["exact", "saturating"] {
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(&format!(
                    "trait `ShiftRule` state `{state}` requires contract"
                )) && diagnostic.message.contains("shift count")
                    && diagnostic.message.contains("not provably below")
            }),
            "missing abstract shift-count diagnostic for {state}: {diagnostics:?}",
        );
    }
}

#[test]
fn abstract_shift_cannot_use_its_containing_fact_as_count_evidence() {
    let source = r#"
        trait ShiftRule {
            machine shift(value: u8 in Saturating, count: u8) -> bool
            requires
                count < 8 && value << count == value;
        }
    "#;

    let diagnostics = checked(source).expect_err("a partial shift cannot justify its own count");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("trait `ShiftRule` state `shift` requires contract")
            && diagnostic.message.contains("shift count")
            && diagnostic.message.contains("not provably below")
    }));
}

#[test]
fn concrete_exact_division_requires_prior_definedness_facts() {
    let accepted = r#"
        machine positive(left: i32, divisor: i32) -> bool
        requires divisor >= 1
        requires
            left / divisor == left
        {
            true
        }

        machine negative(left: i32, divisor: i32) -> bool
        requires divisor <= -2
        requires
            left % divisor == left
        {
            true
        }

        machine minus_one(left: i32, divisor: i32) -> bool
        requires left >= -2147483647
        requires divisor == -1
        requires
            left / divisor == left
        {
            true
        }
    "#;
    checked(accepted).expect("prior facts exclude zero and signed MIN/-1");

    let rejected = r#"
        machine divide(left: i32, divisor: i32) -> bool
        requires
            left / divisor == left
        {
            true
        }

        machine remainder(left: i32, divisor: i32) -> bool
        requires
            left % divisor == left
        {
            true
        }
    "#;
    let diagnostics = checked(rejected).expect_err("Exact division is a partial primitive");
    for operation in ["division", "remainder"] {
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains(&format!("partial exact {operation}"))
                    && diagnostic.message.contains("must be proven nonzero")
                    && diagnostic.message.contains("MIN")
            }),
            "missing Exact {operation} definedness diagnostic: {diagnostics:?}",
        );
    }
}

#[test]
fn concrete_exact_zero_divisor_keeps_the_existing_single_diagnostic() {
    let source = r#"
        machine divide(left: i32) -> bool
        requires
            left / 0 == left
        {
            true
        }
    "#;

    let diagnostics = checked(source).expect_err("a provably zero divisor is always invalid");
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message.contains("division by zero"))
            .count(),
        1,
        "the existing arithmetic analyzer owns the concrete exact-zero diagnostic: {diagnostics:?}",
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("partial exact division")),
        "the specification-definedness pass must not duplicate exact zero: {diagnostics:?}",
    );
}

#[test]
fn exact_division_cannot_use_its_containing_fact_as_definedness_evidence() {
    let source = r#"
        machine divide(left: i32, divisor: i32) -> bool
        requires
            divisor >= 1 && left / divisor == left
        {
            true
        }
    "#;

    let diagnostics = checked(source).expect_err("a partial term cannot prove its own formation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("partial exact division")
            && diagnostic
                .message
                .contains("independently accepted prior facts")
    }));
}

#[test]
fn abstract_exact_division_definedness_uses_only_prior_facts() {
    let accepted = r#"
        trait DivisionRule {
            machine positive(left: i32, divisor: i32) -> bool
            requires divisor >= 1
            requires
                left / divisor == left;

            machine minus_one(left: i32, divisor: i32) -> bool
            requires left >= -2147483647
            requires divisor == -1
            requires
                left % divisor == left;
        }
    "#;
    checked(accepted).expect("abstract prior facts discharge exact definedness");

    let rejected = r#"
        trait DivisionRule {
            machine divide(left: i32, divisor: i32) -> bool
            requires
                divisor >= 1 && left / divisor == left;

            machine remainder(left: i32, divisor: i32) -> bool
            requires divisor == -1
            requires
                left % divisor == left;
        }
    "#;
    let diagnostics = checked(rejected)
        .expect_err("abstract exact operations retain both definedness conditions");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("trait `DivisionRule` state `divide` requires contract")
            && diagnostic.message.contains("must be proven nonzero")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("trait `DivisionRule` state `remainder` requires contract")
            && diagnostic.message.contains("MIN")
    }));
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

#[test]
fn prior_contract_facts_discharge_exact_arithmetic_after_policy_erasure() {
    let source = r#"
        machine add(left: i8 in Trapping, right: i8 in Trapping) -> bool
        requires left >= 0
        requires left <= 100
        requires right >= 0
        requires right <= 20
        requires
            (left as i8) + (right as i8) <= 120
        {
            true
        }
    "#;

    checked(source).expect("prior facts prove the explicitly Exact sum representable");
}

#[test]
fn unproved_exact_arithmetic_after_policy_erasure_is_rejected() {
    let source = r#"
        machine add(left: i8 in Trapping, right: i8 in Trapping) -> bool
        requires
            (left as i8) + (right as i8) >= 0
        {
            true
        }
    "#;

    let diagnostics = checked(source).expect_err("policy erasure does not prove Exact formation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("exact arithmetic in machine `add` requires contract may overflow `i8`")
    }));
}

#[test]
fn exact_operation_cannot_use_its_containing_fact_to_justify_formation() {
    let source = r#"
        machine add(left: i8 in Trapping, right: i8 in Trapping) -> bool
        requires
            (left as i8) + (right as i8) <= 127
        {
            true
        }
    "#;

    let diagnostics = checked(source).expect_err("a proposition cannot form its own partial term");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("exact arithmetic in machine `add` requires contract may overflow `i8`")
    }));
}

#[test]
fn unsigned_widened_product_contract_is_total_by_carrier_width() {
    let source = r#"
        machine bounded(left: u32, right: u32) -> bool
        requires
            (left as u64) * (right as u64) <= 12
        {
            true
        }
    "#;

    checked(source).expect("two u32 factors are exactly representable in one u64 product");
}

#[test]
fn signed_product_contract_does_not_inherit_unsigned_width_proof() {
    let source = r#"
        machine bounded(left: i64, right: i64) -> bool
        requires
            left * right <= 12
        {
            true
        }
    "#;

    let diagnostics = checked(source).expect_err("signed multiplication may overflow");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("exact arithmetic in machine `bounded` requires contract may overflow `i64`")
    }));
}

#[test]
fn abstract_prior_facts_discharge_exact_arithmetic_after_policy_erasure() {
    let source = r#"
        trait ArithmeticRule {
            machine add(left: i8 in Trapping, right: i8 in Trapping) -> bool
            requires left >= 0
            requires left <= 100
            requires right >= 0
            requires right <= 20
            requires
                (left as i8) + (right as i8) <= 120;
        }
    "#;

    checked(source).expect("abstract prior facts prove the explicitly Exact sum representable");
}

#[test]
fn abstract_unproved_policy_erasure_does_not_form_exact_arithmetic() {
    let source = r#"
        trait ArithmeticRule {
            machine add(left: i8 in Trapping, right: i8 in Trapping) -> bool
            requires
                (left as i8) + (right as i8) >= 0;
        }
    "#;

    let diagnostics = checked(source).expect_err("abstract policy erasure retains Exact formation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("exact arithmetic in trait `ArithmeticRule` state `add` requires contract may overflow `i8`")
    }));
}

#[test]
fn abstract_exact_operation_cannot_justify_itself() {
    let source = r#"
        trait ArithmeticRule {
            machine add(left: i8 in Trapping, right: i8 in Trapping) -> bool
            requires
                (left as i8) + (right as i8) <= 127;
        }
    "#;

    let diagnostics = checked(source).expect_err("abstract facts cannot admit their own terms");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("exact arithmetic in trait `ArithmeticRule` state `add` requires contract may overflow `i8`")
    }));
}

#[test]
fn abstract_policy_erasure_with_a_literal_retains_exact_formation() {
    let accepted = r#"
        trait ArithmeticRule {
            machine add(value: i8 in Trapping) -> bool
            requires value <= 126
            requires
                (value as i8) + 1 <= 127;
        }
    "#;
    checked(accepted).expect("a prior bound discharges cast-plus-literal formation");

    let rejected = r#"
        trait ArithmeticRule {
            machine add(value: i8 in Trapping) -> bool
            requires
                (value as i8) + 127 >= 0;
        }
    "#;
    let diagnostics =
        checked(rejected).expect_err("a literal does not erase the cast operand's obligation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("exact arithmetic in trait `ArithmeticRule` state `add` requires contract may overflow `i8`")
    }));
}
