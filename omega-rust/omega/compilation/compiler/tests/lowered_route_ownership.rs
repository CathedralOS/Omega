//! Plain owned data is judged from actual transfers on the lowered form. The
//! positive retains call-result/local identities through publication and runs;
//! the negative borrows the same local after an earlier operand consumed it.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use fixture_package_inputs::{
    FixtureFrontierCheck, check_fixture_keeping_typed_frontier, repo_root,
};
use lowered_discharge::{RouteVerdict, judge};
use structure_elaboration::{Compilation, elaborate};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionStatus, TerminalStructuralInputs,
};

static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn check(fixture: &str) -> FixtureFrontierCheck {
    let main = repo_root().join(fixture).join("main.omg");
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            check_fixture_keeping_typed_frontier(&main, "linux_x86_64")
        })
        .expect("compiler worker")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
}

struct NoEffects;

impl TerminalEffectHandler for NoEffects {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "plain data performs no effect",
        ))
    }
}

#[test]
fn owned_call_results_and_locals_publish_and_execute_the_routes_module() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let FixtureFrontierCheck::Accepted { frontier, .. } =
        check("tests/omega/pass/calls/runtime_affine_record_call_results_reused_exit")
    else {
        panic!("the original owned-data witness must check");
    };
    let closure = match judge(elaborate(
        &frontier.typed,
        Compilation {
            entries: &frontier.entries,
            subject: frontier.subject,
            selected: &frontier.selected,
        },
    )) {
        Ok(RouteVerdict::Accepted(closure)) => closure,
        other => panic!("the owned-data route must accept: {other:?}"),
    };
    assert!(
        closure
            .elaboration
            .module
            .structural_types
            .iter()
            .any(|ty| !ty.copyable)
    );
    let publication = closure.publish().expect("owned-data publication");
    assert_eq!(
        publication.artifact.semantic_module(),
        &closure.elaboration.module
    );
    terminal_psi_to_abstract_operations::lower_module_with_evidence(
        publication.artifact.semantic_module(),
        publication.seals.clone(),
    )
    .expect("Omega 00 carries the checked affine operations and their seals");
    let bytes = terminal_codec::encode_module(publication.artifact.semantic_module())
        .expect("the owned-data module encodes");
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &[], TerminalStructuralInputs::default())
            .expect("the interpreter loads the route's own module");
    let status = execution
        .resume(
            &mut psi_work::WorkMeter::with_budget(10_000_000),
            &mut NoEffects,
        )
        .expect("the route's owned values execute");
    assert!(
        matches!(status, TerminalExecutionStatus::Complete(_)),
        "{status:?}"
    );
}

#[test]
fn borrowing_after_an_earlier_operand_moves_the_local_is_rejected() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let FixtureFrontierCheck::Rejected { frontier, .. } =
        check("tests/omega/fail/borrows/owned_address_after_operand_move")
    else {
        panic!("the move-before-borrow witness must reject after the typed frontier");
    };
    let verdict = judge(elaborate(
        &frontier.typed,
        Compilation {
            entries: &frontier.entries,
            subject: frontier.subject,
            selected: &frontier.selected,
        },
    ))
    .expect("the route judges the ownership obligation");
    let RouteVerdict::Rejected { unestablished, .. } = verdict else {
        panic!("the route must reject the moved local: {verdict:?}");
    };
    let expected = std::fs::read_to_string(
        repo_root().join("tests/omega/fail/borrows/owned_address_after_operand_move/expected.txt"),
    )
    .unwrap();
    assert!(
        unestablished
            .iter()
            .any(|entry| entry.message().contains(expected.trim())),
        "the ownership rejection must retain its pinned transferred/consumed diagnostic: {unestablished:?}"
    );
}
