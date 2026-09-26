use super::{publish_conditional_claim_joins, validate_conditional_claim_joins};
use crate::checks::multiplicity::linear_validation::permission_event_statement_index;
use crate::tests::front_end::checked_program_result;
use checked_trees::{FlowClaimJoinAlternativeSource, FlowClaimJoinExitKind};
use diagnostics::Diagnostic;
use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};

const CHOICE: &str = r#"
data Receipt [linear] { code: i32; }
machine Receipt::ack(self) {}
machine choose(flag: bool, left: Receipt, right: Receipt) -> Receipt {
    transition flag {
        true -> first(left, right)
        _ -> second(left, right)
    }
    state first(left: Receipt, right: Receipt) -> Receipt {
        Receipt::ack(right);
        left
    }
    state second(left: Receipt, right: Receipt) -> Receipt {
        Receipt::ack(left);
        right
    }
}
machine consume(flag: bool, left: Receipt, right: Receipt) {
    let selected: Receipt = choose(flag, left, right);
    Receipt::ack(selected);
}
"#;

fn lower(source: &str) -> Result<checked_trees::CheckedTrees, Vec<Diagnostic>> {
    checked_program_result(source)
}

const OPTIONAL_INPUT: &str = r#"
data Receipt [linear] { code: i32; }
data Slot { case Empty; case Held(receipt: Receipt); }
data Outcome { case Grown(slot: Slot); }
machine Receipt::ack(self) {}
machine Outcome::ack(self) {}
machine empty() -> Slot { Slot::Empty }
machine select(slot: Slot, fresh: Receipt) -> Outcome {
    transition slot {
        Slot::Empty -> (Outcome::Grown { slot: Slot::Held { receipt: fresh } })
        Slot::Held { receipt } -> reject(receipt, fresh)
    }
    state reject(receipt: Receipt, fresh: Receipt) -> Outcome {
        Receipt::ack(fresh);
        Outcome::Grown { slot: Slot::Held { receipt: receipt } }
    }
}
machine consume(fresh: Receipt) {
    let slot: Slot = empty();
    let result: Outcome = select(slot, fresh);
    Outcome::ack(result);
}
"#;

#[test]
fn conditional_result_join_records_exact_excluded_input() {
    let checked = lower(OPTIONAL_INPUT).expect("known empty caller excludes callee Held input");
    assert!(
        checked
            .facts
            .flow
            .ownership
            .claim_join_alternatives
            .iter()
            .any(|(_, alternative)| matches!(
                alternative.source,
                FlowClaimJoinAlternativeSource::ExcludedInput { .. }
            ))
    );
    let mut altered = checked.facts.flow.ownership.clone();
    let handle = altered
        .claim_join_alternatives
        .iter()
        .find_map(|(handle, alternative)| {
            matches!(
                alternative.source,
                FlowClaimJoinAlternativeSource::ExcludedInput { .. }
            )
            .then_some(handle)
        })
        .expect("excluded input alternative");
    if let FlowClaimJoinAlternativeSource::ExcludedInput { established_at, .. } =
        &mut altered.claim_join_alternatives.get_mut(handle).source
    {
        *established_at += 1;
    }
    let mut diagnostics = Vec::new();
    validate_conditional_claim_joins(
        &checked.typed,
        &checked.facts.borrow,
        &altered,
        &mut diagnostics,
    );
    assert!(
        !diagnostics.is_empty(),
        "exclusion establishment must replay exactly"
    );
}

#[test]
fn conditional_result_join_does_not_exclude_unknown_active_input() {
    let source = OPTIONAL_INPUT
        .replace(
            "machine consume(fresh: Receipt)",
            "machine consume(fresh: Receipt, slot: Slot)",
        )
        .replace("    let slot: Slot = empty();\n", "");
    let checked = lower(&source).expect("unknown case retains active alternatives");
    assert!(
        !checked
            .facts
            .flow
            .ownership
            .claim_join_alternatives
            .iter()
            .any(|(_, alternative)| matches!(
                alternative.source,
                FlowClaimJoinAlternativeSource::ExcludedInput { .. }
            ))
    );
}

#[test]
fn conditional_result_join_rejects_reusing_moved_active_input() {
    let source = OPTIONAL_INPUT
        .replace("machine Receipt::ack(self) {}", "machine Receipt::ack(self) {}\nmachine Slot::ack(self) {}")
        .replace("machine consume(fresh: Receipt)", "machine consume(fresh: Receipt, other: Receipt)")
        .replace(
            "    let slot: Slot = empty();",
            "    let slot: Slot = Slot::Held { receipt: fresh };\n    let moved: Slot = slot;\n    Slot::ack(moved);",
        )
        .replace("select(slot, fresh)", "select(slot, other)");
    assert!(lower(&source).is_err());
}

#[test]
fn conditional_result_join_does_not_inherit_a_single_input_identity() {
    let source = r#"
data Receipt [linear] { code: i32; }
machine Receipt::ack(self) {}
machine choose(flag: bool, input: Receipt) -> Receipt {
    transition flag {
        true -> input
        _ -> replace(input)
    }
    state replace(input: Receipt) -> Receipt {
        Receipt::ack(input);
        let replacement: Receipt = Receipt { code: 7 };
        replacement
    }
}
machine consume(flag: bool, input: Receipt) {
    let selected: Receipt = choose(flag, input);
    Receipt::ack(selected);
}
"#;
    let checked = lower(source).expect("forwarding and fresh construction remain distinct origins");
    let ownership = &checked.facts.flow.ownership;
    let (_, receipt) = ownership
        .claim_join_receipts
        .iter()
        .next()
        .expect("receipt");
    assert!(
        ownership
            .permissions
            .iter()
            .filter(|(_, event)| event.state_symbol == receipt.state_symbol
                && event.source == PermissionEventSource::StateEntry)
            .all(|(_, event)| event.claim_identity != receipt.claim_identity
                && !matches!(event.provenance, PermissionProvenance::Joined { .. }))
    );
}

#[test]
fn conditional_result_join_wrapper_calls_keep_call_local_occurrences() {
    let source = CHOICE
        .replace(
            "machine consume",
            r#"
machine wrapped(flag: bool, left: Receipt, right: Receipt) -> Receipt {
    let selected: Receipt = choose(flag, left, right);
    selected
}
machine consume"#,
        )
        .replace(
            "let selected: Receipt = choose(flag, left, right);\n    Receipt::ack(selected);",
            "let selected: Receipt = wrapped(flag, left, right);\n    Receipt::ack(selected);",
        );
    let checked =
        lower(&source).expect("wrapper substitutes the nested join at its own invocation");
    let receipts = checked
        .facts
        .flow
        .ownership
        .claim_join_receipts
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(receipts.len(), 2);
    assert_ne!(receipts[0].1.claim_identity, receipts[1].1.claim_identity);
    assert!(
        checked
            .facts
            .flow
            .ownership
            .claim_join_exits
            .iter()
            .any(|(_, exit)| matches!(exit.kind, FlowClaimJoinExitKind::CallResult { .. }))
    );
}

#[test]
fn conditional_result_join_full_replay_rejects_correlated_argument_origin_swap() {
    let mut checked = lower(CHOICE).expect("conditional result is conserved");
    let (_, receipt) = checked
        .facts
        .flow
        .ownership
        .claim_join_receipts
        .iter()
        .next()
        .expect("receipt");
    let state = receipt.state_symbol;
    let statement = receipt.statement_index;
    let transfers = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.state_symbol == state
                && event.kind == PermissionEventKind::Transfer
                && permission_event_statement_index(event.source) == Some(statement)
        })
        .map(|(handle, event)| (handle, event.claim_identity, event.provenance))
        .collect::<Vec<_>>();
    assert_eq!(transfers.len(), 2);
    for index in 0..2 {
        let replacement = &transfers[1 - index];
        let event = checked
            .facts
            .flow
            .ownership
            .permissions
            .get_mut(transfers[index].0);
        event.claim_identity = replacement.1;
        event.provenance = replacement.2;
    }
    // An attacker can regenerate producer receipts after corrupting operands;
    // independent full replay must still compare them with the live places.
    publish_conditional_claim_joins(&checked.typed, &mut checked.facts);
    assert!(
        crate::checks::multiplicity::linear_validation::validate_linear_permission_events(
            &checked.typed,
            &checked.facts
        )
        .is_err()
    );
}
