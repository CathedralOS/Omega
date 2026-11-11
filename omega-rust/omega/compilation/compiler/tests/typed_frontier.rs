//! The typed-frontier seam gives a second checker the programs the current
//! checker rejects, not only the ones it accepts. Each test drives one existing
//! corpus fixture through `check_fixture_keeping_typed_frontier`, which takes the
//! same review and check path as `check_fixture`.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use fixture_package_inputs::{
    FixtureFrontierCheck, check_fixture, check_fixture_keeping_typed_frontier, discover_fixture,
    fixture_build_snapshot, repo_root, review_fixture,
};

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

// Discovery and checking recurse deeply; give them the compiler worker's stack
// rather than the small one libtest gives each test thread.
fn on_compile_worker<T: Send>(work: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, work)
            .expect("compiler worker")
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
}

fn check_on_compile_worker(fixture: &str) -> FixtureFrontierCheck {
    let main = repo_root().join(fixture).join("main.omg");
    on_compile_worker(|| check_fixture_keeping_typed_frontier(&main, "linux_x86_64"))
}

// The diagnostics `check_fixture` returns without the seam, on the same
// target, so the seam's verdict is compared with the ordinary one.
fn ordinary_diagnostics(fixture: &str) -> Vec<String> {
    let main = repo_root().join(fixture).join("main.omg");
    on_compile_worker(|| {
        let checked = (|| {
            let snapshot = fixture_build_snapshot(&main)?;
            let discovered = discover_fixture(
                &main,
                package_compilation::BuildExecutionInputs {
                    snapshot,
                    ..Default::default()
                },
            )?;
            let target = discovered
                .targets
                .iter()
                .find(|target| target.as_str() == "linux_x86_64")
                .or_else(|| discovered.targets.first())
                .cloned();
            let profile = target::TargetProfile::from_omega_target_name(target.as_deref())
                .map_err(|error| vec![error])?;
            let mut prepared =
                review_fixture(discovered, &[profile], compiler::CompilationProduct::Check)?;
            check_fixture(&mut prepared, profile.target_name(), Default::default())
        })();
        checked
            .expect_err("the ordinary check rejects this fixture")
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect()
    })
}

fn expected_fragment(fixture: &str) -> String {
    std::fs::read_to_string(repo_root().join(fixture).join("expected.txt"))
        .expect("fixture expectation")
        .lines()
        .next()
        .expect("one expected fragment")
        .trim()
        .to_owned()
}

/// Psi 05 rejects this package program after Psi 04 completed: a write leaves
/// a data `requires` unproved. The seam still returns the frozen typed trees
/// with the bound entry, and its verdict is the one `check_fixture` returns.
#[test]
fn a_program_rejected_after_psi_04_still_yields_typed_trees() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let fixture = "tests/omega/fail/dependent/applied_default_domain_unrelated_bound";
    let FixtureFrontierCheck::Rejected {
        frontier,
        diagnostics,
    } = check_on_compile_worker(fixture)
    else {
        panic!("the current checker must reject this program after Psi 04");
    };
    assert!(
        !frontier.typed.machines().is_empty(),
        "the rejected program's typed trees are retained"
    );
    let entry = *frontier.entries.first().expect("the Build binds an entry");
    assert!(
        frontier.typed.machine_by_symbol(entry).is_some(),
        "the entry names a machine in the retained trees"
    );
    let seam: Vec<_> = diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect();
    assert!(!seam.is_empty(), "a rejection carries its diagnostics");
    assert_eq!(
        seam,
        ordinary_diagnostics(fixture),
        "the seam's verdict is the ordinary check's verdict"
    );
}

/// A parse error stops the compilation before Psi 04 produces typed trees, so
/// the seam reports that distinct outcome instead of a verdict. Name and type
/// errors in this compiler are mostly reported by Psi 05 and validation, after
/// Psi 04 (for example `fail/types/usize_rejected`), so a parse error is the
/// fixture that exercises this outcome.
#[test]
fn a_program_that_fails_to_parse_does_not_reach_psi_04() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let fixture = "tests/omega/fail/parse/machine_clause_garbage_rejected";
    let FixtureFrontierCheck::StoppedBeforeTypedFrontier(diagnostics) =
        check_on_compile_worker(fixture)
    else {
        panic!("a parse error must stop before Psi 04");
    };
    let expected = expected_fragment(fixture);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(&expected)),
        "{diagnostics:?}"
    );
}
