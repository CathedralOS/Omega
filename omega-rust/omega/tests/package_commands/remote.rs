use super::fixture::{Fixture, assert_status};
use package_source::ImmutableSourceResolution;

const REPOSITORY: &str = "https://github.com/CathedralOS/arithmetic-kernels.git";
const BASELINE: &str = "998dac4a03109f67b8c2e87d53ff017007526669";
const REVISION: &str = "b65cc9b062f69ef02a586c82cd260d51bf28945c";

#[test]
#[ignore = "requires network access and private CathedralOS GitHub repository access over HTTPS"]
fn pinned_https_install_and_update_publish_and_import_remote_api() {
    install_update_and_import(REPOSITORY);
}

#[test]
#[ignore = "requires network access and private CathedralOS GitHub repository access over SSH"]
fn pinned_ssh_install_and_update_publish_and_import_remote_api() {
    install_update_and_import("git@github.com:CathedralOS/arithmetic-kernels.git");
}

fn install_update_and_import(repository: &str) {
    let pins = include_str!("../../../../tests/fixtures/packages/REMOTE_PINS.md");
    assert!(
        pins.lines()
            .any(|line| line.contains("`arithmetic-kernels`") && line.contains(REVISION)),
        "update this canary only with the reviewed remote fixture pin"
    );
    assert!(
        pins.contains(BASELINE),
        "the upgrade baseline must be recorded"
    );
    assert_ne!(BASELINE, REVISION, "the canary must exercise a real update");
    let fixture = Fixture::new();
    let before = fixture.accepted_files();
    let output = fixture.omega(&["install", repository, "--rev", BASELINE]);
    assert_status(&output, 0);
    fixture.assert_published(&before);
    assert_exact_pin(&fixture, BASELINE);

    let installed_build = fixture.accepted_files().0;
    let output = fixture.omega(&["update", "arithmetic_kernels", "--to", REVISION]);
    assert_status(&output, 0);
    assert_ne!(
        fixture.accepted_files().0,
        installed_build,
        "selected version update did not edit the authored revision"
    );
    assert_exact_pin(&fixture, REVISION);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Source diff: arithmetic-kernels"));
    let source_diff = fixture.read("root/build/package-manager/source-diff.txt");
    assert!(source_diff.contains(&format!("baseline_git_commit {BASELINE}\n")));
    assert!(source_diff.contains(&format!("candidate_git_commit {REVISION}\n")));
    assert!(source_diff.contains("left + right") && source_diff.contains("right + left"));

    let updated_build = fixture.accepted_files().0;
    let output = fixture.omega(&["update", "arithmetic_kernels", "--to", REVISION]);
    assert_status(&output, 0);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Published build.omg and omega.lock"));
    assert_eq!(
        fixture.accepted_files().0,
        updated_build,
        "same-revision update changed the authored dependency"
    );
    assert_exact_pin(&fixture, REVISION);
    for path in fixture.review_paths(&output) {
        let document = std::fs::read_to_string(path).unwrap();
        assert!(
            !document
                .lines()
                .any(|line| line.starts_with("decision ") && line.ends_with(" pending")),
            "same-revision update left unresolved policy decisions: {document}"
        );
    }
    // The mirrored fixture exports add_u64 in main.omg; this check consumes the
    // installed source through the declared default dependency alias.
    fixture.write(
        "root/main.omg",
        "use arithmetic_kernels::main;\nmachine main() -> u64 { add_u64(2, 3) }\n",
    );
    let accepted = fixture.accepted_files();
    assert_status(&fixture.omega(&["--check", "main.omg"]), 0);
    assert_status(&fixture.omega(&["--check", "--offline", "main.omg"]), 0);
    assert_status(&fixture.omega(&["audit", "packages", "--offline"]), 0);
    assert_eq!(fixture.accepted_files(), accepted);
}

fn assert_exact_pin(fixture: &Fixture, expected: &str) {
    let lock = fixture.lock();
    for target in lock.targets() {
        let package = target
            .source()
            .packages()
            .iter()
            .find(|package| package.key().name().as_str() == "arithmetic-kernels")
            .expect("remote package is locked");
        let ImmutableSourceResolution::Git { commit, .. } = package.resolution() else {
            panic!("remote package did not retain a Git resolution");
        };
        assert_eq!(
            commit.to_hex(),
            expected,
            "remote package drifted from the exact fixture pin"
        );
    }
}
