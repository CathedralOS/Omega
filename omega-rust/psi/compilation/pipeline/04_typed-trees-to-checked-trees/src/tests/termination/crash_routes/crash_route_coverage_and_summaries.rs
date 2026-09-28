use crate::tests::front_end::checked_program;
use crate::tests::termination::symbol_of_checked;

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
