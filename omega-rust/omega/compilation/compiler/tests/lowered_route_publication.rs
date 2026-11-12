//! The first piece of a production path from the checker on the lowered
//! form: a program the route accepts is published as canonical Terminal Psi
//! from the route's own module (`AcceptedClosure::publish`), its evidence
//! seals are attached by Omega 00 to that module and fill the accepted
//! obligations' proof fingerprints in Omega 01, and the published program
//! runs in the Terminal interpreter to the exit its fixture pins
//! (`wiki/drafts/designs/check_on_lowered_form.md`, "Route one", Build:
//! "Discharged certificates travel in memory beside `TerminalModule` within
//! one compile so Omega 00 can fill
//! `AcceptedObligationFact::proof_bundle_fingerprint`").
//!
//! The default compile does not take this path yet: a governed program is
//! still built from the current checker's output.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use fixture_package_inputs::{
    FixtureFrontierCheck, check_fixture_keeping_typed_frontier, repo_root,
};
use lowered_discharge::{RouteVerdict, judge};
use psi_work::WorkMeter;
use structure_elaboration::{Compilation, elaborate};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalStructuralInputs,
};

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct NoEffects;

impl TerminalEffectHandler for NoEffects {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "the route's subset performs no effect",
        ))
    }
}

fn accepted(fixture: &str) -> Box<lowered_discharge::AcceptedClosure> {
    let main = repo_root().join(fixture).join("main.omg");
    let checked = std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            check_fixture_keeping_typed_frontier(&main, "linux_x86_64")
        })
        .expect("compiler worker")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    });
    let frontier = match checked {
        FixtureFrontierCheck::Accepted { frontier, .. } => frontier,
        other => panic!("{fixture}: the program must check: {other:?}"),
    };
    match judge(elaborate(
        &frontier.typed,
        Compilation {
            entries: &frontier.entries,
            subject: frontier.subject,
            selected: &frontier.selected,
        },
    )) {
        Ok(RouteVerdict::Accepted(closure)) => closure,
        other => panic!("{fixture}: the route must accept: {other:?}"),
    }
}

#[test]
fn an_accepted_program_is_published_from_the_routes_module_and_runs() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let closure = accepted("tests/omega/run/calls/scalar_contract_route_exit");
    let publication = closure.publish().expect("the accepted module publishes");

    // The artifact holds exactly the module the route checked.
    assert_eq!(
        publication.artifact.semantic_module(),
        &closure.elaboration.module
    );

    // Omega 00 attaches the seals to that module, and each accepted
    // operation obligation carries its certificate's fingerprint.
    let expected: Vec<[u8; 32]> = publication
        .seals
        .seals
        .iter()
        .map(|seal| seal.fingerprint)
        .collect();
    assert!(!expected.is_empty(), "the program has discharged operations");
    let input = terminal_psi_to_abstract_operations::lower_module_with_evidence(
        publication.artifact.semantic_module(),
        publication.seals.clone(),
    )
    .expect("Omega 00 lowers the published module with its seals")
    .into_optimization_artifact()
    .into_optimization_input();
    let (_, unit) = abstract_operations_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        psi_work::WorkSchedule::CURRENT.identity(),
    )
    .expect("Omega 01 builds the verified unit")
    .into_parts();
    let sealed: Vec<[u8; 32]> = unit
        .accepted_obligation_facts
        .iter()
        .map(|fact| fact.proof_bundle_fingerprint)
        .filter(|fingerprint| *fingerprint != [0; 32])
        .collect();
    for fingerprint in &expected {
        assert!(sealed.contains(fingerprint), "{expected:?} / {sealed:?}");
    }

    // The published program runs to completion.
    let bytes = terminal_codec::encode_module(publication.artifact.semantic_module())
        .expect("the published module encodes");
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &[], TerminalStructuralInputs::default())
            .expect("the interpreter loads the published module");
    let status = execution
        .resume(&mut WorkMeter::with_budget(10_000_000), &mut NoEffects)
        .expect("the published program runs");
    let expected_exit = std::fs::read_to_string(
        repo_root().join("tests/omega/run/calls/scalar_contract_route_exit/expected_native_exit.txt"),
    )
    .unwrap();
    assert_eq!(expected_exit.trim(), "0");
    assert!(
        matches!(status, terminal_interpreter::TerminalExecutionStatus::Complete(_)),
        "{status:?}"
    );
}
