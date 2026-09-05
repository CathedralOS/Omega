//! Corpus membership is independent of host eligibility and compile filters.
use super::*;

fn pass_roster() -> Vec<&'static str> {
    CHECKED_ONLY_PASS_CANARIES
        .iter()
        .chain(ACTIVE_PASS_CANARIES)
        .chain(WINDOWS_HOST_PASS_CANARIES)
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
    // Cross-target failure rows select a target; only these two lists actually
    // schedule compilation. A target annotation alone cannot cover a fixture.
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
    // A fixture owns group/name, including nested dependency packages and its
    // disposable build directory. Do not recursively count their main.omg files.
    for group in directories(root)? {
        for fixture in directories(&group)? {
            let name = |path: &Path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_owned)
                    .ok_or_else(|| format!("non-UTF-8 fixture name: {}", path.display()))
            };
            fixtures.push(format!("{}/{}", name(&group)?, name(&fixture)?));
        }
    }
    fixtures.sort();
    Ok(fixtures)
}

fn compare(root: &Path, roster: &[&str], requires_expectation: bool) -> Result<(), String> {
    let fixtures = discover(root)?;
    let mut registered = roster.to_vec();
    registered.sort_unstable();
    // Multiple explicit target runs can deliberately share one source fixture.
    registered.dedup();
    let unregistered = fixtures
        .iter()
        .filter(|fixture| registered.binary_search(&fixture.as_str()).is_err())
        .map(String::as_str)
        .collect::<Vec<_>>();
    let missing = registered
        .iter()
        .copied()
        .filter(|fixture| {
            fixtures
                .binary_search_by(|found| found.as_str().cmp(fixture))
                .is_err()
        })
        .collect::<Vec<_>>();
    let mut incomplete = Vec::new();
    for fixture in &fixtures {
        for filename in ["main.omg"]
            .into_iter()
            .chain(requires_expectation.then_some("expected.txt"))
        {
            let path = root.join(fixture).join(filename);
            if !fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_file()) {
                incomplete.push(format!("{fixture}/{filename}"));
            }
        }
    }
    if unregistered.is_empty() && missing.is_empty() && incomplete.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "canary roster differs from {}\nfixtures with no compile roster entry: {unregistered:?}\nroster entries with no fixture: {missing:?}\nmissing or non-regular fixture files: {incomplete:?}",
            root.display()
        ))
    }
}

pub(super) fn assert_pass_roster() {
    compare(&repo_root().join("tests/omega/pass"), &pass_roster(), false)
        .unwrap_or_else(|error| panic!("{error}"));
}

pub(super) fn assert_fail_roster() {
    let roster = fail_roster();
    compare(&repo_root().join("tests/omega/fail"), &roster, true)
        .unwrap_or_else(|error| panic!("{error}"));
    for (canary, _) in CROSS_TARGET_FAIL_CANARIES {
        assert!(
            ACTIVE_FAIL_CANARIES.contains(canary),
            "cross-target failure annotation has no active compile entry: {canary}"
        );
    }
}

#[test]
fn pass_roster_matches_corpus_on_every_host() {
    assert_pass_roster();
}

#[test]
fn fail_roster_matches_corpus_on_every_host() {
    assert_fail_roster();
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
fn unregistered_fixture_and_missing_roster_entry_are_both_reported() {
    let tree = FixtureTree::new();
    tree.fixture("group/registered", false);
    tree.fixture("group/unregistered", false);
    let error = compare(&tree.0, &["group/registered", "group/missing"], false).unwrap_err();
    assert!(error.contains("fixtures with no compile roster entry: [\"group/unregistered\"]"));
    assert!(error.contains("roster entries with no fixture: [\"group/missing\"]"));
}

#[test]
fn nested_packages_and_generated_builds_are_not_independent_fixtures() {
    let tree = FixtureTree::new();
    tree.fixture("group/registered", true);
    tree.fixture("group/registered/dependency", false);
    tree.fixture("group/registered/build", false);
    assert!(compare(&tree.0, &["group/registered", "group/registered"], true).is_ok());
}

#[test]
fn missing_entrypoint_and_failure_expectation_are_loud() {
    let tree = FixtureTree::new();
    fs::create_dir_all(tree.0.join("group/incomplete")).unwrap();
    let error = compare(&tree.0, &["group/incomplete"], false).unwrap_err();
    assert!(error.contains("main.omg"));
    tree.fixture("group/incomplete", false);
    let error = compare(&tree.0, &["group/incomplete"], true).unwrap_err();
    assert!(error.contains("expected.txt"));
}

#[test]
fn missing_corpus_root_is_not_an_empty_success() {
    let tree = FixtureTree::new();
    assert!(compare(&tree.0.join("absent"), &[], false).is_err());
}
