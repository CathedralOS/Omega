//! Checked crash call tests.

use super::{
    CrashPredicateExpression, SymbolHandle, TypedTrees, infer_checked_crash_causes,
    infer_checked_machine_crash_causes,
};
use crate::facts::crash_calls::private_summaries::PrivateSummaryDependency;
use crate::facts::crash_calls::private_summaries::PrivateSummaryEquation;
use crate::facts::crash_calls::private_summaries::solve_private_summary_fixed_point;
use crate::facts::crash_calls::summary_predicates::CallArgumentSubstitution;
use crate::facts::crash_calls::summary_predicates::SummaryCrashPredicate;
use crate::facts::crash_calls::summary_predicates::SummaryCrashRouteGuard;
use crate::facts::crash_calls::summary_predicates::normalize_summary_guards;
use crate::facts::crash_calls::summary_predicates::summary_boolean_value;
use crate::facts::crash_calls::{
    SummaryCrashBucket, crash_predicate_from_expression, normalize_summary_buckets,
};

fn integer_comparison(
    operator: typed_trees::expression::BinaryOperator,
    left: &str,
    right: &str,
) -> CrashPredicateExpression {
    CrashPredicateExpression::Binary {
        operator: operator as u8,
        left: Box::new(CrashPredicateExpression::Integer(left.into())),
        right: Box::new(CrashPredicateExpression::Integer(right.into())),
    }
}

#[test]
fn closed_summary_integer_comparisons_use_exact_literal_values() {
    use typed_trees::expression::{BinaryOperator, UnaryOperator};
    for (operator, expected) in [(BinaryOperator::And, false), (BinaryOperator::Or, true)] {
        let expression = CrashPredicateExpression::Binary {
            operator: operator as u8,
            left: Box::new(integer_comparison(BinaryOperator::Equal, "2", "2")),
            right: Box::new(integer_comparison(BinaryOperator::Equal, "2", "0")),
        };
        assert_eq!(summary_boolean_value(&expression), Some(expected));
    }
    for (operator, left, right, expected) in [
        (BinaryOperator::Equal, "0xff", "255", true),
        (BinaryOperator::NotEqual, "0b11", "0o3", false),
        (BinaryOperator::Less, "-9223372036854775809", "0", true),
        (
            BinaryOperator::LessOrEqual,
            "18446744073709551615",
            "18446744073709551615",
            true,
        ),
        (
            BinaryOperator::Greater,
            "18446744073709551615",
            "9223372036854775807",
            true,
        ),
        (BinaryOperator::GreaterOrEqual, "-0x10", "-15", false),
    ] {
        let comparison = integer_comparison(operator, left, right);
        assert_eq!(summary_boolean_value(&comparison), Some(expected));
        let inverted = CrashPredicateExpression::Unary {
            operator: UnaryOperator::LogicalNot as u8,
            operand: Box::new(comparison),
        };
        assert_eq!(summary_boolean_value(&inverted), Some(!expected));
    }
    for malformed in ["", "-", "0x", "--1", "+1", "1u64", "1_0", "0b2"] {
        assert_eq!(
            summary_boolean_value(&integer_comparison(BinaryOperator::Equal, malformed, "0")),
            None
        );
    }
    assert_eq!(
        summary_boolean_value(&integer_comparison(BinaryOperator::Add, "1", "1")),
        None
    );
    assert_eq!(
        summary_boolean_value(&CrashPredicateExpression::Parameter(0)),
        None
    );
}

#[test]
fn forwarded_numeric_guards_discharge_without_scalar_annotations() {
    use typed_trees::expression::BinaryOperator;
    let guard = CrashPredicateExpression::Binary {
        operator: BinaryOperator::Equal as u8,
        left: Box::new(CrashPredicateExpression::Parameter(0)),
        right: Box::new(CrashPredicateExpression::Integer("0".into())),
    };
    for (actual, survives) in [
        ("0", true),
        ("2", false),
        ("-1", false),
        ("18446744073709551615", false),
    ] {
        let mut observed = Vec::new();
        for scalar in [
            None,
            Some(checked_trees::CheckedBooleanExpression::Constant(false)),
        ] {
            let bucket = SummaryCrashBucket {
                cause: checked_trees::CrashCause::Trap,
                alternative_guards: vec![SummaryCrashRouteGuard::Predicate(
                    SummaryCrashPredicate {
                        identity: guard.clone(),
                        builtin_meaning: true,
                        scalar,
                    },
                )],
            };
            let forwarded = bucket.substitute(&identity_substitution(vec![Some(
                CrashPredicateExpression::Parameter(0),
            )]));
            let concrete = forwarded.substitute(&identity_substitution(vec![Some(
                CrashPredicateExpression::Integer(actual.into()),
            )]));
            let causes = normalize_summary_buckets(vec![concrete])
                .into_iter()
                .map(|bucket| bucket.cause)
                .collect::<Vec<_>>();
            assert_eq!(!causes.is_empty(), survives);
            observed.push(causes);
        }
        assert_eq!(observed[0], observed[1]);
    }
    let bucket = SummaryCrashBucket {
        cause: checked_trees::CrashCause::Trap,
        alternative_guards: vec![predicate(guard)],
    };
    assert_eq!(
        bucket.substitute(&identity_substitution(vec![None])),
        SummaryCrashBucket::unconditional(checked_trees::CrashCause::Trap)
    );
}

#[test]
fn summary_normalization_cannot_launder_builtin_meaning() {
    use typed_trees::expression::BinaryOperator;
    let identity = CrashPredicateExpression::Binary {
        operator: BinaryOperator::Equal as u8,
        left: Box::new(CrashPredicateExpression::Parameter(0)),
        right: Box::new(CrashPredicateExpression::Integer("0".into())),
    };
    for meanings in [[true, false], [false, true]] {
        let mut guards = meanings
            .into_iter()
            .map(|builtin_meaning| {
                SummaryCrashRouteGuard::Predicate(SummaryCrashPredicate {
                    identity: identity.clone(),
                    builtin_meaning,
                    scalar: None,
                })
            })
            .collect::<Vec<_>>();
        normalize_summary_guards(&mut guards);
        assert_eq!(guards.len(), 2);
        let bucket = SummaryCrashBucket {
            cause: checked_trees::CrashCause::Trap,
            alternative_guards: guards,
        };
        let substituted = bucket.substitute(&identity_substitution(vec![Some(
            CrashPredicateExpression::Integer("2".into()),
        )]));
        let [SummaryCrashRouteGuard::Predicate(survivor)] =
            substituted.alternative_guards.as_slice()
        else {
            panic!("unknown selected meaning must retain its route");
        };
        assert!(!survivor.builtin_meaning);
    }
}

#[test]
fn authored_integer_comparison_does_not_supply_summary_builtin_meaning() {
    for custom in [false, true] {
        let declaration = if custom {
            "boundary operator == Meaning::equal(left: u16, right: u16) -> bool;"
        } else {
            ""
        };
        let source = format!(
            "{declaration} machine value(input: u16) -> u16 crashes Trap input == 0u16 {{ input }}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "value")
            .unwrap();
        let expression = program
            .machine_contracts(machine)
            .iter()
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .find_map(|fact| {
                if let typed_trees::domain::ProofFact::Expression(expression) = fact {
                    Some(*expression)
                } else {
                    None
                }
            })
            .unwrap();
        let builtin_meaning = validation::has_builtin_bound_expression_meaning(
            &program,
            machine,
            program.machine_states(machine).first(),
            expression,
        );
        assert_eq!(builtin_meaning, !custom);
        let identity =
            crash_predicate_from_expression(&program, expression, &["input".into()], None);
        let bucket = SummaryCrashBucket {
            cause: checked_trees::CrashCause::Trap,
            alternative_guards: vec![SummaryCrashRouteGuard::Predicate(SummaryCrashPredicate {
                identity,
                builtin_meaning,
                scalar: None,
            })],
        };
        let concrete = bucket.substitute(&identity_substitution(vec![Some(
            CrashPredicateExpression::Integer("2".into()),
        )]));
        assert_eq!(
            !normalize_summary_buckets(vec![concrete]).is_empty(),
            custom
        );
    }
}

#[test]
fn concrete_call_summary_preserves_authored_comparison_meaning() {
    for custom in [false, true] {
        let declaration = if custom {
            "boundary operator == Meaning::equal(left: u16, right: u16) -> bool;"
        } else {
            ""
        };
        let source = format!(
            "{declaration}
            machine value(input: u16) -> u16 crashes Trap input == 0u16 {{ input }}
            machine caller() -> u16 {{ value(2u16) }}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let caller = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "caller")
            .unwrap()
            .symbol;
        let checked = crate::lower_typed_trees(program);
        if custom {
            if let Ok(checked) = checked {
                assert_ne!(
                    infer_checked_machine_crash_causes(&checked.typed, &checked.facts, caller),
                    Some(Vec::new()),
                    "authored equality cannot become builtin crash discharge"
                );
            }
        } else {
            let checked = checked.expect("builtin guarded call checks");
            assert_eq!(
                infer_checked_machine_crash_causes(&checked.typed, &checked.facts, caller),
                Some(Vec::new())
            );
        }
    }
}

#[test]
fn callee_boolean_meaning_cannot_authorize_custom_actual_comparison() {
    for body in [
        "value(2u16 == 0u16)",
        "let flag: bool = 2u16 == 0u16; value(flag)",
    ] {
        let source = format!(
            "boundary operator == Meaning::equal(left: u16, right: u16) -> bool;
             machine value(flag: bool) -> u16 crashes Trap flag {{ 7u16 }}
             machine caller() -> u16 {{ {body} }}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let caller = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "caller")
            .unwrap()
            .symbol;
        if let Ok(checked) = crate::lower_typed_trees(program) {
            assert_ne!(
                infer_checked_machine_crash_causes(&checked.typed, &checked.facts, caller),
                Some(Vec::new()),
                "callee Boolean guard cannot establish custom actual meaning: {body}"
            );
        }
    }
}

fn predicate(identity: CrashPredicateExpression) -> SummaryCrashRouteGuard {
    SummaryCrashRouteGuard::Predicate(SummaryCrashPredicate {
        identity,
        builtin_meaning: false,
        scalar: None,
    })
}

fn identity_substitution(
    identity: Vec<Option<CrashPredicateExpression>>,
) -> CallArgumentSubstitution {
    CallArgumentSubstitution {
        scalar: vec![None; identity.len()],
        fields: vec![None; identity.len()],
        values: vec![None; identity.len()],
        identity,
    }
}

#[test]
fn arithmetic_actual_guards_discharge_through_checked_scalar_evidence() {
    use checked_trees::{
        CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
        CheckedScalarExpression,
    };
    use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType};
    use typed_trees::expression::BinaryOperator;
    use typed_trees::types::PrimitiveType;

    let literal = |text: &str| CheckedScalarExpression::IntegerLiteral {
        literal: IntegerLiteral::from_parts(false, IntegerRadix::Decimal, text)
            .unwrap()
            .with_landing(IntegerLanding {
                landed_type: LandedIntegerType::U64,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            }),
    };
    // `divide(value - 1)` inside a forwarding body: the published guard
    // `value == 0` retains the arithmetic actual over the caller's entry.
    let identity = CrashPredicateExpression::Binary {
        operator: BinaryOperator::Equal as u8,
        left: Box::new(CrashPredicateExpression::Binary {
            operator: BinaryOperator::Subtract as u8,
            left: Box::new(CrashPredicateExpression::Parameter(0)),
            right: Box::new(CrashPredicateExpression::Integer("1".into())),
        }),
        right: Box::new(CrashPredicateExpression::Integer("0".into())),
    };
    let scalar = CheckedBooleanExpression::IntegerComparison {
        kind: CheckedIntegerComparisonKind::Equal,
        left: Box::new(CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactSubtract,
            primitive_type: PrimitiveType::U64,
            left: Box::new(CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::U64,
            }),
            right: Box::new(literal("1")),
        }),
        right: Box::new(literal("0")),
    };
    let bucket = SummaryCrashBucket {
        cause: checked_trees::CrashCause::Trap,
        alternative_guards: vec![SummaryCrashRouteGuard::Predicate(SummaryCrashPredicate {
            identity,
            builtin_meaning: true,
            scalar: Some(scalar),
        })],
    };
    let substitute = |actual: &str| {
        let mut substitution =
            identity_substitution(vec![Some(CrashPredicateExpression::Integer(actual.into()))]);
        substitution.scalar = vec![Some(literal(actual))];
        normalize_summary_buckets(vec![bucket.substitute(&substitution)])
    };
    // `3 - 1 == 0` is decided false: the guarded Trap discharges.
    assert!(substitute("3").is_empty());
    // `1 - 1 == 0` is decided true: the route is unconditional.
    assert_eq!(
        substitute("1"),
        vec![SummaryCrashBucket::unconditional(
            checked_trees::CrashCause::Trap
        )],
    );
    // `0 - 1` cannot produce a u64 under the exact domain, so the guard
    // stays undecidable and the route survives as a predicate.
    let surviving_buckets = substitute("0");
    let [surviving] = surviving_buckets.as_slice() else {
        panic!("the undecidable arithmetic guard retains its route")
    };
    let [SummaryCrashRouteGuard::Predicate(_)] = surviving.alternative_guards.as_slice() else {
        panic!("exact-domain underflow keeps the guarded route")
    };
    // The identity alone cannot fold arithmetic: without the annotation
    // the same substitution only retains.
    let mut without_scalar = bucket.clone();
    without_scalar.alternative_guards =
        vec![SummaryCrashRouteGuard::Predicate(SummaryCrashPredicate {
            scalar: None,
            ..match &without_scalar.alternative_guards[0] {
                SummaryCrashRouteGuard::Predicate(predicate) => predicate.clone(),
                _ => unreachable!(),
            }
        })];
    let mut substitution =
        identity_substitution(vec![Some(CrashPredicateExpression::Integer("3".into()))]);
    substitution.scalar = vec![Some(literal("3"))];
    let [SummaryCrashRouteGuard::Predicate(_)] = without_scalar
        .substitute(&substitution)
        .alternative_guards
        .as_slice()
    else {
        panic!("an arithmetic identity alone cannot decide the guard")
    };
}

#[test]
fn missing_call_actual_provenance_widens_instead_of_retaining_callee_parameter() {
    let route = SummaryCrashBucket {
        cause: checked_trees::CrashCause::Trap,
        alternative_guards: vec![predicate(CrashPredicateExpression::Parameter(0))],
    };
    for actuals in [Vec::new(), vec![None]] {
        assert_eq!(
            route.substitute(&identity_substitution(actuals)),
            SummaryCrashBucket::unconditional(checked_trees::CrashCause::Trap),
            "a missing actual must not relabel the callee formal as a caller entry input",
        );
    }
}

#[test]
fn unreferenced_unknown_actual_does_not_erase_exact_guard_substitution() {
    let route = SummaryCrashBucket {
        cause: checked_trees::CrashCause::Trap,
        alternative_guards: vec![predicate(CrashPredicateExpression::Parameter(1))],
    };
    let substituted = route.substitute(&identity_substitution(vec![
        None,
        Some(CrashPredicateExpression::Parameter(2)),
    ]));
    assert_eq!(
        substituted.alternative_guards,
        vec![predicate(CrashPredicateExpression::Parameter(2))]
    );
}

#[test]
fn summary_guard_normalization_keeps_checked_scalar_structure() {
    let identity = CrashPredicateExpression::Parameter(0);
    let scalar = checked_trees::CheckedBooleanExpression::Parameter { position: 0 };
    let mut guards = vec![
        predicate(identity.clone()),
        SummaryCrashRouteGuard::Predicate(SummaryCrashPredicate {
            identity,
            builtin_meaning: false,
            scalar: Some(scalar.clone()),
        }),
    ];

    normalize_summary_guards(&mut guards);

    let [SummaryCrashRouteGuard::Predicate(predicate)] = guards.as_slice() else {
        panic!("equivalent predicates should merge into one guarded route")
    };
    assert_eq!(predicate.scalar, Some(scalar));
}

#[test]
fn cause_only_summary_does_not_depend_on_scalar_annotations() {
    use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression, CrashCause};

    for replacement in [
        CrashPredicateExpression::Boolean(false),
        CrashPredicateExpression::Boolean(true),
        CrashPredicateExpression::Parameter(1),
    ] {
        let without_scalar = SummaryCrashBucket {
            cause: CrashCause::Trap,
            alternative_guards: vec![predicate(CrashPredicateExpression::Parameter(0))],
        };
        let with_scalar = SummaryCrashBucket {
            cause: CrashCause::Trap,
            alternative_guards: vec![SummaryCrashRouteGuard::Predicate(SummaryCrashPredicate {
                identity: CrashPredicateExpression::Parameter(0),
                builtin_meaning: false,
                scalar: Some(CheckedBooleanExpression::Parameter { position: 0 }),
            })],
        };
        let scalar = match &replacement {
            CrashPredicateExpression::Boolean(value) => CheckedBooleanExpression::Constant(*value),
            CrashPredicateExpression::Parameter(position) => CheckedBooleanExpression::Parameter {
                position: *position as usize,
            },
            _ => unreachable!(),
        };
        let mut substitution = identity_substitution(vec![Some(replacement)]);
        let without = normalize_summary_buckets(vec![without_scalar.substitute(&substitution)]);
        substitution.scalar = vec![Some(CheckedScalarExpression::Boolean(Box::new(scalar)))];
        let with = normalize_summary_buckets(vec![with_scalar.substitute(&substitution)]);
        assert_eq!(
            without, with,
            "scalar annotations do not select or erase causes"
        );
    }
}

#[test]
fn cause_query_missing_machine_is_unknown_not_complete_empty() {
    assert!(
        infer_checked_crash_causes(
            &TypedTrees::default(),
            &checked_trees::CheckFacts::default(),
        )
        .is_empty()
    );
    assert_eq!(
        infer_checked_machine_crash_causes(
            &TypedTrees::default(),
            &checked_trees::CheckFacts::default(),
            SymbolHandle::from_arena_index(1),
        ),
        None,
    );
}

#[test]
fn private_summary_fixed_point_closes_recursive_components() {
    let first = SymbolHandle::from_arena_index(1);
    let second = SymbolHandle::from_arena_index(2);
    let abort = SummaryCrashBucket::unconditional(checked_trees::CrashCause::Abort);
    let guarded_abort = SummaryCrashBucket {
        cause: checked_trees::CrashCause::Abort,
        alternative_guards: vec![predicate(CrashPredicateExpression::Parameter(0))],
    };
    let trap = SummaryCrashBucket::unconditional(checked_trees::CrashCause::Trap);
    let equations = vec![
        PrivateSummaryEquation {
            machine: first,
            direct: Vec::new(),
            private_dependencies: vec![PrivateSummaryDependency {
                machine: second,
                substitution: identity_substitution(Vec::new()),
                recursive: true,
            }],
            published_dependencies: vec![guarded_abort],
        },
        PrivateSummaryEquation {
            machine: second,
            direct: vec![trap.clone()],
            private_dependencies: vec![PrivateSummaryDependency {
                machine: first,
                substitution: identity_substitution(Vec::new()),
                recursive: true,
            }],
            published_dependencies: Vec::new(),
        },
    ];

    let summaries = solve_private_summary_fixed_point(&equations);
    for machine in [first, second] {
        let buckets = summaries
            .iter()
            .find_map(|(candidate, buckets)| (*candidate == machine).then_some(buckets))
            .expect("each recursive member has a summary");
        assert!(buckets.contains(&abort));
        assert!(buckets.contains(&trap));
    }
}

#[test]
fn private_summary_preserves_acyclic_guard_substitution() {
    let leaf = SymbolHandle::from_arena_index(1);
    let wrapper = SymbolHandle::from_arena_index(2);
    let route = SummaryCrashBucket {
        cause: checked_trees::CrashCause::Trap,
        alternative_guards: vec![predicate(CrashPredicateExpression::Parameter(0))],
    };
    let equations = vec![
        PrivateSummaryEquation {
            machine: leaf,
            direct: Vec::new(),
            private_dependencies: Vec::new(),
            published_dependencies: vec![route],
        },
        PrivateSummaryEquation {
            machine: wrapper,
            direct: Vec::new(),
            private_dependencies: vec![PrivateSummaryDependency {
                machine: leaf,
                substitution: identity_substitution(vec![Some(
                    CrashPredicateExpression::Parameter(1),
                )]),
                recursive: false,
            }],
            published_dependencies: Vec::new(),
        },
    ];

    let summaries = solve_private_summary_fixed_point(&equations);
    let [bucket] = summaries
        .iter()
        .find_map(|(machine, buckets)| (*machine == wrapper).then_some(buckets.as_slice()))
        .expect("wrapper summary")
    else {
        panic!("wrapper should retain one guarded bucket")
    };
    assert_eq!(
        bucket.alternative_guards,
        vec![predicate(CrashPredicateExpression::Parameter(1))]
    );
}

fn call_site_buckets(
    source: &str,
    caller: &str,
) -> Vec<(
    checked_trees::CrashCause,
    Vec<checked_trees::CrashRouteGuard>,
)> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = crate::lower_typed_trees(program)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == caller)
        .expect("caller machine");
    let plan = checked
        .facts
        .contract_plans
        .for_machine(machine.symbol)
        .expect("caller contract plan");
    let [call] = plan.crash.checked_calls() else {
        panic!("one checked call site: {source}")
    };
    call.surviving_buckets()
        .iter()
        .map(|bucket| (bucket.cause(), bucket.alternative_guards().to_vec()))
        .collect()
}

/// A guarded route retained as a caller-entry predicate, or an unconditional
/// cause when the call's own entry contexts decided or widened it.
fn single_surviving_bucket(
    buckets: &[(
        checked_trees::CrashCause,
        Vec<checked_trees::CrashRouteGuard>,
    )],
) -> &checked_trees::CrashRouteGuard {
    let [(checked_trees::CrashCause::Trap, guards)] = buckets else {
        panic!("exactly one surviving Trap bucket: {buckets:?}")
    };
    let [guard] = guards.as_slice() else {
        panic!("exactly one surviving guard: {guards:?}")
    };
    guard
}

#[test]
fn pristine_mutable_actual_keeps_entry_operand_identity() {
    // A pristine `mut` parameter is still an entry snapshot: the retained
    // route names the caller's own entry operand, not current storage.
    let buckets = call_site_buckets(
        "machine inner(input: bool) -> bool crashes Trap input { input }
         machine outer(mut flag: bool) -> bool crashes Trap flag { inner(flag) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the pristine actual retains its guarded entry operand: {buckets:?}")
    };
    assert_eq!(
        identity.expression(),
        Some(&CrashPredicateExpression::Parameter(0))
    );
}

#[test]
fn written_mutable_actual_uses_live_storage_value_not_entry_identity() {
    // `flag = false` ends entry provenance for `inner(flag)`. The call's own
    // entry contexts still prove the actual false, so the guarded route
    // discharges instead of retaining a current-storage operand.
    assert!(
        call_site_buckets(
            "machine inner(input: bool) -> bool crashes Trap input { input }
             machine outer(mut flag: bool) -> bool crashes Trap flag { flag = false; inner(flag) }",
            "outer",
        )
        .is_empty(),
        "a proven-false actual discharges the guarded route"
    );
    // `flag = true` decides the same guard true: the cause survives
    // unconditionally under the caller's own `crashes Trap` ceiling.
    let buckets = call_site_buckets(
        "machine inner(input: bool) -> bool crashes Trap input { input }
         machine outer(mut flag: bool) -> bool crashes Trap { flag = true; inner(flag) }",
        "outer",
    );
    assert_eq!(
        single_surviving_bucket(&buckets),
        &checked_trees::CrashRouteGuard::Truth,
        "a proven-true actual keeps the cause without inventing entry identity"
    );
}

#[test]
fn unknown_written_mutable_actual_stays_conservative() {
    // `flag = !flag` overwrites the storage with a value the call's entry
    // contexts cannot prove. Neither channel may manufacture evidence, so the
    // cause widens to its unconditional bucket rather than discharging.
    let buckets = call_site_buckets(
        "machine inner(input: bool) -> bool crashes Trap input { input }
         machine outer(mut flag: bool) -> bool crashes Trap { flag = !flag; inner(flag) }",
        "outer",
    );
    assert_eq!(
        single_surviving_bucket(&buckets),
        &checked_trees::CrashRouteGuard::Truth,
        "an unproven mutable actual must not silently discharge the route"
    );
}

#[test]
fn sibling_written_mutable_actual_keeps_the_read_fields_entry_identity() {
    // `rec.other = 1` ends `rec`'s whole-operand entry provenance, but the
    // guard reads only `cell.count`: the surviving route names the caller's
    // `rec.count` entry snapshot instead of widening to `Truth`.
    let buckets = call_site_buckets(
        "data Rec { count: i32; other: i32; }
         machine inner(cell: &Rec) -> bool crashes Trap !(cell.count >= 0) { true }
         machine outer(mut rec: Rec) -> bool crashes Trap { rec.other = 1; inner(&rec) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the sibling write leaves the read field's entry operand: {buckets:?}")
    };
    use typed_trees::expression::{BinaryOperator, UnaryOperator};
    assert_eq!(
        identity.expression(),
        Some(&CrashPredicateExpression::Unary {
            operator: UnaryOperator::LogicalNot as u8,
            operand: Box::new(CrashPredicateExpression::Binary {
                operator: BinaryOperator::GreaterOrEqual as u8,
                left: Box::new(CrashPredicateExpression::Member {
                    receiver: Box::new(CrashPredicateExpression::Parameter(0)),
                    member: "count".to_owned(),
                }),
                right: Box::new(CrashPredicateExpression::Integer("0".to_owned())),
            }),
        }),
    );
}

#[test]
fn read_field_rewritten_mutable_actual_widens_to_truth() {
    // `rec.count = -1` reaches the exact projection the guard reads, so no
    // per-field entry identity survives and the cause stays unconditional.
    let buckets = call_site_buckets(
        "data Rec { count: i32; other: i32; }
         machine inner(cell: &Rec) -> bool crashes Trap !(cell.count >= 0) { true }
         machine outer(mut rec: Rec) -> bool crashes Trap { rec.count = -1; inner(&rec) }",
        "outer",
    );
    assert_eq!(
        single_surviving_bucket(&buckets),
        &checked_trees::CrashRouteGuard::Truth,
        "a rewritten read projection keeps the unconditional route: {buckets:?}"
    );
}

#[test]
fn mutable_scalar_storage_feeds_arithmetic_actuals() {
    // A mutable local in the prefix is outside the immutable scalar-prefix
    // namespace, but the literal arithmetic actual still lowers and
    // evaluates: `3 - 1 == 0` discharges the guarded route.
    assert!(
        call_site_buckets(
            "machine inner(input: u64) -> u64 crashes Trap input == 0u64 { input }
             machine outer() -> u64 { let mut seen: bool = false; inner(3u64 - 1u64) }",
            "outer",
        )
        .is_empty(),
        "a proven arithmetic actual discharges under the selected meaning"
    );
    // Storage read inside the actual: `n = 1u64; inner(n - 1u64)` proves
    // `0 == 0`, so the cause survives unconditionally.
    let buckets = call_site_buckets(
        "machine inner(input: u64) -> u64 crashes Trap input == 0u64 { input }
         machine outer() -> u64 crashes Trap { let mut n: u64 = 0u64; n = 1u64; inner(n - 1u64) }",
        "outer",
    );
    assert_eq!(
        single_surviving_bucket(&buckets),
        &checked_trees::CrashRouteGuard::Truth,
        "a mutable storage read may prove the arithmetic guard true"
    );
}

#[test]
fn a_case_payload_actual_keeps_its_qualified_entry_identity() {
    // `Outcome::Second { c }` binds the payload through a synthesized member
    // that retains no field symbol — only the `Second` qualification. The
    // `work` arrival replays it into `value.Second::c`, so the surviving
    // route names `value.Second::c.item` exactly: not a bare `c` member that
    // could collide with `First`'s payload, and not a `Truth` widening.
    let buckets = call_site_buckets(
        "data Cell { item: i32; }
         data Outcome { case First(c: Cell); case Second(c: Cell); }
         machine inner(cell: Cell) -> bool crashes Trap !(cell.item >= 0) { true }
         machine outer(value: Outcome) -> bool crashes Trap {
             transition value {
                 Outcome::Second { c } -> work(c)
                 Outcome::First { c } -> done()
             }
             state work(cell: Cell) -> bool { inner(cell) }
             state done() -> bool { true }
         }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the case-qualified actual keeps its guarded entry operand: {buckets:?}")
    };
    use typed_trees::expression::{BinaryOperator, UnaryOperator};
    assert_eq!(
        identity.expression(),
        Some(&CrashPredicateExpression::Unary {
            operator: UnaryOperator::LogicalNot as u8,
            operand: Box::new(CrashPredicateExpression::Binary {
                operator: BinaryOperator::GreaterOrEqual as u8,
                left: Box::new(CrashPredicateExpression::Member {
                    receiver: Box::new(CrashPredicateExpression::Member {
                        receiver: Box::new(CrashPredicateExpression::Parameter(0)),
                        member: "Second::c".to_owned(),
                    }),
                    member: "item".to_owned(),
                }),
                right: Box::new(CrashPredicateExpression::Integer("0".to_owned())),
            }),
        }),
    );
}

#[test]
fn a_case_payload_actual_below_rewritten_storage_widens_to_truth() {
    // Rebinding `held` before the transition ends its bound-snapshot
    // provenance, so the destructure cannot claim the invocation actual's
    // payload and the cause stays unconditional.
    let buckets = call_site_buckets(
        "data Cell { item: i32; }
         data Outcome { case First(c: Cell); case Second(c: Cell); }
         machine inner(cell: Cell) -> bool crashes Trap !(cell.item >= 0) { true }
         machine outer(value: Outcome) -> bool crashes Trap {
             let mut held: Outcome = value;
             held = Outcome::First { c: Cell { item: 0 } };
             transition held {
                 Outcome::Second { c } -> work(c)
                 Outcome::First { c } -> done()
             }
             state work(cell: Cell) -> bool { inner(cell) }
             state done() -> bool { true }
         }",
        "outer",
    );
    assert_eq!(
        single_surviving_bucket(&buckets),
        &checked_trees::CrashRouteGuard::Truth,
        "a rebound scrutinee keeps the unconditional route: {buckets:?}"
    );
}

/// A `collection[index]` guard leaf keeps both children structured: each may
/// carry a formal, so extraction produces `Indexed` rather than hiding them
/// inside a flattened `Opaque` display. The identity's canonical bytes match
/// the source-route encoder exactly — the same equality
/// `build_published_crash_plan`'s `debug_assert_eq!` replays.
#[test]
fn indexed_guard_leaves_extract_both_children() {
    use typed_trees::expression::BinaryOperator;
    let source = "machine value(items: [i32; 4]) -> bool crashes Trap items[0u64] == 0 { true }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let fact = program
        .machine_contracts(machine)
        .iter()
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .next()
        .expect("one contract fact");
    let typed_trees::domain::ProofFact::Expression(expression) = fact else {
        panic!("the crash route is an expression fact")
    };
    let predicate = crash_predicate_from_expression(&program, *expression, &["items".into()], None);
    let expected = CrashPredicateExpression::Binary {
        operator: BinaryOperator::Equal as u8,
        left: Box::new(CrashPredicateExpression::Indexed {
            collection: Box::new(CrashPredicateExpression::Parameter(0)),
            index: Box::new(CrashPredicateExpression::Integer("0".into())),
        }),
        right: Box::new(CrashPredicateExpression::Integer("0".into())),
    };
    assert_eq!(predicate, expected);
    // `0x0b` is the indexed tag: [fact-expr, binary, ==, 0x0b, param, ...].
    let mut route = Vec::new();
    crate::facts::canonical_encoding::encode_contract_fact_canonical(
        &program,
        fact,
        &["items".into()],
        &[],
        false,
        &mut route,
    );
    assert_eq!(
        checked_trees::CrashPredicateIdentity::from_expression(predicate.clone()).canonical_bytes(),
        route.as_slice(),
        "typed and checked canonical encoders agree on the indexed tag"
    );
    assert!(route.contains(&0x0b));
    // An indexed read is not a closed proof literal: the domain-free reducer
    // cannot fold it, so it never invents builtin `[]` meaning.
    assert_eq!(summary_boolean_value(&predicate), None);
}

/// The private-summary path substitutes both `Indexed` children: `items` and
/// `index` bind the caller's `items`/`position` entry parameters, so the
/// surviving route keeps `items[position] == 0` in caller coordinates.
#[test]
fn an_indexed_actual_guard_substitutes_both_children() {
    let buckets = call_site_buckets(
        "machine inner(items: &[i32], index: u64) -> bool
         requires index < items.len
         crashes Trap !(items[index] == 0) { true }
         machine outer(items: &[i32], position: u64) -> bool
         requires position < items.len
         crashes Trap { inner(items, position) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the indexed guard keeps its guarded route: {buckets:?}")
    };
    use typed_trees::expression::{BinaryOperator, UnaryOperator};
    assert_eq!(
        identity.expression(),
        Some(&CrashPredicateExpression::Unary {
            operator: UnaryOperator::LogicalNot as u8,
            operand: Box::new(CrashPredicateExpression::Binary {
                operator: BinaryOperator::Equal as u8,
                left: Box::new(CrashPredicateExpression::Indexed {
                    collection: Box::new(CrashPredicateExpression::Parameter(0)),
                    index: Box::new(CrashPredicateExpression::Parameter(1)),
                }),
                right: Box::new(CrashPredicateExpression::Integer("0".into())),
            }),
        }),
    );
}

/// `substitute` reaches inside an `Indexed` leaf from either child: the
/// collection's formal binds the actual, while an `Opaque` child — a
/// `start..end` range or flattened projection — still refuses because its
/// display may hide formals.
#[test]
fn indexed_predicates_substitute_through_summary_buckets() {
    use typed_trees::expression::BinaryOperator;
    let indexed = |collection, index| CrashPredicateExpression::Indexed {
        collection: Box::new(collection),
        index: Box::new(index),
    };
    let route = SummaryCrashBucket {
        cause: checked_trees::CrashCause::Trap,
        alternative_guards: vec![predicate(indexed(
            CrashPredicateExpression::Parameter(0),
            CrashPredicateExpression::Parameter(1),
        ))],
    };
    let substituted = route.substitute(&identity_substitution(vec![
        Some(CrashPredicateExpression::Integer("4".into())),
        Some(CrashPredicateExpression::Binary {
            operator: BinaryOperator::Add as u8,
            left: Box::new(CrashPredicateExpression::Parameter(2)),
            right: Box::new(CrashPredicateExpression::Integer("1".into())),
        }),
    ]));
    assert_eq!(
        substituted.alternative_guards,
        vec![predicate(indexed(
            CrashPredicateExpression::Integer("4".into()),
            CrashPredicateExpression::Binary {
                operator: BinaryOperator::Add as u8,
                left: Box::new(CrashPredicateExpression::Parameter(2)),
                right: Box::new(CrashPredicateExpression::Integer("1".into())),
            },
        ))],
    );
    let opaque_index = SummaryCrashBucket {
        cause: checked_trees::CrashCause::Trap,
        alternative_guards: vec![predicate(indexed(
            CrashPredicateExpression::Parameter(0),
            CrashPredicateExpression::Opaque("0..bound".into()),
        ))],
    };
    assert_eq!(
        opaque_index.substitute(&identity_substitution(vec![
            Some(CrashPredicateExpression::Parameter(2)),
            Some(CrashPredicateExpression::Parameter(3)),
        ])),
        SummaryCrashBucket::unconditional(checked_trees::CrashCause::Trap),
        "an opaque index child still widens instead of leaking a callee display",
    );
}

/// A float-field guard's checked scalar annotation crosses the call: each
/// `IeeeFloatComparison` leaf re-roots to the caller parameter the actual
/// reads, so `inner(a, b)` under `left.narrow == right.narrow` retains
/// `a.narrow == b.narrow` as structured evidence instead of dropping to the
/// bare identity. The retained annotation is what later structural lowering
/// replays as the atomic proposition.
#[test]
fn float_field_guards_keep_their_scalar_evidence_through_calls() {
    use checked_trees::{
        CheckedBooleanExpression, CheckedIeeeFloatComparisonKind, CheckedStructuralParameterField,
        CheckedStructuralPredicatePathSegment,
    };
    use typed_trees::expression::BinaryOperator;
    use typed_trees::types::PrimitiveType;

    let field = |position: u32| CheckedStructuralParameterField {
        parameter_position: position,
        path: vec![CheckedStructuralPredicatePathSegment::Field(
            "narrow".to_owned(),
        )],
    };
    let expected_scalar = CheckedBooleanExpression::IeeeFloatComparison {
        kind: CheckedIeeeFloatComparisonKind::Equal,
        primitive_type: PrimitiveType::F32,
        left: field(0),
        right: field(1),
    };
    let expected_identity = CrashPredicateExpression::Binary {
        operator: BinaryOperator::Equal as u8,
        left: Box::new(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Parameter(0)),
            member: "narrow".to_owned(),
        }),
        right: Box::new(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Parameter(1)),
            member: "narrow".to_owned(),
        }),
    };

    let buckets = call_site_buckets(
        "data Pair { narrow: f32; }
         machine inner(left: Pair, right: Pair) -> bool
         crashes Trap left.narrow == right.narrow { true }
         machine outer(a: Pair, b: Pair) -> bool crashes Trap { inner(a, b) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the float-field guard keeps its guarded route: {buckets:?}")
    };
    assert_eq!(identity.expression(), Some(&expected_identity));
    assert_eq!(identity.scalar_expression(), Some(&expected_scalar));
}

/// The actual's own member spine prepends below the caller root: binding
/// `pair.left`/`pair.right` re-roots the leaves to `pair.left.narrow` and
/// `pair.right.narrow` under the same caller parameter rather than leaving
/// callee positions behind.
#[test]
fn float_field_guards_substitute_through_member_projections() {
    use checked_trees::{
        CheckedBooleanExpression, CheckedIeeeFloatComparisonKind, CheckedStructuralParameterField,
        CheckedStructuralPredicatePathSegment,
    };

    let field = |root: &str| CheckedStructuralParameterField {
        parameter_position: 0,
        path: vec![
            CheckedStructuralPredicatePathSegment::Field(root.to_owned()),
            CheckedStructuralPredicatePathSegment::Field("narrow".to_owned()),
        ],
    };

    let buckets = call_site_buckets(
        "data Pair { narrow: f32; }
         data Both { left: Pair; right: Pair; }
         machine inner(left: Pair, right: Pair) -> bool
         crashes Trap left.narrow == right.narrow { true }
         machine outer(pair: Both) -> bool crashes Trap { inner(pair.left, pair.right) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the projected actuals keep the float guard: {buckets:?}")
    };
    assert_eq!(
        identity.scalar_expression(),
        Some(&CheckedBooleanExpression::IeeeFloatComparison {
            kind: CheckedIeeeFloatComparisonKind::Equal,
            primitive_type: typed_trees::types::PrimitiveType::F32,
            left: field("left"),
            right: field("right"),
        }),
    );
}

/// A mutable caller root cannot promise the entry snapshot the contract
/// namespace names, so the annotation stays empty rather than describing
/// stale storage — the route still keeps its substituted identity.
#[test]
fn float_field_guards_drop_scalar_evidence_below_mutable_roots() {
    let buckets = call_site_buckets(
        "data Pair { narrow: f32; }
         data Both { left: Pair; right: Pair; }
         machine inner(left: Pair, right: Pair) -> bool
         crashes Trap left.narrow == right.narrow { true }
         machine outer(mut pair: Both) -> bool crashes Trap { inner(pair.left, pair.right) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the mutable root still keeps the guarded route: {buckets:?}")
    };
    assert!(
        identity.scalar_expression().is_none(),
        "a mutable root keeps no entry-snapshot annotation: {buckets:?}"
    );
}

/// A payload-less sum equality guard's checked scalar annotation crosses the
/// call the same way float-field leaves do: both subjects re-root to the
/// caller parameters the actuals read, so `inner(a, b)` under `left == right`
/// retains `a == b` over the `Off`/`On` roster as structured evidence instead
/// of dropping to the bare identity. The roster names the sum's declared
/// cases — a type-level fact unchanged by the re-root — and the structural
/// lowering replays the leaf as per-case membership implications.
#[test]
fn payloadless_sum_guards_keep_their_scalar_evidence_through_calls() {
    use checked_trees::{CheckedBooleanExpression, CheckedStructuralParameterField};
    use typed_trees::expression::BinaryOperator;

    let expected_scalar = CheckedBooleanExpression::PayloadlessSumEqual {
        left: CheckedStructuralParameterField {
            parameter_position: 0,
            path: Vec::new(),
        },
        right: CheckedStructuralParameterField {
            parameter_position: 1,
            path: Vec::new(),
        },
        cases: vec!["Off".to_owned(), "On".to_owned()],
    };
    let expected_identity = CrashPredicateExpression::Binary {
        operator: BinaryOperator::Equal as u8,
        left: Box::new(CrashPredicateExpression::Parameter(0)),
        right: Box::new(CrashPredicateExpression::Parameter(1)),
    };

    let buckets = call_site_buckets(
        "data Mode { case Off; case On; }
         machine inner(left: Mode, right: Mode) -> bool
         crashes Trap left == right { true }
         machine outer(a: Mode, b: Mode) -> bool crashes Trap { inner(a, b) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the sum-equality guard keeps its guarded route: {buckets:?}")
    };
    assert_eq!(identity.expression(), Some(&expected_identity));
    assert_eq!(identity.scalar_expression(), Some(&expected_scalar));
}

/// The actual's own member spine prepends below the caller root for sum
/// subjects too: binding `pair.left`/`pair.right` re-roots the equality to
/// `pair.left == pair.right` under the same caller parameter rather than
/// leaving callee positions behind.
#[test]
fn payloadless_sum_guards_substitute_through_member_projections() {
    use checked_trees::{
        CheckedBooleanExpression, CheckedStructuralParameterField,
        CheckedStructuralPredicatePathSegment,
    };

    let subject = |root: &str| CheckedStructuralParameterField {
        parameter_position: 0,
        path: vec![CheckedStructuralPredicatePathSegment::Field(
            root.to_owned(),
        )],
    };

    let buckets = call_site_buckets(
        "data Mode { case Off; case On; }
         data Both { left: Mode; right: Mode; }
         machine inner(left: Mode, right: Mode) -> bool
         crashes Trap left == right { true }
         machine outer(pair: Both) -> bool crashes Trap { inner(pair.left, pair.right) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the projected actuals keep the sum-equality guard: {buckets:?}")
    };
    assert_eq!(
        identity.scalar_expression(),
        Some(&CheckedBooleanExpression::PayloadlessSumEqual {
            left: subject("left"),
            right: subject("right"),
            cases: vec!["Off".to_owned(), "On".to_owned()],
        }),
    );
}

/// A mutable caller root cannot promise the entry snapshot the contract
/// namespace names, so the annotation stays empty rather than describing
/// stale storage — the route still keeps its substituted identity.
#[test]
fn payloadless_sum_guards_drop_scalar_evidence_below_mutable_roots() {
    let buckets = call_site_buckets(
        "data Mode { case Off; case On; }
         data Both { left: Mode; right: Mode; }
         machine inner(left: Mode, right: Mode) -> bool
         crashes Trap left == right { true }
         machine outer(mut pair: Both) -> bool crashes Trap { inner(pair.left, pair.right) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the mutable root still keeps the guarded route: {buckets:?}")
    };
    assert!(
        identity.scalar_expression().is_none(),
        "a mutable root keeps no entry-snapshot annotation: {buckets:?}"
    );
}

/// A byte-sequence content-equality guard's checked scalar annotation
/// crosses the call the same way float-field leaves do: both subjects
/// re-root to the caller parameters the actuals read, so `inner(a, b)`
/// under `left == right` retains `a.text == b.text` over the borrowed
/// views as structured evidence instead of dropping to the bare
/// identity. The leaf carries no roster or case identity — content
/// equality compares the bytes the resolved subjects hold — and the
/// lowering rechecks each re-rooted leaf's retained byte-sequence
/// carrier before emitting the atomic proposition.
#[test]
fn byte_sequence_guards_keep_their_scalar_evidence_through_calls() {
    use checked_trees::{
        CheckedBooleanExpression, CheckedStructuralParameterField,
        CheckedStructuralPredicatePathSegment,
    };
    use typed_trees::expression::BinaryOperator;

    let subject = |position: u32| CheckedStructuralParameterField {
        parameter_position: position,
        path: vec![CheckedStructuralPredicatePathSegment::Field(
            "text".to_owned(),
        )],
    };
    let expected_scalar = CheckedBooleanExpression::ByteSequenceEqual {
        left: subject(0),
        right: subject(1),
    };
    let expected_identity = CrashPredicateExpression::Binary {
        operator: BinaryOperator::Equal as u8,
        left: Box::new(CrashPredicateExpression::Parameter(0)),
        right: Box::new(CrashPredicateExpression::Parameter(1)),
    };

    let buckets = call_site_buckets(
        "trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
         data Blob { text: &[u8]; }
         BlobEquatable: Blob satisfies Equatable;
         machine inner(left: Blob, right: Blob) -> bool
         crashes Trap left == right { true }
         machine outer(a: Blob, b: Blob) -> bool crashes Trap { inner(a, b) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the byte-sequence guard keeps its guarded route: {buckets:?}")
    };
    assert_eq!(identity.expression(), Some(&expected_identity));
    assert_eq!(identity.scalar_expression(), Some(&expected_scalar));
}

/// The actual's own member spine prepends below the caller root for
/// byte-sequence subjects too: binding `pair.left`/`pair.right` re-roots
/// the equality to `pair.left.text == pair.right.text` under the same
/// caller parameter rather than leaving callee positions behind.
#[test]
fn byte_sequence_guards_substitute_through_member_projections() {
    use checked_trees::{
        CheckedBooleanExpression, CheckedStructuralParameterField,
        CheckedStructuralPredicatePathSegment,
    };

    let subject = |root: &str| CheckedStructuralParameterField {
        parameter_position: 0,
        path: vec![
            CheckedStructuralPredicatePathSegment::Field(root.to_owned()),
            CheckedStructuralPredicatePathSegment::Field("text".to_owned()),
        ],
    };

    let buckets = call_site_buckets(
        "trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
         data Blob { text: &[u8]; }
         BlobEquatable: Blob satisfies Equatable;
         data Both { left: Blob; right: Blob; }
         machine inner(left: Blob, right: Blob) -> bool
         crashes Trap left == right { true }
         machine outer(pair: Both) -> bool crashes Trap { inner(pair.left, pair.right) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the projected actuals keep the byte-sequence guard: {buckets:?}")
    };
    assert_eq!(
        identity.scalar_expression(),
        Some(&CheckedBooleanExpression::ByteSequenceEqual {
            left: subject("left"),
            right: subject("right"),
        }),
    );
}

/// A mutable caller root cannot promise the entry snapshot the contract
/// namespace names, so the annotation stays empty rather than describing
/// stale storage — the route still keeps its substituted identity.
#[test]
fn byte_sequence_guards_drop_scalar_evidence_below_mutable_roots() {
    let buckets = call_site_buckets(
        "trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
         data Blob { text: &[u8]; }
         BlobEquatable: Blob satisfies Equatable;
         data Both { left: Blob; right: Blob; }
         machine inner(left: Blob, right: Blob) -> bool
         crashes Trap left == right { true }
         machine outer(mut pair: Both) -> bool crashes Trap { inner(pair.left, pair.right) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the mutable root still keeps the guarded route: {buckets:?}")
    };
    assert!(
        identity.scalar_expression().is_none(),
        "a mutable root keeps no entry-snapshot annotation: {buckets:?}"
    );
}

/// A sum case-membership guard's checked scalar annotation crosses the call
/// the same way the other structural leaves do: the subject re-roots to the
/// caller parameter the actual reads, so `inner(a)` under `left in Mode::On`
/// retains `a in Mode::On` as structured evidence instead of dropping to the
/// bare identity. The case names the subject type's declared case — a
/// type-level fact the re-root cannot change — and the structural lowering
/// rechecks it against the resolved subject before emitting the atomic
/// proposition.
#[test]
fn case_membership_guards_keep_their_scalar_evidence_through_calls() {
    use checked_trees::{CheckedBooleanExpression, CheckedStructuralParameterField};
    use typed_trees::expression::BinaryOperator;

    let expected_scalar = CheckedBooleanExpression::StructuralCaseMembership {
        subject: CheckedStructuralParameterField {
            parameter_position: 0,
            path: Vec::new(),
        },
        case: "On".to_owned(),
    };
    let expected_identity = CrashPredicateExpression::Binary {
        operator: BinaryOperator::Equal as u8,
        left: Box::new(CrashPredicateExpression::Parameter(0)),
        right: Box::new(CrashPredicateExpression::Name(vec![
            "Mode".to_owned(),
            "On".to_owned(),
        ])),
    };

    let buckets = call_site_buckets(
        "data Mode { case Off; case On; }
         machine inner(left: Mode) -> bool
         crashes Trap left in Mode::On { true }
         machine outer(a: Mode) -> bool crashes Trap { inner(a) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the case-membership guard keeps its guarded route: {buckets:?}")
    };
    assert_eq!(identity.expression(), Some(&expected_identity));
    assert_eq!(identity.scalar_expression(), Some(&expected_scalar));
}

/// The actual's own member spine prepends below the caller root for case
/// membership too: binding `pair.left` re-roots the subject to `pair.left`
/// under the same caller parameter rather than leaving the callee position
/// behind.
#[test]
fn case_membership_guards_substitute_through_member_projections() {
    use checked_trees::{
        CheckedBooleanExpression, CheckedStructuralParameterField,
        CheckedStructuralPredicatePathSegment,
    };

    let buckets = call_site_buckets(
        "data Mode { case Off; case On; }
         data Both { left: Mode; right: Mode; }
         machine inner(left: Mode) -> bool
         crashes Trap left in Mode::On { true }
         machine outer(pair: Both) -> bool crashes Trap { inner(pair.left) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the projected actual keeps the membership guard: {buckets:?}")
    };
    assert_eq!(
        identity.scalar_expression(),
        Some(&CheckedBooleanExpression::StructuralCaseMembership {
            subject: CheckedStructuralParameterField {
                parameter_position: 0,
                path: vec![CheckedStructuralPredicatePathSegment::Field(
                    "left".to_owned(),
                )],
            },
            case: "On".to_owned(),
        }),
    );
}

/// A mutable caller root cannot promise the entry snapshot the contract
/// namespace names, so the annotation stays empty rather than describing
/// stale storage — the route still keeps its substituted identity.
#[test]
fn case_membership_guards_drop_scalar_evidence_below_mutable_roots() {
    let buckets = call_site_buckets(
        "data Mode { case Off; case On; }
         data Both { left: Mode; right: Mode; }
         machine inner(left: Mode) -> bool
         crashes Trap left in Mode::On { true }
         machine outer(mut pair: Both) -> bool crashes Trap { inner(pair.left) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the mutable root still keeps the guarded route: {buckets:?}")
    };
    assert!(
        identity.scalar_expression().is_none(),
        "a mutable root keeps no entry-snapshot annotation: {buckets:?}"
    );
}

/// A bare Boolean-field guard's checked scalar annotation crosses the call
/// the same way the atomic structural leaves do: the leaf re-roots to the
/// caller parameter the actual reads, so `inner(pair)` under `left.flag`
/// retains `pair.flag` as structured evidence instead of dropping to the
/// bare identity. The lowering rechecks the re-rooted path ends at a
/// retained Boolean field before emitting the scalar term, so a redirected
/// leaf stays fail-closed downstream.
#[test]
fn field_guards_keep_their_scalar_evidence_through_calls() {
    use checked_trees::{CheckedBooleanExpression, CheckedStructuralPredicatePathSegment};

    let expected_scalar = CheckedBooleanExpression::StructuralParameterField {
        parameter_position: 0,
        path: vec![CheckedStructuralPredicatePathSegment::Field(
            "flag".to_owned(),
        )],
    };
    let expected_identity = CrashPredicateExpression::Member {
        receiver: Box::new(CrashPredicateExpression::Parameter(0)),
        member: "flag".to_owned(),
    };

    let buckets = call_site_buckets(
        "data Pair { flag: bool; }
         machine inner(left: Pair) -> bool
         crashes Trap left.flag { true }
         machine outer(pair: Pair) -> bool crashes Trap { inner(pair) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the field guard keeps its guarded route: {buckets:?}")
    };
    assert_eq!(identity.expression(), Some(&expected_identity));
    assert_eq!(identity.scalar_expression(), Some(&expected_scalar));
}

/// The actual's own member spine prepends below the caller root for a
/// standalone field leaf too: binding `pair.left` re-roots the guard to
/// `pair.left.flag` under the same caller parameter rather than leaving
/// the callee position behind.
#[test]
fn field_guards_substitute_through_member_projections() {
    use checked_trees::{CheckedBooleanExpression, CheckedStructuralPredicatePathSegment};

    let buckets = call_site_buckets(
        "data Pair { flag: bool; }
         data Both { left: Pair; right: Pair; }
         machine inner(left: Pair) -> bool
         crashes Trap left.flag { true }
         machine outer(pair: Both) -> bool crashes Trap { inner(pair.left) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the projected actual keeps the field guard: {buckets:?}")
    };
    assert_eq!(
        identity.scalar_expression(),
        Some(&CheckedBooleanExpression::StructuralParameterField {
            parameter_position: 0,
            path: vec![
                CheckedStructuralPredicatePathSegment::Field("left".to_owned()),
                CheckedStructuralPredicatePathSegment::Field("flag".to_owned()),
            ],
        }),
    );
}

/// A mutable caller root cannot promise the entry snapshot the contract
/// namespace names, so the annotation stays empty rather than describing
/// stale storage — the route still keeps its substituted identity.
#[test]
fn field_guards_drop_scalar_evidence_below_mutable_roots() {
    let buckets = call_site_buckets(
        "data Pair { flag: bool; }
         data Both { left: Pair; right: Pair; }
         machine inner(left: Pair) -> bool
         crashes Trap left.flag { true }
         machine outer(mut pair: Both) -> bool crashes Trap { inner(pair.left) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the mutable root still keeps the guarded route: {buckets:?}")
    };
    assert!(
        identity.scalar_expression().is_none(),
        "a mutable root keeps no entry-snapshot annotation: {buckets:?}"
    );
}

/// An integer-field comparison guard's checked scalar annotation crosses the
/// call the same way the standalone Boolean leaf does: the scalar
/// `StructuralParameterField` leaf re-roots to the caller parameter the
/// actual reads, so `inner(pair)` under `left.count == 0` retains
/// `pair.count == 0` as structured evidence instead of dropping the
/// annotation to the bare identity. The structural lowering rechecks the
/// re-rooted path ends at a retained integer field of the declared type
/// before emitting the scalar term, so a redirected leaf stays fail-closed
/// downstream.
#[test]
fn integer_field_guards_keep_their_scalar_evidence_through_calls() {
    use checked_trees::{
        CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarExpression,
        CheckedStructuralPredicatePathSegment,
    };
    use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType};
    use typed_trees::expression::BinaryOperator;
    use typed_trees::types::PrimitiveType;

    let expected_scalar = CheckedBooleanExpression::IntegerComparison {
        kind: CheckedIntegerComparisonKind::Equal,
        left: Box::new(CheckedScalarExpression::StructuralParameterField {
            parameter_position: 0,
            path: vec![CheckedStructuralPredicatePathSegment::Field(
                "count".to_owned(),
            )],
            primitive_type: PrimitiveType::U64,
        }),
        right: Box::new(CheckedScalarExpression::IntegerLiteral {
            literal: IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "0")
                .unwrap()
                .with_landing(IntegerLanding {
                    landed_type: LandedIntegerType::U64,
                    domain: numerics::arithmetic::ArithmeticDomain::Exact,
                }),
        }),
    };
    let expected_identity = CrashPredicateExpression::Binary {
        operator: BinaryOperator::Equal as u8,
        left: Box::new(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Parameter(0)),
            member: "count".to_owned(),
        }),
        right: Box::new(CrashPredicateExpression::Integer("0".into())),
    };

    let buckets = call_site_buckets(
        "data Pair { count: u64; }
         machine inner(left: Pair) -> bool
         crashes Trap left.count == 0 { true }
         machine outer(pair: Pair) -> bool crashes Trap { inner(pair) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the integer-field guard keeps its guarded route: {buckets:?}")
    };
    assert_eq!(identity.expression(), Some(&expected_identity));
    assert_eq!(identity.scalar_expression(), Some(&expected_scalar));
}

/// The actual's own member spine prepends below the caller root for a
/// scalar field leaf too: binding `pair.left` re-roots the comparison to
/// `pair.left.count` under the same caller parameter rather than leaving
/// the callee position behind.
#[test]
fn integer_field_guards_substitute_through_member_projections() {
    use checked_trees::{
        CheckedBooleanExpression, CheckedScalarExpression, CheckedStructuralPredicatePathSegment,
    };
    use typed_trees::types::PrimitiveType;

    let buckets = call_site_buckets(
        "data Pair { count: u64; }
         data Both { left: Pair; right: Pair; }
         machine inner(left: Pair) -> bool
         crashes Trap left.count == 0 { true }
         machine outer(pair: Both) -> bool crashes Trap { inner(pair.left) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the projected actual keeps the integer-field guard: {buckets:?}")
    };
    let Some(CheckedBooleanExpression::IntegerComparison { left, .. }) =
        identity.scalar_expression()
    else {
        panic!("the comparison keeps its checked scalar form: {buckets:?}")
    };
    assert_eq!(
        left.as_ref(),
        &CheckedScalarExpression::StructuralParameterField {
            parameter_position: 0,
            path: vec![
                CheckedStructuralPredicatePathSegment::Field("left".to_owned()),
                CheckedStructuralPredicatePathSegment::Field("count".to_owned()),
            ],
            primitive_type: PrimitiveType::U64,
        },
    );
}

/// A mutable caller root cannot promise the entry snapshot the contract
/// namespace names, so the annotation stays empty rather than describing
/// stale storage — the route still keeps its substituted identity.
#[test]
fn integer_field_guards_drop_scalar_evidence_below_mutable_roots() {
    let buckets = call_site_buckets(
        "data Pair { count: u64; }
         data Both { left: Pair; right: Pair; }
         machine inner(left: Pair) -> bool
         crashes Trap left.count == 0 { true }
         machine outer(mut pair: Both) -> bool crashes Trap { inner(pair.left) }",
        "outer",
    );
    let checked_trees::CrashRouteGuard::Predicate(identity) = single_surviving_bucket(&buckets)
    else {
        panic!("the mutable root still keeps the guarded route: {buckets:?}")
    };
    assert!(
        identity.scalar_expression().is_none(),
        "a mutable root keeps no entry-snapshot annotation: {buckets:?}"
    );
}
