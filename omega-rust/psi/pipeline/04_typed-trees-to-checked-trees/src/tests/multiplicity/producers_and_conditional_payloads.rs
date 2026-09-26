use crate::tests::front_end::{checked_program, checked_program_result};

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
