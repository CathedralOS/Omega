use super::{Fixture, assert_status};

/// The target `accept` settles so a targetless `run` finds its resolved
/// profile already reviewed: the catalogued host profile on catalogued
/// hosts. Every supported development host owns one; a host outside the
/// catalog stops at the no-profile diagnostic before consulting package
/// acceptance, so the choice is unconstrained there and an exact catalogued
/// name is pinned for `update`.
fn probe_target_name() -> String {
    target::TargetProfile::host_if_supported()
        .map(|host| host.target_name().to_owned())
        .unwrap_or_else(|| "linux_x86_64".to_owned())
}

/// A catalogued host resolves the targetless `run` to its own profile, so
/// rejection legs observe the package check. A host outside the catalog
/// reports the missing host profile first: it earns the ordinary diagnostic
/// rather than the panic `TargetProfile::host` retains for callers that
/// intrinsically require one.
fn assert_probe_rejection(output: &std::process::Output, diagnostic: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    match target::TargetProfile::host_if_supported() {
        Some(_) => assert!(stderr.contains(diagnostic), "{stderr}"),
        None => {
            assert!(
                stderr.contains("no catalogued Omega deployment profile"),
                "{stderr}"
            );
            assert!(!stderr.contains("panic"), "{stderr}");
        }
    }
}

fn application() -> Fixture {
    let fixture = Fixture::new();
    fixture.write(
        "root/build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("probe_app");
    builder.depend_as("numbers", Source::Path { location: "../dependency" });
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(linux_arm64::ProgramEntry, Main::main);
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}
"#,
    );
    fixture.write("dependency/main.omg", "pub machine value() -> i32 { 7 }\n");
    fixture.write(
        "root/main.omg",
        "use numbers::main;\ndata Main {}\nmachine Main::main(&mut self) { let result: i32 = value(); }\n",
    );
    fixture
}

fn accept(fixture: &Fixture, target: &str) {
    let output = fixture.omega(&["update", "--target", target, "--offline"]);
    if output.status.success() {
        return;
    }
    assert_status(&output, 3);
    for path in fixture.review_paths(&output) {
        let document = std::fs::read_to_string(&path).unwrap();
        let accepted = document
            .lines()
            .map(|line| {
                if line.starts_with("decision ") {
                    format!("{} accept\n", line.strip_suffix(" pending").unwrap())
                } else {
                    format!("{line}\n")
                }
            })
            .collect::<String>();
        std::fs::write(path, accepted).unwrap();
    }
    assert_status(&fixture.omega(&["update", "--resume", "--offline"]), 0);
}

#[test]
fn native_probe_resolves_package_aliases_for_both_engines() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        eprintln!("skipping: this leg executes a host artifact");
        return;
    };
    let fixture = application();
    accept(&fixture, host.target_name());
    let before = fixture.accepted_files();
    let output = fixture.omega(&["run", "--both", "main.omg"]);
    assert_status(&output, 0);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("native exit 0"), "{stderr}");
    assert!(stderr.contains("interp exit: 0"), "{stderr}");
    assert_eq!(fixture.accepted_files(), before);
}

#[test]
fn native_probe_requires_ordinary_package_acceptance() {
    let fixture = application();
    fixture.write(
        "dependency/main.omg",
        "pub machine value() -> i32 { 7 }\nboundary machine trusted_zero() -> u64 ensures result == 0;\n",
    );
    let before = fixture.accepted_files();
    let output = fixture.omega(&["run", "--both", "main.omg"]);
    assert_status(&output, 200);
    assert_probe_rejection(&output, "run omega update");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("native exit"), "{stderr}");
    assert!(!stderr.contains("interp exit"), "{stderr}");
    assert_eq!(fixture.accepted_files(), before);
}

#[test]
fn native_probe_explicit_target_only_compiles_the_package() {
    let fixture = application();
    accept(&fixture, "linux_x86_64");
    let output = fixture.omega(&["run", "--both", "--target", "linux_x86_64", "main.omg"]);
    assert_status(&output, 0);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("compiled for target `linux_x86_64` OK"),
        "{stderr}"
    );
    assert!(!stderr.contains("native exit"), "{stderr}");
    assert!(!stderr.contains("interp exit"), "{stderr}");
}

#[test]
fn native_probe_observes_generated_package_source_in_both_engines() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        eprintln!("skipping: this leg executes a host artifact");
        return;
    };
    let fixture = application();
    fixture.write(
        "dependency/main.omg",
        "// value is generated by build.omg.\n",
    );
    fixture.write(
        "dependency/build.omg",
        r#"machine build(builder: &mut Build) {
    builder.package("arithmetic_kernels");
    let generated: BuildPath = builder.output.resolve("value.generated.omg");
    let descriptor: i32 = builder.output.create(generated, 438);
    let count: i64 = builder.output.write(descriptor, "pub machine value() -> i32 { 7 }\n");
    let closed: i32 = builder.output.close(descriptor);
    builder.output.include_source(generated);
}
"#,
    );
    accept(&fixture, host.target_name());
    let before = fixture.accepted_files();
    let output = fixture.omega(&["run", "--both", "main.omg"]);
    assert_status(&output, 0);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("native exit 0"), "{stderr}");
    assert!(stderr.contains("interp exit: 0"), "{stderr}");
    assert!(!fixture.path("dependency/value.generated.omg").exists());
    assert_eq!(fixture.accepted_files(), before);
}

#[test]
fn native_probe_rejects_malformed_and_stale_authored_admissions() {
    let fixture = application();
    accept(&fixture, &probe_target_name());
    for (contents, diagnostic) in [
        ("not a trust receipt\n".to_owned(), "malformed"),
        (
            format!("{}  accepted fact: stale\n", "1".repeat(64)),
            "stale trust admission",
        ),
    ] {
        fixture.write("root/omega.admissions", &contents);
        let output = fixture.omega(&["run", "--both", "main.omg"]);
        assert_status(&output, 200);
        assert_probe_rejection(&output, diagnostic);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("native exit"), "{stderr}");
        assert_eq!(fixture.read("root/omega.admissions"), contents);
    }
}

#[test]
fn standalone_probe_preserves_focused_compilation_and_reads_admissions() {
    let fixture = Fixture::new();
    std::fs::remove_file(fixture.path("root/build.omg")).unwrap();
    fixture.write("root/main.omg", "machine main() {}\n");
    let ordinary = fixture.omega(&["--target", "linux_x86_64", "main.omg"]);
    assert_status(&ordinary, 1);
    let output = fixture.omega(&["run", "--target", "linux_x86_64", "main.omg"]);
    assert_status(&output, 200);
    let diagnostic = String::from_utf8_lossy(&ordinary.stderr);
    assert!(!diagnostic.trim().is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(diagnostic.trim()),
        "standalone native admission changed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fixture.write("root/omega.admissions", "not a trust receipt\n");
    let output = fixture.omega(&["run", "main.omg"]);
    assert_status(&output, 200);
    assert_probe_rejection(&output, "malformed");
    assert!(!fixture.path("root/omega.lock").exists());
}

#[test]
fn native_probe_keeps_transitive_package_bindings_and_retained_output() {
    let fixture = application();
    std::fs::create_dir(fixture.path("dependency/leaf")).unwrap();
    fixture.write(
        "dependency/leaf/build.omg",
        "machine build(builder: &mut Build) { builder.package(\"leaf\"); }\n",
    );
    fixture.write(
        "dependency/leaf/main.omg",
        "pub machine leaf_value() -> i32 { 43 }\n",
    );
    fixture.write("dependency/build.omg", "machine build(builder: &mut Build) { builder.package(\"arithmetic_kernels\"); builder.depend(Source::Path { location: \"leaf\" }); }\n");
    fixture.write(
        "dependency/main.omg",
        "use leaf::main;\npub machine value() -> i32 { leaf_value() }\n",
    );
    accept(&fixture, "linux_x86_64");
    let before = fixture.accepted_files();
    let output = fixture.omega(&["run", "--keep", "--target", "linux_x86_64", "main.omg"]);
    assert_status(&output, 0);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let directory = stderr
        .lines()
        .find_map(|line| {
            line.strip_prefix("compiled for target `linux_x86_64` OK (")
                .and_then(|line| line.strip_suffix(')'))
        })
        .expect("retained artifact location");
    let directory = std::path::Path::new(directory);
    assert!(directory.join("omega-program").is_file());
    assert!(directory.starts_with(std::env::temp_dir()));
    assert!(
        directory
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("omega-probe-")
    );
    std::fs::remove_dir_all(directory).unwrap();
    assert_eq!(fixture.accepted_files(), before);
}

#[test]
fn native_probe_rejects_changed_dependency_without_refreshing_acceptance() {
    let fixture = application();
    accept(&fixture, "linux_x86_64");
    fixture.write("dependency/main.omg", "pub machine value() -> i32 { 8 }\n");
    let before = fixture.accepted_files();
    let output = fixture.omega(&["run", "--target", "linux_x86_64", "main.omg"]);
    assert_status(&output, 200);
    assert!(String::from_utf8_lossy(&output.stderr).contains("omega update"));
    assert_eq!(fixture.accepted_files(), before);
}
