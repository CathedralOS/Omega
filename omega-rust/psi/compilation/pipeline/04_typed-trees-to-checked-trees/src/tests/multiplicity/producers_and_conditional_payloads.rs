use crate::tests::front_end::{checked_program, checked_program_result};

const OPTIONAL_RETURN: &str = r#"
    data Receipt [linear] { code: i32; }
    boundary requirement Receipt::ack(self);
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
    let checked = checked_program(&OPTIONAL_RETURN.replace(
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
    checked_program(&held.replace("BODY", "close(holder.slot);"));
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
        let diagnostics = match checked_program_result(&source) {
            Err(diagnostics) => diagnostics,
            Ok(_) => panic!("invalid ownership control was accepted:\n{source}"),
        };
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("already transferred")
            || diagnostic.message.contains("not been established")), "{diagnostics:#?}");
    }
}

#[test]
fn generic_conditional_empty_case_establishes_without_debt() {
    let checked = checked_program(
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
fn active_case_rejects_duplicate_payload_move() {
    let source = r#"
        data Receipt [linear] { code: i32; }
        boundary requirement Receipt::ack(self);
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
    let diagnostics =
        checked_program_result(source).expect_err("one payload claim cannot move twice");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("linear value `state::Live.receipt` was already transferred")
    }));
}
