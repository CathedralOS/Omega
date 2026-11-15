use platform_custody::temporary_directory::TemporaryDirectory;
use std::{
    path::Path,
    process::{Command, Output, Stdio},
};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::TargetMachineCallable;

fn omega(project: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_omega"));
    command.current_dir(project).stdin(Stdio::null());
    command
}

fn inspect(project: &Path, name: &str, artifact: &Path) -> Output {
    omega(project)
        .args(["psi", "inspect", "--machine", name, "--output"])
        .arg(artifact)
        .arg("main.omg")
        .output()
        .expect("run source inspection")
}

#[test]
fn named_inspection_publishes_the_requested_reference_machine() {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/run/ownership/incoming_record_reference_return");
    let scratch = TemporaryDirectory::create("named-psi-inspection", false).unwrap();
    let artifact = scratch.path().join("retained.psi");
    let output = inspect(&project, "retained", &artifact);
    assert!(output.status.success(), "{:?}", output);
    let bytes = std::fs::read(&artifact).expect("inspection publishes an artifact");
    let decoded = CanonicalTerminalArtifact::from_bytes(&bytes).expect("canonical Psi artifact");
    let module = decoded.semantic_module();
    let Some(TargetMachineCallable::Machine(entry)) = module.entry else {
        panic!("the artifact must have the requested entry");
    };
    let declared = module
        .machines
        .iter()
        .find(|machine| machine.id == entry)
        .and_then(|machine| machine.declaration.as_ref())
        .expect("entry has a source declaration");
    let identity = module.identities.text(declared.machine.identity()).unwrap();
    assert!(identity.ends_with("path(retained))"), "{identity}");

    let loaded = omega(&project)
        .args(["psi", "inspect", "--artifact"])
        .arg(&artifact)
        .output()
        .expect("run artifact inspection");
    assert!(loaded.status.success(), "{:?}", loaded);
    let source_output = String::from_utf8(output.stdout).unwrap();
    let artifact_output = String::from_utf8(loaded.stdout).unwrap();
    assert_eq!(source_output.lines().last(), artifact_output.lines().last());
}

#[test]
fn named_inspection_reopens_owned_record_sequences_stored_in_locals() {
    let scratch = TemporaryDirectory::create("named-sequence-inspection", false).unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/fixtures");
    for (tier, name) in [
        ("run", "owned_record_sequence"),
        ("pending", "runtime_record_sequence"),
    ] {
        let project = fixtures.join(tier).join("ownership").join(name);
        let artifact = scratch.path().join(format!("{name}.psi"));
        let source = inspect(&project, "main", &artifact);
        assert!(source.status.success(), "{name}: {source:?}");
        let loaded = omega(&project)
            .args(["psi", "inspect", "--artifact"])
            .arg(&artifact)
            .output()
            .expect("reopen the saved sequence artifact");
        assert!(loaded.status.success(), "{name}: {loaded:?}");
        let source = String::from_utf8(source.stdout).unwrap();
        let loaded = String::from_utf8(loaded.stdout).unwrap();
        assert_eq!(source.lines().last(), loaded.lines().last(), "{name}");
    }
}

#[test]
fn named_inspection_refuses_missing_or_ambiguous_names_before_writing() {
    let scratch = TemporaryDirectory::create("unnamed-psi-inspection", false).unwrap();
    let project = scratch.path();
    std::fs::write(project.join("main.omg"), "machine retained() -> i32 { 42 }").unwrap();
    std::fs::write(
        project.join("other.omg"),
        "machine retained() -> i32 { 17 }",
    )
    .unwrap();
    std::fs::write(
        project.join("build.omg"),
        r#"machine build(builder: &mut Build) { Build::package(builder, "inspection_ambiguity"); }"#,
    )
    .unwrap();
    let checked = omega(project)
        .args(["--check", "main.omg"])
        .output()
        .unwrap();
    assert!(checked.status.success(), "{:?}", checked);

    for (name, expected) in [
        ("missing", "no declaration has that source name"),
        ("retained", "the source name is ambiguous"),
    ] {
        let artifact = project.join("inspection.psi");
        let output = inspect(project, name, &artifact);
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(output.status.code(), Some(1), "{stderr}");
        assert!(
            stderr.contains(&format!("cannot inspect machine `{name}`: {expected}")),
            "{stderr}"
        );
        assert!(!artifact.exists());
    }
}
