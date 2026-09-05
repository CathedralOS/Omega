use super::fixture::{Fixture, assert_status};
use package_manager::lock::HistoricalPackagePolicyDecisionSubject;
use package_manager::resolution::graph::CanonicalDependencySourceRequest;
use package_manager::review::ReviewOnlyRootPolicyDisposition;
use package_source::ImmutableSourceResolution;
use std::fs;
use std::process::Output;

const TARGET: target::TargetProfile = target::TargetProfile::LinuxX64;
const HOST_REPOSITORY: &str = "git@github.com:CathedralOS/host-services.git";
const HOST: &str = "25c18b37f4891aa31b83e1434562fb2ab0994450";
const PROCESS: &str = "15beea1a49aecce2362e4700e791a46d48bab598";
const FILE_BASELINE: &str = "3f1e20615b1226aef011b5cfe651a179daca59ad";
const FILE_CANDIDATE: &str = "ae37f95cf856d85c05fd4f113a0d32fe6f7229fa";

#[test]
#[ignore = "requires network and private CathedralOS process-exit/host-services access over SSH"]
fn pinned_ssh_process_install_requires_exact_authority_decisions() {
    install_authority("process-exit", PROCESS, "Console", "Process");
}

#[test]
#[ignore = "requires network and private CathedralOS file-journal/host-services access over SSH"]
fn pinned_ssh_filesystem_install_and_retained_authority_update() {
    assert_recorded_pin(FILE_CANDIDATE);
    assert_ne!(FILE_BASELINE, FILE_CANDIDATE);
    let fixture = install_authority(
        "file-journal",
        FILE_BASELINE,
        "FilesystemHost",
        "Filesystem",
    );
    let before = fixture.accepted_files();
    let baseline = fixture.lock();
    let output = fixture.omega(&[
        "update",
        "file_journal",
        "--to",
        FILE_CANDIDATE,
        "--target",
        "linux_x86_64",
    ]);
    assert_status(&output, 0);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Published build.omg and omega.lock"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Audit recommended: file-journal"),
        "{stdout}"
    );
    assert!(stdout.contains("Source diff: file-journal"), "{stdout}");
    let (_, document) = review(&fixture, &output);
    let section = package_section(&document, "file-journal");
    assert!(section.contains("source-changed true\n"), "{section}");
    assert!(section.contains("audit-recommended true\n"), "{section}");
    assert!(!section.contains("\nchange "), "{section}");
    assert!(
        !document.lines().any(|line| line.starts_with("decision ")),
        "implementation-only update asked for policy acceptance: {document}"
    );
    assert_locked_authority(
        &fixture,
        "file-journal",
        FILE_CANDIDATE,
        "FilesystemHost",
        "Filesystem",
    );
    let candidate = fixture.lock();
    let old_target = baseline.target(TARGET).unwrap();
    let new_target = candidate.target(TARGET).unwrap();
    assert_eq!(
        new_target.baselines(),
        old_target.baselines(),
        "normalized policy changed"
    );
    assert_eq!(
        new_target
            .source()
            .packages()
            .iter()
            .map(|package| package.key())
            .collect::<Vec<_>>(),
        old_target
            .source()
            .packages()
            .iter()
            .map(|package| package.key())
            .collect::<Vec<_>>(),
        "implementation update changed package identities"
    );
    assert_ne!(fixture.accepted_files().0, before.0);
    assert_ne!(fixture.accepted_files().1, before.1);
    let source_diff = fixture.read("root/build/package-manager/source-diff.txt");
    let source_diff = source_diff
        .split("OMEGA_PACKAGE_SOURCE_PATCH_V1\n")
        .find(|section| section.starts_with("mode update\npackage file-journal\n"))
        .expect("source diff must identify the file-journal package");
    assert!(
        source_diff.contains(&format!("baseline_git_commit {FILE_BASELINE}\n")),
        "{source_diff}"
    );
    assert!(
        source_diff.contains(&format!("candidate_git_commit {FILE_CANDIDATE}\n")),
        "{source_diff}"
    );
    assert!(source_diff.contains("changed_entries 1\n"), "{source_diff}");
    assert!(source_diff.contains("entry main.omg\n"), "{source_diff}");
    assert_eq!(
        source_diff
            .lines()
            .filter(|line| line.starts_with("removed ") || line.starts_with("added "))
            .collect::<Vec<_>>(),
        [
            "removed lf     let bytes_written: i64 = self.files.write(descriptor, line);",
            "removed lf     self.written = bytes_written;",
            "added lf     self.written = self.files.write(descriptor, line);",
        ],
        "the exact implementation-only before/after patch must be retained"
    );
    assert_no_proposal(&fixture);
    check_import(&fixture, "file-journal");

    let updated = fixture.accepted_files();
    let output = fixture.omega(&[
        "update",
        "file_journal",
        "--to",
        FILE_CANDIDATE,
        "--target",
        "linux_x86_64",
    ]);
    assert_status(&output, 0);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Published build.omg and omega.lock"));
    assert_eq!(fixture.accepted_files().0, updated.0);
    let (_, document) = review(&fixture, &output);
    assert!(
        !document
            .lines()
            .any(|line| line.starts_with("decision ") && line.ends_with(" pending")),
        "same-pin update left unresolved decisions: {document}"
    );
    assert_locked_authority(
        &fixture,
        "file-journal",
        FILE_CANDIDATE,
        "FilesystemHost",
        "Filesystem",
    );
    assert_eq!(
        fixture.lock().target(TARGET).unwrap().baselines(),
        new_target.baselines()
    );
    assert_no_proposal(&fixture);
    check_import(&fixture, "file-journal");
}

fn assert_recorded_pin(pin: &str) {
    assert!(
        pin.len() == 40 && pin.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "remote authority fixture pin is still pending: {pin}"
    );
    assert!(
        include_str!("../../../../tests/fixtures/packages/REMOTE_PINS.md").contains(pin),
        "only reviewed commits recorded in REMOTE_PINS.md may run: {pin}"
    );
}

fn install_authority(package: &str, pin: &str, service: &str, class: &str) -> Fixture {
    assert_recorded_pin(HOST);
    assert_recorded_pin(pin);
    let repository = format!("git@github.com:CathedralOS/{package}.git");
    let fixture = Fixture::new();
    let before = fixture.accepted_files();
    let output = fixture.omega(&[
        "install",
        &repository,
        "--rev",
        pin,
        "--target",
        "linux_x86_64",
    ]);
    assert_status(&output, 3);
    assert_eq!(fixture.accepted_files(), before);
    let (path, document) = review(&fixture, &output);
    assert!(document.contains("baseline none\n"), "{document}");
    let section = package_section(&document, package);
    assert!(section.contains("audit-recommended true\n"), "{section}");
    let consumer_rows = dangerous_decisions(section);
    assert!(
        consumer_rows.iter().any(|(row, _)| row.contains(service)),
        "{section}"
    );
    let dangerous = dangerous_decisions(&document);
    assert_eq!(
        dangerous.len(),
        1,
        "the consumer uses one dangerous service"
    );
    assert!(
        document
            .lines()
            .filter(|line| line.starts_with("decision "))
            .count()
            > 1,
        "individual authority acceptance must be distinct from the other policy choices"
    );
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
    assert_status(&fixture.omega(&["install", "--resume"]), 3);
    assert_eq!(fixture.accepted_files(), before);

    // Each dangerous row blocks independently even when every other row is accepted.
    for (_, decision) in dangerous {
        let accepted_decision = decision.replace(" pending", " accept");
        assert_eq!(
            accepted
                .lines()
                .filter(|line| *line == accepted_decision)
                .count(),
            1
        );
        for choice in ["pending", "reject"] {
            fs::write(
                &path,
                accepted.replace(
                    &accepted_decision,
                    &decision.replace(" pending", &format!(" {choice}")),
                ),
            )
            .unwrap();
            assert_status(&fixture.omega(&["install", "--resume"]), 3);
            assert_eq!(fixture.accepted_files(), before);
        }
        // Another accepted fingerprint cannot substitute for the missing row.
        let other = accepted
            .lines()
            .find(|line| line.starts_with("decision ") && *line != accepted_decision)
            .unwrap();
        fs::write(&path, accepted.replace(&accepted_decision, other)).unwrap();
        assert_status(&fixture.omega(&["install", "--resume"]), 1);
        assert_eq!(fixture.accepted_files(), before);
    }
    fs::write(path, accepted).unwrap();
    assert_status(&fixture.omega(&["install", "--resume"]), 0);
    assert_ne!(fixture.accepted_files().0, before.0);
    assert_ne!(fixture.accepted_files().1, before.1);
    assert_locked_authority(&fixture, package, pin, service, class);
    assert_eq!(
        fixture
            .lock()
            .target(TARGET)
            .unwrap()
            .decisions()
            .decisions()
            .len(),
        document
            .lines()
            .filter(|line| line.starts_with("decision "))
            .count()
    );
    let lock = fixture.lock();
    let resolution = lock.target(TARGET).unwrap().decisions();
    let mut locked_rows = resolution
        .decisions()
        .iter()
        .map(|decision| {
            assert_eq!(
                decision.disposition(),
                ReviewOnlyRootPolicyDisposition::AcceptCandidateChange
            );
            let HistoricalPackagePolicyDecisionSubject::Row(digest) = decision.subject() else {
                panic!("initial install must retain exact row decisions");
            };
            let fingerprint = digest
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            format!("decision row {fingerprint} pending")
        })
        .collect::<Vec<_>>();
    let mut reviewed_rows = document
        .lines()
        .filter(|line| line.starts_with("decision "))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    locked_rows.sort();
    reviewed_rows.sort();
    assert_eq!(
        locked_rows, reviewed_rows,
        "lock changed accepted row identities"
    );
    assert_no_proposal(&fixture);
    check_import(&fixture, package);
    fixture
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

fn dangerous_decisions(document: &str) -> Vec<(&str, &str)> {
    document
        .split("\nchange ")
        .filter(|row| row.starts_with("dangerous_capability added\n"))
        .map(|row| {
            let row = row.split("end-package").next().unwrap();
            let decision = row
                .lines()
                .find(|line| line.starts_with("decision ") && line.ends_with(" pending"))
                .unwrap_or_else(|| panic!("dangerous row lacks an exact pending decision: {row}"));
            (row, decision)
        })
        .collect()
}

fn assert_locked_authority(fixture: &Fixture, name: &str, pin: &str, service: &str, class: &str) {
    let lock = fixture.lock();
    assert_eq!(lock.targets().len(), 1);
    let target = lock.target(TARGET).unwrap();
    let source = target.source();
    assert_eq!(
        source.packages().len(),
        3,
        "project, consumer, and host must be locked"
    );
    let consumer = source
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == name)
        .unwrap();
    let host = source
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "host-services")
        .unwrap();
    for (package, expected) in [(consumer, pin), (host, HOST)] {
        let ImmutableSourceResolution::Git { commit, .. } = package.resolution() else {
            panic!(
                "{} must retain a Git resolution",
                package.key().name().as_str()
            );
        };
        assert_eq!(commit.to_hex(), expected);
    }
    let requests = source.dependency_requests();
    assert_eq!(requests.len(), 2);
    for (requester, selected, repository, revision) in [
        (
            source.root().selected().key(),
            consumer,
            format!("git@github.com:CathedralOS/{name}.git"),
            pin,
        ),
        (consumer.key(), host, HOST_REPOSITORY.to_owned(), HOST),
    ] {
        let edge = requests
            .iter()
            .find(|edge| edge.requester() == requester && edge.selected().key() == selected.key())
            .expect("lock must retain the root-to-consumer-to-host dependency path");
        assert_eq!(edge.selected().resolution(), selected.resolution());
        let CanonicalDependencySourceRequest::Git {
            repository: actual_repository,
            revision: actual_revision,
            ..
        } = edge.request()
        else {
            panic!("both dependency transports must be SSH Git requests");
        };
        assert_eq!(actual_repository, &repository);
        assert_eq!(actual_revision, revision);
    }
    let consumer_policy = target
        .baselines()
        .iter()
        .find(|baseline| baseline.package() == consumer.key().identity())
        .unwrap();
    let host_policy = target
        .baselines()
        .iter()
        .find(|baseline| baseline.package() == host.key().identity())
        .unwrap();
    let host_declaration = host_policy
        .public_traits()
        .iter()
        .find(|declaration| declaration.identity().path() == service)
        .expect("service declaration belongs to the exact locked host package");
    let callable = consumer_policy
        .callables()
        .callables()
        .iter()
        .find(|callable| {
            callable
                .declared_service_reach()
                .is_some_and(|reach| reach.contains(host_declaration.identity()))
        })
        .expect("installed API must retain its checked authority-bearing callable");
    assert_eq!(
        callable.declared_service_reach().unwrap(),
        &[host_declaration.identity().clone()]
    );
    assert_eq!(
        callable.checked_service_reach().realized().unwrap(),
        &[host_declaration.identity().clone()]
    );
    assert!(!callable.realized_synchronous_invocations().is_empty());
    // Declaring host services is not itself use of their authority. The consumer
    // retains the dangerous row, joined to the host's exact public declaration.
    assert!(host_policy.dangerous_capabilities().is_empty());
    assert_eq!(consumer_policy.dangerous_capabilities().len(), 1);
    {
        let authority = consumer_policy
            .dangerous_capabilities()
            .iter()
            .find(|authority| authority.service().path() == service)
            .expect("accepted policy must retain dangerous authority");
        assert_eq!(
            authority.service(),
            host_declaration.identity(),
            "authority must retain its package-qualified declaration"
        );
        assert_eq!(format!("{:?}", authority.class()), class);
    }
}

fn assert_no_proposal(fixture: &Fixture) {
    assert!(!fixture.path("root/build/package-manager/proposal").exists());
}

fn check_import(fixture: &Fixture, package: &str) {
    fixture.write(
        "root/main.omg",
        &format!(
            "use {}::main;\nmachine main() {{}}\n",
            package.replace('-', "_")
        ),
    );
    let accepted = fixture.accepted_files();
    assert_status(
        &fixture.omega(&["--check", "--target", "linux_x86_64", "main.omg"]),
        0,
    );
    assert_eq!(fixture.accepted_files(), accepted);
}
