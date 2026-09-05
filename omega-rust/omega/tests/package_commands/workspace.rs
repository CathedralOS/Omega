use super::fixture::{Fixture, assert_status};
use package_source::ImmutableSourceResolution;

const BASELINE: &str = "f487125e6fc58d01a2b584424ac5194cdff4f810";
const REVISION: &str = "664d771bbb851201807532e9ed8c444639f65c8f";
const ARITHMETIC: &str = "b65cc9b062f69ef02a586c82cd260d51bf28945c";

fn check_member_import(fixture: &Fixture) {
    fixture.write(
        "root/main.omg",
        "use exact_math::main;\nmachine main() -> u64 { workspace_value() }\n",
    );
    let accepted = fixture.accepted_files();
    assert_status(&fixture.omega(&["--check", "main.omg"]), 0);
    assert_eq!(fixture.accepted_files(), accepted);
}

#[test]
fn workspace_fixture_member_and_relative_dependency_compile_locally() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/package-workspaces/library-workbench/libraries/exact-math");
    let fixture = Fixture::new();
    assert_status(&fixture.omega(&["install", root.to_str().unwrap()]), 0);
    check_member_import(&fixture);
    assert_eq!(fixture.lock().targets()[0].source().packages().len(), 3);
}

#[test]
#[ignore = "requires network and private CathedralOS library-workbench/arithmetic-kernels access over SSH"]
fn pinned_ssh_named_member_updates_repository_and_preserves_unrelated_pin() {
    remote_workspace(
        "git@github.com:CathedralOS/library-workbench.git",
        "git@github.com:CathedralOS/arithmetic-kernels.git",
    );
}

#[test]
#[ignore = "requires network and private CathedralOS library-workbench/arithmetic-kernels access over HTTPS"]
fn pinned_https_named_member_updates_repository_and_preserves_unrelated_pin() {
    remote_workspace(
        "https://github.com/CathedralOS/library-workbench.git",
        "https://github.com/CathedralOS/arithmetic-kernels.git",
    );
}

fn remote_workspace(repository: &str, arithmetic: &str) {
    let pins = include_str!("../../../../tests/fixtures/packages/REMOTE_PINS.md");
    assert!(pins.contains(BASELINE) && pins.contains(REVISION) && pins.contains(ARITHMETIC));
    assert_ne!(BASELINE, REVISION);
    let fixture = Fixture::new();
    assert_status(
        &fixture.omega(&["install", arithmetic, "--rev", ARITHMETIC]),
        0,
    );
    assert_status(
        &fixture.omega(&[
            "install",
            repository,
            "--rev",
            BASELINE,
            "--package",
            "exact-math",
        ]),
        0,
    );
    assert_pins(&fixture, BASELINE);
    check_member_import(&fixture);
    let baseline_keys: Vec<_> = fixture.lock().targets()[0]
        .source()
        .packages()
        .iter()
        .map(|package| package.key().clone())
        .collect();
    assert_status(
        &fixture.omega(&["update", "exact_math", "--to", REVISION]),
        0,
    );
    assert_pins(&fixture, REVISION);
    check_member_import(&fixture);
    let candidate_keys: Vec<_> = fixture.lock().targets()[0]
        .source()
        .packages()
        .iter()
        .map(|package| package.key().clone())
        .collect();
    assert_eq!(
        candidate_keys, baseline_keys,
        "source update changed package identities"
    );
    assert!(!fixture.path("root/build/package-manager/proposal").exists());
}

fn assert_pins(fixture: &Fixture, expected: &str) {
    for target in fixture.lock().targets() {
        assert_eq!(target.source().packages().len(), 4);
        for name in ["exact-math", "integer-constants", "arithmetic-kernels"] {
            let package = target
                .source()
                .packages()
                .iter()
                .find(|package| package.key().name().as_str() == name)
                .expect("selected and transitive packages remain in the accepted graph");
            let ImmutableSourceResolution::Git { commit, .. } = package.resolution() else {
                panic!("remote workspace members must retain a Git resolution");
            };
            assert_eq!(
                commit.to_hex(),
                if name == "arithmetic-kernels" {
                    ARITHMETIC
                } else {
                    expected
                }
            );
        }
    }
}
