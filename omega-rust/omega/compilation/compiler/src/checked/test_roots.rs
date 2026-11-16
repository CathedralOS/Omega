/*
Discover a registered test group's roster and select each test as its own root
(wiki/spec/build/testing.md#registration-and-discovery).

After the package's final source is checked, every concrete, executable,
package-local machine with a checked exact satisfaction edge to a registered
requirement is one test. Discovery reads that edge from the checked program; it
never matches names or method conventions. Each test is then a separate
selection over the same checked source: a copy of the application source whose
only `ProgramEntry` root is the test machine. Roots decide which machines the
Terminal program retains, so each copy is checked again from the unprepared
source with its own roster (`product_batch.rs`): the lowered-form checker
judges the test machine's closure, the copy publishes its own Terminal
program, and none of its test-only machines join the application's roots.
*/

use diagnostics::Diagnostic;
use products::{
    CheckedProductSource, ComputedBuildConfig, TestFilesystem, TestRootIdentity, TestRosterEntry,
    TestRosterStatus,
};
use std::sync::Arc;

pub(crate) struct TestRootSource {
    pub(crate) identity: TestRootIdentity,
    // The application's Build result with the test machine as its only root.
    pub(crate) build: Arc<ComputedBuildConfig>,
}

pub(crate) struct TestRootSelection {
    pub(crate) roots: Vec<TestRootSource>,
    // Disabled groups and groups with no satisfiers, reported but never run.
    pub(crate) notes: Vec<TestRosterEntry>,
}

/*
The runner backs exactly one service: std's `FilesystemService` slot, with a
private filesystem created empty for each invocation
(wiki/spec/build/testing.md#group-configuration-and-authority). But std is an
ordinary package whose name, path and same-spelled declarations confer nothing
(wiki/spec/packages/toolchain.md), and no accepted semantic binding names that
slot yet, so the runner cannot tell std's slot from another package's. Until
the owner settles how it is identified, every named slot is refused before any
test runs rather than recognized by its package name.
*/
fn group_filesystem(
    source: &CheckedProductSource,
    group: &build_time_evaluation::test_groups::TestGroupRegistration,
) -> Result<TestFilesystem, Vec<Diagnostic>> {
    let Some(service) = group.services.first() else {
        return Ok(TestFilesystem::Unprovided);
    };
    Err(vec![
        Diagnostic::error(format!(
            "the test runner cannot back `{}`: it backs only std's `FilesystemService`, and no accepted binding identifies that slot without trusting a package name",
            source.program.symbols.display_path(service.slot, "::"),
        ))
        .with_source_span(service.source_span),
    ])
}

// The roster for one application occurrence and target, in deterministic
// group-registration then test-path order.
pub(crate) fn select_test_roots(
    source: &CheckedProductSource,
    target: target::TargetProfile,
) -> Result<TestRootSelection, Vec<Diagnostic>> {
    let config = &source.computed_build_config.config;
    let typed = source.program.as_ref();
    let note = |requirement, status| TestRosterEntry {
        group: typed.symbols.display_path(requirement, "::"),
        test: None,
        target,
        status,
    };
    if config.settings.tests_disabled {
        return Ok(TestRootSelection {
            roots: Vec::new(),
            notes: config
                .test_groups
                .iter()
                .map(|group| note(group.requirement, TestRosterStatus::GroupDisabled))
                .collect(),
        });
    }
    if config.test_groups.is_empty() {
        return Ok(TestRootSelection {
            roots: Vec::new(),
            notes: Vec::new(),
        });
    }
    let mut roots = Vec::new();
    let mut notes = Vec::new();
    for group in &config.test_groups {
        let package = typed
            .symbols
            .symbol_package_identity(group.selecting_machine);
        let group_path = typed.symbols.display_path(group.requirement, "::");
        let mut tests = Vec::new();
        for machine in typed.machines() {
            if typed.symbols.symbol_package_identity(machine.symbol) != package {
                continue;
            }
            let satisfies = typed
                .machine_trait_conformances(machine)
                .iter()
                .filter_map(|conformance| {
                    typed_trees::machine::resolve_satisfied_declaration(typed, machine, conformance)
                })
                .any(|declaration| declaration.symbol() == group.requirement);
            if !satisfies {
                continue;
            }
            let test_path = typed.symbols.display_path(machine.symbol, "::");
            // Discovery never guesses generic arguments; an open satisfier
            // needs an ordinary concrete wrapper before it can run.
            if !typed.machine_type_parameters(machine).is_empty() || !machine.body_is_present {
                return Err(vec![
                    Diagnostic::error(format!(
                        "test `{test_path}` in group `{group_path}` is not runnable: a test must be a concrete machine with a body; wrap a generic satisfier in a concrete test"
                    ))
                    .with_source_span(group.source_span),
                ]);
            }
            // A test entry receives only arguments its installed runner
            // contract establishes; this runner installs none, so a
            // parameterized satisfier cannot run and is reported, not dropped.
            if typed
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.parameters.count() != 0)
            {
                return Err(vec![
                    Diagnostic::error(format!(
                        "test `{test_path}` in group `{group_path}` is not runnable: its entry takes parameters, and no installed runner contract supplies them"
                    ))
                    .with_source_span(group.source_span),
                ]);
            }
            tests.push((test_path, machine.symbol));
        }
        tests.sort_by(|left, right| left.0.cmp(&right.0));
        // Every invocation of the group runs over the same selected backing,
        // each with its own fresh state.
        let filesystem = group_filesystem(source, group)?;
        // A group with no tests ran nothing; it is not evidence that any
        // test passed, so it is reported rather than silently omitted.
        if tests.is_empty() {
            notes.push(note(group.requirement, TestRosterStatus::GroupWithoutTests));
        }
        for (test_path, symbol) in tests {
            let mut computed = (*source.computed_build_config).clone();
            computed.config.root_bindings = vec![provider_planning::program_entry::RootBinding {
                slot: format!("{}::ProgramEntry", target.target_name()),
                implementation: test_path.clone(),
                implementation_symbol: symbol,
                source_span: typed.symbols.symbol_source_span(symbol),
            }];
            roots.push(TestRootSource {
                identity: TestRootIdentity {
                    group: group_path.clone(),
                    test: test_path,
                    target,
                    filesystem,
                },
                build: Arc::new(computed),
            });
        }
    }
    Ok(TestRootSelection { roots, notes })
}
