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
        .join("../../tests/fixtures/run/ownership/incoming_record_reference_return");
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
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    for (tier, name) in [
        ("run", "owned_record_sequence"),
        ("run", "runtime_record_sequence"),
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
fn saved_console_provider_keeps_the_direct_mutable_byte_window() {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run/providers/default_console_provider_reached");
    let scratch = TemporaryDirectory::create("console-provider-inspection", false).unwrap();
    let artifact = scratch.path().join("console.psi");
    let published = inspect(&project, "entry_start", &artifact);
    assert!(published.status.success(), "{published:?}");

    let reopened = omega(&project)
        .args(["psi", "inspect", "--artifact"])
        .arg(&artifact)
        .output()
        .expect("reopen the saved Console artifact");
    assert!(reopened.status.success(), "{reopened:?}");
    let inspection = String::from_utf8(reopened.stdout).unwrap();
    assert!(
        inspection
            .lines()
            .any(|row| { row.starts_with("boundary ") && row.contains("Console::read_line") }),
        "the original mutable requirement is retained"
    );
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
        r#"machine build(builder: @mut Build) { Build::package(builder, "inspection_ambiguity"); }"#,
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

#[test]
fn reopened_program_runs_in_both_engines_after_the_source_is_gone() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = root.join("tests/fixtures/run/calls/scalar_contract_route_exit");
    let targets = [
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
        "windows_x86_64",
    ];
    let consumer = TemporaryDirectory::create("saved-psi-consumer", false).unwrap();
    let program = consumer.path().join("relocated.psi");
    let (manifest, issued) = {
        let producer = TemporaryDirectory::create("saved-psi-producer", false).unwrap();
        for file in ["main.omg", "build.omg"] {
            std::fs::copy(fixture.join(file), producer.path().join(file)).unwrap();
        }
        let result = omega(producer.path())
            .env("OMEGA_LIBRARY_ROOT", root.join("source/library"))
            .args(["--emit-psi", "saved/main.psi"])
            .args(targets.iter().flat_map(|name| ["--target", *name]))
            .arg("main.omg")
            .output()
            .unwrap();
        assert!(result.status.success(), "{result:?}");
        assert_eq!(
            std::fs::read_dir(producer.path().join("saved"))
                .unwrap()
                .count(),
            1
        );
        std::fs::copy(producer.path().join("saved/main.psi"), &program).unwrap();
        let stdout = String::from_utf8(result.stdout).unwrap();
        let manifests: Vec<_> = stdout
            .lines()
            .filter_map(|line| line.strip_prefix("Psi manifest "))
            .collect();
        assert_eq!(manifests.len(), targets.len(), "{stdout}");
        assert!(manifests.iter().all(|id| *id == manifests[0]), "{stdout}");
        let file = std::fs::read(producer.path().join("saved/main.psi")).unwrap();
        let published = CanonicalTerminalArtifact::from_bytes(&file).unwrap();
        assert_eq!(published.manifest().identity().to_string(), manifests[0]);
        (manifests[0].to_owned(), published)
    };
    assert_eq!(std::fs::read_dir(consumer.path()).unwrap().count(), 1);
    let bytes = std::fs::read(&program).unwrap();
    let reopened = CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
    let metadata = reopened
        .publication_metadata()
        .expect("reopened entry publication");
    assert_eq!(
        metadata.machine_provenance(),
        issued.publication_metadata().unwrap().machine_provenance()
    );
    let module = reopened.semantic_module();
    let contract = module.program_entry_contract("linux_x86_64").unwrap();
    let schema = contract.schema_declaration;
    assert_eq!(
        module.identities.text(schema),
        Some("toolchain-contract::LinuxX86_64Application")
    );
    assert_eq!(
        metadata.entry_schema_custody(schema).unwrap(),
        semantic_vocabulary::compiler_evaluation::DeclarationCustody::ToolchainContract
    );
    let entry_declaration = module
        .machine(contract.entry)
        .unwrap()
        .declaration
        .as_ref()
        .unwrap()
        .semantic_declaration;
    assert_ne!(entry_declaration, schema);
    assert_eq!(
        metadata
            .entry_schema_custody(entry_declaration)
            .unwrap_err(),
        "selected declaration has no authenticated source custody"
    );

    // Equal saved bytes can be reopened in two independent invocations. One's
    // calling-custody metadata cannot issue observations for the other's view.
    assert_eq!(issued.manifest().identity(), reopened.manifest().identity());
    assert_eq!(
        reopened
            .calling_declaration_owner(issued.view().into(), schema)
            .unwrap_err(),
        "source declaration custody belongs to another program or invocation"
    );
    let command = |args: &[&str]| {
        omega(consumer.path())
            .env("OMEGA_LIBRARY_ROOT", consumer.path().join("absent-library"))
            .args(args)
            .arg(&program)
            .output()
            .unwrap()
    };
    let inspected = command(&["psi", "inspect", "--artifact"]);
    assert!(inspected.status.success(), "{inspected:?}");
    let withheld = command(&["run", "--both"]);
    assert!(!withheld.status.success());
    assert!(String::from_utf8_lossy(&withheld.stderr).contains("requires --trusted-artifact"));
    let mut wrong = manifest.clone();
    wrong.replace_range(..1, if manifest.starts_with('0') { "1" } else { "0" });
    let refused = command(&["run", "--both", "--trusted-artifact", &wrong]);
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("differs from the artifact authorized")
    );
    for target in targets {
        let compiled = command(&[
            "--trusted-artifact",
            &manifest,
            "--target",
            target,
            "--build-dir",
            target,
        ]);
        assert!(compiled.status.success(), "{target}: {compiled:?}");
        let executable = if target == "windows_x86_64" {
            "omega-program.exe"
        } else {
            "omega-program"
        };
        assert!(consumer.path().join(target).join(executable).is_file());
    }
    let observed = command(&["run", "--both", "--trusted-artifact", &manifest]);
    assert!(observed.status.success(), "{observed:?}");
    assert!(observed.stdout.is_empty(), "{observed:?}");
    assert!(
        String::from_utf8_lossy(&observed.stderr).contains("interp exit: 0"),
        "{observed:?}"
    );
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
#[test]
fn reopened_program_uses_the_host_pinned_package_lock() {
    use sha2::{Digest, Sha256};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = root.join("tests/fixtures/run/providers/native_boundary_supply");
    let consumer = TemporaryDirectory::create("psi-lock-consumer", false).unwrap();
    let host = TemporaryDirectory::create("psi-lock-host", false).unwrap();
    let lock = host.path().join("omega.lock");
    let program = consumer.path().join("pid.psi");
    let manifest = {
        let producer = TemporaryDirectory::create("psi-lock-producer", false).unwrap();
        for file in ["build.omg", "main.omg"] {
            std::fs::copy(fixture.join(file), producer.path().join(file)).unwrap();
        }
        let invoke = |args: &[&str]| {
            omega(producer.path())
                .env("OMEGA_LIBRARY_ROOT", root.join("source/library"))
                .args(args)
                .output()
                .unwrap()
        };
        let issued = invoke(&[
            "--emit-psi",
            program.to_str().unwrap(),
            "--target",
            "windows_x86_64",
            "--build-input",
            "build.omg",
            "--build-input",
            "main.omg",
            "main.omg",
        ]);
        assert!(issued.status.success(), "{issued:?}");
        let output = String::from_utf8(issued.stdout).unwrap();
        let manifest = output
            .lines()
            .find_map(|line| line.strip_prefix("Psi manifest "))
            .unwrap()
            .to_owned();
        // Only this disposable application supplies the test host's _getpid binding.
        let review = invoke(&[
            "packages",
            "update",
            "--project",
            ".",
            "--target",
            "windows_x86_64",
            "--offline",
            "--build-input",
            "build.omg",
            "--build-input",
            "main.omg",
            "--dangerously-accept-all",
        ]);
        assert!(review.status.success(), "{review:?}");
        std::fs::copy(producer.path().join("omega.lock"), &lock).unwrap();
        manifest
    };
    assert_eq!(std::fs::read_dir(consumer.path()).unwrap().count(), 1);
    let bytes = std::fs::read(&lock).unwrap();
    assert!(
        std::str::from_utf8(&bytes)
            .unwrap()
            .contains("\nnative_bindings_v1 ")
    );
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let receive = |args: &[&str], pin: Option<&str>| {
        let mut command = omega(consumer.path());
        command
            .env_remove("OMEGA_HOST_PACKAGE_LOCK")
            .env_remove("OMEGA_HOST_PACKAGE_LOCK_SHA256")
            .env("OMEGA_LIBRARY_ROOT", consumer.path().join("absent-library"));
        if let Some(pin) = pin {
            command
                .env("OMEGA_HOST_PACKAGE_LOCK", &lock)
                .env("OMEGA_HOST_PACKAGE_LOCK_SHA256", pin);
        }
        command
            .args(args)
            .args(["--trusted-artifact", &manifest])
            .arg(&program)
            .output()
            .unwrap()
    };
    let refused = receive(&["--build-dir", "uninstalled"], None);
    assert!(!refused.status.success(), "{refused:?}");
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("HostProcessId"),
        "{refused:?}"
    );
    assert!(
        !consumer
            .path()
            .join("uninstalled/omega-program.exe")
            .exists()
    );
    let wrong = receive(&["--build-dir", "substituted"], Some(&"0".repeat(64)));
    assert!(!wrong.status.success(), "{wrong:?}");
    assert!(
        String::from_utf8_lossy(&wrong.stderr).contains("approved SHA-256"),
        "{wrong:?}"
    );
    assert!(
        !consumer
            .path()
            .join("substituted/omega-program.exe")
            .exists()
    );
    let compiled = receive(&["--build-dir", "installed"], Some(&digest));
    assert!(compiled.status.success(), "{compiled:?}");
    let status = Command::new(consumer.path().join("installed/omega-program.exe"))
        .stdin(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(0));
    let compared = receive(&["run", "--both"], Some(&digest));
    assert!(compared.status.success(), "{compared:?}");
    assert!(
        String::from_utf8_lossy(&compared.stderr).contains("interp exit: 0"),
        "{compared:?}"
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn authored_getpid_build_runs_and_reopens_its_published_psi() {
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../source/library");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run/providers/native_boundary_supply");
    let scratch = TemporaryDirectory::create("authored-getpid", false).unwrap();
    for file in ["build.omg", "main.omg"] {
        std::fs::copy(fixture.join(file), scratch.path().join(file)).unwrap();
    }
    let build = |flags: &[&str]| {
        let output = omega(scratch.path())
            .env("OMEGA_LIBRARY_ROOT", &library)
            .args(flags)
            .args([
                "--target", "linux_x86_64", "--build-input", "build.omg",
                "--build-input", "main.omg", "main.omg",
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).unwrap()
    };
    let stdout = build(&["--build-dir", "native"]);
    let executable = stdout.lines()
        .find_map(|line| line.strip_prefix("published native output to "))
        .unwrap_or_else(|| panic!("build did not report a native executable: {stdout}"));
    // The fixture traps unless the selected syscall returns a positive process ID.
    let guest = Command::new(scratch.path().join(executable))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(guest.status.code(), Some(0), "{guest:?}");

    build(&["--emit-psi", "getpid.psi"]);
    let reopened = omega(scratch.path())
        .args(["psi", "inspect", "--artifact", "getpid.psi"])
        .output()
        .unwrap();
    assert!(reopened.status.success(), "{reopened:?}");
    let stdout = String::from_utf8(reopened.stdout).unwrap();
    assert!(
        stdout.lines().any(|line| line.starts_with("boundary ") && line.contains("HostProcessId::get")),
        "{stdout}"
    );
}
