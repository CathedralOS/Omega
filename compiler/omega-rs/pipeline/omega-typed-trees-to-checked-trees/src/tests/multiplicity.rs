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
    assert!(events
        .iter()
        .all(|event| event.access == omega_core::semantics::PermissionAccess::Owned));
    assert!(events[..4]
        .iter()
        .all(|event| event.multiplicity == omega_core::semantics::Multiplicity::Linear));
    assert_eq!(events[4].multiplicity, omega_core::semantics::Multiplicity::Affine);
    let origin = events[0].provenance;
    assert_ne!(origin, omega_core::semantics::PermissionProvenance::Unknown);
    assert!(events[..4].iter().all(|event| event.provenance == origin),
        "transfers preserve one obligation origin rather than minting a new one per binding");
    assert_eq!(
        events[4].provenance,
        omega_core::semantics::PermissionProvenance::Unknown,
        "legacy-derived affine cleanup must not invent establishment provenance"
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
        Multiplicity, PermissionAccess, PermissionEventKind, PermissionProvenance,
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
        assert_ne!(pair[0].provenance, PermissionProvenance::Unknown);
    }
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
