//! Fixture inventory is independent of host eligibility and compile filters.
use super::abi_runtime_values_and_strings::fixture_roster as abi_runtime_values_and_strings;
use super::artifact_footprints::fixture_roster as artifact_footprints;
use super::atomics_and_target_canaries::fixture_roster as atomics_and_target_canaries;
use super::content_text_and_carriers::fixture_roster as content_text_and_carriers;
use super::domains_control_and_structures::fixture_roster as domains_control_and_structures;
use super::float_plans_and_policies::fixture_roster as float_plans_and_policies;
use super::generics_and_dependent_facts::fixture_roster as generics_and_dependent_facts;
use super::host_text_filesystem_and_abi::fixture_roster as host_text_filesystem_and_abi;
use super::layouts_and_pending::fixture_roster as layouts_and_pending;
use super::providers_float_and_console::fixture_roster as providers_float_and_console;
use super::ranges_storage_and_entries::fixture_roster as ranges_storage_and_entries;
use super::reports_and_capabilities::fixture_roster as reports_and_capabilities;
use super::structural_selected_operator::fixture_roster as structural_selected_operator;
use super::surface_and_targets::fixture_roster as surface_and_targets;
use super::time_hosts_and_indexed_storage::fixture_roster as time_hosts_and_indexed_storage;
use super::value_and_type_checks::fixture_roster as value_and_type_checks;
use super::value_calls_and_dispatch::fixture_roster as value_calls_and_dispatch;
use super::wire_and_algorithms::fixture_roster as wire_and_algorithms;
use super::*;

#[path = "../fixture_rosters/build_target_activation.rs"]
mod build_target_activation;
#[path = "../fixture_rosters/call_acknowledgements.rs"]
mod call_acknowledgements;
#[path = "../fixture_rosters/concurrency_carry.rs"]
mod concurrency_carry;
#[path = "../fixture_rosters/layout_plans.rs"]
mod layout_plans;
#[path = "../../../../../../tests/native-differential/fixture_rosters/content_custody.rs"]
mod native_content_custody;
#[path = "../fixture_rosters/native_filesystem_canaries.rs"]
mod native_filesystem_canaries;
#[path = "../../../../../../tests/native-differential/fixture_rosters/recast_views.rs"]
mod native_recast_views;
#[path = "../../../../../../tests/native-differential/fixture_rosters/structural_return.rs"]
mod native_structural_return;
#[path = "../../../../../../tests/native-differential/fixture_rosters/terminal_sources.rs"]
mod native_terminal_sources;
#[path = "../fixture_rosters/no_selection_golden.rs"]
mod no_selection_golden;
#[path = "../fixture_rosters/package_compilation_inputs.rs"]
mod package_compilation_inputs;
#[path = "../fixture_rosters/plan_laid_repeated_runtime.rs"]
mod plan_laid_repeated_runtime;
#[path = "../fixture_rosters/recast_views.rs"]
mod recast_views;
#[path = "../fixture_rosters/subslice_runtime_end_bounds.rs"]
mod subslice_runtime_end_bounds;

fn pass_roster() -> Vec<&'static str> {
    CHECKED_ONLY_PASS_CANARIES
        .iter()
        .chain(ACTIVE_PASS_CANARIES)
        .chain(atomics_and_target_canaries::PASS_CANARIES)
        .chain(surface_and_targets::PASS_CANARIES)
        .chain(surface_and_targets::RECENT_ENCODER_PASS_CANARIES)
        .chain(native_content_custody::PASS_CANARIES)
        .chain(native_recast_views::PASS_CANARIES)
        .chain(native_structural_return::PASS_CANARIES)
        .chain(native_terminal_sources::PASS_CANARIES)
        .chain(layouts_and_pending::PASS_CANARIES)
        .chain(artifact_footprints::PASS_CANARIES)
        .chain(structural_selected_operator::PASS_CANARIES)
        .chain(reports_and_capabilities::PASS_CANARIES)
        .chain(reports_and_capabilities::CHECKED_CAPABILITY_PASS_CANARIES)
        .chain(reports_and_capabilities::SIGNED_RAT_PASS_CANARIES)
        .chain(host_text_filesystem_and_abi::PASS_CANARIES)
        .chain(host_text_filesystem_and_abi::CROSS_WINDOWS_PASS_CANARIES)
        .chain(WINDOWS_HOST_PASS_CANARIES)
        .chain(concurrency_carry::PASS_CANARIES)
        .chain(recast_views::PASS_CANARIES)
        .chain(call_acknowledgements::PASS_CANARIES)
        .chain(layout_plans::PASS_CANARIES)
        .chain(plan_laid_repeated_runtime::PASS_CANARIES)
        .chain(subslice_runtime_end_bounds::PASS_CANARIES)
        .chain(build_target_activation::PASS_CANARIES)
        .chain(no_selection_golden::PASS_CANARIES)
        .chain(package_compilation_inputs::PASS_CANARIES)
        .chain(value_and_type_checks::PASS_CANARIES)
        .chain(content_text_and_carriers::PASS_CANARIES)
        .chain(domains_control_and_structures::PASS_CANARIES)
        .chain(domains_control_and_structures::BOUNDARY_DOMAIN_ESTABLISHMENT_PASS_CANARIES)
        .chain(domains_control_and_structures::RECURSIVE_WALK_PASS_CANARIES)
        .chain(float_plans_and_policies::PASS_CANARIES)
        .chain(float_plans_and_policies::FLOAT_TO_INTEGER_TRAP_PASS_CANARIES)
        .chain(abi_runtime_values_and_strings::PASS_CANARIES)
        .chain(abi_runtime_values_and_strings::BOUNDED_CARRIER_PASS_CANARIES)
        .chain(providers_float_and_console::PASS_CANARIES)
        .chain(providers_float_and_console::CONSOLE_LINE_REPLAY_CANARIES)
        .chain(ranges_storage_and_entries::PASS_CANARIES)
        .chain(value_calls_and_dispatch::PASS_CANARIES)
        .chain(wire_and_algorithms::PASS_CANARIES)
        .chain(generics_and_dependent_facts::PASS_CANARIES)
        .chain(generics_and_dependent_facts::STRUCTURED_CONST_PASS_CANARIES)
        .chain(time_hosts_and_indexed_storage::PASS_CANARIES)
        .copied()
        .chain(
            reports_and_capabilities::CAPABILITY_VERB_PASS_CANARIES
                .iter()
                .map(|entry| entry.0),
        )
        .chain(
            reports_and_capabilities::CAPABILITY_FLOW_PASS_CANARIES
                .iter()
                .map(|entry| entry.0),
        )
        .chain(
            float_plans_and_policies::POLICY_ADAPTER_PASS_CANARIES
                .iter()
                .map(|entry| entry.0),
        )
        .chain(
            float_plans_and_policies::POLICY_DIFFERENTIAL_PASS_CANARIES
                .iter()
                .map(|entry| entry.0),
        )
        .chain(
            domains_control_and_structures::ROOTED_RESIDUAL_SCALAR_ENTRY_PASS_CANARIES
                .iter()
                .map(|entry| entry.0),
        )
        .chain(
            abi_runtime_values_and_strings::PRNG_REPOSITORY_PASS_CANARIES
                .iter()
                .chain(abi_runtime_values_and_strings::FILESYSTEM_REPOSITORY_PASS_CANARIES)
                .chain(abi_runtime_values_and_strings::REPOSITORY_PASS_CANARIES)
                .map(|relative| {
                    relative
                        .strip_prefix("tests/omega/pass/")
                        .expect("repository-relative pass fixture prefix")
                }),
        )
        .chain(
            ranges_storage_and_entries::ENTRY_SCALAR_OPERATION_RESULTS
                .iter()
                .map(|fixture| fixture.path),
        )
        .chain(
            time_hosts_and_indexed_storage::STORAGE_RESULT_IMPORT_CANARIES
                .iter()
                .chain(time_hosts_and_indexed_storage::AUTHORED_SCALAR_IMPORT_CANARIES)
                .map(|entry| entry.1),
        )
        .chain(
            native_filesystem_canaries::PASS_CANARIES
                .iter()
                .map(|fixture| fixture.path),
        )
        .chain(CROSS_TARGET_PASS_CANARIES.iter().map(|entry| entry.0))
        .chain(
            ROOTED_TARGET_BACKEND_PASS_CANARIES
                .iter()
                .map(|entry| entry.0),
        )
        .collect()
}

fn file_expectation_fail_roster() -> Vec<&'static str> {
    // Cross-target rows only annotate compilation scheduled by these arrays.
    CHECKED_ONLY_FAIL_CANARIES
        .iter()
        .chain(ACTIVE_FAIL_CANARIES)
        .chain(surface_and_targets::FILE_EXPECTATION_FAIL_CANARIES)
        .chain(no_selection_golden::FILE_EXPECTATION_FAIL_CANARIES)
        .chain(generics_and_dependent_facts::FILE_EXPECTATION_FAIL_CANARIES)
        .chain(generics_and_dependent_facts::CLOSED_INDEXED_FAIL_CANARIES)
        .copied()
        .collect()
}

fn fail_roster() -> Vec<&'static str> {
    file_expectation_fail_roster()
        .into_iter()
        .chain(reports_and_capabilities::FAIL_CANARIES.iter().copied())
        .chain(atomics_and_target_canaries::FAIL_CANARIES.iter().copied())
        .chain(concurrency_carry::FAIL_CANARIES.iter().map(|entry| entry.0))
        .chain(recast_views::FAIL_CANARIES.iter().copied())
        .chain(layout_plans::FAIL_CANARIES.iter().copied())
        .chain(value_and_type_checks::FAIL_CANARIES.iter().copied())
        .chain(float_plans_and_policies::FAIL_CANARIES.iter().copied())
        .chain(
            float_plans_and_policies::FLOAT_TO_INTEGER_FAIL_CANARIES
                .iter()
                .map(|entry| entry.0),
        )
        .chain(
            content_text_and_carriers::UNAUTHORIZED_ESTABLISHMENT_FAIL_CANARIES
                .iter()
                .copied(),
        )
        .chain(wire_and_algorithms::FAIL_CANARIES.iter().copied())
        .chain(
            generics_and_dependent_facts::STRUCTURED_CONST_FAIL_CANARIES
                .iter()
                .map(|entry| entry.0),
        )
        .chain(
            time_hosts_and_indexed_storage::FAIL_CANARIES
                .iter()
                .copied(),
        )
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
    // Every owner needs source; only file-based owners require expected.txt.
    assert_registered_fixtures(&root, &fail_roster(), false);
    assert_registered_fixtures(&root, &file_expectation_fail_roster(), true);
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
