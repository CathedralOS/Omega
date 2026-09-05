//! Fixture inventory is independent of host eligibility and compile filters.
use super::*;

#[path = "../fixture_rosters/concurrency_carry.rs"]
mod concurrency_carry;

fn pass_roster() -> Vec<&'static str> {
    CHECKED_ONLY_PASS_CANARIES
        .iter()
        .chain(ACTIVE_PASS_CANARIES)
        .chain(WINDOWS_HOST_PASS_CANARIES)
        .chain(concurrency_carry::PASS_CANARIES)
        .copied()
        .chain(CROSS_TARGET_PASS_CANARIES.iter().map(|entry| entry.0))
        .chain(
            ROOTED_TARGET_BACKEND_PASS_CANARIES
                .iter()
                .map(|entry| entry.0),
        )
        .collect()
}

fn fail_roster() -> Vec<&'static str> {
    // Cross-target rows only annotate compilation scheduled by these arrays.
    CHECKED_ONLY_FAIL_CANARIES
        .iter()
        .chain(ACTIVE_FAIL_CANARIES)
        .copied()
        .collect()
}

fn directories(parent: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(parent)
        .map_err(|error| format!("cannot enumerate {}: {error}", parent.display()))?;
    let mut directories = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("cannot inspect {}: {error}", parent.display()))?;
        let kind = entry
            .file_type()
            .map_err(|error| format!("cannot inspect {}: {error}", entry.path().display()))?;
        if kind.is_symlink() {
            return Err(format!(
                "corpus directories cannot be symlinks: {}",
                entry.path().display()
            ));
        }
        if kind.is_dir() {
            directories.push(entry.path());
        }
    }
    directories.sort();
    Ok(directories)
}

fn discover(root: &Path) -> Result<Vec<String>, String> {
    let mut fixtures = Vec::new();
    // Nested packages and disposable builds belong to their group/name fixture.
    for group in directories(root)? {
        for fixture in directories(&group)? {
            let name = |path: &Path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| format!("non-UTF-8 fixture name: {}", path.display()))
                    .map(str::to_owned)
            };
            fixtures.push(format!("{}/{}", name(&group)?, name(&fixture)?));
        }
    }
    fixtures.sort();
    Ok(fixtures)
}

#[derive(Debug, Default)]
struct InventoryDifference {
    unregistered: Vec<String>,
    missing: Vec<String>,
    incomplete: Vec<String>,
}

fn compare(
    root: &Path,
    roster: &[&str],
    requires_expectation: bool,
) -> Result<InventoryDifference, String> {
    let fixtures = discover(root)?;
    let mut registered = roster.to_vec();
    registered.sort_unstable();
    // A fixture may deliberately execute on more than one target or test owner.
    registered.dedup();
    let mut difference = InventoryDifference::default();
    for fixture in &fixtures {
        if registered.binary_search(&fixture.as_str()).is_err() {
            difference.unregistered.push(fixture.clone());
        }
    }
    for fixture in registered {
        if fixtures
            .binary_search_by(|found| found.as_str().cmp(fixture))
            .is_err()
        {
            difference.missing.push(fixture.to_owned());
            continue;
        }
        for filename in ["main.omg"]
            .into_iter()
            .chain(requires_expectation.then_some("expected.txt"))
        {
            let path = root.join(fixture).join(filename);
            if !fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_file()) {
                difference.incomplete.push(format!("{fixture}/{filename}"));
            }
        }
    }
    Ok(difference)
}

fn assert_registered_fixtures(root: &Path, roster: &[&str], requires_expectation: bool) {
    let difference =
        compare(root, roster, requires_expectation).unwrap_or_else(|error| panic!("{error}"));
    assert!(
        difference.missing.is_empty() && difference.incomplete.is_empty(),
        "registered fixture inventory differs from {}: {difference:#?}",
        root.display()
    );
    // Reverse closure remains TASKS.md's CANARY-ROSTER-DERIVATION: other
    // dedicated owners must expose their executing tables before this can be
    // an error. Never infer their coverage from arbitrary source strings.
}

#[test]
fn registered_pass_canaries_have_source_on_every_host() {
    assert_registered_fixtures(&repo_root().join("tests/omega/pass"), &pass_roster(), false);
}

#[test]
fn registered_fail_canaries_have_source_and_their_owned_expectations() {
    let root = repo_root().join("tests/omega/fail");
    assert_registered_fixtures(&root, &fail_roster(), true);
    let dedicated = concurrency_carry::FAIL_CANARIES
        .iter()
        .map(|entry| entry.0)
        .collect::<Vec<_>>();
    // This owner reads inline diagnostics, not expected.txt.
    assert_registered_fixtures(&root, &dedicated, false);
    for (canary, _) in CROSS_TARGET_FAIL_CANARIES {
        assert!(
            ACTIVE_FAIL_CANARIES.contains(canary),
            "cross-target failure annotation has no executing roster entry: {canary}"
        );
    }
}

struct FixtureTree(PathBuf);

impl FixtureTree {
    fn new() -> Self {
        let path = unique_no_output_build_dir();
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn fixture(&self, name: &str, expectation: bool) {
        let path = self.0.join(name);
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("main.omg"), "").unwrap();
        if expectation {
            fs::write(path.join("expected.txt"), "expected diagnostic").unwrap();
        }
    }
}

impl Drop for FixtureTree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn comparison_retains_both_missing_and_unregistered_fixtures() {
    let tree = FixtureTree::new();
    tree.fixture("group/registered", false);
    tree.fixture("group/unregistered", false);
    let difference = compare(&tree.0, &["group/registered", "group/missing"], false).unwrap();
    assert_eq!(difference.missing, ["group/missing"]);
    assert_eq!(difference.unregistered, ["group/unregistered"]);
    assert!(difference.incomplete.is_empty());
}

#[test]
fn nested_packages_and_builds_are_not_independent_fixtures() {
    let tree = FixtureTree::new();
    tree.fixture("group/registered", true);
    tree.fixture("group/registered/dependency", false);
    tree.fixture("group/registered/build", false);
    let difference = compare(&tree.0, &["group/registered", "group/registered"], true).unwrap();
    assert!(
        difference.unregistered.is_empty()
            && difference.missing.is_empty()
            && difference.incomplete.is_empty()
    );
}

#[test]
fn missing_source_and_file_expectations_are_separate_from_inline_expectations() {
    let tree = FixtureTree::new();
    fs::create_dir_all(tree.0.join("group/incomplete")).unwrap();
    let difference = compare(&tree.0, &["group/incomplete"], false).unwrap();
    assert_eq!(difference.incomplete, ["group/incomplete/main.omg"]);
    tree.fixture("group/incomplete", false);
    let difference = compare(&tree.0, &["group/incomplete"], true).unwrap();
    assert_eq!(difference.incomplete, ["group/incomplete/expected.txt"]);
    assert!(
        compare(&tree.0, &["group/incomplete"], false)
            .unwrap()
            .incomplete
            .is_empty()
    );
}

#[test]
fn absent_corpus_root_cannot_be_an_empty_success() {
    let tree = FixtureTree::new();
    assert!(compare(&tree.0.join("absent"), &[], false).is_err());
}
