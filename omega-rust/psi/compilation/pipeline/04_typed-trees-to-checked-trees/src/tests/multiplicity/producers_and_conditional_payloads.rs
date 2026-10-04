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

const COMPUTED_AFFINE_SUBJECT: &str = r#"
    data Output { value: u64; }
    data Choice { case Left(value: u64); case Right(value: u64); }
    data Counter { calls: u64 in Wrapping; }
    machine identity(choice: Choice) -> Choice { choice }
    machine forward(choice: Choice, counter: &mut Counter) -> Choice {
        counter.calls = counter.calls + 1;
        choice
    }
    machine make(value: u64) -> Output { Output { value: value } }
    machine again(value: u64, choice: Choice) -> Output { Output { value: value } }
    machine choose(choice: Choice, counter: &mut Counter) -> Output {
        BEFORE
        transition SUBJECT {
            Choice::Left { value } -> make(value)
            Choice::Right { value } -> make(value)
        }
    }
"#;

#[test]
fn computed_dispatch_moves_its_affine_input_once_including_nested_operands() {
    for subject in [
        "forward(choice, counter)",
        "forward(identity(choice), counter)",
    ] {
        let checked = checked_program(
            &COMPUTED_AFFINE_SUBJECT
                .replace("BEFORE", "")
                .replace("SUBJECT", subject),
        );
        let owner = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "choose")
            .expect("dispatch owner");
        let state = &checked.machine_states(owner)[0];
        let choice = checked.state_parameters(state)[0].symbol;
        let transfers = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.state_symbol == state.symbol
                    && event.root == facts::PlaceRoot::Symbol(choice)
                    && event.kind == language_semantics::PermissionEventKind::Transfer
            })
            .count();
        assert_eq!(transfers, 1, "{subject}");
    }
}

#[test]
fn computed_dispatch_does_not_hide_independent_affine_input_moves() {
    let source = COMPUTED_AFFINE_SUBJECT.replace("SUBJECT", "forward(choice, counter)");
    for invalid in [
        source.replace(
            "BEFORE",
            "let independent: Choice = forward(choice, counter);",
        ),
        source
            .replace("BEFORE", "")
            .replace("make(value)", "again(value, choice)"),
    ] {
        let diagnostics = checked_program_result(&invalid)
            .expect_err("independently evaluated affine moves still consume the input");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("already transferred or consumed")
                    || diagnostic.message.contains("holds no value")
            }),
            "{diagnostics:#?}"
        );
    }
}
