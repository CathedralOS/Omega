use super::checked;
use crate::lower_typed_trees;
use crate::tests::{
    Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve,
};

const OPTIONAL_RETURN: &str = r#"
    data Receipt [linear] { code: i32; }
    machine Receipt::ack(self) {}
    data Slot { case Empty; case Held(receipt: Receipt); }
    data Holder { slot: Slot; }
    machine empty() -> Holder { Holder { slot: Slot::Empty } }
    machine close(slot: Slot) {
        transition slot {
            Slot::Empty -> {}
            Slot::Held { receipt } -> consume(receipt)
        }
        state consume(receipt: Receipt) { Receipt::ack(receipt); }
    }
    machine run() {
        let holder: Holder = empty();
        BODY
    }
"#;

#[test]
fn inactive_call_payload_moves_with_its_carrier_without_a_transfer() {
    let checked = checked(&OPTIONAL_RETURN.replace(
        "BODY",
        "let rebuilt: Holder = Holder { slot: holder.slot }; close(rebuilt.slot);",
    ));
    crate::checks::validate_linear_permission_events(&checked.typed, &checked.facts)
        .expect("a statically absent payload supplies no move event or debt");
}

#[test]
fn inactive_call_payload_reconciliation_does_not_waive_real_moves() {
    let held = OPTIONAL_RETURN.replace(
        "machine empty() -> Holder { Holder { slot: Slot::Empty } }",
        "machine empty() -> Holder { let receipt: Receipt = Receipt { code: 3 }; Holder { slot: Slot::Held { receipt: receipt } } }",
    );
    checked(&held.replace("BODY", "close(holder.slot);"));
    for source in [
        OPTIONAL_RETURN.replace(
            "BODY",
            "let receipt: Receipt = holder.slot.receipt; Receipt::ack(receipt);",
        ),
        held.replace(
                "BODY",
                "let first: Slot = holder.slot; let second: Slot = holder.slot; close(first); close(second);",
            ),
    ] {
        let tokens = Lexer::new(&source).tokenize().expect("tokenize ownership control");
        let syntax = parse_syntax_trees(&tokens).expect("parse ownership control");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve ownership control");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type ownership control");
        let diagnostics = match lower_typed_trees(typed) {
            Err(diagnostics) => diagnostics,
            Ok(_) => panic!("invalid ownership control was accepted:\n{source}"),
        };
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("already transferred")
            || diagnostic.message.contains("not been established")), "{diagnostics:#?}");
    }
}

#[test]
fn permission_producer_reconstructs_transfers_from_typed_flow() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Main {}
        machine Main::run() -> i32 {
            let issued: Receipt = Receipt { code: 7 };
            let forwarded: Receipt = issued;
            Receipt::ack(forwarded);
            0
        }
        "#,
    );
    let expected = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event.clone())
        .collect::<Vec<_>>();

    let mut facts = checked.facts.clone();
    facts.flow.ownership.permissions = Default::default();
    crate::checks::record_permission_events(&checked.typed, &mut facts);
    let actual = facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event.clone())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

#[test]
fn permission_producer_reconstructs_affine_cleanup_from_typed_flow() {
    let checked = checked(
        r#"
        data Box { value: i32; }
        data Main {}
        machine Main::run() -> i32 { 0 }
        machine Main::consume(input: Box) -> i32 {
            let first: Box = Box { value: 1 };
            let second: Box = Box { value: 2 };
            0
        }
        "#,
    );
    let expected = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.kind == language_semantics::PermissionEventKind::AffineDrop)
                .then_some(event.root)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        expected.len(),
        3,
        "two locals and one owned parameter clean up"
    );

    let mut facts = checked.facts.clone();
    facts.flow.ownership.permissions = Default::default();
    crate::checks::record_permission_events(&checked.typed, &mut facts);
    let actual = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.kind == language_semantics::PermissionEventKind::AffineDrop)
                .then_some(event.root)
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

#[test]
fn nested_conditional_payload_extraction_preserves_its_origin() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data ReceiptState {
            case Empty;
            case Live(receipt: Receipt);
        }
        data Main {}
        machine Main::run() -> i32 {
            let issued: Receipt = Receipt { code: 7 };
            let state: ReceiptState = ReceiptState::Live { receipt: issued };
            let extracted: Receipt = state.receipt;
            Receipt::ack(extracted);
            0
        }
        "#,
    );

    use language_semantics::{PermissionAccess, PermissionEventKind};
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| event.access == PermissionAccess::Owned)
        .collect::<Vec<_>>();
    assert_eq!(
        events.iter().map(|event| event.kind).collect::<Vec<_>>(),
        [
            PermissionEventKind::Establish,
            PermissionEventKind::Transfer,
            PermissionEventKind::Establish,
            PermissionEventKind::Transfer,
            PermissionEventKind::Establish,
            PermissionEventKind::Consume,
        ]
    );
    assert!(
        events
            .iter()
            .all(|event| event.provenance == events[0].provenance),
        "nested transfers must conserve one origin: {events:#?}"
    );
}

#[test]
fn generic_conditional_payload_substitution_preserves_linear_debt() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Outcome<T> {
            case Empty;
            case Returned(value: T);
        }
        data Main {}
        machine Main::run() -> i32 {
            let issued: Receipt = Receipt { code: 7 };
            let outcome: Outcome<Receipt> = Outcome::Returned { value: issued };
            let extracted: Receipt = outcome.value;
            Receipt::ack(extracted);
            0
        }
        "#,
    );

    use language_semantics::{PermissionAccess, PermissionEventKind};
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| event.access == PermissionAccess::Owned)
        .collect::<Vec<_>>();
    assert_eq!(
        events.iter().map(|event| event.kind).collect::<Vec<_>>(),
        [
            PermissionEventKind::Establish,
            PermissionEventKind::Transfer,
            PermissionEventKind::Establish,
            PermissionEventKind::Transfer,
            PermissionEventKind::Establish,
            PermissionEventKind::Consume,
        ]
    );
    assert!(events.iter().all(|event| event.obligation_live));
    assert!(
        events
            .iter()
            .all(|event| event.provenance == events[0].provenance),
        "generic substitution must conserve the payload origin: {events:#?}"
    );
}

#[test]
fn generic_conditional_empty_case_establishes_without_debt() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        data Outcome<T> {
            case Empty;
            case Returned(value: T);
        }
        data Main {}
        machine Main::run() -> i32 {
            let outcome: Outcome<Receipt> = Outcome::Empty;
            0
        }
        "#,
    );
    let event = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .find(|event| event.kind == language_semantics::PermissionEventKind::Establish)
        .expect("generic conditional establishment event");
    assert!(!event.obligation_live);
}

#[test]
fn active_case_frontier_preserves_independent_payload_claims() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data ReceiptState {
            case Empty;
            case Pair(left: Receipt, right: Receipt);
            case Single(value: Receipt);
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let state: ReceiptState = ReceiptState::Pair {
                left: left,
                right: right,
            };
            let extracted_left: Receipt = state.left;
            let extracted_right: Receipt = state.right;
            Receipt::ack(extracted_left);
            Receipt::ack(extracted_right);
            0
        }
        "#,
    );

    use language_semantics::{PermissionAccess, PermissionEventKind};
    let state_symbol = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .flat_map(|state| checked.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "state" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("state local");
    let establishments = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.access == PermissionAccess::Owned
                && event.kind == PermissionEventKind::Establish
                && event.obligation_live
                && event.root == facts::PlaceRoot::Symbol(state_symbol)
        })
        .collect::<Vec<_>>();
    assert_eq!(establishments.len(), 2);
    assert!(establishments.iter().all(|event| {
        matches!(
            checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments),
            [
                facts::PlaceSegment::Case { .. },
                facts::PlaceSegment::Field { .. }
            ]
        )
    }));
    assert_ne!(
        establishments[0].claim_identity,
        establishments[1].claim_identity
    );
}

#[test]
fn active_case_partial_move_leaves_same_case_sibling_live() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data ReceiptState {
            case Empty;
            case Pair(left: Receipt, right: Receipt);
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let state: ReceiptState = ReceiptState::Pair {
                left: left,
                right: right,
            };
            let extracted: Receipt = state.left;
            Receipt::ack(extracted);
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("the active-case sibling remains live");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `state::Pair.right` reaches scope exit")
    }));
}

#[test]
fn active_case_rejects_duplicate_payload_move() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data ReceiptState {
            case Empty;
            case Live(receipt: Receipt);
        }
        data Main {}
        machine Main::run() -> i32 {
            let receipt: Receipt = Receipt { code: 1 };
            let state: ReceiptState = ReceiptState::Live { receipt: receipt };
            let first: Receipt = state.receipt;
            let duplicate: Receipt = state.receipt;
            Receipt::ack(first);
            Receipt::ack(duplicate);
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("one payload claim cannot move twice");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `state::Live.receipt` was already transferred")
    }));
}

#[test]
fn active_case_result_map_omits_proven_inactive_alternatives() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data ReceiptState {
            case Empty;
            case Pair(left: Receipt, right: Receipt);
            case Single(value: Receipt);
        }
        data Main {}
        machine Main::pack(receipt: Receipt) -> ReceiptState {
            ReceiptState::Single { value: receipt }
        }
        machine Main::run() -> i32 {
            let receipt: Receipt = Receipt { code: 1 };
            let state: ReceiptState = Main::pack(receipt);
            let extracted: Receipt = state.value;
            Receipt::ack(extracted);
            0
        }
        "#,
    );

    let pack_symbol = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .find(|state| state.name.as_str() == "pack")
        .map(|state| state.symbol)
        .expect("pack state");
    let map = checked
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .map(|(_, map)| map)
        .find(|map| map.state_symbol == pack_symbol)
        .expect("active-case outcome map");
    let entries = checked
        .facts
        .flow
        .ownership
        .claim_outcome_entries
        .span_or_empty(map.entries);
    assert_eq!(entries.len(), 1);
    assert!(matches!(
        checked
            .facts
            .flow
            .ownership
            .segments
            .span_or_empty(entries[0].output_segments),
        [
            facts::PlaceSegment::Case { .. },
            facts::PlaceSegment::Field { .. }
        ]
    ));
}
