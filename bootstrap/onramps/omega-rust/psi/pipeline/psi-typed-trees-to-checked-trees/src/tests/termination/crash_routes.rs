use super::*;

#[test]
fn crash_bucket_identity_includes_cause_routes_and_unconditional_presence() {
    let source = r#"
    machine baseline() {}
    machine trap_activation(flag: bool)
    crashes Trap
        flag
    {}
    machine trap_domain(flag: bool)
    crashes Trap
        flag
    {}
    machine abort_activation(flag: bool)
    crashes Abort
        flag
    {}
    machine unconditional_abort()
    crashes Abort
    {}
    machine explicit_true_abort()
    crashes Abort
        true
    {}
    machine grouped(first: bool, second: bool)
    crashes Trap
        first
        second
    {}
    machine split(first: bool, second: bool)
    crashes Trap
        first
    crashes Trap
        second
    {}
    machine reordered(first: bool, second: bool)
    crashes Trap
        second
        first
    {}
    machine duplicated(first: bool, second: bool)
    crashes Trap
        first
        second
        first
    {}
    machine unconditional_with_guard(flag: bool)
    crashes Abort
        flag
    crashes Abort
    {}
    machine unconditional_only(flag: bool)
    crashes Abort
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let fingerprint = |name: &str| {
        let symbol = symbol_of_checked(&checked, name);
        checked
            .facts
            .contract_plans
            .for_machine(symbol)
            .expect("contract plan")
            .fingerprint
    };

    assert_ne!(fingerprint("baseline"), fingerprint("unconditional_abort"));
    assert_eq!(
        fingerprint("unconditional_abort"),
        fingerprint("explicit_true_abort")
    );
    assert_eq!(fingerprint("trap_activation"), fingerprint("trap_domain"));
    assert_ne!(
        fingerprint("trap_activation"),
        fingerprint("abort_activation")
    );
    assert_eq!(fingerprint("grouped"), fingerprint("split"));
    assert_eq!(fingerprint("grouped"), fingerprint("reordered"));
    assert_eq!(fingerprint("grouped"), fingerprint("duplicated"));
    assert_eq!(
        fingerprint("unconditional_with_guard"),
        fingerprint("unconditional_only")
    );

    let crash = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
            .crash
            .clone()
    };
    assert_eq!(crash("grouped"), crash("split"));
    assert_eq!(crash("grouped"), crash("reordered"));
    assert_eq!(crash("grouped"), crash("duplicated"));
    assert_eq!(crash("unconditional_abort"), crash("explicit_true_abort"));
    let grouped = crash("grouped");
    assert_eq!(
        grouped.interface(),
        psi_checked_trees::CrashInterface::PublishedCeiling
    );
    assert_eq!(grouped.published().len(), 1);
    assert_eq!(
        grouped.published()[0].cause(),
        psi_checked_trees::CrashCause::Trap
    );
    assert_eq!(grouped.published()[0].alternative_guards().len(), 2);
    assert!(!grouped.published()[0].is_unconditional());
}

#[test]
fn empty_record_equality_retains_existing_boolean_constant_carriers() {
    let source = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Empty {}
    EmptyEquatable: Empty satisfies Equatable;

    machine equal(left: Empty, right: Empty)
    crashes Abort
        left == right
    {}

    machine not_equal(left: Empty, right: Empty)
    crashes Abort
        left != right
    {}

    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let scalar = |name: &str| {
        let contract = checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan");
        let [bucket] = contract.crash.published() else {
            panic!("{name} should publish one crash bucket")
        };
        let [psi_checked_trees::CrashRouteGuard::Predicate(predicate)] =
            bucket.alternative_guards()
        else {
            panic!("{name} should publish one predicate")
        };
        predicate.scalar_expression().cloned()
    };

    assert_eq!(
        scalar("equal"),
        Some(psi_checked_trees::CheckedBooleanExpression::Constant(true))
    );
    assert_eq!(
        scalar("not_equal"),
        Some(psi_checked_trees::CheckedBooleanExpression::Not(Box::new(
            psi_checked_trees::CheckedBooleanExpression::Constant(true)
        )))
    );
}

#[test]
fn erased_record_equality_is_not_mistaken_for_empty_record_equality() {
    let source = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Erased { proof [erased]: bool; }
    ErasedEquatable: Erased satisfies Equatable;

    machine erased_equal(left: Erased, right: Erased)
    crashes Abort
        left == right
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("erased semantic fields must not be treated as an empty record");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .to_string()
            .contains("erased field `proof` has no runtime value")
    }));
}

#[test]
fn address_field_equality_stays_outside_structural_crash_predicates() {
    let source = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Addressed { pointer: addr; }
    AddressedEquatable: Addressed satisfies Equatable;

    machine whole_equal(left: Addressed, right: Addressed)
    crashes Abort
        left == right
    {}

    machine field_equal(left: Addressed, right: Addressed)
    crashes Abort
        left.pointer == right.pointer
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    for name in ["whole_equal", "field_equal"] {
        let contract = checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan");
        let [bucket] = contract.crash.published() else {
            panic!("{name} should publish one crash bucket")
        };
        let [psi_checked_trees::CrashRouteGuard::Predicate(predicate)] =
            bucket.alternative_guards()
        else {
            panic!("{name} should publish one predicate")
        };
        assert!(
            predicate.scalar_expression().is_none(),
            "{name} must not retain addr as a fixed-integer structural term"
        );
    }
}

#[test]
fn ieee_float_fields_retain_atomic_structural_equality() {
    let source = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Samples { narrow: f32; wide: f64; }
    SamplesEquatable: Samples satisfies Equatable;

    machine whole(left: Samples, right: Samples)
    crashes Abort
        left == right
    {}

    machine whole_not_equal(left: Samples, right: Samples)
    crashes Abort
        left != right
    {}

    machine narrow(left: Samples, right: Samples)
    crashes Abort
        left.narrow == right.narrow
    {}

    machine wide(left: Samples, right: Samples)
    crashes Abort
        left.wide == right.wide
    {}

    machine narrow_not_equal(left: Samples, right: Samples)
    crashes Abort
        left.narrow != right.narrow
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let scalar = |name: &str| {
        let contract = checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan");
        let [bucket] = contract.crash.published() else {
            panic!("{name} should publish one crash bucket")
        };
        let [psi_checked_trees::CrashRouteGuard::Predicate(predicate)] =
            bucket.alternative_guards()
        else {
            panic!("{name} should publish one predicate")
        };
        predicate.scalar_expression().cloned().expect("scalar term")
    };
    let format = |expression: &psi_checked_trees::CheckedBooleanExpression| match expression {
        psi_checked_trees::CheckedBooleanExpression::IeeeFloatComparison {
            primitive_type, ..
        } => Some(*primitive_type),
        _ => None,
    };

    assert_eq!(
        format(&scalar("narrow")),
        Some(psi_typed_trees::types::PrimitiveType::F32)
    );
    assert_eq!(
        format(&scalar("wide")),
        Some(psi_typed_trees::types::PrimitiveType::F64)
    );
    assert!(matches!(
        scalar("narrow_not_equal"),
        psi_checked_trees::CheckedBooleanExpression::IeeeFloatComparison {
            kind: psi_checked_trees::CheckedIeeeFloatComparisonKind::NotEqual,
            primitive_type: psi_typed_trees::types::PrimitiveType::F32,
            ..
        }
    ));
    let psi_checked_trees::CheckedBooleanExpression::And { left, right } = scalar("whole") else {
        panic!("two-field float record equality is one conjunction")
    };
    assert!(matches!(
        scalar("whole_not_equal"),
        psi_checked_trees::CheckedBooleanExpression::Not(operand)
            if matches!(*operand, psi_checked_trees::CheckedBooleanExpression::And { .. })
    ));
    let formats = [format(&left), format(&right)];
    assert!(formats.contains(&Some(psi_typed_trees::types::PrimitiveType::F32)));
    assert!(formats.contains(&Some(psi_typed_trees::types::PrimitiveType::F64)));
}

#[test]
fn byte_sequence_fields_retain_atomic_content_equality() {
    let source = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    domain [u8]::Utf8
    requires
        valid_utf8(self);
    domain [u8; 8]::Utf8
    requires
        valid_utf8(self);

    data Borrowed { active: bool; text: &[u8] in Utf8; }
    BorrowedEquatable: Borrowed satisfies Equatable;
    data Bounded { active: bool; text: [u8; 8] in Utf8; }
    BoundedEquatable: Bounded satisfies Equatable;

    machine borrowed(left: Borrowed, right: Borrowed)
    crashes Abort
        left == right
    {}

    machine bounded(left: Bounded, right: Bounded)
    crashes Abort
        left == right
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");

    for name in ["borrowed", "bounded"] {
        let contract = checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan");
        let [bucket] = contract.crash.published() else {
            panic!("{name} should publish one crash bucket")
        };
        let [psi_checked_trees::CrashRouteGuard::Predicate(predicate)] =
            bucket.alternative_guards()
        else {
            panic!("{name} should publish one predicate")
        };
        let psi_checked_trees::CheckedBooleanExpression::And { left, right } = predicate
            .scalar_expression()
            .expect("whole-record equality remains a checked expression")
        else {
            panic!("{name} should compare its Boolean and byte-sequence fields")
        };
        assert!(
            matches!(
                left.as_ref(),
                psi_checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
            ) || matches!(
                right.as_ref(),
                psi_checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
            ),
            "{name} should retain byte content equality as one atomic leaf"
        );
        assert!(
            matches!(
                left.as_ref(),
                psi_checked_trees::CheckedBooleanExpression::Equal { .. }
            ) || matches!(
                right.as_ref(),
                psi_checked_trees::CheckedBooleanExpression::Equal { .. }
            ),
            "{name} should retain its scalar sibling independently"
        );
    }
}

#[test]
fn payloadless_sum_equality_retains_closed_case_roster() {
    let source = r#"
    data Mode {
        case Off;
        case On;
    }

    machine equal(left: Mode, right: Mode)
    crashes Abort
        left == right
    {}

    machine not_equal(left: Mode, right: Mode)
    crashes Abort
        left != right
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let expression = |name: &str| {
        let contract = checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan");
        let [bucket] = contract.crash.published() else {
            panic!("{name} should publish one crash bucket")
        };
        let [psi_checked_trees::CrashRouteGuard::Predicate(predicate)] =
            bucket.alternative_guards()
        else {
            panic!("{name} should publish one predicate")
        };
        predicate.scalar_expression().cloned().expect("scalar term")
    };

    assert!(matches!(
        expression("equal"),
        psi_checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { cases, .. }
            if cases == ["Off", "On"]
    ));
    assert!(matches!(
        expression("not_equal"),
        psi_checked_trees::CheckedBooleanExpression::Not(operand)
            if matches!(operand.as_ref(),
                psi_checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { cases, .. }
                    if cases.len() == 2 && cases[0] == "Off" && cases[1] == "On")
    ));
}

#[test]
fn nested_payload_bearing_sum_equality_retains_record_case_payload_paths() {
    use psi_checked_trees::{
        CheckedBooleanExpression, CheckedScalarExpression,
        CheckedStructuralPredicatePathSegment as Path,
    };

    fn collect_paths(
        expression: &CheckedBooleanExpression,
        membership_paths: &mut Vec<Vec<Path>>,
        payload_paths: &mut Vec<Vec<Path>>,
    ) {
        match expression {
            CheckedBooleanExpression::StructuralCaseMembership { subject, .. } => {
                membership_paths.push(subject.path.clone());
            }
            CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
                for operand in [left.as_ref(), right.as_ref()] {
                    if let CheckedScalarExpression::StructuralParameterField { path, .. } = operand
                    {
                        payload_paths.push(path.clone());
                    }
                }
            }
            CheckedBooleanExpression::Not(operand) => {
                collect_paths(operand, membership_paths, payload_paths);
            }
            CheckedBooleanExpression::Equal { left, right }
            | CheckedBooleanExpression::And { left, right }
            | CheckedBooleanExpression::Or { left, right } => {
                collect_paths(left, membership_paths, payload_paths);
                collect_paths(right, membership_paths, payload_paths);
            }
            CheckedBooleanExpression::Constant(_)
            | CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::Local { .. }
            | CheckedBooleanExpression::StructuralParameterField { .. }
            | CheckedBooleanExpression::IeeeFloatComparison { .. }
            | CheckedBooleanExpression::ByteSequenceEqual { .. }
            | CheckedBooleanExpression::PayloadlessSumEqual { .. } => {}
        }
    }

    let source = r#"
    trait Equatable {
        machine equals(&self, rhs: &Self) -> bool;
    }

    data Message {
        case Empty;
        case Data(value: i32);
    }
    MessageEquatable: Message satisfies Equatable;

    data Envelope { active: bool; message: Message; }
    EnvelopeEquatable: Envelope satisfies Equatable;

    machine equal(left: Envelope, right: Envelope)
    crashes Abort
        left == right
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let contract = checked
        .facts
        .contract_plans
        .for_machine(symbol_of_checked(&checked, "equal"))
        .expect("contract plan");
    let [bucket] = contract.crash.published() else {
        panic!("equal should publish one crash bucket")
    };
    let [psi_checked_trees::CrashRouteGuard::Predicate(predicate)] = bucket.alternative_guards()
    else {
        panic!("equal should publish one predicate")
    };
    let expression = predicate
        .scalar_expression()
        .expect("nested sum equality remains a checked expression");
    let mut membership_paths = Vec::new();
    let mut payload_paths = Vec::new();
    collect_paths(expression, &mut membership_paths, &mut payload_paths);

    assert_eq!(membership_paths.len(), 4);
    assert!(
        membership_paths
            .iter()
            .all(|path| { path == &[Path::Field("message".to_owned())] })
    );
    assert_eq!(payload_paths.len(), 2);
    assert!(payload_paths.iter().all(|path| {
        path == &[
            Path::Field("message".to_owned()),
            Path::Case("Data".to_owned()),
            Path::Field("value".to_owned()),
        ]
    }));
}

#[test]
fn checked_crash_sites_are_body_evidence_not_contract_identity() {
    let source = r#"
    machine clear_body() -> i32
    crashes Abort
    { 0 }

    machine crashing_body() -> i32
    crashes Abort
    {
        crash Abort;
    }

    machine guarded_body(flag: bool) -> i32
    crashes Trap
        flag
    {
        crash Trap;
    }

    machine path_guarded_body(flag: bool) -> i32
    crashes Trap
        flag
    {
        transition {
            flag -> fail()
            _ -> 0i32
        }

        state fail() -> i32 {
            crash Trap;
        }
    }

    machine fallthrough_guarded_body(flag: bool) -> i32
    crashes Trap
        !flag
    {
        transition {
            flag -> 0i32
            _ -> fail()
        }

        state fail() -> i32 {
            crash Trap;
        }
    }

    machine conjunct_guarded_body(flag: bool, other: bool) -> i32
    crashes Trap
        flag
    {
        transition {
            flag && other -> fail()
            _ -> 0i32
        }

        state fail() -> i32 {
            crash Trap;
        }
    }

    machine demorgan_guarded_body(flag: bool, other: bool) -> i32
    crashes Trap
        !flag
    {
        transition {
            flag || other -> 0i32
            _ -> fail()
        }

        state fail() -> i32 {
            crash Trap;
        }
    }

    machine disjunction_does_not_cover(flag: bool, other: bool) -> i32
    crashes Trap
        flag
    {
        transition {
            flag || other -> fail()
            _ -> 0i32
        }

        state fail() -> i32 {
            crash Trap;
        }
    }

    machine negated_conjunction_does_not_cover(flag: bool, other: bool) -> i32
    crashes Trap
        !flag
    {
        transition {
            flag && other -> 0i32
            _ -> fail()
        }

        state fail() -> i32 {
            crash Trap;
        }
    }

    machine narrow_abort() -> i32
    crashes Abort
    {
        crash Abort;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let plan = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
    };

    assert_eq!(
        plan("clear_body").fingerprint,
        plan("crashing_body").fingerprint,
        "changing the checked body must not change a published contract identity"
    );
    assert!(plan("clear_body").crash.checked_sites().is_empty());
    let [site] = plan("crashing_body").crash.checked_sites() else {
        panic!("the explicit crash should produce exactly one checked site")
    };
    assert_eq!(site.cause(), psi_checked_trees::CrashCause::Abort);
    assert_eq!(site.location().statement_ordinal(), 0);
    let [covering_bucket] = site.guard_covering_buckets() else {
        panic!("an unconditional same-cause route should cover every site guard")
    };
    assert!(
        plan("crashing_body")
            .crash
            .published_bucket(*covering_bucket)
            .is_some_and(|bucket| bucket.is_unconditional()
                && bucket.cause() == psi_checked_trees::CrashCause::Abort)
    );
    assert_eq!(
        site.location().state(),
        checked.machine_states(
            checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "crashing_body")
                .expect("crashing machine")
        )[0]
        .symbol
    );

    let [guarded_site] = plan("guarded_body").crash.checked_sites() else {
        panic!("the guarded machine should retain its explicit crash site")
    };
    assert!(
        guarded_site.guard_covering_buckets().is_empty(),
        "a route predicate is not unconditional guard-coverage evidence"
    );
    assert!(
        guarded_site.path_guard_conjuncts().is_empty(),
        "an unconditional body crash has no incoming path predicate"
    );

    let [path_guarded_site] = plan("path_guarded_body").crash.checked_sites() else {
        panic!("the guarded target state should retain its explicit crash site")
    };
    let [path_covering_bucket] = path_guarded_site.guard_covering_buckets() else {
        panic!("the exact incoming path guard should cover its published route")
    };
    assert_eq!(path_guarded_site.path_guard_conjuncts().len(), 1);
    assert!(
        plan("path_guarded_body")
            .crash
            .published_bucket(*path_covering_bucket)
            .is_some_and(|bucket| !bucket.is_unconditional()
                && bucket.cause() == psi_checked_trees::CrashCause::Trap)
    );

    let [fallthrough_guarded_site] = plan("fallthrough_guarded_body").crash.checked_sites() else {
        panic!("the fallthrough target state should retain its explicit crash site")
    };
    let [fallthrough_covering_bucket] = fallthrough_guarded_site.guard_covering_buckets() else {
        panic!("the negated incoming path guard should cover its published route")
    };
    assert_eq!(fallthrough_guarded_site.path_guard_conjuncts().len(), 1);
    assert!(
        plan("fallthrough_guarded_body")
            .crash
            .published_bucket(*fallthrough_covering_bucket)
            .is_some_and(|bucket| !bucket.is_unconditional()
                && bucket.cause() == psi_checked_trees::CrashCause::Trap)
    );

    for name in ["conjunct_guarded_body", "demorgan_guarded_body"] {
        let [site] = plan(name).crash.checked_sites() else {
            panic!("{name} should retain one explicit crash site")
        };
        let [bucket] = site.guard_covering_buckets() else {
            panic!("{name} should prove its structurally implied route")
        };
        assert_eq!(
            site.path_guard_conjuncts().len(),
            1,
            "the exact derived guard remains separate from its consequences"
        );
        assert!(
            !site.path_guard_consequences().is_empty(),
            "the implication witness remains available to terminal lowering"
        );
        assert!(
            plan(name)
                .crash
                .published_bucket(*bucket)
                .is_some_and(|bucket| bucket.cause() == psi_checked_trees::CrashCause::Trap)
        );
    }
    for name in [
        "disjunction_does_not_cover",
        "negated_conjunction_does_not_cover",
    ] {
        let [site] = plan(name).crash.checked_sites() else {
            panic!("{name} should retain one explicit crash site")
        };
        assert!(
            site.guard_covering_buckets().is_empty(),
            "{name} must not use the unsound converse implication"
        );
    }

    let [narrow_abort_site] = plan("narrow_abort").crash.checked_sites() else {
        panic!("the narrow abort should retain its explicit crash site")
    };
    assert_eq!(narrow_abort_site.guard_covering_buckets().len(), 1);
    assert_eq!(
        plan("narrow_abort")
            .crash
            .covering_buckets_for_site(narrow_abort_site)
            .count(),
        1,
        "a same-cause unconditional route covers the crash site"
    );
}

#[test]
fn crash_guard_entailment_normalizes_boolean_literal_relations() {
    let source = r#"
    machine risky() -> i32
    crashes Trap
    { 1 }

    machine equal_true(flag: bool) -> i32
    crashes Trap
        flag
    {
        transition {
            flag == true -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine equal_false(flag: bool) -> i32
    crashes Trap
        !flag
    {
        transition {
            flag == false -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine not_equal_true(flag: bool) -> i32
    crashes Trap
        !flag
    {
        transition {
            flag != true -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine not_equal_false(flag: bool) -> i32
    crashes Trap
        flag
    {
        transition {
            flag != false -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine fallthrough_equal_true(flag: bool) -> i32
    crashes Trap
        !flag
    {
        transition {
            flag == true -> 0i32
            _ -> fail()
        }
        state fail() -> i32 { crash Trap; }
    }

    machine guarded_call(flag: bool) -> i32
    crashes Trap
        flag
    {
        transition {
            flag == true -> invoke()
            _ -> 0i32
        }
        state invoke() -> i32 { risky() }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed)
        .expect("Boolean literal relations should imply their normalized operand polarity");
    let plan = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
    };

    for name in [
        "equal_true",
        "equal_false",
        "not_equal_true",
        "not_equal_false",
        "fallthrough_equal_true",
    ] {
        let [site] = plan(name).crash.checked_sites() else {
            panic!("{name} should retain one crash site")
        };
        assert_eq!(
            site.guard_covering_buckets().len(),
            1,
            "{name} should cover its route through the normalized relation"
        );
    }

    let [call] = plan("guarded_call").crash.checked_calls() else {
        panic!("guarded_call should retain one checked call")
    };
    assert_eq!(call.path_guard_conjuncts().len(), 1);
    assert_eq!(
        call.path_guard_consequences().len(),
        3,
        "the exact equality, reversed equality, and implied operand remain separate"
    );
}

#[test]
fn crash_guard_entailment_normalizes_comparison_equivalences() {
    let source = r#"
    machine risky() -> i32
    crashes Trap
    { 1 }

    machine reversed_order(left: i32, right: i32) -> i32
    crashes Trap
        right > left
    {
        transition {
            left < right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine strict_order_weakens(left: i32, right: i32) -> i32
    crashes Trap
        left <= right
    {
        transition {
            left < right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine strict_order_is_distinct(left: i32, right: i32) -> i32
    crashes Trap
        left != right
    {
        transition {
            left < right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine integer_equality_bounds(left: i32, right: i32) -> i32
    crashes Trap
        right >= left
    {
        transition {
            left == right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine integer_order_fallthrough(left: i32, right: i32) -> i32
    crashes Trap
        left >= right
    {
        transition {
            left < right -> 0i32
            _ -> fail()
        }
        state fail() -> i32 { crash Trap; }
    }

    machine float_order_fallthrough_stays_opaque(left: f32, right: f32) -> i32
    crashes Trap
        left >= right
    {
        transition {
            left < right -> 0i32
            _ -> fail()
        }
        state fail() -> i32 { crash Trap; }
    }

    machine negated_equality(left: i32, right: i32) -> i32
    crashes Trap
        left != right
    {
        transition {
            left == right -> 0i32
            _ -> fail()
        }
        state fail() -> i32 { crash Trap; }
    }

    machine reversed_equality(left: i32, right: i32) -> i32
    crashes Trap
        right == left
    {
        transition {
            left == right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine transitive_integer_order(left: i32, middle: i32, right: i32) -> i32
    crashes Trap
        left < right
    {
        transition {
            left < middle && middle <= right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine nontransitive_integer_order(left: i32, middle: i32, right: i32) -> i32
    crashes Trap
        left < right
    {
        transition {
            left < middle && right < middle -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine transitive_nonstrict_order(left: i32, middle: i32, right: i32) -> i32
    crashes Trap
        left <= right
    {
        transition {
            left <= middle && middle <= right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine integer_order_antisymmetry(left: i32, right: i32) -> i32
    crashes Trap
        left == right
    {
        transition {
            left <= right && right <= left -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine integer_nonstrict_plus_disequality(left: i32, right: i32) -> i32
    crashes Trap
        left < right
    {
        transition {
            left <= right && left != right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine one_sided_order_does_not_prove_equality(left: i32, right: i32) -> i32
    crashes Trap
        left == right
    {
        transition {
            left <= right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine float_order_antisymmetry_stays_opaque(left: f32, right: f32) -> i32
    crashes Trap
        left == right
    {
        transition {
            left <= right && right <= left -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine float_nonstrict_plus_disequality_stays_opaque(left: f32, right: f32) -> i32
    crashes Trap
        left < right
    {
        transition {
            left <= right && left != right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine transitive_order_across_states(left: i32, middle: i32, right: i32) -> i32
    crashes Trap
        left < right
    {
        transition {
            left < middle -> compare(left, middle, right)
            _ -> 0i32
        }
        state compare(left: i32, middle: i32, right: i32) -> i32 {
            transition {
                middle <= right -> fail()
                _ -> 0i32
            }
        }
        state fail() -> i32 { crash Trap; }
    }

    machine nonstrict_chain_does_not_prove_strict(
        left: i32,
        middle: i32,
        right: i32
    ) -> i32
    crashes Trap
        left < right
    {
        transition {
            left <= middle && middle <= right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine guarded_call(left: i32, right: i32) -> i32
    crashes Trap
        left != right
    {
        transition {
            left == right -> 0i32
            _ -> invoke()
        }
        state invoke() -> i32 { risky() }
    }

    machine transitive_guarded_call(left: i32, middle: i32, right: i32) -> i32
    crashes Trap
        left < right
    {
        transition {
            left <= middle && middle < right -> invoke()
            _ -> 0i32
        }
        state invoke() -> i32 { risky() }
    }

    machine antisymmetric_guarded_call(left: i32, right: i32) -> i32
    crashes Trap
        left == right
    {
        transition {
            left <= right && right <= left -> invoke()
            _ -> 0i32
        }
        state invoke() -> i32 { risky() }
    }


    machine strict_refined_guarded_call(left: i32, right: i32) -> i32
    crashes Trap
        left < right
    {
        transition {
            left <= right && left != right -> invoke()
            _ -> 0i32
        }
        state invoke() -> i32 { risky() }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed)
        .expect("equivalent comparison spellings should cover crash routes");
    let plan = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
    };

    for name in [
        "reversed_order",
        "strict_order_weakens",
        "strict_order_is_distinct",
        "integer_equality_bounds",
        "integer_order_fallthrough",
        "negated_equality",
        "reversed_equality",
        "transitive_integer_order",
        "transitive_nonstrict_order",
        "integer_order_antisymmetry",
        "integer_nonstrict_plus_disequality",
        "transitive_order_across_states",
    ] {
        let [site] = plan(name).crash.checked_sites() else {
            panic!("{name} should retain one crash site")
        };
        assert_eq!(
            site.guard_covering_buckets().len(),
            1,
            "{name} should cover its equivalent comparison route"
        );
    }
    let [opaque_site] = plan("float_order_fallthrough_stays_opaque")
        .crash
        .checked_sites()
    else {
        panic!("float ordered fallthrough should retain one crash site")
    };
    assert!(
        opaque_site.guard_covering_buckets().is_empty(),
        "unordered float comparison negation must remain opaque"
    );
    for (name, reason) in [
        (
            "nontransitive_integer_order",
            "relations without a shared ordered endpoint must not compose",
        ),
        (
            "nonstrict_chain_does_not_prove_strict",
            "an all-nonstrict chain must not imply a strict endpoint relation",
        ),
        (
            "one_sided_order_does_not_prove_equality",
            "one nonstrict direction must not imply integer equality",
        ),
        (
            "float_order_antisymmetry_stays_opaque",
            "unordered float relations must not enter integer antisymmetry",
        ),
        (
            "float_nonstrict_plus_disequality_stays_opaque",
            "unordered float relations must not enter integer strict refinement",
        ),
    ] {
        let [site] = plan(name).crash.checked_sites() else {
            panic!("{name} should retain one crash site")
        };
        assert!(site.guard_covering_buckets().is_empty(), "{reason}");
    }

    let [call] = plan("guarded_call").crash.checked_calls() else {
        panic!("guarded_call should retain one checked call")
    };
    assert!(
        call.path_guard_consequences().len() >= 3,
        "the exact fallthrough predicate and normalized comparison forms remain distinct"
    );
    let [transitive_call] = plan("transitive_guarded_call").crash.checked_calls() else {
        panic!("transitive_guarded_call should retain one checked call")
    };
    assert!(
        transitive_call.path_guard_consequences().len() > call.path_guard_consequences().len(),
        "transitive integer order should add source-independent call-path consequences"
    );
    let [antisymmetric_call] = plan("antisymmetric_guarded_call").crash.checked_calls() else {
        panic!("antisymmetric_guarded_call should retain one checked call")
    };
    assert!(
        antisymmetric_call.path_guard_consequences().len() > call.path_guard_consequences().len(),
        "integer antisymmetry should add source-independent call-path equality"
    );
    let [strict_refined_call] = plan("strict_refined_guarded_call").crash.checked_calls() else {
        panic!("strict_refined_guarded_call should retain one checked call")
    };
    assert!(
        strict_refined_call.path_guard_consequences().len() > call.path_guard_consequences().len(),
        "integer disequality should sharpen a nonstrict call-path bound"
    );
}

#[test]
fn checked_crash_calls_retain_invocation_specific_route_refinement() {
    let source = r#"
    machine risky(flag: bool) -> i32
    crashes Trap
        flag
    { 1 }

    machine safe() -> i32 { risky(false) }

    machine certain() -> i32
    crashes Trap
    { risky(true) }

    machine forwarded(flag: bool) -> i32
    crashes Trap
        flag
    { risky(flag) }

    machine local_forwarded(flag: bool) -> i32 {
        let forwarded: bool = flag;
        risky(forwarded)
    }

    machine computed_local_forwarded(flag: bool) -> i32
    crashes Trap
    {
        let forwarded: bool = !flag;
        risky(forwarded)
    }

    machine conditioned(flag: bool) -> i32
    crashes Trap
        flag
    {
        transition {
            flag -> invoke()
            _ -> 0i32
        }

        state invoke() -> i32 { risky(true) }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let plan = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
    };

    for name in [
        "risky",
        "safe",
        "certain",
        "forwarded",
        "local_forwarded",
        "computed_local_forwarded",
        "conditioned",
    ] {
        let machine = symbol_of_checked(&checked, name);
        assert_eq!(
            checked
                .facts
                .contract_plans
                .realized_envelope(machine)
                .expect("realized contract envelope")
                .checked_crash,
            plan(name).crash,
            "the realized envelope must retain post-validation crash evidence for {name}",
        );
    }

    let [safe_call] = plan("safe").crash.checked_calls() else {
        panic!("the crash-capable invocation should retain one checked call row")
    };
    assert!(
        safe_call.surviving_buckets().is_empty(),
        "a concrete false argument disproves the callee's only crash route"
    );
    assert_eq!(safe_call.location().statement_ordinal(), 0);
    assert_eq!(safe_call.location().call_ordinal(), 0);
    assert_eq!(
        safe_call.target_machine(),
        symbol_of_checked(&checked, "risky")
    );
    assert_eq!(
        safe_call.target_contract_fingerprint(),
        plan("risky").fingerprint
    );

    let [certain_call] = plan("certain").crash.checked_calls() else {
        panic!("the concrete true invocation should retain one checked call row")
    };
    let [certain_bucket] = certain_call.surviving_buckets() else {
        panic!("the concrete true route should survive")
    };
    assert!(certain_bucket.is_unconditional());
    assert_eq!(certain_bucket.cause(), psi_checked_trees::CrashCause::Trap);

    let [forwarded_call] = plan("forwarded").crash.checked_calls() else {
        panic!("the unresolved invocation should retain one checked call row")
    };
    let [forwarded_bucket] = forwarded_call.surviving_buckets() else {
        panic!("the unresolved route should survive")
    };
    let [psi_checked_trees::CrashRouteGuard::Predicate(forwarded_route)] =
        forwarded_bucket.alternative_guards()
    else {
        panic!("the unresolved route should remain a predicate")
    };
    let [psi_checked_trees::CrashRouteGuard::Predicate(published_route)] =
        plan("forwarded").crash.published()[0].alternative_guards()
    else {
        panic!("the caller should publish its forwarded predicate")
    };
    assert_eq!(
        forwarded_route, published_route,
        "argument substitution should move the callee route into the caller's positional namespace"
    );
    assert_eq!(
        forwarded_route.scalar_expression(),
        Some(&psi_checked_trees::CheckedBooleanExpression::Parameter { position: 0 }),
        "invocation refinement must retain checked scalar meaning, not only predicate identity",
    );

    let [local_forwarded_call] = plan("local_forwarded").crash.checked_calls() else {
        panic!("the local-argument invocation should retain one checked call row")
    };
    let [local_forwarded_bucket] = local_forwarded_call.surviving_buckets() else {
        panic!("the local-argument route should survive")
    };
    let [psi_checked_trees::CrashRouteGuard::Predicate(local_forwarded_route)] =
        local_forwarded_bucket.alternative_guards()
    else {
        panic!("the local-argument route should remain a predicate")
    };
    assert_eq!(
        local_forwarded_route.scalar_expression(),
        Some(&psi_checked_trees::CheckedBooleanExpression::Local { position: 1 }),
        "direct refinement should retain the caller-local value position assigned after its one parameter",
    );

    let [computed_local_call] = plan("computed_local_forwarded").crash.checked_calls() else {
        panic!("the computed-local invocation should retain one checked call row")
    };
    let [computed_local_bucket] = computed_local_call.surviving_buckets() else {
        panic!("the computed-local route should survive")
    };
    let [psi_checked_trees::CrashRouteGuard::Predicate(computed_local_route)] =
        computed_local_bucket.alternative_guards()
    else {
        panic!("the computed-local route should remain a predicate")
    };
    assert_eq!(
        computed_local_route.scalar_expression(),
        Some(&psi_checked_trees::CheckedBooleanExpression::Local { position: 1 }),
        "computed refinement should retain the caller-local value position",
    );

    let [conditioned_call] = plan("conditioned").crash.checked_calls() else {
        panic!("the named transition itself is not a public machine invocation")
    };
    assert_eq!(
        conditioned_call.path_guard_conjuncts().len(),
        1,
        "the checked call retains the exact incoming path conjunction"
    );
}

#[test]
fn published_caller_must_cover_every_surviving_call_crash_route() {
    let source = r#"
    machine risky() -> i32
    crashes Abort
    {
        crash Abort;
    }

    machine wrong_cause() -> i32
    crashes Trap
    {
        risky()
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("the caller's Trap ceiling cannot cover a surviving Abort route");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("call from `wrong_cause` to `risky`")
            && diagnostic.message.contains("uncovered Abort crash route")
    }));
}

#[test]
fn checked_crash_calls_select_acyclic_private_body_summaries() {
    let source = r#"
    machine inferred_abort() -> i32 {
        crash Abort;
    }

    machine inferred_safe() -> i32 { 1 }

    machine call_abort() -> i32 { inferred_abort() }
    machine call_safe() -> i32 { inferred_safe() }

    machine nonleaf() -> i32 { inferred_abort() }
    machine call_nonleaf() -> i32 { nonleaf() }

    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let plan = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
    };

    assert_eq!(
        plan("inferred_abort").crash.interface(),
        psi_checked_trees::CrashInterface::InternalInferred
    );
    let [abort_call] = plan("call_abort").crash.checked_calls() else {
        panic!("a call to a private crashing leaf should retain one selected body summary")
    };
    let [abort_bucket] = abort_call.surviving_buckets() else {
        panic!("the private leaf's explicit crash should survive as one inferred bucket")
    };
    assert!(abort_bucket.is_unconditional());
    assert_eq!(abort_bucket.cause(), psi_checked_trees::CrashCause::Abort);

    let [safe_call] = plan("call_safe").crash.checked_calls() else {
        panic!("a call to a private crash-free leaf should retain positive empty evidence")
    };
    assert!(safe_call.surviving_buckets().is_empty());

    assert_eq!(plan("nonleaf").crash.checked_calls().len(), 1);
    let [nonleaf_call] = plan("call_nonleaf").crash.checked_calls() else {
        panic!("the acyclic private wrapper should publish one selected body summary")
    };
    let [nonleaf_bucket] = nonleaf_call.surviving_buckets() else {
        panic!("the nested abort should propagate through the private wrapper")
    };
    assert!(nonleaf_bucket.is_unconditional());
    assert_eq!(nonleaf_bucket.cause(), psi_checked_trees::CrashCause::Abort);
}

#[test]
fn private_crash_summaries_compose_guarded_routes_across_nonleaf_calls() {
    let source = r#"
    machine risky(flag: bool) -> i32
    crashes Trap
        flag
    { 1 }

    machine inner(flag: bool) -> i32 { risky(flag) }
    machine outer(flag: bool) -> i32 { inner(flag) }

    machine covered(flag: bool) -> i32
    crashes Trap
        flag
    { outer(flag) }

    machine disproved() -> i32 { outer(false) }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed)
        .expect("a published caller should cover a guard retained through private wrappers");
    let plan = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
    };

    let [outer_to_inner] = plan("outer").crash.checked_calls() else {
        panic!("outer should retain its private call")
    };
    let [outer_bucket] = outer_to_inner.surviving_buckets() else {
        panic!("the inner summary should retain its guarded route")
    };
    let [psi_checked_trees::CrashRouteGuard::Predicate(outer_route)] =
        outer_bucket.alternative_guards()
    else {
        panic!("the private nonleaf route should remain guarded")
    };

    let [covered_call] = plan("covered").crash.checked_calls() else {
        panic!("covered should retain its outer call")
    };
    let [covered_bucket] = covered_call.surviving_buckets() else {
        panic!("covered should retain the composed route")
    };
    let [psi_checked_trees::CrashRouteGuard::Predicate(covered_route)] =
        covered_bucket.alternative_guards()
    else {
        panic!("the composed route should remain guarded")
    };
    let [psi_checked_trees::CrashRouteGuard::Predicate(published_route)] =
        plan("covered").crash.published()[0].alternative_guards()
    else {
        panic!("covered should publish one guarded route")
    };
    assert_eq!(outer_route, covered_route);
    assert_eq!(covered_route, published_route);
    assert_eq!(
        covered_route.scalar_expression(),
        Some(&psi_checked_trees::CheckedBooleanExpression::Parameter { position: 0 }),
        "acyclic private-summary substitution must preserve terminal-lowerable scalar meaning",
    );

    let [disproved_call] = plan("disproved").crash.checked_calls() else {
        panic!("disproved should retain positive evidence for its outer call")
    };
    assert!(
        disproved_call.surviving_buckets().is_empty(),
        "substitution through both private wrappers should prove false"
    );
}

#[test]
fn checked_crash_calls_select_machine_requirement_capsules() {
    let source = r#"
    machine apply<machine Selected>(flag: bool)
    where machine Selected(value: bool)
        crashes Abort
            value;
    crashes Abort
        flag
    {
        Selected(flag);
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("requirement crash capsule should lower");
    let apply = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .expect("apply machine");
    let plan = checked
        .facts
        .contract_plans
        .for_machine(apply.symbol)
        .expect("apply contract plan");
    let [call] = plan.crash.checked_calls() else {
        panic!("the requirement call should retain one checked crash row");
    };
    let capsule = checked
        .facts
        .contract_plans
        .crash_capsule(call.target_machine(), call.target_state())
        .expect("the abstract target should retain its normalized capsule");
    assert_eq!(
        call.target_contract_fingerprint(),
        capsule.target_contract_fingerprint()
    );
    let [bucket] = call.surviving_buckets() else {
        panic!("the unknown flag should retain the guarded Abort bucket");
    };
    assert_eq!(bucket.cause(), psi_checked_trees::CrashCause::Abort);
    assert!(matches!(
        bucket.alternative_guards(),
        [psi_checked_trees::CrashRouteGuard::Predicate(_)]
    ));
}
