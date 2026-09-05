use super::fixture::{Fixture, assert_status};
use package_manager::resolution::graph::CanonicalDependencySourceRequest;
use package_manager::review::ReviewOnlyRootPolicyDisposition;
use package_source::ImmutableSourceResolution;
use std::fs;
use std::process::Output;

const TARGET: target::TargetProfile = target::TargetProfile::LinuxX64;
const GRAPH: &str = "85e962bf5120f84d5f2d8c18c14a6d96d1ec5c64";
const AXIOM: &str = "8f5bd07f166bcad08842e6bab1ba8b031e3afcb9";
const OPAQUE: &str = "6a29daa655b2aaee0bdb8d85e4a651845e165896";
const ARITHMETIC: &str = "b65cc9b062f69ef02a586c82cd260d51bf28945c";
const FILE: &str = "ae37f95cf856d85c05fd4f113a0d32fe6f7229fa";
const HOST: &str = "25c18b37f4891aa31b83e1434562fb2ab0994450";

#[test]
#[ignore = "requires network and private CathedralOS graph-workbench closure access over SSH"]
fn pinned_ssh_graph_install_reviews_transitive_authority_and_audits_pins() {
    let fixture = Fixture::new();
    let output = install(&fixture, "graph-workbench", GRAPH, 3);
    let document = review_and_resume(
        &fixture,
        &output,
        "file-journal",
        "dangerous_capability",
        "FilesystemHost",
    );
    for package in [
        "graph-workbench",
        "arithmetic-kernels",
        "file-journal",
        "host-services",
    ] {
        package_section(&document, package);
    }
    assert_locked_graph(
        &fixture,
        &[
            ("cli-project", "graph-workbench", GRAPH),
            ("graph-workbench", "arithmetic-kernels", ARITHMETIC),
            ("graph-workbench", "file-journal", FILE),
            ("file-journal", "host-services", HOST),
        ],
    );
    let lock = fixture.lock();
    let target = lock.target(TARGET).unwrap();
    let policy = |name: &str| {
        let package = target
            .source()
            .packages()
            .iter()
            .find(|package| package.key().name().as_str() == name)
            .unwrap();
        target
            .baselines()
            .iter()
            .find(|baseline| baseline.package() == package.key().identity())
            .unwrap()
    };
    let host = policy("host-services");
    let filesystem = host
        .public_traits()
        .iter()
        .find(|declaration| declaration.identity().path() == "FilesystemHost")
        .unwrap();
    let journal = policy("file-journal");
    let [authority] = journal.dangerous_capabilities() else {
        panic!("file-journal must retain exactly one dangerous authority")
    };
    assert_eq!(authority.service(), filesystem.identity());
    assert_eq!(format!("{:?}", authority.class()), "Filesystem");
    assert!(journal.callables().callables().iter().any(|callable| {
        callable
            .checked_service_reach()
            .realized()
            .is_some_and(|reach| reach.contains(filesystem.identity()))
            && !callable.realized_synchronous_invocations().is_empty()
    }));
    for name in [
        "cli-project",
        "graph-workbench",
        "arithmetic-kernels",
        "host-services",
    ] {
        assert!(policy(name).dangerous_capabilities().is_empty(), "{name}");
    }

    let report = audit(&fixture, 0);
    for pin in [GRAPH, ARITHMETIC, FILE, HOST] {
        assert!(report.contains(pin), "missing pin {pin}: {report}");
    }
    let journal = fresh_package(&report, "file-journal");
    assert!(journal.contains("audit-recommended true"), "{journal}");
    assert!(journal.contains("FilesystemHost"), "{journal}");
    assert_dependency_path(journal, &["graph_workbench", "file_journal"]);
    assert_dependency_path(
        fresh_package(&report, "host-services"),
        &["graph_workbench", "file_journal", "host_services"],
    );
    check_import(
        &fixture,
        "graph-workbench",
        "machine main() -> u64 { run_graph_probe() }",
    );
}

#[test]
#[ignore = "requires network and private CathedralOS axiom-ledger access over SSH"]
fn pinned_ssh_initial_boundary_claim_requires_decisions_and_remains_in_audit() {
    let fixture = Fixture::new();
    let output = install(&fixture, "axiom-ledger", AXIOM, 3);
    let document = review_and_resume(
        &fixture,
        &output,
        "axiom-ledger",
        "callable",
        "trusted_zero",
    );
    let section = package_section(&document, "axiom-ledger");
    assert!(section.contains("trusted_zero"), "{section}");
    assert!(section.contains("is_zero"), "{section}");
    assert!(section.contains("audit-recommended true"), "{section}");
    assert_locked_graph(&fixture, &[("cli-project", "axiom-ledger", AXIOM)]);
    let lock = fixture.lock();
    let target = lock.target(TARGET).unwrap();
    let package = target
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "axiom-ledger")
        .unwrap();
    let policy = target
        .baselines()
        .iter()
        .find(|baseline| baseline.package() == package.key().identity())
        .unwrap();
    let [proposition] = policy.public_propositions() else {
        panic!("axiom-ledger must retain its public is_zero proposition")
    };
    assert_eq!(proposition.identity().path(), "is_zero");
    let callable = policy
        .callables()
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("trusted_zero"))
        .unwrap();
    // The fixture's boundary claim is private. The lock retains it as an
    // assumption even though it is not an exported boundary API.
    assert_eq!(format!("{:?}", callable.role()), "PrivateAssumption");
    assert_eq!(callable.identity().owner(), proposition.identity().owner());
    assert_eq!(format!("{:?}", callable.supply()), "AdmissionClaim");
    let [contract] = callable.contracts() else {
        panic!("trusted_zero must retain its one authored claim")
    };
    assert_eq!(format!("{:?}", contract.kind()), "Ensures");
    let claim = format!("{:?}", contract.fact());
    assert!(claim.starts_with("Proposition("), "{claim}");
    assert!(claim.contains("is_zero"), "{claim}");
    assert!(claim.contains("arguments: [Result]"), "{claim}");
    assert!(claim.contains("evidence: FactOnly"), "{claim}");
    assert!(callable.checked_service_reach().realized().is_none());
    assert!(policy.dangerous_capabilities().is_empty());

    let report = audit(&fixture, 0);
    assert!(report.contains(AXIOM), "{report}");
    let section = fresh_package(&report, "axiom-ledger");
    for expected in [
        "trusted_zero",
        "is_zero",
        "Ensures",
        "FactOnly",
        "role PrivateAssumption supply AdmissionClaim",
        "audit-recommended false",
    ] {
        assert!(section.contains(expected), "missing {expected}: {section}");
    }
    check_import(&fixture, "axiom-ledger", "machine main() {}");
}

#[test]
#[ignore = "requires network and private CathedralOS opaque-carrier access over SSH"]
fn pinned_ssh_claim_free_opaque_representation_installs_with_audit_advice() {
    let fixture = Fixture::new();
    let output = install(&fixture, "opaque-carrier", OPAQUE, 0);
    let stdout = text(&output);
    assert!(
        stdout.contains("Audit recommended: opaque-carrier"),
        "{stdout}"
    );
    let paths = fixture.review_paths(&output);
    assert_eq!(paths.len(), 1);
    let document = fs::read_to_string(&paths[0]).unwrap();
    let section = package_section(&document, "opaque-carrier");
    assert!(section.contains("audit-recommended true"), "{section}");
    assert!(
        !document.lines().any(|line| line.starts_with("decision ")),
        "{document}"
    );
    assert_no_proposal(&fixture);
    assert_locked_graph(&fixture, &[("cli-project", "opaque-carrier", OPAQUE)]);
    let lock = fixture.lock();
    let target = lock.target(TARGET).unwrap();
    assert!(target.decisions().decisions().is_empty());
    let package = target
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "opaque-carrier")
        .unwrap();
    let policy = target
        .baselines()
        .iter()
        .find(|baseline| baseline.package() == package.key().identity())
        .unwrap();
    let [opaque] = policy.public_data() else {
        panic!("opaque-carrier must retain exactly PlatformToken")
    };
    assert_eq!(opaque.identity().path(), "PlatformToken");
    assert_eq!(format!("{:?}", opaque.supply()), "BoundaryOpaque");
    assert!(opaque.members().is_empty());
    assert!(opaque.invariants().is_empty());
    let representation = policy.representation();
    assert_eq!(representation.package(), package.key().identity());
    assert_eq!(representation.declarations(), &[opaque.identity().clone()]);
    assert!(representation.producer_availability().is_empty());
    assert!(representation.selected_availability().is_empty());
    assert!(representation.demands().is_empty());
    assert!(policy.public_propositions().is_empty());
    assert!(policy.dangerous_capabilities().is_empty());
    assert!(policy.external_supplies().is_empty());
    let callable = policy
        .callables()
        .callables()
        .iter()
        .find(|callable| format!("{:?}", callable.role()) == "Boundary")
        .unwrap();
    assert!(callable.identity().path().contains("inspect"));
    assert_eq!(callable.identity().owner(), opaque.identity().owner());
    assert_eq!(format!("{:?}", callable.supply()), "Boundary");
    assert!(callable.contracts().is_empty());
    assert!(callable.declared_service_reach().unwrap().is_empty());
    assert!(callable.checked_service_reach().realized().is_none());

    let report = audit(&fixture, 0);
    assert!(report.contains(OPAQUE), "{report}");
    let section = fresh_package(&report, "opaque-carrier");
    // Representation advice is emitted on introduction/change. An unchanged
    // accepted declaration remains visible without repeating that advice.
    for expected in [
        "audit-recommended false",
        "PlatformToken",
        "inspect",
        "role Boundary supply Boundary",
        "checked-reach unknown (no checked body)",
        "opaque-representation declarations 1 candidates 0 selections 0 demands 0",
    ] {
        assert!(section.contains(expected), "missing {expected}: {section}");
    }
    // Importing the declaration introduces no runtime by-value crossing.
    check_import(&fixture, "opaque-carrier", "machine main() {}");
}

fn install(fixture: &Fixture, package: &str, pin: &str, status: i32) -> Output {
    assert_recorded_pin(pin);
    let before = fixture.accepted_files();
    let output = fixture.omega(&[
        "install",
        &repository(package),
        "--rev",
        pin,
        "--target",
        "linux_x86_64",
    ]);
    assert_status(&output, status);
    if status == 3 {
        assert_eq!(fixture.accepted_files(), before);
    } else {
        assert_ne!(fixture.accepted_files().0, before.0);
        assert_ne!(fixture.accepted_files().1, before.1);
        assert!(text(&output).contains("Published build.omg and omega.lock"));
    }
    output
}

fn review_and_resume(
    fixture: &Fixture,
    output: &Output,
    package: &str,
    kind: &str,
    subject: &str,
) -> String {
    let paths = fixture.review_paths(output);
    assert_eq!(paths.len(), 1);
    let document = fs::read_to_string(&paths[0]).unwrap();
    assert!(document.contains("baseline none\n"), "{document}");
    let section = package_section(&document, package);
    let row = section
        .split("\nchange ")
        .find(|row| row.starts_with(&format!("{kind} added\n")) && row.contains(subject))
        .unwrap_or_else(|| panic!("missing {kind} review row: {section}"));
    let decision = row
        .lines()
        .find(|line| line.starts_with("decision ") && line.ends_with(" pending"))
        .unwrap_or_else(|| panic!("row must require its own decision: {row}"));
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
    let before = fixture.accepted_files();
    assert_status(&fixture.omega(&["install", "--resume"]), 3);
    assert_eq!(fixture.accepted_files(), before);
    let accepted_decision = decision.replace(" pending", " accept");
    for choice in ["pending", "reject"] {
        let edited = accepted.replace(
            &accepted_decision,
            &decision.replace(" pending", &format!(" {choice}")),
        );
        fs::write(&paths[0], &edited).unwrap();
        assert_status(&fixture.omega(&["install", "--resume"]), 3);
        assert_eq!(fixture.accepted_files(), before);
        let proposal = fixture.read("root/build/package-manager/proposal");
        // Audit inspects the accepted project while this installation is proposed.
        audit(fixture, 0);
        assert_eq!(
            fixture.read("root/build/package-manager/proposal"),
            proposal
        );
        assert_eq!(fs::read_to_string(&paths[0]).unwrap(), edited);
    }
    fs::write(&paths[0], accepted).unwrap();
    assert_status(&fixture.omega(&["install", "--resume"]), 0);
    assert_ne!(fixture.accepted_files().0, before.0);
    assert_ne!(fixture.accepted_files().1, before.1);
    let lock = fixture.lock();
    let decisions = lock.target(TARGET).unwrap().decisions().decisions();
    assert_eq!(
        decisions.len(),
        document
            .lines()
            .filter(|line| line.starts_with("decision "))
            .count()
    );
    assert!(
        decisions.iter().all(|decision| decision.disposition()
            == ReviewOnlyRootPolicyDisposition::AcceptCandidateChange)
    );
    assert_no_proposal(fixture);
    document
}

fn assert_locked_graph(fixture: &Fixture, edges: &[(&str, &str, &str)]) {
    let lock = fixture.lock();
    assert_eq!(lock.targets().len(), 1);
    let source = lock.target(TARGET).unwrap().source();
    assert_eq!(
        source.root().selected().key().name().as_str(),
        "cli-project"
    );
    assert_eq!(source.packages().len(), edges.len() + 1);
    assert_eq!(source.dependency_requests().len(), edges.len());
    for &(requester, selected, pin) in edges {
        assert_recorded_pin(pin);
        let package = source
            .packages()
            .iter()
            .find(|package| package.key().name().as_str() == selected)
            .unwrap();
        let ImmutableSourceResolution::Git { commit, .. } = package.resolution() else {
            panic!("{selected} must resolve through Git")
        };
        assert_eq!(commit.to_hex(), pin);
        let edge = source
            .dependency_requests()
            .iter()
            .find(|edge| {
                edge.requester().name().as_str() == requester
                    && edge.selected().key() == package.key()
            })
            .unwrap_or_else(|| panic!("missing {requester} -> {selected}"));
        assert_eq!(edge.selected().resolution(), package.resolution());
        assert_eq!(edge.alias().as_str(), selected.replace('-', "_"));
        let CanonicalDependencySourceRequest::Git {
            repository: actual_repository,
            revision,
            ..
        } = edge.request()
        else {
            panic!("{requester} -> {selected} must request SSH Git")
        };
        assert_eq!(actual_repository, &repository(selected));
        assert_eq!(revision, pin);
    }
}

fn audit(fixture: &Fixture, status: i32) -> String {
    let before = fixture.accepted_files();
    let output = fixture.omega(&["audit", "packages", "--target", "linux_x86_64"]);
    assert_status(&output, status);
    assert_eq!(fixture.accepted_files(), before);
    let report = text(&output);
    assert!(report.contains("fresh-analysis complete"), "{report}");
    assert!(report.contains("requires-review false"), "{report}");
    assert!(
        !report.lines().any(|line| line.starts_with("decision ")),
        "{report}"
    );
    if before.1.is_some() {
        assert!(
            report.contains("accepted-policy equal-to-fresh"),
            "{report}"
        );
    }
    report
}

fn assert_dependency_path(section: &str, aliases: &[&str]) {
    let path = section
        .lines()
        .find(|line| line.starts_with("fresh dependency-path "))
        .unwrap();
    let actual = path
        .split(" -> ")
        .skip(1)
        .map(|step| step.split('"').nth(1).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(actual, aliases, "{path}");
}

fn package_section<'a>(document: &'a str, package: &str) -> &'a str {
    document
        .split("\npackage ")
        .find(|section| section.starts_with(&format!("\"{package}\" ")))
        .unwrap_or_else(|| panic!("missing {package}: {document}"))
        .split("end-package")
        .next()
        .unwrap()
}

fn fresh_package<'a>(report: &'a str, package: &str) -> &'a str {
    report
        .split("\nfresh package ")
        .find(|section| section.starts_with(&format!("\"{package}\" ")))
        .unwrap_or_else(|| panic!("missing fresh {package}: {report}"))
}

fn check_import(fixture: &Fixture, package: &str, entry: &str) {
    fixture.write(
        "root/main.omg",
        &format!("use {}::main;\n{entry}\n", package.replace('-', "_")),
    );
    let before = fixture.accepted_files();
    assert_status(
        &fixture.omega(&["--check", "--target", "linux_x86_64", "main.omg"]),
        0,
    );
    assert_eq!(fixture.accepted_files(), before);
}

fn assert_no_proposal(fixture: &Fixture) {
    assert!(!fixture.path("root/build/package-manager/proposal").exists());
}

fn assert_recorded_pin(pin: &str) {
    assert!(
        pin.len() == 40 && pin.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "remote fixture requires an exact commit: {pin}"
    );
    assert!(
        include_str!("../../../../tests/fixtures/packages/REMOTE_PINS.md").contains(pin),
        "pin is not recorded in REMOTE_PINS.md: {pin}"
    );
}

fn repository(package: &str) -> String {
    format!("git@github.com:CathedralOS/{package}.git")
}

fn text(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}
