use crate::tests::front_end::checked_program_result;

fn lower(body: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let source = format!(
        "data Choice {{ case Empty; case Some(value: u32); }}
         machine choose(selected: bool, other: bool) -> bool {{
             let left: Choice = Choice::Some {{ value: 37 }};
             let right: Choice = Choice::Empty;
             {body}
         }}"
    );
    lower_program(&source)
}

fn lower_program(
    source: &str,
) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    checked_program_result(source)
}

#[test]
fn owned_selection_unreachable_mentions_do_not_move_a_source() {
    for selection in [
        "match selected { _ -> left, true -> right }",
        "match selected { true -> left, false -> left, _ -> right }",
        "match selected { true -> left, true -> right, false -> left }",
    ] {
        let checked = lower(&format!(
            "let result: Choice = {selection}; right in Choice::Some"
        ))
        .expect("unreachable source remains available");
        let ownership = &checked.facts.flow.ownership;
        let (_, receipt) = ownership.owned_selections.iter().next().expect("receipt");
        assert_eq!(
            ownership
                .selection_sources
                .span_or_empty(receipt.sources)
                .len(),
            1
        );
    }
}

#[test]
fn owned_selection_rejects_previously_moved_source() {
    assert!(lower("let saved: Choice = left; let result: Choice = match selected { true -> left, false -> right }; result in Choice::Some").is_err());
}

#[test]
fn owned_selection_inline_membership_retains_exact_subject_type_rejection() {
    let body = "let observed: bool = (match selected { true -> left, false -> right }) in Choice::Some; observed";
    // This spelling is rejected by type validation before ownership checking.
    // It does not witness the selected-destination receipt fence.
    let errors = lower(body).expect_err("inline membership retains its type rejection");
    for expected in [
        "case membership must test a value of the exact declaring data type",
        "case `Choice::Some` has a payload; construct it with a case literal",
    ] {
        assert!(
            errors.iter().any(|error| error.message == expected),
            "missing {expected:?} for {body}: {errors:#?}"
        );
    }
}

#[test]
fn owned_selection_in_call_argument_is_rejected_or_retains_a_receipt() {
    let result = lower_program(
        "data Choice { case Empty; case Some(value:u32); } machine inspect(value:Choice)->bool { value in Choice::Some } machine choose(selected:bool)->bool { let left:Choice=Choice::Some{value:37}; let right:Choice=Choice::Empty; inspect(match selected { true->left, false->right }) }",
    );
    match result {
        Ok(checked) => assert!(
            checked
                .facts
                .flow
                .ownership
                .owned_selections
                .iter()
                .next()
                .is_some(),
            "argument normalization must retain selected custody"
        ),
        Err(errors) => assert!(
            format!("{errors:?}").contains("owned match"),
            "explicit selected-custody rejection: {errors:?}"
        ),
    }
}

#[test]
fn owned_selection_replay_rejects_a_loan_made_live_past_the_edge() {
    let checked = lower(
        "let view: &Choice = &left; let result: Choice = match selected { true -> left, false -> right }; result in Choice::Some",
    )
    .expect("a closed loan admits the once-borrowed source");
    let mut facts = checked.facts.clone();
    // Reattach the closed loan to the selection edge's entry constraint set,
    // the same shape the flow builder records while the loan is still live.
    // The replay seeds both arenas and must refuse to re-derive the recorded
    // receipt under a custody the edge cannot discharge.
    let loan = facts.borrow.loans.iter().next().expect("recorded loan").0;
    let ordinal = facts
        .flow
        .ownership
        .owned_selections
        .iter()
        .next()
        .expect("receipt")
        .1
        .statement_ordinal;
    let statement_handle = facts
        .flow
        .control
        .statements
        .iter()
        .find(|(_, statement)| statement.statement_index == ordinal as usize)
        .expect("match statement fact")
        .0;
    let mut entry_constraints = facts.flow.contexts.constraint_refs.copy_span_pair(
        facts
            .flow
            .control
            .statements
            .get(statement_handle)
            .entry_constraints,
        arena::HandleSpan::empty(),
    );
    facts.flow.contexts.constraint_refs.append_to_span(
        &mut entry_constraints,
        checked_trees::FlowConstraintRef {
            kind: checked_trees::FlowConstraintKind::BorrowLoan { loan },
        },
    );
    facts
        .flow
        .control
        .statements
        .get_mut(statement_handle)
        .entry_constraints = entry_constraints;
    let errors = crate::checks::validate_linear_permission_events(&checked.typed, &facts)
        .expect_err("a live loan must not replay as closed");
    assert!(
        format!("{errors:?}").contains("selected loan-closure evidence"),
        "replay rejection: {errors:#?}"
    );
}

#[test]
fn owned_selection_keeps_the_unrecorded_borrow_fence() {
    // A call-argument borrow records no persistent loan occurrence, so the
    // ledger cannot show whether its custody closed before the edge. The
    // conservative rejection stays until call-scoped access evidence joins.
    let errors = lower_program(
        "data Choice { case Empty; case Some(value: u32); }
         machine inspect(value: &Choice) -> bool { true }
         machine choose(selected: bool) -> bool {
             let left: Choice = Choice::Some { value: 37 };
             let right: Choice = Choice::Empty;
             let seen: bool = inspect(&right);
             let result: Choice = match selected { true -> left, false -> right };
             seen && result in Choice::Some
         }",
    )
    .expect_err("an unrecorded call-argument borrow keeps the closure fence");
    assert!(
        format!("{errors:?}").contains("requires selected loan-closure evidence"),
        "{errors:#?}"
    );
}

#[test]
fn owned_selection_membership_subject_observes_without_transferring_it() {
    // `subject in Type::Case` lowers to an exact case-membership comparison
    // whose operands are a read-only subject and a symbol-stamped case
    // reference with no result type. Neither is an ownership transfer, so the
    // subject keeps its custody and stays observable after the selection.
    for declaration in ["view: &Kind", "view: Kind"] {
        let checked = lower_program(&format!(
            "data Kind {{ case Missing; case Other; }}
             data Choice {{ case Empty; case Some(value: u32); }}
             machine choose({declaration}, left: Choice, right: Choice) -> bool {{
                 let result: Choice = match view in Kind::Missing {{ true -> left, false -> right }};
                 let observed: bool = view in Kind::Other;
                 observed || result in Choice::Some
             }}"
        ))
        .expect("membership subject remains observable after the selection");
        let ownership = &checked.facts.flow.ownership;
        let (_, receipt) = ownership
            .owned_selections
            .iter()
            .next()
            .expect("selection receipt");
        assert_eq!(
            ownership
                .selection_sources
                .span_or_empty(receipt.sources)
                .len(),
            2
        );
    }
    lower_program(
        "data Kind { case Missing; case Other; }
         data Choice { case Empty; case Some(value: u32); }
         machine Kind::choose(&self, left: Choice, right: Choice) -> bool {
             let result: Choice = match self in Kind::Missing { true -> left, false -> right };
             self in Kind::Other || result in Choice::Some
         }",
    )
    .expect("self membership subject remains observable after the selection");
    // The affine local subject is only read by the guard, so a later
    // statement can still move its whole owned custody.
    lower_program(
        "data Kind { case Missing; case Other; }
         data Choice { case Empty; case Some(value: u32); }
         machine choose(flag: bool, left: Choice, right: Choice) -> bool {
             let kind: Kind = Kind::Missing;
             let result: Choice = match kind in Kind::Missing { true -> left, false -> right };
             let moved: Kind = kind;
             moved in Kind::Missing || result in Choice::Some
         }",
    )
    .expect("observed affine local remains movable after the selection");
}

fn lower_projection_program() -> checked_trees::CheckedTrees {
    lower_program(
        "data Payload { left:u64; right:u64; }
         data Pair { first:Payload; second:Payload; }
         machine supply(first:u64, second:u64) -> Pair {
             Pair { first: Payload { left:first, right:second }, second: Payload { left:second, right:first } }
         }
         machine choose(selected:u64, first:u64, second:u64) -> u64 {
             let pair: Pair = Pair { first: Payload { left:first, right:second }, second: Payload { left:second, right:first } };
             let result: Payload = match selected { 0 -> pair.first, _ -> supply(first, second).second };
             result.left ^ result.right
         }",
    )
    .expect("projected selection checks")
}

#[test]
fn owned_selection_record_fields_reject_overlapping_moved_children() {
    // Two fields of one record arm cannot name the same projected place: the
    // arm's edge would consume `a.first` twice.
    let errors = lower_program(
        "data Payload { left:u64; right:u64; }
         data Pair { first:Payload; second:Payload; }
         machine choose(selected:bool, a:Pair, b:Pair) -> u64 {
             let result: Pair = match selected {
                 true -> Pair { first: a.first, second: a.first },
                 false -> a
             };
             result.first.left ^ result.second.right
         }",
    )
    .expect_err("overlapping moved children on one arm reject");
    assert!(format!("{errors:?}").contains("disjoint moved places"));
}

#[test]
fn owned_selection_projected_replay_rejects_mutated_paths_and_sources() {
    let checked = lower_projection_program();
    for mutation in 0..5 {
        let mut facts = checked.facts.clone();
        match mutation {
            // Rewrite the recorded moved path below the local source.
            0 => {
                let ownership = &mut facts.flow.ownership;
                let path = ownership
                    .selection_transfers
                    .iter()
                    .map(|(_, transfer)| transfer)
                    .find(|transfer| transfer.source.is_valid())
                    .expect("local transfer")
                    .path;
                *ownership.segments.get_mut(path.start()) =
                    facts::PlaceSegment::FixedIndex { index: 0 };
            }
            // A call-product transfer must not gain a roster source.
            1 => {
                let ownership = &mut facts.flow.ownership;
                let source = ownership.selection_sources.iter().next().expect("source").0;
                let handle = ownership
                    .selection_transfers
                    .iter()
                    .find(|(_, transfer)| !transfer.source.is_valid())
                    .expect("product transfer")
                    .0;
                ownership.selection_transfers.get_mut(handle).source = source;
            }
            // The recorded leaf occurrence is part of the transfer identity.
            2 => {
                let ownership = &mut facts.flow.ownership;
                let mut handles = ownership
                    .selection_transfers
                    .iter()
                    .map(|(handle, _)| handle);
                let first = handles.next().expect("first transfer");
                let second = handles.next().expect("second transfer");
                let expression = ownership.selection_transfers.get(second).expression;
                ownership.selection_transfers.get_mut(first).expression = expression;
            }
            // Source claim identity is authoritative replay evidence.
            3 => {
                let ownership = &mut facts.flow.ownership;
                let receipt = ownership.owned_selections.iter().next().expect("receipt").0;
                let (machine, state) = {
                    let receipt = ownership.owned_selections.get(receipt);
                    (receipt.machine, receipt.state)
                };
                let handle = ownership.selection_sources.iter().next().expect("source").0;
                ownership.selection_sources.get_mut(handle).claim_identity =
                    language_semantics::PermissionClaimIdentity::Established {
                        machine_symbol: machine,
                        state_symbol: state,
                        source: language_semantics::PermissionEventSource::StateEntry,
                        ordinal: u32::MAX,
                    };
            }
            // The receipt's projected leaf type is exact.
            _ => {
                let handle = facts
                    .flow
                    .ownership
                    .owned_selections
                    .iter()
                    .next()
                    .expect("receipt")
                    .0;
                facts
                    .flow
                    .ownership
                    .owned_selections
                    .get_mut(handle)
                    .type_reference = Default::default();
            }
        }
        let result = crate::checks::validate_linear_permission_events(&checked.typed, &facts);
        assert!(result.is_err(), "mutation {mutation} accepted");
        let errors = result.unwrap_err();
        assert!(
            format!("{errors:?}").contains("owned selection receipts differ"),
            "mutation {mutation}: {errors:?}"
        );
    }
}

fn lower_linear_program(
    source: &str,
) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    lower_program(&format!(
        "pub data Token [linear] {{ id: u64; }}
         boundary machine Token::settle(self) ensures true;
         {source}"
    ))
}

#[test]
fn linear_owned_selection_moves_one_whole_claim_on_every_edge() {
    for (name, body) in [
        (
            "parameter source",
            "machine choose(selected: bool, a: Token) -> u64 {
                 let picked: Token = match selected { true -> a, false -> a };
                 picked.settle();
                 0
             }",
        ),
        (
            "local source",
            "machine choose(selected: bool) -> u64 {
                 let a: Token = Token { id: 1 };
                 let picked: Token = match selected { true -> a, false -> a };
                 picked.settle();
                 0
             }",
        ),
    ] {
        let checked =
            lower_linear_program(body).unwrap_or_else(|errors| panic!("{name}: {errors:#?}"));
        let ownership = &checked.facts.flow.ownership;
        let (_, receipt) = ownership
            .owned_selections
            .iter()
            .next()
            .expect("linear selection receipt");
        assert_eq!(
            ownership
                .selection_sources
                .span_or_empty(receipt.sources)
                .len(),
            1,
            "{name}: one source joins both edges"
        );
        let transfers = ownership
            .selection_transfers
            .span_or_empty(receipt.transfers);
        assert_eq!(transfers.len(), 2, "{name}");
        assert!(
            transfers.iter().all(|transfer| transfer.source.is_valid()
                && ownership.segments.span_or_empty(transfer.path).is_empty()),
            "{name}: each edge moves the same whole-root claim: {transfers:?}"
        );
    }
}

#[test]
fn linear_owned_selection_rejects_a_nonuniform_frontier() {
    for (name, source) in [
        (
            "distinct sources",
            "machine choose(selected: bool, a: Token, b: Token) -> u64 {
                 let picked: Token = match selected { true -> a, false -> b };
                 picked.settle();
                 0
             }",
        ),
        (
            "fresh arm beside a source",
            "machine choose(selected: bool, a: Token) -> u64 {
                 let picked: Token = match selected { true -> a, false -> Token { id: 2 } };
                 picked.settle();
                 0
             }",
        ),
        (
            "source only on nested edges",
            "machine choose(selected: bool, other: bool, a: Token) -> u64 {
                 let picked: Token = match selected {
                     true -> match other { true -> a, false -> a },
                     false -> Token { id: 2 }
                 };
                 picked.settle();
                 0
             }",
        ),
    ] {
        let errors = lower_linear_program(source)
            .expect_err(&format!("{name}: a partial frontier cannot join"));
        assert!(
            format!("{errors:?}").contains(
                "linear match custody requires every reachable arm to move the same live source place"
            ),
            "{name}: {errors:#?}"
        );
    }
}

#[test]
fn linear_owned_selection_rejects_reusing_the_consumed_claim() {
    for (name, source, expected) in [
        // A whole-source receiver use is not itself a transfer, so the
        // selection-use fence reports the consumed claim directly.
        (
            "whole source receiver use",
            "machine choose(selected: bool, a: Token) -> u64 {
                 let picked: Token = match selected { true -> a, false -> a };
                 a.settle();
                 picked.settle();
                 0
             }",
            "linear match transfer consumed this exact claim on every edge",
        ),
        (
            "projected child receiver use",
            "data Pair { first: Token; second: Token; }
             machine choose(selected: bool, pair: Pair) -> u64 {
                 let picked: Token = match selected { true -> pair.first, false -> pair.first };
                 pair.first.settle();
                 Token::settle(pair.second);
                 picked.settle();
                 0
             }",
            "linear match transfer consumed this exact claim on every edge",
        ),
        // An argument move of the dead claim hits the general
        // moved-place fence before the selection-use fence can run.
        (
            "projected child move",
            "data Pair { first: Token; second: Token; }
             machine choose(selected: bool, pair: Pair) -> u64 {
                 let picked: Token = match selected { true -> pair.first, false -> pair.first };
                 Token::settle(pair.first);
                 Token::settle(pair.second);
                 picked.settle();
                 0
             }",
            "was already transferred or consumed",
        ),
    ] {
        let errors = lower_linear_program(source)
            .expect_err(&format!("{name}: the joined claim is dead on every edge"));
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{name}: expected {expected:?}: {errors:#?}"
        );
    }
}

#[test]
fn linear_owned_selection_all_fresh_edges_need_no_tracked_source() {
    let checked = lower_linear_program(
        "machine choose(selected: bool) -> u64 {
             let picked: Token = match selected { true -> Token { id: 1 }, false -> Token { id: 2 } };
             picked.settle();
             0
         }",
    )
    .expect("fresh per-edge products join vacuously");
    assert_eq!(
        checked.facts.flow.ownership.owned_selections.iter().count(),
        0,
        "fresh edges record no ownership join"
    );
}

#[test]
fn linear_owned_selection_return_maps_the_result_to_the_source_claim() {
    let checked = lower_linear_program(
        "machine choose(selected: bool, a: Token) -> Token {
             match selected { true -> a, false -> a }
         }",
    )
    .expect("a linear return selection checks");
    let ownership = &checked.facts.flow.ownership;
    let (_, receipt) = ownership
        .owned_selections
        .iter()
        .next()
        .expect("linear selection receipt");
    assert!(
        !receipt.destination.is_valid(),
        "a return destination is anonymous"
    );
    let (_, map) = ownership
        .claim_outcome_maps
        .iter()
        .next()
        .expect("a linear result publishes its claim outcome map");
    let entries = ownership.claim_outcome_entries.span_or_empty(map.entries);
    assert!(
        entries.iter().any(|entry| {
            entry.output_segments.is_empty()
                && matches!(
                    entry.source,
                    checked_trees::FlowClaimOutcomeSource::Input { .. }
                )
        }),
        "the selected result maps to the caller's parameter claim: {entries:?}"
    );
}

/// A carrier whose frontier child was already consumed cannot join: the whole
/// move would discharge a dead claim.
#[test]
fn carrier_owned_selection_rejects_a_source_with_a_dead_child() {
    let errors = lower_linear_program(
        "data Holder { left: Token; right: Token; }
         machine choose(selected: bool, x: Holder) -> u64 {
             Token::settle(x.left);
             let picked: Holder = match selected { true -> x, false -> x };
             Token::settle(picked.right);
             0
         }",
    )
    .expect_err("a half-consumed carrier cannot move whole");
    assert!(
        format!("{errors:?}").contains("owned match source must be an available"),
        "{errors:#?}"
    );
}

/// The recorded claim set is part of the replay contract: mutating a claim's
/// path, identity, provenance, or the claim count makes the independently
/// re-derived receipt disagree.
#[test]
fn carrier_owned_selection_replay_rejects_mutated_claim_evidence() {
    let checked = lower_linear_program(
        "data Holder { left: Token; right: Token; }
         machine choose(selected: bool, x: Holder) -> u64 {
             let picked: Holder = match selected { true -> x, false -> x };
             Token::settle(picked.left);
             Token::settle(picked.right);
             0
         }",
    )
    .expect("carrier selection checks");
    for mutation in 0..4 {
        let mut facts = checked.facts.clone();
        let ownership = &mut facts.flow.ownership;
        let (_, receipt) = ownership.owned_selections.iter().next().expect("receipt");
        let receipt = receipt.clone();
        let transfer_handle = receipt.transfers.start();
        let claims_span = ownership.selection_transfers.get(transfer_handle).claims;
        match mutation {
            // A claim's consumed path is exact source-place identity.
            0 => {
                let claim_path = ownership
                    .selection_transfer_claims
                    .get(claims_span.start())
                    .path;
                *ownership.segments.get_mut(claim_path.start()) =
                    facts::PlaceSegment::FixedIndex { index: 0 };
            }
            // The consumed place's minted claim identity rides the row.
            1 => {
                ownership
                    .selection_transfer_claims
                    .get_mut(claims_span.start())
                    .claim_identity = language_semantics::PermissionClaimIdentity::Unknown;
            }
            // The consumed place's establishment provenance rides the row.
            2 => {
                ownership
                    .selection_transfer_claims
                    .get_mut(claims_span.start())
                    .provenance = language_semantics::PermissionProvenance::Unknown;
            }
            // Dropping a claim silently keeps one child live to scope exit.
            _ => {
                ownership
                    .selection_transfers
                    .get_mut(transfer_handle)
                    .claims = arena::HandleSpan::empty();
            }
        }
        let errors = crate::checks::validate_linear_permission_events(&checked.typed, &facts)
            .expect_err(&format!("mutation {mutation} accepted"));
        assert!(
            format!("{errors:?}").contains("owned selection receipts differ"),
            "mutation {mutation}: {errors:?}"
        );
    }
}
