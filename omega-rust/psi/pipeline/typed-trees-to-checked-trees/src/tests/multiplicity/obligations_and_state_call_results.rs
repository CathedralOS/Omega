use super::checked;
use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::{
    Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve,
};

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

    use language_semantics::{
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
fn linear_judgment_reads_canonical_permission_events() {
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
    crate::checks::validate_linear_permission_events(&checked.typed, &checked.facts)
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

    use language_semantics::{PermissionAccess, PermissionClaimIdentity, PermissionEventKind};
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
                    language_semantics::PermissionEventSource::Statement { statement_index: 0 }
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
                    language_semantics::PermissionEventSource::Statement { statement_index: 0 }
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("checked outcome maps should compose");
    use language_semantics::PermissionEventKind;
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
                    language_semantics::PermissionEventSource::Statement { statement_index: 0 }
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
