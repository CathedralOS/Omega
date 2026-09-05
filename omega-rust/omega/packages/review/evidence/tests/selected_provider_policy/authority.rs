use super::*;

#[test]
fn same_spelled_service_reach_and_invocation_keep_their_exact_source_owner() {
    let extra = "pub boundary trait Extra {}\n";
    let source = r#"pub boundary trait Worker {
    machine work() reaches Extra invokes Extra;
}
pub data WorkerProvider {}
pub WorkerProviderWorker: WorkerProvider satisfies Worker;
pub machine WorkerProvider::work() satisfies Worker::work {}
"#;
    let local = Fixture::local(
        &format!("{extra}{source}"),
        fixtures::BUILD,
        TargetProfile::WindowsX64,
    );
    let foreign = Fixture::foreign(
        &format!("use producer::bindings;\n{source}"),
        extra,
        TargetProfile::WindowsX64,
    );
    let local = project(&local);
    let foreign = project(&foreign);
    let local_method = &local
        .plans()
        .iter()
        .find(|plan| plan.schema_declaration().path() == "Worker")
        .unwrap()
        .methods()[0];
    let foreign_method = &foreign
        .plans()
        .iter()
        .find(|plan| plan.schema_declaration().path() == "Worker")
        .unwrap()
        .methods()[0];
    assert_eq!(local_method.service_reach(), foreign_method.service_reach());
    assert_eq!(
        local_method.synchronous_invocations(),
        foreign_method.synchronous_invocations()
    );
    assert!(
        local_method
            .service_reach()
            .iter()
            .any(|service| service == "Extra")
    );
    assert!(
        local_method
            .synchronous_invocations()
            .iter()
            .any(|service| service == "Extra")
    );
    let local_authority = local_method.authority();
    let foreign_authority = foreign_method.authority();
    for (authority, package) in [
        (local_authority, package_identity()),
        (foreign_authority, fixtures::foreign_identity()),
    ] {
        let reach = authority
            .service_reach()
            .iter()
            .find(|service| service.path() == "Extra")
            .unwrap();
        assert_eq!(reach.owner(), PackageReviewNominalOwner::Package(package));
        let invocation = authority
            .synchronous_invocations()
            .iter()
            .find_map(|invocation| {
                invocation
                    .service()
                    .filter(|service| service.path() == "Extra")
            })
            .unwrap();
        assert_eq!(invocation, reach);
    }
    assert_ne!(local_authority, foreign_authority);
    assert_ne!(local, foreign);
    assert_ne!(
        local.canonical_bytes().unwrap(),
        foreign.canonical_bytes().unwrap()
    );
}

#[test]
fn selected_progress_premise_retains_profile_projection_and_establishment_owners() {
    let source = r#"pub data SchedulerHandle {}
pub data Context { scheduler: SchedulerHandle; }
pub domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
pub boundary trait SchedulerRuntime {
    machine wait(context: Context)
    requires context.scheduler in WeakFair
    terminates;
}
pub data SchedulerProvider {}
pub SchedulerProviderRuntime: SchedulerProvider satisfies SchedulerRuntime;
pub machine SchedulerProvider::wait(context: Context) satisfies SchedulerRuntime::wait {}
"#;
    let fixture = Fixture::local(source, fixtures::BUILD, TargetProfile::WindowsX64);
    let policy = project(&fixture);
    let method = &policy
        .plans()
        .iter()
        .find(|plan| plan.schema_declaration().path() == "SchedulerRuntime")
        .unwrap()
        .methods()[0];
    assert!(method.terminates_guarantee());
    let [premise] = method.authority().progress_premises() else {
        panic!("one exact parameter-field progress premise")
    };
    let owner = PackageReviewNominalOwner::Package(package_identity());
    assert_eq!(premise.profile().path(), "SchedulerHandle::WeakFair");
    assert_eq!(premise.profile().owner(), owner);
    assert_eq!(
        premise.subject(),
        effects::provider_plan::ServiceProgressSubject::Parameter(0)
    );
    let [projection] = premise.subject_projections() else {
        panic!("one retained field projection")
    };
    assert_eq!(projection.path(), "Context::scheduler");
    assert_eq!(projection.owner(), owner);
    let [route] = premise.establishment_routes() else {
        panic!("one owner-authored establishment route")
    };
    assert_eq!(
        route.kind(),
        effects::provider_plan::ServiceProgressEstablishmentRouteKind::BoundaryRequirement
    );
    assert_eq!(route.requirement_owner().path(), "SchedulerAdmission");
    assert_eq!(route.requirement_owner().owner(), owner);
    assert!(
        route
            .requirement()
            .path()
            .contains("SchedulerAdmission::grant")
    );
    assert_eq!(route.requirement().owner(), owner);
}
