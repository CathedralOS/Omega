use super::super::lower_typed_trees;
use super::parse_typed_trees;
use crate::CheckingRequest;

#[test]
fn carrier_safety_proofs_preserve_tighter_argument_bounds() {
    for (carrier, guard, value, maximum) in [
        ("u32", "left <= 4294967295 - right", "left + right", 7),
        ("u32", "left >= right", "left - right", 4),
        ("u32", "left <= 4294967295 / right", "left * right", 10),
        ("u64", "true", "left << right", 20),
    ] {
        let source = format!(
            "machine run(left: {carrier} [0..=5], right: {carrier} [1..=2]) -> {carrier} {{
                transition {guard} {{ true -> accept({value}) false -> 0 }}
                state accept(delivered: {carrier} [0..={maximum}]) -> {carrier} {{ delivered }}
            }}"
        );
        lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
            .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
        assert!(
            lower_typed_trees(
                parse_typed_trees(
                    &source.replace(&format!("0..={maximum}"), &format!("0..={}", maximum - 1)),
                ),
                &CheckingRequest::settled()
            )
            .is_err(),
            "a tighter-than-proved target accepted: {source}"
        );
    }
}

#[test]
fn named_transition_nonliteral_bounds_reject_insufficient_or_unrelated_guards() {
    for (guard, step_range) in [
        ("pending >= 0", "1..=1"),
        ("pending > 0", "1..=2"),
        ("other > 0", "1..=1"),
        ("!(pending > 0)", "1..=1"),
        ("pending > 0 || other > 0", "1..=1"),
    ] {
        let source = format!(
            "machine run(pending: u32 [0..=5], step: u32 [{step_range}], other: u32) -> u32 {{
                transition {guard} {{ true -> accept(pending - step) false -> 0 }}
                state accept(delivered: u32 [0..=4]) -> u32 {{ delivered }}
            }}"
        );
        let program = parse_typed_trees(&source);
        let plan = proof::obligations::build_proof_plan(&program);
        assert!(proof::checker::check_proof_plan(&plan).is_err(), "{source}");
    }
}

#[test]
fn named_transition_arithmetic_query_rejects_selected_and_effectful_trees() {
    for (declarations, guard, first, delivered) in [
        (
            "operator - u32::custom(left: u32, right: u32) -> u32;",
            "pending > 0",
            "0",
            "pending - step",
        ),
        (
            "operator > u32::custom(left: u32, right: u32) -> bool;",
            "pending > 0",
            "0",
            "pending - step",
        ),
        (
            "operator + u32::custom(left: u32, right: u32) -> u32;",
            "pending > 0",
            "pending + step",
            "pending - step",
        ),
        (
            "machine zero(target: &mut u32) -> u32 { target = 0; 0 }",
            "pending > 0",
            "zero(&mut pending)",
            "pending - step",
        ),
        (
            "",
            "pending > 0",
            "0",
            "((pending as u32 in Wrapping) - step) as u32",
        ),
    ] {
        let source = format!(
            "{declarations}
            machine run(input: u32 [0..=5], step: u32 [1..=1]) -> u32 {{
                let mut pending: u32 = input;
                transition {guard} {{ true -> accept({first}, {delivered}) false -> 0 }}
                state accept(first: u32, delivered: u32 [0..=4]) -> u32 {{ delivered }}
            }}"
        );
        let program = parse_typed_trees(&source);
        let plan = proof::obligations::build_proof_plan(&program);
        let obligation = plan
            .obligations
            .iter()
            .find_map(|(_, obligation)| match obligation {
                proof::obligations::ProofObligation::BoundedTransitionArgument(argument)
                    if argument.parameter.as_str() == "delivered" =>
                {
                    Some(argument)
                }
                _ => None,
            })
            .expect("bounded argument occurrence");
        assert_eq!(
            validation::arrival_integer_expression_bounds(
                &program,
                obligation.machine_symbol,
                obligation.state_symbol,
                obligation.statement_index,
                obligation.argument,
            ),
            None,
            "{source}"
        );
    }
}

#[test]
fn named_transition_range_evidence_belongs_to_its_exact_occurrence() {
    let program = parse_typed_trees(
        "machine run(pending: u32 [0..=5], step: u32 [1..=1]) -> u32 {
            transition pending > 0 { true -> accept(pending - step) false -> 0 }
            state accept(delivered: u32 [0..=4]) -> u32 { delivered }
        }",
    );
    let plan = proof::obligations::build_proof_plan(&program);
    let obligation = plan
        .obligations
        .iter()
        .find_map(|(_, obligation)| match obligation {
            proof::obligations::ProofObligation::BoundedTransitionArgument(argument) => {
                Some(argument)
            }
            _ => None,
        })
        .expect("bounded argument occurrence");
    let query = |statement_index, expression| {
        validation::arrival_integer_expression_bounds(
            &program,
            obligation.machine_symbol,
            obligation.state_symbol,
            statement_index,
            expression,
        )
    };
    assert_eq!(
        query(obligation.statement_index, obligation.argument),
        Some((0, 4))
    );
    assert_eq!(query(usize::MAX, obligation.argument), None);
    assert_eq!(
        query(
            obligation.statement_index,
            typed_trees::expression::ExpressionHandle::invalid()
        ),
        None
    );
}

#[test]
fn strict_float_calls_retain_and_enforce_the_authored_endpoint() {
    for argument in ["1.0", "1.4999999", "0.0"] {
        let source = format!(
            "machine accept(value: f64 [0.0..1.5]) -> f64 {{ value }}
             machine run() {{ _ = accept({argument}); }}"
        );
        lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
            .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
    }

    for (argument, expected) in [("1.5", "expected 0..1.5"), ("2.0", "expected 0..1.5")] {
        let source = format!(
            "machine accept(value: f64 [0.0..1.5]) -> f64 {{ value }}
             machine run() {{ _ = accept({argument}); }}"
        );
        let diagnostics =
            lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
                .expect_err("out-of-range float call must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{diagnostics:#?}"
        );
    }

    let unknown = r#"
        machine accept(value: f64 [0.0..1.5]) -> f64 { value }
        machine run(value: f64) { _ = accept(value); }
    "#;
    assert!(
        lower_typed_trees(parse_typed_trees(unknown), &CheckingRequest::settled()).is_err(),
        "an unconstrained float call must reject"
    );

    let strict_to_inclusive = r#"
        machine wide(value: f64 [0.0..=1.5]) -> f64 { value }
        machine run(value: f64 [0.0..1.5]) { _ = wide(value); }
    "#;
    lower_typed_trees(
        parse_typed_trees(strict_to_inclusive),
        &CheckingRequest::settled(),
    )
    .expect("a strict source range fits an inclusive target");

    let inclusive_to_strict = r#"
        machine narrow(value: f64 [0.0..1.5]) -> f64 { value }
        machine run(value: f64 [0.0..=1.5]) { _ = narrow(value); }
    "#;
    assert!(
        lower_typed_trees(
            parse_typed_trees(inclusive_to_strict),
            &CheckingRequest::settled()
        )
        .is_err(),
        "an inclusive source range must not fit a strict target"
    );
}

#[test]
fn guarded_transition_argument_certificate_stays_uncovered_when_unproven() {
    // `fuel >= 1` narrows `fuel - 1` only to `[0, 127]`: the certificate
    // route emits nothing rather than a certificate the kernel must refuse,
    // and the ordinary derivation reports the gap.
    let program = parse_typed_trees(
        "machine run(fuel: u64 [1..=128], matched: bool) -> u64 {
            transition { fuel >= 1 -> advance(fuel - 1) _ -> 0 }
            state advance(delivered: u64 [1..=128]) -> u64 { delivered }
        }",
    );
    let plan = proof::obligations::build_proof_plan(&program);
    let obligation = plan
        .obligations
        .iter()
        .find_map(|(_, obligation)| match obligation {
            proof::obligations::ProofObligation::BoundedTransitionArgument(argument) => {
                Some(argument)
            }
            _ => None,
        })
        .expect("bounded argument occurrence");
    let target = proof::obligations::IntegerRange {
        minimum: numerics::bignum::BigInt::from_i64(1),
        maximum: numerics::bignum::BigInt::from_i64(128),
    };
    assert_eq!(
        proof::checker::guarded_transition_integer_verdict(&plan, obligation, &target, 1),
        proof::checker::CertificateVerdict::Uncovered,
    );
    assert!(proof::checker::check_proof_plan(&plan).is_err());
}

#[test]
fn anonymous_landed_arguments_stay_off_the_guarded_certificate_route() {
    // `7 / 2 * 2` lands at 7 by exact-rational anonymous arithmetic while its
    // derived constraint interval claims `[6, 6]`: the anonymous leg owns the
    // verdict, so the guarded route must not re-litigate it from premises the
    // landing already falsified.
    let program = parse_typed_trees(
        "machine run() -> i32 {
            transition { _ -> finish(7 / 2 * 2) }
            state finish(delivered: i32 [6..=6]) -> i32 { delivered }
        }",
    );
    let plan = proof::obligations::build_proof_plan(&program);
    let obligation = plan
        .obligations
        .iter()
        .find_map(|(_, obligation)| match obligation {
            proof::obligations::ProofObligation::BoundedTransitionArgument(argument) => {
                Some(argument)
            }
            _ => None,
        })
        .expect("bounded argument occurrence");
    let target = proof::obligations::IntegerRange {
        minimum: numerics::bignum::BigInt::from_i64(6),
        maximum: numerics::bignum::BigInt::from_i64(6),
    };
    assert_eq!(
        proof::checker::guarded_transition_integer_verdict(&plan, obligation, &target, 1),
        proof::checker::CertificateVerdict::Uncovered,
    );
    assert!(proof::checker::check_proof_plan(&plan).is_err());
}

#[test]
fn bounded_argument_proof_independently_preserves_negation_polarity() {
    // Exercise the proof consumer directly so rejection cannot be attributed
    // solely to the earlier arithmetic validation pass.
    for (condition, accepted) in [
        ("!(fuel <= 1)", true),
        ("!!(fuel > 1)", true),
        ("!(fuel > 1)", false),
        ("!!(fuel <= 1)", false),
    ] {
        let source = format!(
            "machine run(fuel: u64 [1..=128]) -> u64 {{
                transition {{ {condition} -> advance(fuel - 1) _ -> 0 }}
                state advance(delivered: u64 [1..=128]) -> u64 {{ delivered }}
            }}"
        );
        let program = parse_typed_trees(&source);
        let plan = proof::obligations::build_proof_plan(&program);
        match proof::checker::check_proof_plan(&plan) {
            Ok(()) => assert!(accepted, "{source}"),
            Err(diagnostics) => {
                assert!(!accepted, "{source}\n{diagnostics:#?}");
                assert!(
                    diagnostics.iter().any(|diagnostic| {
                        diagnostic
                            .message
                            .contains("cannot prove transition argument")
                            && diagnostic.message.contains("bounded parameter")
                    }),
                    "{diagnostics:#?}"
                );
            }
        }
    }
}
