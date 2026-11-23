//! A call into a dependency must run the body that dependency provides. The
//! consumer below names its dependency `kit` and calls `kit\value()`; the
//! test compiles it with the provider returning 7, and again with only that
//! literal changed to 9, and reads the saved Psi file: the consumer's call
//! must reach the provider's machine, and that machine must return the
//! provider's literal. A compiler that resolved the call to a stale or
//! local body would return the same number both times.

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::Path;
use std::process::{Command, Stdio};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{OperationKind, TerminalModule, Terminator};

const CONSUMER: &str = "use kit;\n\nmachine entry() -> i32 {\n    kit\\value()\n}\n";

const CONSUMER_BUILD: &str = "machine build(builder: @mutable Build) {
    Build::application(builder, \"consumer\");
    let mutable description: DependencyDescription = dependency\\from_source(Source::Path { location: \"dep\" });
    dependency\\with_alias(@mutable description, \"kit\");
    let registered: DependencyRegistration = Build::dependency(builder, description);
}
";

const PROVIDER_BUILD: &str =
    "machine build(builder: @mutable Build) {\n    Build::package(builder, \"kernel\");\n}\n";

/// Compile the consumer against a provider whose `value` returns `literal`,
/// and read back the Psi file the compiler saved.
fn saved_module(scratch: &Path, literal: i32) -> TerminalModule {
    let root = scratch.join(format!("returns-{literal}"));
    std::fs::create_dir_all(root.join("dep")).unwrap();
    std::fs::write(root.join("main.omg"), CONSUMER).unwrap();
    std::fs::write(root.join("build.omg"), CONSUMER_BUILD).unwrap();
    std::fs::write(
        root.join("dep").join("main.omg"),
        format!("public machine value() -> i32 {{\n    {literal}\n}}\n"),
    )
    .unwrap();
    std::fs::write(root.join("dep").join("build.omg"), PROVIDER_BUILD).unwrap();
    let artifact = root.join("consumer.psi");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("--emit-psi")
        .arg(&artifact)
        .arg("build.omg")
        .current_dir(&root)
        .stdin(Stdio::null())
        .output()
        .expect("omega runs");
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
