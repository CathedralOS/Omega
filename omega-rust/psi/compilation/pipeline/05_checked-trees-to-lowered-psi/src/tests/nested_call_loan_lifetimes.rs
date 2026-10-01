//! A scalar actual's nested call may lend the same storage that an earlier
//! copied actual observed. Stage 04 records the nested `&mut` as a mutable
//! loan that ends when its call returns, and the copied read as ending at
//! once (`borrow/accesses/collection.rs`). The mixed-argument replay
//! (`mixed_arguments/access_occurrences.rs`) and the owned-copy rejoin
//! (`borrow_occurrences.rs`) accept that roster only while those lifetimes
//! keep the two rows apart; a copy retained through invocation, a substituted
//! access, or an end outside the roster still rejects.

use super::{checked_source_with_core_service, lower_machine};
use crate::TerminalMachineSelection;

const COPIED_OPERAND_BEFORE_NESTED_LOAN: &str = r#"
    data Settings [copy] { count: i32; }
    machine Settings::stamp(settings: &mut Settings, count: i32) -> i32 {
        settings.count = count;
        count
    }
    machine Settings::observe(settings: Settings, later: i32) -> i32 { settings.count }
    machine probe() -> bool {
        let mut settings: Settings = Settings { count: 17 };
        let before: i32 = Settings::observe(settings, Settings::stamp(&mut settings, 25));
        before == 17 && settings.count == 25
    }
"#;

/// The outer call's roster: the copied `settings` operand, then the nested
/// call's mutable loan of the same local.
fn outer_call_accesses(
    checked: &checked_trees::CheckedTrees,
) -> arena::HandleSpan<checked_trees::BorrowArgumentAccessFact> {
    let plans = &checked.facts.values.scalar_computations;
    let mut calls = plans.nodes.iter().filter_map(|(_, node)| {
        let checked_trees::CheckedScalarComputationKind::Call {
            source_call,
            arguments,
            ..
        } = node.kind
        else {
            return None;
        };
        plans
            .operands
            .span(arguments)?
            .iter()
            .any(|argument| {
                matches!(
                    plans.nodes.get(*argument).kind,
                    checked_trees::CheckedScalarComputationKind::Call { .. }
                )
            })
            .then(|| checked.facts.flow.control.calls.get(source_call).accesses)
    });
    let accesses = calls
        .next()
        .expect("the outer call observes its nested call");
    assert!(calls.next().is_none(), "one outer call");
    accesses
}

#[test]
fn a_nested_mutable_loan_follows_a_copied_operand_of_the_same_local() {
    let original = checked_source_with_core_service(COPIED_OPERAND_BEFORE_NESTED_LOAN);
    let lowered = lower_machine(&original, TerminalMachineSelection::Name("probe"))
        .expect("the copy ends before the nested call lends the same local");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the snapshot and the nested loan verify independently");

    let accesses = outer_call_accesses(&original);
    let rows = original
        .facts
        .borrow
        .argument_accesses
        .span_or_empty(accesses);
    assert_eq!(
        rows.iter()
            .map(|row| (row.kind.clone(), row.end_before_access))
            .collect::<Vec<_>>(),
        [
            (checked_trees::BorrowAccessKind::Read, Some(1)),
            (checked_trees::BorrowAccessKind::Mutable, Some(2)),
        ],
        "a copied read, then a loan that ends when its nested call returns"
    );
    assert_eq!(rows[0].root_symbol, rows[1].root_symbol);

    for mutation in 0..4 {
        let mut changed = original.clone();
        let rows = changed
            .facts
            .borrow
            .argument_accesses
            .span_mut_or_empty(accesses);
        match mutation {
            // The copy would stay live beside the mutable loan.
            0 => rows[0].end_before_access = None,
            // The nested borrow would no longer be the authored `&mut`.
            1 => rows[1].kind = checked_trees::BorrowAccessKind::Read,
            // A loan cannot end before it begins or outside its roster.
            2 => rows[1].end_before_access = Some(1),
            3 => rows[1].end_before_access = Some(3),
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("probe")).is_err(),
            "mutation {mutation} must reject"
        );
    }
}
