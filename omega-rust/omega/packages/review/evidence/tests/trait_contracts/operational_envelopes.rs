use crate::support::*;
use compiler::CheckedCompileRequest;

#[test]
fn public_trait_operational_envelope_is_exact_review_shape() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait Console { }
pub boundary trait Worker {
    machine wait(handler: &mut Console)
    reaches <= Console
    invokes handler;
    invokes Console;
    suspends;
    blocks;
}
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect("public trait suspension fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public trait operational review should close");
    let worker = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Worker")
        .expect("worker trait row");
    let [wait] = worker.requirements() else {
        panic!("one worker requirement")
    };
    let [console] = wait.service_reach() else {
        panic!("one exact service-reach row")
    };
    assert_eq!(console.path(), "Console");
    assert_eq!(
        console.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert!(wait.service_reach_is_installation_bound());
    assert_eq!(wait.synchronous_invocations().len(), 2);
    assert_eq!(wait.synchronous_invocations()[0].parameter(), Some(0));
    assert_eq!(
        wait.synchronous_invocations()[1]
            .service()
            .expect("service invocation")
            .path(),
        "Console"
    );
    assert!(wait.suspends());
    assert!(wait.blocks());
}

#[test]
fn public_trait_termination_is_parameter_rooted_review_shape() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data SchedulerHandle { }
pub domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
pub boundary trait SchedulerRuntime {
    machine wait(scheduler: SchedulerHandle)
    requires scheduler in WeakFair
    terminates;
}
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect("public trait termination fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public trait termination review should close");
    let runtime = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "SchedulerRuntime")
        .expect("scheduler runtime trait row");
    let wait = runtime
        .requirements()
        .iter()
        .find(|requirement| {
            requirement
                .identity()
                .path()
                .contains("SchedulerRuntime::wait")
        })
        .expect("wait requirement row");
    let premises = wait
        .termination()
        .premises()
        .expect("wait must promise termination");
    let [premise] = premises else {
        panic!("one parameter-rooted premise")
    };
    assert_eq!(premise.profile().path(), "SchedulerHandle::WeakFair");
    assert_eq!(
        premise.profile().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert!(premise.projections().is_empty());
    assert_eq!(premise.subject().parameter(), Some(0));
}

#[test]
fn public_trait_termination_rejects_a_non_public_progress_profile() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data SchedulerHandle { }
domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
pub boundary trait SchedulerRuntime {
    machine wait(scheduler: SchedulerHandle)
    requires scheduler in WeakFair
    terminates;
}
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let diagnostics = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect_err("ordinary visibility must reject a private profile in a public trait contract");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private domain `SchedulerHandle::WeakFair`")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn public_trait_termination_certifies_a_public_declaration_subject() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data SchedulerHandle { }
pub domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
pub boundary trait SchedulerRuntime {
    machine wait(scheduler: SchedulerHandle)
    requires scheduler in WeakFair
    terminates;
}
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect("public trait termination fixture should check");
    let handle = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "SchedulerHandle")
        .map(|data| data.symbol)
        .expect("scheduler handle data symbol");
    let mut supplied = checked.clone();
    supplied
        .typed
        .tables
        .trait_machine_signatures
        .for_each_mut(|_, signature| {
            let language_semantics::TerminationGuarantee::Terminates { premises } =
                &mut signature.termination_guarantee
            else {
                return;
            };
            for premise in premises {
                premise.subject.root = handle;
            }
        });
    let review = project_checked_package_review(&supplied)
        .expect("a premise rooted at a public declaration must certify");
    let runtime = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "SchedulerRuntime")
        .expect("scheduler runtime trait row");
    let wait = runtime
        .requirements()
        .iter()
        .find(|requirement| {
            requirement
                .identity()
                .path()
                .contains("SchedulerRuntime::wait")
        })
        .expect("wait requirement row");
    let premises = wait
        .termination()
        .premises()
        .expect("wait must promise termination");
    let [premise] = premises else {
        panic!("one declaration-rooted premise")
    };
    assert_eq!(premise.profile().path(), "SchedulerHandle::WeakFair");
    assert_eq!(
        premise
            .subject()
            .declaration()
            .map(|identity| identity.path()),
        Some("SchedulerHandle"),
    );
}

#[test]
fn public_trait_termination_rejects_a_private_subject_root() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data SchedulerHandle { }
data PrivateHandle { }
pub domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
pub boundary trait SchedulerRuntime {
    machine wait(scheduler: SchedulerHandle)
    requires scheduler in WeakFair
    terminates;
}
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect("public trait termination fixture should check");
    let private = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "PrivateHandle")
        .map(|data| data.symbol)
        .expect("private handle data symbol");
    let mut supplied = checked.clone();
    supplied
        .typed
        .tables
        .trait_machine_signatures
        .for_each_mut(|_, signature| {
            let language_semantics::TerminationGuarantee::Terminates { premises } =
                &mut signature.termination_guarantee
            else {
                return;
            };
            for premise in premises {
                premise.subject.root = private;
            }
        });
    let diagnostics = project_checked_package_review(&supplied)
        .expect_err("a premise rooted at a private declaration must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rooted at neither a parameter nor a public declaration")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}
