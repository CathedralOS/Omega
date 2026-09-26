use super::super::super::super::TerminationGuarantee;

use crate::tests::termination::progress_mutation::assert_unproved_tail_requirement;
use crate::tests::termination::progress_mutation::fixture_source;
use crate::tests::termination::progress_mutation::fixture_with_body;
use crate::tests::termination::symbol_of_checked;

fn assert_no_subject(program: &checked_trees::CheckedTrees, operation: &str) {
    let plan = program
        .facts
        .termination
        .for_machine(symbol_of_checked(program, "replace"))
        .unwrap();
    assert_eq!(
        plan.checked_summary,
        TerminationGuarantee::NoGuarantee,
        "a shared result cannot export a stale input subject: {operation}"
    );
}

fn assert_incoming_carrier_has_no_subject(operation: &str) {
    let source = fixture_source(
        "let original: Carrier = Carrier { context: &context, other: &replacement };
         let returned: Carrier = forward(original);
         let borrowed: &Context = returned.context;
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        &format!(
            "data Carrier<'source> {{
                 context: &'source Context;
                 other: &'source Context;
             }}
             machine inspect_context(context: &mut Context) -> u64 {{ 0 }}
             machine Carrier::touch(&mut self) -> u64 {{ 0 }}
             machine forward<'source>(mut input: Carrier<'source>) -> Carrier<'source> {{
                 {operation}
                 input
             }}"
        ),
    );
    assert_unproved_tail_requirement(&source);
}

#[test]
fn replacing_an_incoming_shared_slot_cannot_export_its_original_subject() {
    assert_incoming_carrier_has_no_subject("input.context = input.other;");
}

#[test]
fn a_shared_aggregate_result_cannot_restore_a_subject_after_overlapping_producer_writes() {
    let program = fixture_with_body(
        "let returned: Carrier = forward(context, replacement);
         let borrowed: &Context = returned.context;
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        "data Carrier<'context> { context: &'context Context; }
         machine forward<'context, 'replacement>(
             context: &'context mut Context,
             replacement: &'replacement Context
         ) -> Carrier<'context>
         requires replacement.scheduler in WeakFair
         ensures context.scheduler in WeakFair
         terminates;
         {
             context.scheduler = replacement.scheduler;
             Carrier { context: &context }
         }",
    );
    // The returned reference and restored qualification do not identify the
    // scheduler value installed by the producer's may-write frame.
    assert_no_subject(&program, "producer replaces the scheduler");
}
