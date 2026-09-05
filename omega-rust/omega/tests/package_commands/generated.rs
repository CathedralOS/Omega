use super::fixture::{Fixture, assert_status};
use package_source::ImmutableSourceResolution;

#[path = "generated/build_scope.rs"]
mod build_scope;

const REVISION: &str = "cc5fc1addda6aa565f254ad2e002d9e0be189fd4";
const BUILD: &str = include_str!("../../../../tests/fixtures/packages/generated-table/build.omg");
const SOURCE: &str = include_str!("../../../../tests/fixtures/packages/generated-table/main.omg");
const INPUT: &str =
    include_str!("../../../../tests/fixtures/packages/generated-table/inputs/table.txt");

fn generated_fixture() -> Fixture {
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.path("dependency/inputs")).unwrap();
    fixture.write("dependency/build.omg", BUILD);
    fixture.write("dependency/main.omg", SOURCE);
    fixture.write("dependency/inputs/table.txt", INPUT);
    fixture
}

fn check_generated_import(fixture: &Fixture) {
    fixture.write(
        "root/main.omg",
        "use generated_table::main;\nmachine main() -> u64 { table_size() }\n",
    );
    let accepted = fixture.accepted_files();
    assert_status(&fixture.omega(&["--check", "main.omg"]), 0);
    assert_eq!(fixture.accepted_files(), accepted);
    assert!(
        !fixture.path("root/table.generated.omg").exists(),
        "dependency output escaped into the consumer source root"
    );
}

#[test]
fn installed_generated_dependency_is_available_to_ordinary_checking() {
    let fixture = generated_fixture();
    assert_status(&fixture.omega(&["install", "../dependency"]), 0);
    check_generated_import(&fixture);
    assert_status(&fixture.omega(&["update", "generated_table"]), 0);
    check_generated_import(&fixture);
}

#[test]
fn invalid_generated_proof_does_not_publish_a_dependency() {
    let fixture = generated_fixture();
    fixture.write(
        "dependency/build.omg",
        &BUILD.replace(
            "table_size() -> u64 {",
            "table_size() -> u64 ensures result == 4 {",
        ),
    );
    let before = fixture.accepted_files();
    let output = fixture.omega(&["install", "../dependency"]);
    assert_status(&output, 1);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("checked compilation failed"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fixture.accepted_files(), before);
}

#[test]
fn generated_output_cannot_escape_its_supplied_root() {
    let fixture = generated_fixture();
    fixture.write(
        "dependency/build.omg",
        &BUILD.replace("table.generated.omg", "../escaped.omg"),
    );
    let before = fixture.accepted_files();
    let output = fixture.omega(&["install", "../dependency"]);
    assert_status(&output, 1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("build-root path must use canonical relative components"),
        "{stderr}"
    );
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("escaped.omg").exists());
    assert!(!fixture.path("root/escaped.omg").exists());
    assert!(!fixture.path("dependency/escaped.omg").exists());
}

#[test]
#[ignore = "requires network and private CathedralOS generated-table repository access over SSH"]
fn pinned_ssh_generated_dependency_installs_updates_and_imports() {
    remote_generated("git@github.com:CathedralOS/generated-table.git");
}

#[test]
#[ignore = "requires network and private CathedralOS generated-table repository access over HTTPS"]
fn pinned_https_generated_dependency_installs_updates_and_imports() {
    remote_generated("https://github.com/CathedralOS/generated-table.git");
}

fn remote_generated(repository: &str) {
    let pins = include_str!("../../../../tests/fixtures/packages/REMOTE_PINS.md");
    assert!(
        pins.lines()
            .any(|line| { line.contains("`generated-table`") && line.contains(REVISION) })
    );
    let fixture = Fixture::new();
    assert_status(
        &fixture.omega(&["install", repository, "--rev", REVISION]),
        0,
    );
    check_generated_import(&fixture);
    assert_status(
        &fixture.omega(&["update", "generated_table", "--to", REVISION]),
        0,
    );
    check_generated_import(&fixture);
    for target in fixture.lock().targets() {
        let package = target
            .source()
            .packages()
            .iter()
            .find(|package| package.key().name().as_str() == "generated-table")
            .expect("generated dependency remains in the accepted graph");
        let ImmutableSourceResolution::Git { commit, .. } = package.resolution() else {
            panic!("remote generated source must retain its exact Git resolution");
        };
        assert_eq!(commit.to_hex(), REVISION);
    }
}
