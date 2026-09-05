use super::*;

#[test]
fn callable_progress_premises_retain_profile_projection_and_establishment_owner() {
    let source = r#"
pub data SchedulerHandle {}
pub data Context { scheduler: SchedulerHandle; }
pub domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
pub machine wait(context: Context)
requires context.scheduler in WeakFair
terminates;
{}
"#;
    let fixture = Fixture::local(source);
    let machine = fixture
        .checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "wait")
        .unwrap();
    let Some(psi_language_semantics::TerminationGuarantee::Terminates { premises }) =
        machine.termination_plan.interface.published()
    else {
        panic!("source publishes its conditional termination promise")
    };
    assert_eq!(premises.len(), 1);
    let checked_plan = fixture
        .checked
        .facts
        .termination
        .for_machine(machine.symbol)
        .unwrap();
    assert!(
        matches!(&checked_plan.checked_summary, psi_language_semantics::TerminationGuarantee::Terminates {premises} if premises.is_empty()),
        "the empty implementation terminates independently of its published premise"
    );
    let original = project(&fixture);
    let wait = callable(&original, "wait");
    assert!(
        matches!(wait.checked_termination(), PackagePolicyTermination::Terminates { premises } if premises.is_empty())
    );
    let Some(PackagePolicyTermination::Terminates { premises }) = wait.declared_termination()
    else {
        panic!("published termination retains its explicit progress premise")
    };
    let [premise] = premises.as_slice() else {
        panic!("one parameter-field progress premise")
    };
    assert_eq!(premise.profile().path(), "SchedulerHandle::WeakFair");
    assert_eq!(
        premise.profile().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(premise.projections().len(), 1);
    assert_eq!(premise.projections()[0].path(), "Context::scheduler");
    let [route] = premise.establishment_routes() else {
        panic!("one declared establishment route")
    };
    assert_eq!(route.requirement_owner().path(), "SchedulerAdmission");
    assert_eq!(
        route.requirement_owner().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    let changed = project(&Fixture::local(&source.replace("WeakFair", "StrongFair")));
    assert_ne!(
        wait.declared_termination(),
        callable(&changed, "wait").declared_termination()
    );
    assert_eq!(
        wait.checked_termination(),
        callable(&changed, "wait").checked_termination()
    );
    assert_ne!(
        original.canonical_bytes().unwrap(),
        changed.canonical_bytes().unwrap()
    );
    let mut stale = fixture.checked.clone();
    stale
        .typed
        .machines_mut()
        .iter_mut()
        .find(|candidate| candidate.symbol == machine.symbol)
        .unwrap()
        .termination_plan
        .interface = psi_language_semantics::TerminationInterface::Published(
        psi_language_semantics::TerminationGuarantee::NoGuarantee,
    );
    assert!(
        project_checked_callable_policy(&stale, fixture.target, package_identity()).is_err(),
        "changed authored termination cannot reuse the retained checked interface"
    );
}

#[test]
fn published_operational_ceilings_remain_checked_caller_summaries() {
    let quiet = project(&Fixture::local("pub machine operation() {}\n"));
    let published = project(&Fixture::local(
        "pub machine operation() suspends; blocks; {}\n",
    ));
    let quiet_row = callable(&quiet, "operation");
    let published_row = callable(&published, "operation");
    assert_eq!(quiet_row.declared_may_suspend(), Some(false));
    assert_eq!(quiet_row.declared_may_block(), Some(false));
    assert_eq!(published_row.declared_may_suspend(), Some(true));
    assert_eq!(published_row.declared_may_block(), Some(true));
    assert!(!quiet_row.checked_may_suspend());
    assert!(!quiet_row.checked_may_block());
    assert!(published_row.checked_may_suspend());
    assert!(published_row.checked_may_block());
    assert_ne!(
        quiet.canonical_bytes().unwrap(),
        published.canonical_bytes().unwrap()
    );
}

#[test]
fn explicit_termination_promise_differs_from_public_omission_despite_equal_body_proof() {
    let omitted = project(&Fixture::local("pub machine operation() {}\n"));
    let promised = project(&Fixture::local("pub machine operation() terminates; {}\n"));
    let omitted_row = callable(&omitted, "operation");
    let promised_row = callable(&promised, "operation");
    assert_eq!(omitted_row.declared_termination(), None);
    assert!(
        matches!(promised_row.declared_termination(), Some(PackagePolicyTermination::Terminates { premises }) if premises.is_empty())
    );
    assert_eq!(
        omitted_row.checked_termination(),
        promised_row.checked_termination()
    );
    assert!(
        matches!(omitted_row.checked_termination(), PackagePolicyTermination::Terminates { premises } if premises.is_empty())
    );
    assert_ne!(
        omitted.canonical_bytes().unwrap(),
        promised.canonical_bytes().unwrap()
    );
}
