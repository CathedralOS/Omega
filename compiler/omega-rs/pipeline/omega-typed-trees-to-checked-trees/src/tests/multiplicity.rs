use super::*;

fn checked(source: &str) -> omega_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

#[test]
fn retains_semantic_permission_events_beside_legacy_moves_and_drops() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Box { value: i32; }
        data Main {}

        machine Main::run() -> i32 {
            let issued: Receipt = Receipt { code: 7 };
            let forwarded: Receipt = issued;
            Receipt::ack(forwarded);
            let affine: Box = Box { value: 1 };
            0
        }
        "#,
    );

    use omega_core::semantics::PermissionEventKind as Kind;
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let kinds = events.iter().map(|event| event.kind).collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            Kind::Establish,
            Kind::Transfer,
            Kind::Establish,
            Kind::Consume,
            Kind::AffineDrop,
        ]
    );
    assert!(
        !checked.facts.flow.ownership.moves.is_empty(),
        "legacy compatibility moves remain while downstream migration is staged"
    );
    assert!(
        events
            .iter()
            .all(|event| event.access == omega_core::semantics::PermissionAccess::Owned)
    );
    assert!(
        events[..4]
            .iter()
            .all(|event| event.multiplicity == omega_core::semantics::Multiplicity::Linear)
    );
    assert_eq!(
        events[4].multiplicity,
        omega_core::semantics::Multiplicity::Affine
    );
    let origin = events[0].provenance;
    let claim_identity = events[0].claim_identity;
    assert_ne!(origin, omega_core::semantics::PermissionProvenance::Unknown);
    assert_ne!(
        claim_identity,
        omega_core::semantics::PermissionClaimIdentity::Unknown
    );
    assert!(
        events[..4].iter().all(|event| event.provenance == origin),
        "transfers preserve one obligation origin rather than minting a new one per binding"
    );
    assert!(
        events[..4]
            .iter()
            .all(|event| event.claim_identity == claim_identity),
        "transfers preserve one claim identity rather than minting a new claim per binding"
    );
    assert_eq!(
        events[4].provenance,
        omega_core::semantics::PermissionProvenance::Unknown,
        "legacy-derived affine cleanup must not invent establishment provenance"
    );
    assert_eq!(
        events[4].claim_identity,
        omega_core::semantics::PermissionClaimIdentity::Unknown,
        "no-debt affine cleanup must not invent a live claim identity"
    );
}

#[test]
fn empty_conditional_sum_records_establishment_without_payload_debt() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        data ReceiptState {
            case Empty;
            case Live(receipt: Receipt);
        }
        data Main {}
        machine Main::run() -> i32 {
            let state: ReceiptState = ReceiptState::Empty;
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
        .find(|event| event.kind == omega_core::semantics::PermissionEventKind::Establish)
        .expect("conditional establishment event");
    assert!(!event.obligation_live);
}

#[test]
fn borrow_loans_share_the_permission_context_with_access_and_origin() {
    let checked = checked(
        r#"
        data Main { items: [i32; 2]; }

        machine observe(items: &[i32]) {}
        machine mutate(items: &mut [i32]) {}
        machine Main::run() -> i32 { 0 }

        machine Main::read(&self) {
            let view: &[i32] = self.items.as_slice();
            observe(view);
        }

        machine Main::write(&mut self) {
            let view: &mut [i32] = self.items.as_mut_slice();
            mutate(view);
        }
        "#,
    );

    use omega_core::semantics::{
        Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
        PermissionProvenance,
    };
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| event.access != PermissionAccess::Owned)
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 4, "each loan has a begin and release event");

    for access in [PermissionAccess::Shared, PermissionAccess::Exclusive] {
        let pair = events
            .iter()
            .copied()
            .filter(|event| event.access == access)
            .collect::<Vec<_>>();
        assert_eq!(pair.len(), 2);
        assert_eq!(pair[0].kind, PermissionEventKind::Establish);
        assert_eq!(pair[1].kind, PermissionEventKind::Consume);
        assert_eq!(pair[0].provenance, pair[1].provenance);
        assert_eq!(pair[0].claim_identity, pair[1].claim_identity);
        assert_ne!(pair[0].provenance, PermissionProvenance::Unknown);
        assert_ne!(pair[0].claim_identity, PermissionClaimIdentity::Unknown);
    }
    assert_ne!(events[0].claim_identity, events[2].claim_identity);
    assert_eq!(
        events
            .iter()
            .find(|event| event.access == PermissionAccess::Shared)
            .expect("shared loan")
            .multiplicity,
        Multiplicity::Unrestricted
    );
    assert_eq!(
        events
            .iter()
            .find(|event| event.access == PermissionAccess::Exclusive)
            .expect("exclusive loan")
            .multiplicity,
        Multiplicity::Affine
    );
}

#[test]
fn linear_judgment_reads_permission_events_not_legacy_move_drop_arenas() {
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
    let mut facts = checked.facts.clone();
    facts.flow.ownership.moves = Default::default();
    facts.flow.ownership.drops = Default::default();
    crate::checks::validate_linear_permission_events(&checked.typed, &facts)
        .expect("semantic permission events are sufficient for the judgment");
}

#[test]
fn consuming_call_that_returns_an_obligation_transfers_its_origin() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::forward(self) -> Receipt { self }
        machine Receipt::ack(self) {}
        data Main {}
        machine Main::run() -> i32 {
            let issued: Receipt = Receipt { code: 7 };
            let returned: Receipt = Receipt::forward(issued);
            Receipt::ack(returned);
            0
        }
        "#,
    );

    use omega_core::semantics::{PermissionAccess, PermissionEventKind};
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
            PermissionEventKind::Consume,
        ]
    );
    assert!(
        events
            .iter()
            .all(|event| event.provenance == events[0].provenance)
    );
}

#[test]
fn state_call_result_preserves_a_locally_created_obligation_origin() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Main {}
        machine Main::issue(&mut self) -> Receipt {
            let issued: Receipt = Receipt { code: 7 };
            transition { _ -> issued }
        }
        machine Main::run(&mut self) -> i32 {
            let returned: Receipt = self.issue();
            Receipt::ack(returned);
            0
        }
        "#,
    );

    use omega_core::semantics::{PermissionAccess, PermissionEventKind};
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
            PermissionEventKind::Consume,
        ]
    );
    assert!(
        events
            .iter()
            .all(|event| event.provenance == events[0].provenance),
        "the state-call result must not mint a caller-side origin: {events:#?}"
    );
    assert!(
        events
            .iter()
            .all(|event| event.claim_identity == events[0].claim_identity),
        "the state-call result must preserve the callee claim identity: {events:#?}"
    );
}

#[test]
fn state_call_result_maps_multiple_claims_by_unique_output_path() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::issue(&mut self) -> Pair {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let pair: Pair = Pair { left: left, right: right };
            transition { _ -> pair }
        }
        machine Main::run(&mut self) -> i32 {
            let returned: Pair = self.issue();
            let left: Receipt = returned.left;
            let right: Receipt = returned.right;
            Receipt::ack(left);
            Receipt::ack(right);
            0
        }
        "#,
    );

    use omega_core::semantics::{PermissionAccess, PermissionClaimIdentity, PermissionEventKind};
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| event.access == PermissionAccess::Owned)
        .collect::<Vec<_>>();
    let run_symbol = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .find(|state| state.name.as_str() == "run")
        .map(|state| state.symbol)
        .expect("run state");
    let caller_establishments = events
        .iter()
        .copied()
        .filter(|event| {
            event.kind == PermissionEventKind::Establish
                && matches!(
                    event.source,
                    omega_core::semantics::PermissionEventSource::Statement { statement_index: 0 }
                )
        })
        .filter(|event| event.state_symbol == run_symbol)
        .collect::<Vec<_>>();
    assert_eq!(caller_establishments.len(), 2);
    assert_ne!(
        caller_establishments[0].claim_identity,
        caller_establishments[1].claim_identity
    );
    assert!(
        caller_establishments
            .iter()
            .all(|event| event.claim_identity != PermissionClaimIdentity::Unknown)
    );
    for establishment in caller_establishments {
        assert_eq!(
            events
                .iter()
                .filter(|event| event.claim_identity == establishment.claim_identity)
                .count(),
            8,
            "each callee claim must remain independently conserved through the caller: {events:#?}"
        );
    }
}

#[test]
fn state_call_result_maps_direct_aggregate_constructor_fields() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::issue(&mut self) -> Pair {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            transition { _ -> Pair { left: left, right: right } }
        }
        machine Main::run(&mut self) -> i32 {
            let returned: Pair = self.issue();
            let left: Receipt = returned.left;
            let right: Receipt = returned.right;
            Receipt::ack(left);
            Receipt::ack(right);
            0
        }
        "#,
    );

    use omega_core::semantics::{PermissionAccess, PermissionEventKind};
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| event.access == PermissionAccess::Owned)
        .collect::<Vec<_>>();
    let (issue_symbol, run_symbol) = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .fold((None, None), |(issue, run), state| {
            match state.name.as_str() {
                "issue" => (Some(state.symbol), run),
                "run" => (issue, Some(state.symbol)),
                _ => (issue, run),
            }
        });
    let issue_symbol = issue_symbol.expect("issue state");
    let run_symbol = run_symbol.expect("run state");
    let callee_transfers = events
        .iter()
        .copied()
        .filter(|event| {
            event.state_symbol == issue_symbol
                && event.kind == PermissionEventKind::Transfer
                && event.obligation_live
        })
        .collect::<Vec<_>>();
    let caller_establishments = events
        .iter()
        .copied()
        .filter(|event| {
            event.state_symbol == run_symbol
                && event.kind == PermissionEventKind::Establish
                && matches!(
                    event.source,
                    omega_core::semantics::PermissionEventSource::Statement { statement_index: 0 }
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(callee_transfers.len(), 2);
    assert_eq!(caller_establishments.len(), 2);
    assert_ne!(
        caller_establishments[0].claim_identity,
        caller_establishments[1].claim_identity
    );
    assert!(caller_establishments.iter().all(|establishment| {
        callee_transfers
            .iter()
            .any(|transfer| transfer.claim_identity == establishment.claim_identity)
    }));
}

#[test]
fn state_call_result_consumes_checked_opaque_multi_output_map() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::pack(left: Receipt, right: Receipt) -> Pair {
            transition { _ -> Pair { left: left, right: right } }
        }
        machine Main::issue(&mut self) -> Pair {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            transition { _ -> Main::pack(left, right) }
        }
        machine Main::forward(&mut self) -> Pair {
            transition { _ -> (self.issue()) }
        }
        machine Main::run(&mut self) -> i32 {
            let returned: Pair = self.forward();
            let left: Receipt = returned.left;
            let right: Receipt = returned.right;
            Receipt::ack(left);
            Receipt::ack(right);
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("checked outcome maps should compose");
    use omega_core::semantics::PermissionEventKind;
    let maps = checked
        .facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .map(|(_, map)| map)
        .collect::<Vec<_>>();
    let (pack_symbol, issue_symbol, forward_symbol, run_symbol) = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .fold(
            (None, None, None, None),
            |(pack, issue, forward, run), state| match state.name.as_str() {
                "pack" => (Some(state.symbol), issue, forward, run),
                "issue" => (pack, Some(state.symbol), forward, run),
                "forward" => (pack, issue, Some(state.symbol), run),
                "run" => (pack, issue, forward, Some(state.symbol)),
                _ => (pack, issue, forward, run),
            },
        );
    let pack_symbol = pack_symbol.expect("pack state");
    let issue_symbol = issue_symbol.expect("issue state");
    let forward_symbol = forward_symbol.expect("forward state");
    let run_symbol = run_symbol.expect("run state");
    assert_eq!(
        maps.iter()
            .find(|map| map.state_symbol == pack_symbol)
            .map(|map| map.entries.count()),
        Some(2),
    );
    assert_eq!(
        maps.iter()
            .find(|map| map.state_symbol == forward_symbol)
            .map(|map| map.entries.count()),
        Some(2),
    );
    assert_eq!(
        maps.iter()
            .find(|map| map.state_symbol == issue_symbol)
            .map(|map| map.entries.count()),
        Some(2),
    );
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let issue_transfers = events
        .iter()
        .copied()
        .filter(|event| {
            event.state_symbol == issue_symbol
                && event.kind == PermissionEventKind::Transfer
                && event.obligation_live
        })
        .collect::<Vec<_>>();
    let run_establishments = events
        .iter()
        .copied()
        .filter(|event| {
            event.state_symbol == run_symbol
                && event.kind == PermissionEventKind::Establish
                && matches!(
                    event.source,
                    omega_core::semantics::PermissionEventSource::Statement { statement_index: 0 }
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(issue_transfers.len(), 2);
    assert_eq!(run_establishments.len(), 2);
    assert_ne!(
        run_establishments[0].claim_identity,
        run_establishments[1].claim_identity,
    );
    assert!(run_establishments.iter().all(|establishment| {
        issue_transfers
            .iter()
            .any(|transfer| transfer.claim_identity == establishment.claim_identity)
    }));
}

#[test]
fn permission_producer_discovers_transfers_without_legacy_moves() {
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
    facts.flow.ownership.moves = Default::default();
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
fn permission_producer_discovers_affine_cleanup_without_legacy_drops() {
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
            (event.kind == omega_core::semantics::PermissionEventKind::AffineDrop)
                .then_some(event.root)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        expected.len(),
        3,
        "two locals and one owned parameter clean up"
    );

    let mut facts = checked.facts.clone();
    facts.flow.ownership.drops = Default::default();
    facts.flow.ownership.permissions = Default::default();
    crate::checks::record_permission_events(&checked.typed, &mut facts);
    let actual = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.kind == omega_core::semantics::PermissionEventKind::AffineDrop)
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

    use omega_core::semantics::{PermissionAccess, PermissionEventKind};
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

    use omega_core::semantics::{PermissionAccess, PermissionEventKind};
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
        .find(|event| event.kind == omega_core::semantics::PermissionEventKind::Establish)
        .expect("generic conditional establishment event");
    assert!(!event.obligation_live);
}

#[test]
fn nested_linear_record_extraction_stays_conservative_without_field_algebra() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair [linear] {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let pair: Pair = Pair { left: left, right: right };
            let extracted: Receipt = pair.left;
            Receipt::ack(extracted);
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("partial linear-record extraction needs per-field resource accounting");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `pair` reaches scope exit")
    }));
}

#[test]
fn transparent_record_frontier_preserves_independent_field_origins() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let pair: Pair = Pair { left: left, right: right };
            let forwarded: Pair = pair;
            let extracted_left: Receipt = forwarded.left;
            let extracted_right: Receipt = forwarded.right;
            Receipt::ack(extracted_left);
            Receipt::ack(extracted_right);
            0
        }
        "#,
    );

    use omega_core::semantics::{PermissionAccess, PermissionEventKind};
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| event.access == PermissionAccess::Owned)
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 16);

    let pair_symbol = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .flat_map(|state| checked.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            omega_typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "pair" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("pair local");
    let pair_establishments = events
        .iter()
        .copied()
        .filter(|event| {
            event.kind == PermissionEventKind::Establish
                && event.root == omega_facts::PlaceRoot::Symbol(pair_symbol)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        pair_establishments.len(),
        2,
        "transparent records establish one frontier entry per contained linear field"
    );
    assert!(pair_establishments.iter().all(|event| {
        checked
            .facts
            .flow
            .ownership
            .segments
            .span_or_empty(event.segments)
            .len()
            == 1
    }));
    assert_ne!(
        pair_establishments[0].provenance, pair_establishments[1].provenance,
        "constructing an aggregate must not collapse independent field lineages"
    );
    assert_ne!(
        pair_establishments[0].claim_identity, pair_establishments[1].claim_identity,
        "constructing an aggregate must not collapse independent field claims"
    );
    for establishment in pair_establishments {
        assert_eq!(
            events
                .iter()
                .filter(|event| event.provenance == establishment.provenance)
                .count(),
            8,
            "each source claim must map through its field and extracted local independently"
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.claim_identity == establishment.claim_identity)
                .count(),
            8,
            "each source claim identity must survive aggregate and local transfers"
        );
    }
}

#[test]
fn transparent_record_entry_claims_share_lineage_but_have_distinct_identities() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::run() -> i32 { 0 }
        machine consume(pair: Pair) {
            let left: Receipt = pair.left;
            let right: Receipt = pair.right;
            Receipt::ack(left);
            Receipt::ack(right);
        }
        "#,
    );

    use omega_core::semantics::{
        PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    };
    let entries = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.source == PermissionEventSource::StateEntry
                && event.kind == PermissionEventKind::Establish
        })
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].provenance, entries[1].provenance);
    assert_ne!(entries[0].claim_identity, entries[1].claim_identity);
    assert!(
        entries
            .iter()
            .all(|event| event.claim_identity != PermissionClaimIdentity::Unknown)
    );
}

#[test]
fn transparent_record_partial_move_leaves_sibling_obligation_live() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let pair: Pair = Pair { left: left, right: right };
            let extracted: Receipt = pair.left;
            Receipt::ack(extracted);
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics =
        lower_typed_trees(typed).expect_err("the untouched sibling remains an obligation");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `pair.right` reaches scope exit")
    }));
}

#[test]
fn transparent_record_rejects_duplicate_field_move() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let pair: Pair = Pair { left: left, right: right };
            let first: Receipt = pair.left;
            let duplicate: Receipt = pair.left;
            let remaining: Receipt = pair.right;
            Receipt::ack(first);
            Receipt::ack(duplicate);
            Receipt::ack(remaining);
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("one field claim cannot move twice");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `pair.left` was already transferred")
    }));
}

#[test]
fn transparent_record_sibling_assignment_transfers_the_source_claim() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Pair {
            left: Receipt;
            right: Receipt;
        }
        data Main {}
        machine Main::run() -> i32 {
            let left: Receipt = Receipt { code: 1 };
            let right: Receipt = Receipt { code: 2 };
            let pair: Pair = Pair { left: left, right: right };
            let old_left: Receipt = pair.left;
            Receipt::ack(old_left);
            pair.left = pair.right;
            let forwarded: Receipt = pair.left;
            Receipt::ack(forwarded);
            let duplicate: Receipt = pair.right;
            Receipt::ack(duplicate);
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics =
        lower_typed_trees(typed).expect_err("assigning from a sibling must transfer its claim");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `pair.right` was already transferred")
    }));
}

#[test]
fn nested_generic_transparent_record_retains_the_concrete_claim_path() {
    let checked = checked(
        r#"
        data Receipt [linear] { code: i32; }
        machine Receipt::ack(self) {}
        data Box<T> { value: T; }
        data Envelope<T> { boxed: Box<T>; }
        data Main {}
        machine Main::run() -> i32 {
            let issued: Receipt = Receipt { code: 7 };
            let boxed: Box<Receipt> = Box { value: issued };
            let envelope: Envelope<Receipt> = Envelope { boxed: boxed };
            let extracted: Receipt = envelope.boxed.value;
            Receipt::ack(extracted);
            0
        }
        "#,
    );

    let envelope_symbol = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .flat_map(|state| checked.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            omega_typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "envelope" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("envelope local");
    let establishment = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .find(|event| {
            event.kind == omega_core::semantics::PermissionEventKind::Establish
                && event.root == omega_facts::PlaceRoot::Symbol(envelope_symbol)
        })
        .expect("nested generic frontier establishment");
    assert_eq!(
        checked
            .facts
            .flow
            .ownership
            .segments
            .span_or_empty(establishment.segments)
            .len(),
        2,
        "the concrete claim must remain nested under both generic record fields"
    );
}
