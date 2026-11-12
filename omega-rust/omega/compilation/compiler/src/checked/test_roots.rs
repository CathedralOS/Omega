/*
Discover a registered test group's roster and select each test as its own root
(wiki/spec/build/testing.md#registration-and-discovery).

After the package's final source is checked, every concrete, executable,
package-local machine with a checked exact satisfaction edge to a registered
requirement is one test. Discovery reads that edge from the checked trees; it
never matches names or method conventions. Each test is then a separate
selection over the same checked source: a copy of the application source whose
only `ProgramEntry` root is the test machine. Roots decide which machines the
Terminal program retains, so each copy publishes its own Terminal program and
none of its test-only machines join the application's roots.
*/

use compilation_products::{
    CheckedProductSource, TestFilesystem, TestRootIdentity, TestRosterEntry, TestRosterStatus,
};
use diagnostics::Diagnostic;
use std::sync::Arc;

pub(crate) struct TestRootSource {
    pub(crate) identity: TestRootIdentity,
    pub(crate) source: CheckedProductSource,
}

pub(crate) struct TestRootSelection {
    pub(crate) roots: Vec<TestRootSource>,
    // Disabled groups and groups with no satisfiers, reported but never run.
    pub(crate) notes: Vec<TestRosterEntry>,
}

/*
The runner backs exactly one service: std's `FilesystemService` slot, with a
private filesystem created empty for each invocation
(wiki/spec/build/testing.md#group-configuration-and-authority). The slot is
recognized by its exact declaration in the package named `omega_language_std`,
never by a leaf name another package could reuse; any other slot is refused
rather than run without its service or over the host's.
*/
const STD_PACKAGE: &str = "omega_language_std";
const STD_FILESYSTEM_SLOT: &str = "FilesystemService";

fn group_filesystem(
    source: &CheckedProductSource,
    inputs: &package_compilation::PackageCompilationInputs,
    group: &build_time_evaluation::test_groups::TestGroupRegistration,
) -> Result<TestFilesystem, Vec<Diagnostic>> {
    let symbols = &source.program.typed.symbols;
    let mut filesystem = TestFilesystem::Unprovided;
    for service in &group.services {
        let std = symbols
            .symbol_package_identity(service.slot)
            .and_then(|package| inputs.package_name(package))
            == Some(STD_PACKAGE);
        if !std || symbols.display_path(service.slot, "::") != STD_FILESYSTEM_SLOT {
            return Err(vec![
                Diagnostic::error(format!(
                    "the test runner supplies no `{}` service; it backs only `{STD_PACKAGE}::{STD_FILESYSTEM_SLOT}`",
                    symbols.display_path(service.slot, "::"),
                ))
                .with_source_span(service.source_span),
            ]);
        }
        filesystem = TestFilesystem::Private;
    }
    Ok(filesystem)
}

// The roster for one application occurrence and target, in deterministic
// group-registration then test-path order.
pub(crate) fn select_test_roots(
    source: &CheckedProductSource,
    target: target::TargetProfile,
    inputs: &package_compilation::PackageCompilationInputs,
) -> Result<TestRootSelection, Vec<Diagnostic>> {
    let config = &source.computed_build_config.config;
    let typed = &source.program.typed;
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
    // Pruning keeps only application roots; running tests against that
    // program would depend on optional dead-code elimination.
    if source.product_selection.is_some() {
        return Err(vec![Diagnostic::error(
            "registered test groups cannot run against a checked program pruned to its application roots",
        )]);
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
        let filesystem = group_filesystem(source, inputs, group)?;
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
            }];
            let mut test_source = source.clone();
            test_source.computed_build_config = Arc::new(computed);
            test_source.terminal_publication =
                Arc::new(source.terminal_publication.for_other_roots());
            test_source.calling_program = Default::default();
            test_source.binding_declarations = Default::default();
            roots.push(TestRootSource {
                identity: TestRootIdentity {
                    group: group_path.clone(),
                    test: test_path,
                    target,
                    filesystem,
                },
                source: test_source,
            });
        }
    }
    Ok(TestRootSelection { roots, notes })
}
