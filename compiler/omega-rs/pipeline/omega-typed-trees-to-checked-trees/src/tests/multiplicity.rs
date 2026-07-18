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
    let kinds = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event.kind)
        .collect::<Vec<_>>();
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
