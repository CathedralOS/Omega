use super::{
    TARGET, assert_locked_authority, assert_no_proposal, assert_recorded_pin, assert_status,
    dangerous_decisions, install_authority, package_section, review,
};
use crate::fixture::Fixture;
use std::fs;

const REPOSITORY: &str = "git@github.com:CathedralOS/generated-table.git";
const PURE: &str = "cc5fc1addda6aa565f254ad2e002d9e0be189fd4";
const AUTHORITY: &str = "46cb5db4f74d745b735f383d3f22940ac3909c28";

#[test]
#[ignore = "requires network and private CathedralOS generated-table/host-services access over SSH"]
fn pinned_ssh_generated_process_authority_requires_initial_review() {
    let fixture = install_authority("generated-table", AUTHORITY, "Console", "Process");
    check_generated_apis(&fixture);
}

#[test]
#[ignore = "requires network and private CathedralOS generated-table/host-services access over SSH"]
fn pinned_ssh_pure_to_generated_authority_update_requires_review() {
    assert_recorded_pin(PURE);
    assert_recorded_pin(AUTHORITY);
    let fixture = Fixture::new();
    assert_status(
        &fixture.omega(&[
            "install",
            REPOSITORY,
            "--rev",
            PURE,
            "--target",
            "linux_x86_64",
        ]),
        0,
    );
    let baseline = fixture.lock();
    let old_target = baseline.target(TARGET).unwrap();
    assert_eq!(old_target.source().packages().len(), 2);
    assert!(
        old_target
            .baselines()
            .iter()
            .all(|policy| policy.dangerous_capabilities().is_empty())
    );
    let old_package = old_target
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "generated-table")
        .unwrap();
    let before = fixture.accepted_files();
    let output = fixture.omega(&["update", "generated_table", "--to", AUTHORITY]);
    assert_status(&output, 3);
    assert_eq!(fixture.accepted_files(), before);
    let (path, document) = review(&fixture, &output);
    let section = package_section(&document, "generated-table");
    assert!(section.contains("source-changed true\n"), "{section}");
    assert!(section.contains("audit-recommended true\n"), "{section}");
    assert!(section.contains("terminate"), "{section}");
    let dangerous = dangerous_decisions(section);
    let [(row, decision)] = dangerous.as_slice() else {
        panic!("the generated callable must introduce one dangerous row: {section}");
    };
    assert!(row.contains("Console"), "{row}");
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
    let accepted_decision = decision.replace(" pending", " accept");
    for disposition in ["pending", "reject"] {
        fs::write(
            &path,
            accepted.replace(
                &accepted_decision,
                &decision.replace(" pending", &format!(" {disposition}")),
            ),
        )
        .unwrap();
        assert_status(&fixture.omega(&["update", "--resume"]), 3);
        assert_eq!(fixture.accepted_files(), before);
    }
    fs::write(&path, accepted).unwrap();
    assert_status(&fixture.omega(&["update", "--resume"]), 0);
    assert_ne!(fixture.accepted_files().0, before.0);
    assert_ne!(fixture.accepted_files().1, before.1);
    let candidate = fixture.lock();
    let new_package = candidate
        .target(TARGET)
        .unwrap()
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "generated-table")
        .unwrap();
    assert_eq!(old_package.key(), new_package.key());
    assert_ne!(old_package.resolution(), new_package.resolution());
    let patch = fixture.read("root/build/package-manager/source-diff.txt");
    assert!(
        patch.contains(&format!("baseline_git_commit {PURE}\n")),
        "{patch}"
    );
    assert!(
        patch.contains(&format!("candidate_git_commit {AUTHORITY}\n")),
        "{patch}"
    );
    assert!(patch.contains("entry build.omg\n"), "{patch}");
    assert!(
        patch.contains("console.exit_process(return_code)"),
        "{patch}"
    );
    check_generated_apis(&fixture);
}

fn check_generated_apis(fixture: &Fixture) {
    assert_locked_authority(fixture, "generated-table", AUTHORITY, "Console", "Process");
    assert_no_proposal(fixture);
    let lock = fixture.lock();
    let target = lock.target(TARGET).unwrap();
    let package = target
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "generated-table")
        .unwrap();
    let policy = target
        .baselines()
        .iter()
        .find(|policy| policy.package() == package.key().identity())
        .unwrap();
    for name in ["table_size", "terminate"] {
        assert!(
            policy.callables().callables().iter().any(|callable| {
                format!("{:?}", callable.role()) == "Public"
                    && callable.identity().path().contains(name)
            }),
            "missing generated public API {name}"
        );
    }
    let build = policy
        .callables()
        .callables()
        .iter()
        .find(|callable| format!("{:?}", callable.role()) == "Build")
        .unwrap();
    assert!(
        build.checked_service_reach().realized().unwrap().is_empty(),
        "generated runtime authority is not build execution authority"
    );
    fixture.write(
        "root/main.omg",
        "use generated_table::main;\nmachine main() -> u64 { table_size() }\n",
    );
    let before = fixture.accepted_files();
    assert_status(
        &fixture.omega(&["--check", "--target", "linux_x86_64", "main.omg"]),
        0,
    );
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("root/table.generated.omg").exists());
    let output = fixture.omega(&["audit", "packages", "--target", "linux_x86_64"]);
    assert_status(&output, 0);
    assert_eq!(fixture.accepted_files(), before);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let section = stdout
        .split("fresh package ")
        .find(|section| section.starts_with("\"generated-table\" "))
        .unwrap();
    assert!(section.contains("terminate"), "{section}");
    assert!(
        section.contains("Process \"host-services\"::\"Console\""),
        "{section}"
    );
    assert!(section.contains("audit-recommended true"), "{section}");
}
