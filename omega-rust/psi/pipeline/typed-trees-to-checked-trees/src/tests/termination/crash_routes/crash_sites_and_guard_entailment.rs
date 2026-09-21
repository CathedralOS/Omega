use crate::tests::termination::{
    Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve,
    symbol_of_checked,
};
use crate::{CheckingRequest, lower_typed_trees};

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
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed, &CheckingRequest::crash_fact_inspection())
        .expect("raw crash-fact inspection should succeed before production admission");
    let plan = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
    };

    assert_eq!(
        plan("clear_body").report_fingerprint,
        plan("crashing_body").report_fingerprint,
        "changing the checked body must not change a published contract identity"
    );
    assert!(plan("clear_body").crash.checked_sites().is_empty());
    let [site] = plan("crashing_body").crash.checked_sites() else {
        panic!("the explicit crash should produce exactly one checked site")
    };
    assert_eq!(site.cause(), checked_trees::CrashCause::Abort);
    assert_eq!(site.location().statement_ordinal(), 0);
    let [covering_bucket] = site.guard_covering_buckets() else {
        panic!("an unconditional same-cause route should cover every site guard")
    };
    assert!(
        plan("crashing_body")
            .crash
            .published_bucket(*covering_bucket)
            .is_some_and(|bucket| bucket.is_unconditional()
                && bucket.cause() == checked_trees::CrashCause::Abort)
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
                && bucket.cause() == checked_trees::CrashCause::Trap)
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
                && bucket.cause() == checked_trees::CrashCause::Trap)
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
                .is_some_and(|bucket| bucket.cause() == checked_trees::CrashCause::Trap)
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
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
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
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed, &CheckingRequest::crash_fact_inspection())
        .expect("raw comparison coverage facts should form before production admission");
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
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("checked lowering should succeed");
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
        safe_call.target_contract_report_fingerprint(),
        plan("risky").report_fingerprint
    );

    let [certain_call] = plan("certain").crash.checked_calls() else {
        panic!("the concrete true invocation should retain one checked call row")
    };
    let [certain_bucket] = certain_call.surviving_buckets() else {
        panic!("the concrete true route should survive")
    };
    assert!(certain_bucket.is_unconditional());
    assert_eq!(certain_bucket.cause(), checked_trees::CrashCause::Trap);

    let [forwarded_call] = plan("forwarded").crash.checked_calls() else {
        panic!("the unresolved invocation should retain one checked call row")
    };
    let [forwarded_bucket] = forwarded_call.surviving_buckets() else {
        panic!("the unresolved route should survive")
    };
    let [checked_trees::CrashRouteGuard::Predicate(forwarded_route)] =
        forwarded_bucket.alternative_guards()
    else {
        panic!("the unresolved route should remain a predicate")
    };
    let [checked_trees::CrashRouteGuard::Predicate(published_route)] =
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
        Some(&checked_trees::CheckedBooleanExpression::Parameter { position: 0 }),
        "invocation refinement must retain checked scalar meaning, not only predicate identity",
    );

    let [local_forwarded_call] = plan("local_forwarded").crash.checked_calls() else {
        panic!("the local-argument invocation should retain one checked call row")
    };
    let [local_forwarded_bucket] = local_forwarded_call.surviving_buckets() else {
        panic!("the local-argument route should survive")
    };
    let [checked_trees::CrashRouteGuard::Predicate(local_forwarded_route)] =
        local_forwarded_bucket.alternative_guards()
    else {
        panic!("the local-argument route should remain a predicate")
    };
    assert_eq!(
        local_forwarded_route.scalar_expression(),
        Some(&checked_trees::CheckedBooleanExpression::Local { position: 1 }),
        "direct refinement should retain the caller-local value position assigned after its one parameter",
    );

    let [computed_local_call] = plan("computed_local_forwarded").crash.checked_calls() else {
        panic!("the computed-local invocation should retain one checked call row")
    };
    let [computed_local_bucket] = computed_local_call.surviving_buckets() else {
        panic!("the computed-local route should survive")
    };
    let [checked_trees::CrashRouteGuard::Predicate(computed_local_route)] =
        computed_local_bucket.alternative_guards()
    else {
        panic!("the computed-local route should remain a predicate")
    };
    assert_eq!(
        computed_local_route.scalar_expression(),
        Some(&checked_trees::CheckedBooleanExpression::Local { position: 1 }),
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
