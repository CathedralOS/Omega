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
fn owned_selection_admits_fresh_arms_and_keeps_the_borrowed_custody_fence() {
    lower("let result: Choice = match selected { true -> left, false -> Choice::Empty }; result in Choice::Some")
        .expect("a fresh exact-type arm joins an existing source");
    assert!(lower("let view: &Choice = &left; let result: Choice = match selected { true -> left, false -> right }; result in Choice::Some").is_err());
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
