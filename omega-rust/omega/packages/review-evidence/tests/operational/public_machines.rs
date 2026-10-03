use crate::support::*;
use compiler::CheckedCompileRequest;

#[test]
fn borrowed_bindings_retain_exact_invocation_parameter_in_review() {
    for borrow in ["&", "&mut "] {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"use omega::language::core::binding;
pub boundary trait Host {{ machine ping() reaches Host; }}
pub machine dispatch(first: {borrow}Binding<Host>, second: {borrow}Binding<Host>)
reaches Host
invokes second;
{{ second.ping(); }}
"#,
            ),
        );
        package.write(
            "build.omg",
            "machine build(builder: &mut Build) { Build::package(builder, \"review_fixture\"); }\n",
        );
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
        })
        .expect("borrowed binding invocation checks");
        let review = project_checked_package_review(&checked)
            .expect("review resolves the borrowed binding's exact service");
        let dispatch = review
            .callables()
            .iter()
            .find(|callable| callable.role() == PackageReviewCallableRole::Public)
            .expect("public dispatch row");
        let expected = [PackageReviewSynchronousInvocation::Parameter(1)];
        assert_eq!(
            dispatch.declared_synchronous_invocations(),
            Some(&expected[..])
        );
        assert_eq!(dispatch.realized_synchronous_invocations(), &expected);
    }
}

#[test]
fn public_machine_cannot_hide_realized_reach_invocation_or_operational_effects() {
    let cases = [
        (
            "invocation",
            r#"use omega::language::core::binding;
pub boundary trait Handler { machine handle(); }
pub machine public_api(handler: &mut Binding<Handler>) { handler.handle(); }
"#,
            &["omits `invokes handler;`"][..],
        ),
        (
            "operational",
            r#"use omega::language::core::binding;
pub boundary trait Waiting { machine wait() reaches Waiting suspends; blocks; }
pub machine public_api(waiting: &mut Binding<Waiting>)
reaches Waiting
invokes waiting;
{
    suspend block waiting.wait();
}
"#,
            &["omits `suspends;`", "omits `blocks;`"][..],
        ),
        (
            "crash",
            r#"pub machine public_api() { crash Abort; }
"#,
            &["crash"][..],
        ),
    ];

    for (label, source, expected_messages) in cases {
        let package = TempPackage::new();
        package.write("main.omg", source);
        let diagnostics = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), None)
        })
        .unwrap_err();
        for expected in expected_messages {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "{label} omission should mention `{expected}`: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn exact_synchronous_invocations_change_comparison_encoding() {
    let quiet = TempPackage::new();
    let invoking = TempPackage::new();
    quiet.write(
        "main.omg",
        r#"use omega::language::core::binding;
pub boundary trait Handler { machine handle(); }
pub boundary trait Host { machine ping() reaches Host; }
pub machine dispatch(handler: &mut Binding<Handler>)
reaches Host, Handler
invokes handler;
invokes Host;
{ }
"#,
    );
    invoking.write(
        "main.omg",
        r#"use omega::language::core::binding;
pub boundary trait Handler { machine handle(); }
pub boundary trait Host { machine ping() reaches Host; }
pub machine dispatch(handler: &mut Binding<Handler>)
reaches Host, Handler
invokes handler;
invokes Host;
{
    handler.handle();
    Host::ping();
}
"#,
    );
    let build = r#"machine build(builder: &mut Build) { Build::package(builder, "review_fixture"); }
"#;
    quiet.write("build.omg", build);
    invoking.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
        })
        .expect("invocation comparison fixture should check")
    };
    let quiet = project_checked_package_review(&compile(&quiet)).expect("quiet review");
    let invoking = project_checked_package_review(&compile(&invoking)).expect("invoking review");
    let dispatch = invoking
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("public dispatch row");
    let quiet_dispatch = quiet
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("quiet public dispatch row");
    let declared = dispatch
        .declared_synchronous_invocations()
        .expect("published invocation ceiling");
    assert_eq!(declared.len(), 2);
    assert_eq!(
        declared[0],
        PackageReviewSynchronousInvocation::Parameter(0)
    );
    let PackageReviewSynchronousInvocation::Service(service) = &declared[1] else {
        panic!("second exact invocation should be a service identity")
    };
    assert_eq!(service.path(), "Host");
    assert_eq!(
        service.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(
        quiet_dispatch.declared_synchronous_invocations(),
        Some(declared)
    );
    assert!(quiet_dispatch.realized_synchronous_invocations().is_empty());
    assert_eq!(quiet_dispatch.contracts(), dispatch.contracts());
    assert_eq!(dispatch.realized_synchronous_invocations(), declared);
    assert_ne!(
        quiet.canonical_review_bytes().expect("quiet encoding"),
        invoking
            .canonical_review_bytes()
            .expect("invoking encoding")
    );
}
