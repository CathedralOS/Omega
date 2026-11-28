//! Check the same authored call before provider selection and at a closed receipt.
use super::super::build_purpose::judge_build_published;
use super::{
    AWAIT_CHANGE, expect_unproved_progress, reject_deadlock, requires_an_external_enabler,
};
use crate::RouteVerdict;
use terminal_semantics::synchronization::{ProgressFailure, ProgressScope, private_wait_failure};

fn program(loan: &str, after: &str) -> String {
    format!("{AWAIT_CHANGE}
        data Pair {{ first: Signal; second: Signal; }}
        boundary trait Gate {{ machine receive(signal: @Signal) reaches none crashes none terminates; }}
        machine build_entry() reaches Gate crashes none {{
            let mutable pair: Pair;
            pair.first.word.store(1, Publish);
            Gate::receive(@pair.{loan});
            {after}
            await_change(@pair.first);
        }}")
}

fn selected(source: &str) -> Option<ProgressFailure> {
    let verdict =
        judge_build_published(source, &[], terminal_codec::TerminalPublication::Portable).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("{verdict:?}");
    };
    let artifact = accepted.publish().unwrap().artifact;
    let bytes =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    let module = bytes.semantic_module();
    let Some(terminal_psi::TargetMachineCallable::Machine(entry)) = module.entry else {
        panic!()
    };
    private_wait_failure(module, entry, ProgressScope::Executable, |_| None)
        .map(|failure| failure.cause)
}

#[test]
fn a_provider_borrow_preserves_disjoint_cells_and_subsequent_stores() {
    reject_deadlock(&program("second", ""));
    assert_eq!(
        selected(&program("first", "")),
        Some(ProgressFailure::Unproved)
    );
    assert_eq!(
        selected(&program("first", "pair.first.word.store(0, Publish);")),
        None
    );
    reject_deadlock(&program("first", "pair.first.word.store(1, Publish);"));
}

#[test]
fn lending_supplied_storage_does_not_create_an_external_enabler() {
    let source = format!(
        "{AWAIT_CHANGE}
        boundary trait Gate {{ machine receive(signal: @Signal) reaches none crashes none; }}
        machine build_entry(signal: @Signal) reaches Gate crashes none {{
            Gate::receive(signal);
            await_change(signal);
        }}"
    );
    requires_an_external_enabler(&source, "await_change");
}

#[test]
fn an_opaque_continuation_only_owes_progress_for_a_wait_that_can_block() {
    let source = format!(
        "boundary trait Peer {{ machine ping() reaches none crashes none; }} {}",
        program("first", "")
    )
    .replace(
        "reaches none crashes none terminates",
        "reaches Peer crashes none terminates",
    )
    .replace("reaches Gate crashes", "reaches Gate + Peer crashes");
    assert_eq!(
        selected(&source),
        Some(ProgressFailure::UnprovedContinuation)
    );
    let descriptor = source.replace(
        "await_change(@pair.first);",
        "let run: Wait = await_change; run(@pair.first);",
    );
    let descriptor =
        format!("data Wait = machine(signal: @Signal) reaches none crashes none; {descriptor}");
    assert_eq!(
        selected(&descriptor),
        Some(ProgressFailure::UnprovedContinuation)
    );
    let self_enabling = source.replace("word.load(Receive)", "word.swap(0, ReceivePublish)");
    assert_eq!(selected(&self_enabling), None);
}

#[test]
fn the_provider_alternative_cannot_erase_the_other_arms_wait() {
    let source = format!("{AWAIT_CHANGE}
        boundary trait Gate {{ machine receive(signal: @Signal) reaches none crashes none terminates; }}
        machine choose(signal: @Signal, first: bool) reaches Gate crashes none {{
            transition first {{ true -> lend(signal), _ -> release(signal) }}
            machine lend(signal: @Signal) reaches Gate crashes none {{ Gate::receive(signal); }}
            machine release(signal: @Signal) {{ signal.word.store(0, Publish); }}
        }}
        machine build_entry(first: bool) reaches Gate crashes none {{
            let mutable local: Signal;
            choose(@local, first);
            await_change(@local);
        }}");
    assert_eq!(selected(&source), Some(ProgressFailure::Unproved));
    expect_unproved_progress(&source.replace("store(0", "store(1"));
}
