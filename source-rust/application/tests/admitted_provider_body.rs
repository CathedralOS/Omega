//! A call into a dependency must run the body that dependency provides. The
//! consumer below names its dependency `kit` and calls `|kit|::value()`; the
//! test compiles it with the provider returning 7, and again with only that
//! literal changed to 9, and reads the saved Psi file: the consumer's call
//! must reach the provider's machine, and that machine must return the
//! provider's literal. A compiler that resolved the call to a stale or
//! local body would return the same number both times. A dependency that is
//! missing, registered under another alias, renamed or retyped is refused in
//! the consumer, at the use that names what is missing.

#[path = "support/project.rs"]
mod project;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::Path;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{OperationKind, TerminalModule, Terminator};

const CONSUMER: &str =
    "use {kit}/main.omg as kit;\n\nmachine entry() -> i32 {\n    |kit|::value()\n}\n";

const CONSUMER_BUILD: &str = "use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: \"consumer\" };
    let mutable description: DependencyDescription = |dependency|::from_source(Source::Path { location: \"dep\" });
    |dependency|::with_alias(@mutable description, \"kit\");
    let registered: DependencyRegistration = Build::dependency(builder, description);
}
";

const PROVIDER_BUILD: &str = "machine build(builder: @mutable BuildPlan) {\n    builder.product = BuildProduct::Package { name: \"kernel\" };\n}\n";

/// Compile the consumer against a provider whose `value` returns `literal`,
/// and read back the Psi file the compiler saved.
fn saved_module(scratch: &Path, literal: i32) -> TerminalModule {
    let root = scratch.join(format!("returns-{literal}"));
    let provider = format!("public machine value() -> i32 {{\n    {literal}\n}}\n");
    project::write(
        &root,
        &[
            ("main.omg", CONSUMER),
            ("build.omg", CONSUMER_BUILD),
            ("dep/main.omg", &provider),
            ("dep/build.omg", PROVIDER_BUILD),
        ],
    );
    let artifact = root.join("consumer.psi");
    let output = project::omega(
        &root,
        &["--emit-psi", artifact.to_str().unwrap(), "build.omg"],
    );
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = std::fs::read(&artifact).unwrap();
    CanonicalTerminalArtifact::from_bytes(&bytes)
        .expect("the saved file reopens")
        .semantic_module()
        .clone()
}

/// The machine `entry` calls and the literal that machine returns. The saved
/// product holds exactly the consumer's `entry` and the body it calls.
fn called_literal(module: &TerminalModule) -> i128 {
    let callers: Vec<_> = module
        .machines
        .iter()
        .filter(|machine| {
            machine
                .operations()
                .any(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        })
        .collect();
    let [entry] = callers[..] else {
        panic!("one machine calls another: {:?}", module.machines)
    };
    assert_eq!(module.machines.len(), 2, "{:?}", module.machines);
    let [operation] = entry.operations().collect::<Vec<_>>()[..] else {
        panic!("`entry` is one call")
    };
    let OperationKind::Call { callee, .. } = operation.kind else {
        unreachable!()
    };
    assert!(entry.blocks.iter().any(|block| matches!(
        block.terminator,
        Terminator::Return { value, .. } if Some(value) == operation.result.scalar().map(|result| result.id)
    )));
    let body = module
        .machines
        .iter()
        .find(|machine| machine.id == callee && machine.id != entry.id)
        .expect("the saved file holds the called body");
    let [operation] = body.operations().collect::<Vec<_>>()[..] else {
        panic!("the provider's `value` is one literal")
    };
    let OperationKind::IntegerConstant { value } = operation.kind else {
        panic!("{operation:?}")
    };
    assert!(body.blocks.iter().any(|block| matches!(
        block.terminator,
        Terminator::Return { value, .. } if Some(value) == operation.result.scalar().map(|result| result.id)
    )));
    let semantic_vocabulary::IntegerValue::Signed(value) = value else {
        panic!("an `i32` literal is signed: {value:?}")
    };
    value
}

#[test]
fn a_dependency_call_runs_the_body_its_provider_supplies() {
    let scratch = TemporaryDirectory::create("admitted-provider-body", false).unwrap();
    // Only the provider's literal differs between the two compilations.
    assert_eq!(called_literal(&saved_module(scratch.path(), 7)), 7);
    assert_eq!(called_literal(&saved_module(scratch.path(), 9)), 9);
}

/// `omega --check` on the consumer staged with `build` as its Build and
/// `provider` as the dependency's `main.omg`: the exit status and report.
fn checked(scratch: &Path, name: &str, build: &str, provider: &str) -> (bool, String) {
    let root = scratch.join(name);
    // An executable application needs its selected ProgramEntry
    // (build/configuration.md), and `entry` returns a value, so the consumer
    // is checked as a package: every body is still checked, with no entry.
    let build = build.replace("BuildProduct::Application", "BuildProduct::Package");
    project::write(
        &root,
        &[
            ("main.omg", CONSUMER),
            ("build.omg", &build),
            ("dep/main.omg", provider),
            ("dep/build.omg", PROVIDER_BUILD),
        ],
    );
    let output = project::omega(&root, &["--check", "build.omg"]);
    (
        output.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

/// Each way the dependency can fail to supply `kit::value` is refused in the
/// consumer where it names what is missing: without the registration, or
/// with the dependency registered under another alias, `{kit}/main.omg` selects
/// nothing; with the provider's machine renamed the call names no
/// declaration; with its result changed to `bool` the consumer's return is
/// mistyped. The unchanged program checks.
#[test]
fn a_missing_or_changed_dependency_is_refused_in_the_consumer() {
    let scratch = TemporaryDirectory::create("dependency-refusal-placement", false).unwrap();
    let value = "public machine value() -> i32 {\n    7\n}\n";
    let (ok, report) = checked(scratch.path(), "unchanged", CONSUMER_BUILD, value);
    assert!(ok, "{report}");

    let unregistered = "machine build(builder: @mutable BuildPlan) {\n    builder.product = BuildProduct::Application { name: \"consumer\" };\n}\n";
    let other_alias = CONSUMER_BUILD.replace("\"kit\"", "\"other\"");
    for (name, build) in [
        ("unregistered", unregistered),
        ("other-alias", other_alias.as_str()),
    ] {
        let (ok, report) = checked(scratch.path(), name, build, value);
        assert!(!ok, "{name}: {report}");
        assert!(
            report.contains("no direct product dependency named `kit` supplies `main.omg`"),
            "{name}: {report}"
        );
    }

    let (ok, report) = checked(
        scratch.path(),
        "renamed",
        CONSUMER_BUILD,
        "public machine different() -> i32 {\n    7\n}\n",
    );
    assert!(!ok, "{report}");
    assert!(
        report.contains("this call has no visible declaration in its source scope")
            && report.contains("main.omg:4:5"),
        "{report}"
    );

    let (ok, report) = checked(
        scratch.path(),
        "retyped",
        CONSUMER_BUILD,
        "public machine value() -> bool {\n    true\n}\n",
    );
    assert!(!ok, "{report}");
    assert!(
        report.contains(
            "this value has scalar carrier `bool` but the machine's result requires `i32`"
        ) && report.contains("main.omg:4:5"),
        "{report}"
    );
}
