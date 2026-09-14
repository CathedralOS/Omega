use super::*;

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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed)
}

#[test]
fn owned_selection_direct_return_has_anonymous_destination_and_exact_type() {
    let checked = lower_program("data Choice { case Empty; case Some(value:u64); } machine choose(selected:bool,first:u64,second:u64)->Choice { let left:Choice=Choice::Some{value:first}; let right:Choice=Choice::Some{value:second}; match selected {true->left,false->right} }")
        .expect("direct structural return checks");
    let (_, receipt) = checked
        .facts
        .flow
        .ownership
        .owned_selections
        .iter()
        .next()
        .expect("receipt");
    assert!(!receipt.destination.is_valid());
    assert!(receipt.type_reference.is_valid());
    assert_eq!(receipt.statement_ordinal, 2);
}

#[test]
fn owned_selection_does_not_route_fresh_case_names_as_storage() {
    let checked = lower("let result: Choice = match selected { true -> Choice::Empty, false -> Choice::Some { value: 9 } }; result in Choice::Some")
        .expect("fresh selection remains supported");
    assert_eq!(
        checked.facts.flow.ownership.owned_selections.iter().count(),
        0
    );
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
fn owned_selection_preserves_two_origins_and_reverse_residual_order() {
    let checked = lower("let result: Choice = match selected { true -> left, false -> right }; result in Choice::Some")
        .expect("selected existing owners check");
    let ownership = &checked.facts.flow.ownership;
    let (_, receipt) = ownership
        .owned_selections
        .iter()
        .next()
        .expect("selection receipt");
    let sources = ownership.selection_sources.span_or_empty(receipt.sources);
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0].statement_ordinal, 1);
    assert_eq!(sources[1].statement_ordinal, 0);
    assert_ne!(sources[0].provenance, sources[1].provenance);
    assert!(sources.iter().all(|source| matches!(
        source.provenance,
        language_semantics::PermissionProvenance::Established { .. }
    )));
    assert_eq!(
        receipt.death,
        language_semantics::PermissionEventSource::StateExit
    );
    let transfers = ownership
        .selection_transfers
        .span_or_empty(receipt.transfers);
    assert_eq!(transfers.len(), 2);
    assert_ne!(transfers[0].source_arm, transfers[1].source_arm);
    assert!(
        ownership.permissions.iter().all(|(_, event)| {
            event.kind != language_semantics::PermissionEventKind::Transfer
                || !sources
                    .iter()
                    .any(|source| event.root == facts::PlaceRoot::Symbol(source.symbol))
        }),
        "conditional alternatives are not unconditional transfers"
    );
}

#[test]
fn owned_selection_same_source_in_exclusive_arms_is_one_owner() {
    let checked = lower("let result: Choice = match selected { true -> left, false -> left }; result in Choice::Some")
        .expect("exclusive reuse checks");
    let ownership = &checked.facts.flow.ownership;
    let (_, receipt) = ownership.owned_selections.iter().next().expect("receipt");
    assert_eq!(
        ownership
            .selection_sources
            .span_or_empty(receipt.sources)
            .len(),
        1
    );
    let transfers = ownership
        .selection_transfers
        .span_or_empty(receipt.transfers);
    assert_eq!(transfers.len(), 2);
    assert_eq!(transfers[0].source, transfers[1].source);
}

#[test]
fn owned_selection_rejects_possibly_moved_transfer_and_observation() {
    for continuation in [
        "let again: Choice = left; result in Choice::Some",
        "left in Choice::Some",
    ] {
        let errors = lower(&format!(
            "let result: Choice = match selected {{ true -> left, false -> right }}; {continuation}"
        ))
        .expect_err("possibly moved source rejects");
        assert!(format!("{errors:?}").contains("may have been transferred"));
    }
}

#[test]
fn owned_selection_nested_arms_retain_occurrences_without_path_products() {
    let checked = lower("let result: Choice = match selected { true -> match other { true -> left, false -> right }, false -> left }; result in Choice::Some")
        .expect("nested selection checks");
    let ownership = &checked.facts.flow.ownership;
    assert_eq!(ownership.owned_selections.iter().count(), 1);
    let (_, receipt) = ownership.owned_selections.iter().next().expect("receipt");
    assert_eq!(
        ownership
            .selection_sources
            .span_or_empty(receipt.sources)
            .len(),
        2
    );
    assert_eq!(
        ownership
            .selection_transfers
            .span_or_empty(receipt.transfers)
            .len(),
        3
    );
}

#[test]
fn owned_selection_forwarding_references_prior_origins() {
    let checked = lower("let first: Choice = match selected { true -> left, false -> right }; let result: Choice = match other { true -> first, false -> first }; result in Choice::Some")
        .expect("selected result can transfer onward");
    let ownership = &checked.facts.flow.ownership;
    let receipts = ownership.owned_selections.iter().collect::<Vec<_>>();
    assert_eq!(receipts.len(), 2);
    let sources = ownership
        .selection_sources
        .span_or_empty(receipts[1].1.sources);
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].origin_selection, receipts[0].0);
    assert_eq!(
        sources[0].provenance,
        language_semantics::PermissionProvenance::Unknown
    );
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
fn owned_selection_inside_aggregate_requires_its_own_destination_receipt() {
    let errors = lower_program("data Choice { case Empty; case Some(value:u32); } data Wrapper { value:Choice; } machine choose(selected:bool)->Wrapper { let left:Choice=Choice::Some{value:37}; let right:Choice=Choice::Empty; Wrapper { value: match selected { true->left, false->right } } }")
        .expect_err("aggregate wrapper cannot hide selected ownership");
    assert!(format!("{errors:?}").contains("owned match requires"));
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
fn owned_selection_admits_fresh_arms_and_closed_loan_sources() {
    lower("let result: Choice = match selected { true -> left, false -> Choice::Empty }; result in Choice::Some")
        .expect("a fresh exact-type arm joins an existing source");
    for body in [
        // The stored view is never used again: its loan expired before the
        // selection edge, so the source's owned custody is intact.
        "let view: &Choice = &left; let result: Choice = match selected { true -> left, false -> right }; result in Choice::Some",
        // The loan's last use is a real read before the selection edge.
        "let view: &Choice = &left; let seen: bool = view in Choice::Some; let result: Choice = match selected { true -> left, false -> right }; seen && result in Choice::Some",
    ] {
        lower(body).expect("closed loans admit the once-borrowed source");
    }
}

#[test]
fn owned_selection_keeps_the_live_loan_custody_fence() {
    // `view` is still read after the match, so its loan on `left` is live at
    // the selection edge and the conditional move stays rejected.
    let errors = lower(
        "let view: &Choice = &left; let result: Choice = match selected { true -> left, false -> right }; view in Choice::Some",
    )
    .expect_err("a live loan keeps the borrowed-custody fence");
    assert!(
        format!("{errors:?}").contains("requires selected loan-closure evidence"),
        "{errors:#?}"
    );
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
fn owned_selection_projected_children_record_exact_paths_and_roots() {
    let checked = lower_projection_program();
    let ownership = &checked.facts.flow.ownership;
    let (_, receipt) = ownership
        .owned_selections
        .iter()
        .next()
        .expect("selection receipt");
    let transfers = ownership
        .selection_transfers
        .span_or_empty(receipt.transfers);
    assert_eq!(transfers.len(), 2);
    // `pair.first` moves through its local roster source; the call product
    // carries no roster entry and is identified by its authored root instead.
    let local = transfers
        .iter()
        .find(|transfer| transfer.source.is_valid())
        .expect("local source transfer");
    let product = transfers
        .iter()
        .find(|transfer| !transfer.source.is_valid())
        .expect("call product transfer");
    let local_path = ownership.segments.span_or_empty(local.path);
    let product_path = ownership.segments.span_or_empty(product.path);
    assert!(
        matches!(local_path, [facts::PlaceSegment::Field { .. }])
            && matches!(product_path, [facts::PlaceSegment::Field { .. }])
            && local_path != product_path,
        "each transfer records its own exact field path: {local_path:?} {product_path:?}"
    );
    assert_eq!(
        ownership
            .selection_sources
            .span_or_empty(receipt.sources)
            .len(),
        1,
        "only the whole-local root joins the candidate roster"
    );
    // Each arm's structural value is a Projection over its exact root, and the
    // normalized leaf identity is the receipt's result type.
    let values = &checked.facts.values.structural_values;
    let root = values
        .root_at(receipt.state, receipt.statement_ordinal)
        .expect("structural result root");
    let checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } =
        &values.nodes.get(root.root).kind
    else {
        panic!("selected result is a structural dispatch")
    };
    let mut sources = Vec::new();
    for arm in values.dispatch_arms.span(*arms).expect("arm span") {
        let checked_trees::CheckedStructuralValueKind::Projection {
            source,
            path,
            type_identity,
        } = &values.nodes.get(arm.value).kind
        else {
            panic!("each selected arm projects one affine child")
        };
        assert_eq!(path.len(), 1);
        assert_eq!(
            type_identity.as_str(),
            checked
                .normalized_type_identity(receipt.type_reference)
                .as_str()
        );
        sources.push(*source);
    }
    assert!(matches!(
        values.nodes.get(sources[0]).kind,
        checked_trees::CheckedStructuralValueKind::Place(_)
    ));
    assert!(matches!(
        values.nodes.get(sources[1]).kind,
        checked_trees::CheckedStructuralValueKind::Call { .. }
    ));
}

#[test]
fn owned_selection_rejects_reusing_a_partially_moved_source() {
    let errors = lower_program(
        "data Payload { left:u64; right:u64; }
         data Pair { first:Payload; second:Payload; }
         machine choose(selected:u64, first:u64, second:u64) -> u64 {
             let pair: Pair = Pair { first: Payload { left:first, right:second }, second: Payload { left:second, right:first } };
             let result: Payload = match selected { 0 -> pair.first, _ -> pair.second };
             let again: Pair = pair;
             result.left
         }",
    )
    .expect_err("a possibly projected source cannot move again");
    assert!(format!("{errors:?}").contains("may have been transferred"));
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

#[test]
fn owned_selection_replay_rejects_changed_origin_arm_and_death() {
    let checked = lower("let result: Choice = match selected { true -> left, false -> right }; result in Choice::Some")
        .expect("source checks");
    for mutation in 0..3 {
        let mut facts = checked.facts.clone();
        match mutation {
            0 => {
                let handle = facts
                    .flow
                    .ownership
                    .selection_sources
                    .iter()
                    .next()
                    .expect("source")
                    .0;
                facts
                    .flow
                    .ownership
                    .selection_sources
                    .get_mut(handle)
                    .provenance = language_semantics::PermissionProvenance::Unknown;
            }
            1 => {
                let handle = facts
                    .flow
                    .ownership
                    .selection_transfers
                    .iter()
                    .next()
                    .expect("transfer")
                    .0;
                facts
                    .flow
                    .ownership
                    .selection_transfers
                    .get_mut(handle)
                    .source_arm = arena::Handle::invalid();
            }
            _ => {
                let handle = facts
                    .flow
                    .ownership
                    .owned_selections
                    .iter()
                    .next()
                    .expect("receipt")
                    .0;
                facts.flow.ownership.owned_selections.get_mut(handle).death =
                    language_semantics::PermissionEventSource::Statement { statement_index: 2 };
            }
        }
        let errors = crate::checks::validate_linear_permission_events(&checked.typed, &facts)
            .expect_err("changed custody rejects");
        assert!(format!("{errors:?}").contains("owned selection receipts differ"));
    }
}
