use super::{publish_conditional_claim_joins, validate_conditional_claim_joins};
use crate::checks::multiplicity::linear_validation::permission_event_statement_index;
use crate::tests::front_end::checked_program_result;
use arena::HandleSpan;
use checked_trees::{FlowClaimJoinAlternativeSource, FlowClaimJoinExitKind};
use diagnostics::Diagnostic;
use facts::PlaceRoot;
use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};

const CHOICE: &str = r#"
data Receipt [linear] { code: i32; }
boundary requirement Receipt::ack(self);
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
boundary requirement Receipt::ack(self);
boundary requirement Outcome::ack(self);
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
fn conditional_result_join_does_not_reuse_absence_before_live_overwrite() {
    let source = OPTIONAL_INPUT.replace("    let slot: Slot = empty();",
        "    let mut slot: Slot = empty();\n    slot = Slot::Held { receipt: Receipt { code: 2 } };");
    let checked = lower(&source).expect("live overwritten case retains its actual claim");
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
        .replace("boundary requirement Receipt::ack(self);", "boundary requirement Receipt::ack(self);\nboundary requirement Slot::ack(self);")
        .replace("machine consume(fresh: Receipt)", "machine consume(fresh: Receipt, other: Receipt)")
        .replace(
            "    let slot: Slot = empty();",
            "    let slot: Slot = Slot::Held { receipt: fresh };\n    let moved: Slot = slot;\n    Slot::ack(moved);",
        )
        .replace("select(slot, fresh)", "select(slot, other)");
    assert!(lower(&source).is_err());
}

#[test]
fn conditional_result_join_rejects_unconsumed_other_input() {
    assert!(lower(&CHOICE.replace("Receipt::ack(right);", "")).is_err());
}

#[test]
fn conditional_result_join_does_not_inherit_a_single_input_identity() {
    let source = r#"
data Receipt [linear] { code: i32; }
boundary requirement Receipt::ack(self);
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

#[test]
fn conditional_result_join_full_replay_rejects_forged_local_origin() {
    let source = CHOICE.replace(
        "        left\n",
        "        let forwarded: Receipt = left;\n        forwarded\n",
    );
    let mut checked = lower(&source).expect("forwarded local keeps the exact input");
    let first = checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine))
        .find(|state| checked.typed.symbols.name(state.symbol) == "first")
        .expect("first state")
        .symbol;
    let right = checked.facts.flow.ownership.permissions.iter().find(|(_, event)|
        event.state_symbol == first && event.source == PermissionEventSource::StateEntry
            && matches!(event.root, PlaceRoot::Symbol(symbol) if checked.typed.symbols.name(symbol) == "right"))
        .map(|(_, event)| (event.claim_identity, event.provenance)).expect("right input");
    let targets = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.state_symbol == first
                && matches!(event.root, PlaceRoot::Symbol(symbol)
            if checked.typed.symbols.name(symbol) == "forwarded")
        })
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 2);
    for handle in targets {
        let event = checked.facts.flow.ownership.permissions.get_mut(handle);
        event.claim_identity = right.0;
        event.provenance = right.1;
    }
    publish_conditional_claim_joins(&checked.typed, &mut checked.facts);
    assert!(
        crate::checks::multiplicity::linear_validation::validate_linear_permission_events(
            &checked.typed,
            &checked.facts
        )
        .is_err()
    );
}

#[test]
fn conditional_result_join_replay_rejects_missing_or_changed_rows() {
    let checked = lower(CHOICE).expect("conditional result is conserved");
    for mutation in 0..5 {
        let mut ownership = checked.facts.flow.ownership.clone();
        let (receipt_handle, receipt) = ownership
            .claim_join_receipts
            .iter()
            .next()
            .expect("receipt");
        let receipt = receipt.clone();
        match mutation {
            0 => ownership.claim_join_receipts = arena::Arena::default(),
            1 => {
                ownership
                    .claim_join_receipts
                    .get_mut(receipt_handle)
                    .call_ordinal += 1
            }
            2 => {
                ownership
                    .claim_join_receipts
                    .get_mut(receipt_handle)
                    .alternatives = HandleSpan::default()
            }
            3 => {
                let (handle, _) = ownership
                    .claim_join_alternatives
                    .iter()
                    .next()
                    .expect("alternative");
                ownership.claim_join_alternatives.get_mut(handle).source =
                    FlowClaimJoinAlternativeSource::Inactive;
            }
            _ => {
                let (handle, _) = ownership.claim_join_exits.iter().next().expect("exit");
                ownership.claim_join_exits.get_mut(handle).kind =
                    FlowClaimJoinExitKind::TransitionContinuation;
            }
        }
        let mut diagnostics = Vec::new();
        validate_conditional_claim_joins(
            &checked.typed,
            &checked.facts.borrow,
            &ownership,
            &mut diagnostics,
        );
        assert!(!diagnostics.is_empty(), "mutation {mutation}: {receipt:?}");
    }
}
