use crate::tests::front_end::{checked_program, checked_program_result};
use crate::tests::termination::symbol_of_checked;

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

    let diagnostics = checked_program_result(source)
        .expect_err("the caller's Trap ceiling cannot cover a surviving Abort route");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("call from `wrong_cause` to `risky`")
            && diagnostic.message.contains("uncovered Abort crash route")
    }));
}

#[test]
fn private_explicit_crash_ceiling_must_cover_every_direct_site() {
    let uncovered = r#"
    machine private_uncovered(flag: bool)
    crashes Trap
        flag
    {
        crash Trap;
    }
    "#;

    let diagnostics = checked_program_result(uncovered)
        .expect_err("a private published ceiling cannot retain an uncovered direct site");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("machine `private_uncovered` has an uncovered Trap crash")
    }));

    let covered = r#"
    machine private_covered()
    crashes Trap
    {
        crash Trap;
    }

    machine private_inferred() {
        crash Abort;
    }
    "#;

    let checked = checked_program(covered);
    assert_eq!(
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, "private_covered"))
            .expect("covered private contract")
            .crash
            .interface(),
        checked_trees::CrashInterface::PublishedCeiling,
    );
    assert_eq!(
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, "private_inferred"))
            .expect("inferred private contract")
            .crash
            .interface(),
        checked_trees::CrashInterface::InternalInferred,
    );
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

    let checked = checked_program(source);
    let plan = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
    };

    assert_eq!(
        plan("inferred_abort").crash.interface(),
        checked_trees::CrashInterface::InternalInferred
    );
    let [abort_call] = plan("call_abort").crash.checked_calls() else {
        panic!("a call to a private crashing leaf should retain one selected body summary")
    };
    let [abort_bucket] = abort_call.surviving_buckets() else {
        panic!("the private leaf's explicit crash should survive as one inferred bucket")
    };
    assert!(abort_bucket.is_unconditional());
    assert_eq!(abort_bucket.cause(), checked_trees::CrashCause::Abort);

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
    assert_eq!(nonleaf_bucket.cause(), checked_trees::CrashCause::Abort);
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

    let checked = checked_program(source);
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
    let [checked_trees::CrashRouteGuard::Predicate(outer_route)] =
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
    let [checked_trees::CrashRouteGuard::Predicate(covered_route)] =
        covered_bucket.alternative_guards()
    else {
        panic!("the composed route should remain guarded")
    };
    let [checked_trees::CrashRouteGuard::Predicate(published_route)] =
        plan("covered").crash.published()[0].alternative_guards()
    else {
        panic!("covered should publish one guarded route")
    };
    assert_eq!(outer_route, covered_route);
    assert_eq!(covered_route, published_route);
    assert_eq!(
        covered_route.scalar_expression(),
        Some(&checked_trees::CheckedBooleanExpression::Parameter { position: 0 }),
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

    let checked = checked_program(source);
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
        call.target_contract_report_fingerprint(),
        capsule.target_contract_report_fingerprint()
    );
    let [bucket] = call.surviving_buckets() else {
        panic!("the unknown flag should retain the guarded Abort bucket");
    };
    assert_eq!(bucket.cause(), checked_trees::CrashCause::Abort);
    assert!(matches!(
        bucket.alternative_guards(),
        [checked_trees::CrashRouteGuard::Predicate(_)]
    ));
}
