//! A provider returns a reference-carrying value with a destructor. Its
//! shared permission survives transport to the exact cleanup action.

#[path = "support/project.rs"]
mod project;

const SOURCE: &str = r#"public data Cell { word: u32; }
public data View<'a> dropped by end { cell: @'a Cell; }
machine end<'a>(view: View<'a>) { let { cell } = move view; }

public boundary trait Loan {
    machine lend<'a>(cell: @'a Cell) -> View<'a> reaches none crashes none;
}

machine work(first: @Cell, second: @Cell) reaches Loan crashes none {
    let outer: View = Loan::lend(first);
    let inner: View = Loan::lend(second);
    let kept: View = move outer;
}"#;

#[test]
fn automatic_cleanup_observes_its_provider_permission_and_retains_its_input_loan() {
    let build = r#"machine build(build: @mutable BuildPlan) {
        build.product = BuildProduct::Package { name: "cells" };
    }"#;
    let scratch = project::stage(
        "provider-result-cleanup",
        &[("main.omg", SOURCE), ("build.omg", build)],
    );
    let (ok, report) = project::status(&project::omega(
        scratch.path(),
        &[
            "psi",
            "inspect",
            "--machine",
            "work",
            "--output",
            "result.psi",
            "build.omg",
        ],
    ));
    assert!(ok, "{report}");
    let bytes = std::fs::read(scratch.path().join("result.psi")).unwrap();
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
    let module = artifact.semantic_module();
    let Some(terminal_psi::TargetMachineCallable::Machine(entry)) = module.entry else {
        panic!("missing checked entry");
    };
    let body = module.machine(entry).unwrap();
    assert_eq!(body.boundary_invocations().count(), 2);
    let cleanups = body
        .module_invocations()
        .filter(|call| {
            matches!(
                call.site,
                semantic_vocabulary::InvocationSite::Cleanup { .. }
            )
        })
        .count();
    assert_eq!(cleanups, 2);

    let bad = SOURCE
        .replace("first: @Cell", "first: @mutable Cell")
        .replace("let inner: View = Loan::lend(second);", "first.word = 7;");
    for (program, expected) in [
        (bad.clone(), false),
        (bad.replace("let kept: View = move outer;", ""), true),
    ] {
        let (ok, report) = project::check(
            "provider-cleanup-input-debt",
            &[("main.omg", &program), ("build.omg", build)],
            &["--check", "main.omg"],
        );
        assert_eq!(ok, expected, "{report}");
        if !expected {
            assert!(
                report.contains("storage is changed while a conflicting reference remains live"),
                "{report}"
            );
        }
    }
}
