use super::fixture::{Fixture, assert_status};
use std::fs;
use std::path::Path;
use std::process::Output;

#[path = "authority/missing_baseline.rs"]
mod missing_baseline;

const TARGET: target::TargetProfile = target::TargetProfile::LinuxX64;
const FILE_BUILD: &str = include_str!("../../../../tests/fixtures/packages/file-journal/build.omg");
const FILE_SOURCE: &str = include_str!("../../../../tests/fixtures/packages/file-journal/main.omg");
const PROCESS_BUILD: &str =
    include_str!("../../../../tests/fixtures/packages/process-exit/build.omg");
const PROCESS_SOURCE: &str =
    include_str!("../../../../tests/fixtures/packages/process-exit/main.omg");

pub(super) fn authority_fixture(build: &str, source: &str) -> Fixture {
    let fixture = Fixture::new();
    fixture.write("dependency/build.omg", build);
    fixture.write("dependency/main.omg", source);
    fs::create_dir(fixture.path("host-services")).unwrap();
    for (name, contents) in [
        (
            "build.omg",
            include_str!("../../../../tests/fixtures/packages/host-services/build.omg"),
        ),
        (
            "main.omg",
            include_str!("../../../../tests/fixtures/packages/host-services/main.omg"),
        ),
        (
            "console.omg",
            include_str!("../../../../tests/fixtures/packages/host-services/console.omg"),
        ),
        (
            "filesystem_host.omg",
            include_str!("../../../../tests/fixtures/packages/host-services/filesystem_host.omg"),
        ),
    ] {
        fixture.write(&format!("host-services/{name}"), contents);
    }
    fixture
}

fn install(fixture: &Fixture) -> Output {
    fixture.omega(&["install", "../dependency", "--target", "linux_x86_64"])
}

fn review(fixture: &Fixture, output: &Output) -> (std::path::PathBuf, String) {
    let paths = fixture.review_paths(output);
    assert_eq!(paths.len(), 1, "one explicitly selected target");
    let document = fs::read_to_string(&paths[0]).unwrap();
    (paths[0].clone(), document)
}

fn package_section<'a>(document: &'a str, package: &str) -> &'a str {
    document
        .split("\npackage ")
        .find(|section| section.starts_with(&format!("\"{package}\" ")))
        .unwrap_or_else(|| panic!("missing package {package}: {document}"))
        .split("end-package")
        .next()
        .unwrap()
}

fn authority_decision<'a>(section: &'a str, change: &str, service: &str) -> &'a str {
    let row = section
        .split("\nchange ")
        .find(|row| {
            row.starts_with(&format!("dangerous_capability {change}\n")) && row.contains(service)
        })
        .unwrap_or_else(|| panic!("missing {change} {service} authority: {section}"));
    row.lines()
        .find(|line| line.starts_with("decision ") && line.ends_with(" pending"))
        .unwrap_or_else(|| panic!("authority row has no exact pending decision: {row}"))
}

fn accept_document(path: &Path, document: &str) {
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
    fs::write(path, accepted).unwrap();
}

fn accept_install(fixture: &Fixture) {
    let before = fixture.accepted_files();
    let output = install(fixture);
    assert_status(&output, 3);
    assert_eq!(fixture.accepted_files(), before);
    let (path, document) = review(fixture, &output);
    assert!(document.lines().any(|line| line.starts_with("decision ")));
    accept_document(&path, &document);
    assert_status(&fixture.omega(&["install", "--resume"]), 0);
    assert_ne!(fixture.accepted_files(), before);
    fixture.lock();
}

fn check_import(fixture: &Fixture, alias: &str) {
    fixture.write(
        "root/main.omg",
        &format!("use {alias}::main;\nmachine main() {{}}\n"),
    );
    let accepted = fixture.accepted_files();
    assert_status(
        &fixture.omega(&["--check", "--target", "linux_x86_64", "main.omg"]),
        0,
    );
    assert_eq!(fixture.accepted_files(), accepted);
}

#[test]
fn initial_process_authority_requires_each_exact_decision() {
    initial_authority(PROCESS_BUILD, PROCESS_SOURCE, "process-exit", "Console");
}

#[test]
fn initial_filesystem_authority_requires_each_exact_decision() {
    initial_authority(FILE_BUILD, FILE_SOURCE, "file-journal", "FilesystemHost");
}

fn initial_authority(build: &str, source: &str, package: &str, service: &str) {
    let fixture = authority_fixture(build, source);
    let before = fixture.accepted_files();
    let output = install(&fixture);
    assert_status(&output, 3);
    assert_eq!(fixture.accepted_files(), before);
    let (path, document) = review(&fixture, &output);
    assert!(document.contains("baseline none\n"), "{document}");
    let section = package_section(&document, package);
    assert!(section.contains("audit-recommended true\n"), "{section}");
    let decision = authority_decision(section, "added", service);
    assert!(
        document
            .lines()
            .filter(|line| line.starts_with("decision "))
            .count()
            > 1,
        "the fixture must distinguish individual choices from blanket acceptance: {document}"
    );
    assert_status(&fixture.omega(&["install", "--resume"]), 3);
    assert_eq!(fixture.accepted_files(), before);

    // Accept every other row; the exact dangerous row must still block.
    accept_document(&path, &document);
    let accepted = fs::read_to_string(&path).unwrap();
    let accepted_decision = decision.replace(" pending", " accept");
    fs::write(&path, accepted.replace(&accepted_decision, decision)).unwrap();
    assert_status(&fixture.omega(&["install", "--resume"]), 3);
    assert_eq!(fixture.accepted_files(), before);
    fs::write(
        &path,
        accepted.replace(&accepted_decision, &decision.replace(" pending", " reject")),
    )
    .unwrap();
    assert_status(&fixture.omega(&["install", "--resume"]), 3);
    assert_eq!(fixture.accepted_files(), before);
    fs::write(&path, accepted).unwrap();
    assert_status(&fixture.omega(&["install", "--resume"]), 0);
    let lock = fixture.lock();
    let target = lock.target(TARGET).unwrap();
    assert!(target.baselines().iter().any(|baseline| {
        baseline
            .dangerous_capabilities()
            .iter()
            .any(|authority| authority.service().path() == service)
    }));
    assert_eq!(
        target.decisions().decisions().len(),
        document
            .lines()
            .filter(|line| line.starts_with("decision "))
            .count()
    );
    check_import(&fixture, &package.replace('-', "_"));
}

#[test]
fn retained_filesystem_implementation_upgrade_recommends_audit_without_reapproval() {
    let fixture = authority_fixture(FILE_BUILD, FILE_SOURCE);
    accept_install(&fixture);
    let before = fixture.accepted_files();
    let accepted = fixture.lock();
    // Change the actual body while preserving its API, reach, and invocation.
    let revised = FILE_SOURCE.replace(
        "self.written = self.files.write(descriptor, line);",
        "let bytes_written: i64 = self.files.write(descriptor, line);\n    self.written = bytes_written;",
    );
    assert_ne!(revised, FILE_SOURCE);
    fixture.write("dependency/main.omg", &revised);
    let output = fixture.omega(&["update", "file_journal"]);
    assert_status(&output, 0);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Audit recommended: file-journal"));
    let (_, document) = review(&fixture, &output);
    let section = package_section(&document, "file-journal");
    assert!(section.contains("source-changed true\n"), "{section}");
    assert!(section.contains("audit-recommended true\n"), "{section}");
    assert!(!section.contains("\nchange "), "{section}");
    assert!(
        !document.lines().any(|line| line.starts_with("decision ")),
        "{document}"
    );
    let updated = fixture.lock();
    let old_target = accepted.target(TARGET).unwrap();
    let new_target = updated.target(TARGET).unwrap();
    assert_eq!(new_target.baselines(), old_target.baselines());
    let old_package = old_target
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "file-journal")
        .unwrap();
    let new_package = new_target
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "file-journal")
        .unwrap();
    assert_eq!(old_package.key(), new_package.key());
    assert_ne!(old_package.resolution(), new_package.resolution());
    assert_eq!(fixture.accepted_files().0, before.0);
    assert_ne!(fixture.accepted_files().1, before.1);
    assert!(!fixture.path("root/build/package-manager/proposal").exists());
    check_import(&fixture, "file_journal");
}

#[test]
fn added_and_removed_process_authority_require_exact_update_decisions() {
    let fixture = authority_fixture(FILE_BUILD, FILE_SOURCE);
    accept_install(&fixture);
    for (build, source, change) in [
        (
            PROCESS_BUILD.replace("process-exit", "file-journal"),
            format!("{FILE_SOURCE}\n{PROCESS_SOURCE}"),
            "added",
        ),
        (FILE_BUILD.to_owned(), FILE_SOURCE.to_owned(), "removed"),
    ] {
        let before = fixture.accepted_files();
        fixture.write("dependency/build.omg", &build);
        fixture.write("dependency/main.omg", &source);
        let output = fixture.omega(&["update", "file_journal"]);
        assert_status(&output, 3);
        assert_eq!(fixture.accepted_files(), before);
        let (path, document) = review(&fixture, &output);
        let decision = authority_decision(
            package_section(&document, "file-journal"),
            change,
            "Console",
        );
        accept_document(&path, &document);
        let accepted = fs::read_to_string(&path).unwrap();
        let accepted_decision = decision.replace(" pending", " accept");
        fs::write(&path, accepted.replace(&accepted_decision, decision)).unwrap();
        assert_status(&fixture.omega(&["update", "--resume"]), 3);
        assert_eq!(fixture.accepted_files(), before);
        // Duplicating another choice cannot stand in for this row's identity.
        let other = accepted
            .lines()
            .find(|line| line.starts_with("decision ") && *line != accepted_decision)
            .unwrap();
        fs::write(&path, accepted.replace(&accepted_decision, other)).unwrap();
        assert_status(&fixture.omega(&["update", "--resume"]), 1);
        assert_eq!(fixture.accepted_files(), before);
        fs::write(&path, accepted).unwrap();
        assert_status(&fixture.omega(&["update", "--resume"]), 0);
        assert_eq!(fixture.accepted_files().0, before.0);
        assert_ne!(fixture.accepted_files().1, before.1);
        fixture.lock();
    }
}

#[test]
fn false_filesystem_reach_ceiling_rejects_install_without_accepted_writes() {
    false_filesystem_ceiling(false);
}

#[test]
fn false_filesystem_reach_ceiling_rejects_update_without_accepted_writes() {
    false_filesystem_ceiling(true);
}

fn false_filesystem_ceiling(installed: bool) {
    let fixture = authority_fixture(FILE_BUILD, FILE_SOURCE);
    if installed {
        accept_install(&fixture);
    }
    let false_ceiling = format!(
        "use host_services::console;\n{}",
        // Authored invokes contributes to normalized reach. Omit it so the
        // checked filesystem call actually exceeds the Console-only ceiling.
        FILE_SOURCE
            .replace("reaches FilesystemHost", "reaches Console")
            .replace("invokes FilesystemHost;\n", "")
    );
    fixture.write("dependency/main.omg", &false_ceiling);
    let before = fixture.accepted_files();
    let output = if installed {
        fixture.omega(&["update", "file_journal"])
    } else {
        install(&fixture)
    };
    assert_eq!(fixture.accepted_files(), before);
    assert_status(&output, 1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("checked compilation failed for package `file-journal`"),
        "{stderr}"
    );
    assert!(!fixture.path("root/build/package-manager/proposal").exists());
}

#[test]
fn generated_process_authority_requires_review_before_install_publication() {
    generated_authority(PROCESS_SOURCE, "terminate", &["Console"]);
}

#[test]
fn generated_combined_authority_requires_both_service_decisions_before_install() {
    let source = r#"
use host_services::console;
use host_services::filesystem_host;
use omega::language::core::service;

pub machine write_and_exit(console: Service<Console> in Bound, files: FilesystemHost, descriptor: i32, line: &[u8])
reaches FilesystemHost + Console
invokes console;
invokes files;
{
    let bytes_written: i64 = files.write(descriptor, line);
    console.exit_process(0);
}
"#;
    generated_authority(source, "write_and_exit", &["FilesystemHost", "Console"]);
}

fn generated_authority(source: &str, callable: &str, services: &[&str]) {
    const GENERATED_BUILD: &str =
        include_str!("../../../../tests/fixtures/packages/generated-table/build.omg");
    let generated_source = source
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    let build = GENERATED_BUILD
        .replace("builder.package(\"generated-table\");", "builder.package(\"generated-table\");\n    builder.depend(Source::Path { location: \"../host-services\" });\n    builder.select_provider<Console, ConsoleNativeProvider>();")
        .replace("pub machine table_size() -> u64 {\\n    3\\n}\\n", &generated_source);
    assert!(build.contains(callable));
    // Imports belong to the pre-build source closure; generation adds the
    // callable using those retained declarations, not new dependency discovery.
    let fixture = authority_fixture(
        &build,
        "use host_services::console;\nuse host_services::filesystem_host;\nuse omega::language::core::service;\n",
    );
    fs::create_dir(fixture.path("dependency/inputs")).unwrap();
    fixture.write(
        "dependency/inputs/table.txt",
        include_str!("../../../../tests/fixtures/packages/generated-table/inputs/table.txt"),
    );
    let before = fixture.accepted_files();
    let output = install(&fixture);
    assert_eq!(fixture.accepted_files(), before);
    assert_status(&output, 3);
    let (path, document) = review(&fixture, &output);
    let section = package_section(&document, "generated-table");
    for service in services {
        authority_decision(section, "added", service);
    }
    assert!(section.contains(callable), "{section}");
    assert_status(&fixture.omega(&["install", "--resume"]), 3);
    assert_eq!(fixture.accepted_files(), before);
    accept_document(&path, &document);
    assert_status(&fixture.omega(&["install", "--resume"]), 0);
    fixture.lock();
    assert!(!fixture.path("root/table.generated.omg").exists());
    assert!(!fixture.path("dependency/table.generated.omg").exists());
    check_import(&fixture, "generated_table");
}
