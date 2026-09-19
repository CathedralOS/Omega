use super::{publish_conditional_claim_joins, validate_conditional_claim_joins};
use crate::checks::multiplicity::linear_validation::permission_event_statement_index;
use arena::HandleSpan;
use checked_trees::{FlowClaimJoinAlternativeSource, FlowClaimJoinExitKind};
use diagnostics::Diagnostic;
use facts::PlaceRoot;
use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

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
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolved");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typed");
    crate::lower_typed_trees(typed)
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
fn conditional_result_join_retains_each_exact_named_exit() {
    let checked = lower(CHOICE).expect("conditional result is conserved");
    let ownership = &checked.facts.flow.ownership;
    let receipts = ownership.claim_join_receipts.iter().collect::<Vec<_>>();
    assert_eq!(receipts.len(), 1);
    let alternatives = ownership
        .claim_join_alternatives
        .span_or_empty(receipts[0].1.alternatives);
    assert_eq!(alternatives.len(), 2);
    assert!(alternatives.iter().all(|alternative| {
        ownership
            .claim_join_exits
            .span_or_empty(alternative.exits)
            .len()
            == 2
    }));
    assert_ne!(alternatives[0].source, alternatives[1].source);
}

#[test]
fn conditional_result_join_rejects_unconsumed_other_input() {
    assert!(lower(&CHOICE.replace("Receipt::ack(right);", "")).is_err());
}

#[test]
fn conditional_result_join_repeated_calls_keep_distinct_occurrences() {
    let source = CHOICE
        .replace(
            "machine consume(flag: bool, left: Receipt, right: Receipt)",
            "machine consume(flag: bool, left: Receipt, right: Receipt, third: Receipt)",
        )
        .replace(
            "Receipt::ack(selected);",
            "let next: Receipt = choose(flag, selected, third); Receipt::ack(next);",
        );
    let checked = lower(&source).expect("returned joined custody can enter the next call");
    let receipts = checked
        .facts
        .flow
        .ownership
        .claim_join_receipts
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(receipts.len(), 2);
    assert_ne!(receipts[0].1.claim_identity, receipts[1].1.claim_identity);
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
fn conditional_result_join_binds_constructed_argument_fields() {
    let source = CHOICE.replace("machine consume", r#"
data Pair { left: Receipt; right: Receipt; }
machine choose_pair(flag: bool, pair: Pair) -> Receipt {
    transition flag {
        true -> first(pair.left, pair.right)
        _ -> second(pair.left, pair.right)
    }
    state first(left: Receipt, right: Receipt) -> Receipt { Receipt::ack(right); left }
    state second(left: Receipt, right: Receipt) -> Receipt { Receipt::ack(left); right }
}
machine consume"#).replace(
        "let selected: Receipt = choose(flag, left, right);\n    Receipt::ack(selected);",
        "let selected: Receipt = choose_pair(flag, Pair { left: left, right: right });\n    Receipt::ack(selected);",
    );
    let checked = lower(&source).expect("typed constructor paths retain actual source claims");
    assert_eq!(
        checked
            .facts
            .flow
            .ownership
            .claim_join_receipts
            .iter()
            .count(),
        1
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
fn conditional_result_join_retains_inactive_case_exit() {
    let source = r#"
data Receipt [linear] { code: i32; }
machine Receipt::ack(self) {}
data OptionalReceipt { case Empty; case Held(receipt: Receipt); }
machine choose(flag: u32, left: Receipt, right: Receipt) -> OptionalReceipt {
    transition flag {
        0 -> first(left, right)
        1 -> second(left, right)
        _ -> empty(left, right)
    }
    state first(left: Receipt, right: Receipt) -> OptionalReceipt {
        Receipt::ack(right);
        OptionalReceipt::Held { receipt: left }
    }
    state second(left: Receipt, right: Receipt) -> OptionalReceipt {
        Receipt::ack(left);
        OptionalReceipt::Held { receipt: right }
    }
    state empty(left: Receipt, right: Receipt) -> OptionalReceipt {
        Receipt::ack(left);
        Receipt::ack(right);
        OptionalReceipt::Empty
    }
}

machine consume(flag: u32, left: Receipt, right: Receipt) {
    let selected: OptionalReceipt = choose(flag, left, right);
    transition selected {
        OptionalReceipt::Empty -> done()
        OptionalReceipt::Held { receipt } -> finish(receipt)
    }
    state done() {}
    state finish(receipt: Receipt) { Receipt::ack(receipt); }
}
"#;
    let checked = lower(source).expect("an exact inactive case is not an unknown source");
    let ownership = &checked.facts.flow.ownership;
    let (_, receipt) = ownership
        .claim_join_receipts
        .iter()
        .next()
        .expect("receipt");
    let alternatives = ownership
        .claim_join_alternatives
        .span_or_empty(receipt.alternatives);
    assert_eq!(alternatives.len(), 3);
    assert_eq!(
        alternatives
            .iter()
            .filter(|alternative| matches!(
                alternative.source,
                FlowClaimJoinAlternativeSource::Inactive
            ))
            .count(),
        1
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

#[test]
fn conditional_result_join_preserves_one_active_origin_and_an_inactive_exit() {
    let source = r#"
data Receipt [linear] { code: i32; }
machine Receipt::ack(self) {}
data OptionalReceipt { case Empty; case Held(receipt: Receipt); }
data Bundle { selected: Receipt; optional: OptionalReceipt; }
machine choose(flag: bool, left: Receipt, right: Receipt) -> Bundle {
    transition flag {
        true -> first(left, right)
        _ -> second(left, right)
    }
    state first(left: Receipt, right: Receipt) -> Bundle {
        Receipt::ack(right);
        Bundle { selected: left, optional: OptionalReceipt::Empty }
    }
    state second(left: Receipt, right: Receipt) -> Bundle {
        Bundle { selected: right, optional: OptionalReceipt::Held { receipt: left } }
    }
}
machine consume(flag: bool, left: Receipt, right: Receipt) {
    let selected: Bundle = choose(flag, left, right);
    Receipt::ack(selected.selected);
    transition selected.optional {
        OptionalReceipt::Empty -> done()
        OptionalReceipt::Held { receipt } -> finish(receipt)
    }
    state done() {}
    state finish(receipt: Receipt) { Receipt::ack(receipt); }
}
"#;
    let checked = lower(source).expect("one active origin plus absent custody is a complete join");
    let ownership = &checked.facts.flow.ownership;
    assert_eq!(ownership.claim_join_receipts.iter().count(), 2);
    assert!(ownership.claim_join_receipts.iter().any(|(_, receipt)| {
        let alternatives = ownership
            .claim_join_alternatives
            .span_or_empty(receipt.alternatives);
        alternatives.len() == 2
            && alternatives
                .iter()
                .filter(|alternative| {
                    matches!(alternative.source, FlowClaimJoinAlternativeSource::Inactive)
                })
                .count()
                == 1
    }));
}
