//! A check hands the checker the program's own package and what that package
//! reaches (`wiki/spec/build/validation_subjects.md`). A dependency's
//! provider-default machine chooses the provider for one boundary trait, so it
//! belongs to a check only when the program reaches that trait. Each test
//! drives an existing corpus fixture and reads the trees the checker was given.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use fixture_package_inputs::{
    FixtureFrontierCheck, check_fixture_keeping_typed_frontier, repo_root,
};
use typed_trees::machine::MachinePurpose;

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// What the checker was given for `fixture`: the names of its boundary traits
/// and of the traits its provider-default machines choose providers for.
struct Projected {
    traits: Vec<String>,
    default_owners: Vec<String>,
    machines: usize,
}

fn projected(fixture: &str) -> Projected {
    let main = repo_root().join(fixture).join("main.omg");
    // Discovery and checking recurse deeply; use the compiler worker's stack.
    let check = std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            check_fixture_keeping_typed_frontier(&main, "linux_x86_64")
        })
        .expect("compiler worker")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    });
    let frontier = match check {
        FixtureFrontierCheck::Accepted { frontier, .. }
        | FixtureFrontierCheck::Rejected { frontier, .. }
        | FixtureFrontierCheck::ReviewRefused { frontier, .. } => frontier,
        FixtureFrontierCheck::StoppedBeforeTypedFrontier(diagnostics) => {
            panic!("`{fixture}` must reach the checker: {diagnostics:?}")
        }
    };
    let typed = &frontier.typed;
    Projected {
        traits: typed
            .traits()
            .iter()
            .map(|definition| definition.name.as_str().to_owned())
            .collect(),
        default_owners: typed
            .machines()
            .iter()
            .filter(|machine| machine.purpose == MachinePurpose::ProviderDefaults)
            .map(|machine| machine.name.as_str().to_owned())
            .collect(),
        machines: typed.machines().len(),
    }
}

/// Thirty lines of scalar arithmetic call no service. The checker used to
/// receive every target's filesystem and window providers with them, because
/// each provider-default machine was demanded whatever the program reached.
#[test]
fn a_program_that_calls_no_service_is_checked_without_service_providers() {
    let _compilation = one_at_a_time();
    let scalar = projected("tests/fixtures/run/calls/scalar_contract_route_exit");
    for service in [
        "FilesystemService",
        "PosixFilesystemHost",
        "WindowsFilesystemHost",
        "WindowsGuiHost",
    ] {
        assert!(
            !scalar.traits.iter().any(|name| name == service),
            "`{service}` is reached by nothing in this program: {:?}",
            scalar.traits
        );
    }
    assert!(
        !scalar
            .default_owners
            .iter()
            .any(|name| name.contains("Filesystem") || name.contains("Gui")),
        "no filesystem or window default is chosen for a program that calls neither: {:?}",
        scalar.default_owners
    );
    // The whole library holds about three thousand machines; before this rule
    // this program's check kept 1,858 of them.
    assert!(
        scalar.machines < 1000,
        "the check kept {} machines",
        scalar.machines
    );
}

/// The same rule keeps a default with the trait it serves. This program
/// writes two bytes through `Console` and its Build selects no provider, so
/// the machines choosing each target's console provider are still checked and
/// still found by settlement. What a kept provider itself reaches stays too:
/// the Windows console provider calls Windows host services, so their
/// defaults come with it.
#[test]
fn a_program_that_calls_a_service_keeps_that_services_default_provider() {
    let _compilation = one_at_a_time();
    let console = projected("tests/fixtures/run/providers/default_console_provider_reached");
    assert!(
        console.traits.iter().any(|name| name == "Console"),
        "{:?}",
        console.traits
    );
    assert!(
        console
            .default_owners
            .iter()
            .any(|name| name.starts_with("ConsoleNativeProvider::")),
        "the reached trait keeps the machine that chooses its provider: {:?}",
        console.default_owners
    );
}
