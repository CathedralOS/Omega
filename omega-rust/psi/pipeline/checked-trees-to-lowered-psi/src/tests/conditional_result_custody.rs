use super::{LoweringError, checked_source, lower_machine};

const SOURCE: &str = r#"
    data Receipt [linear] { code: i32; }
    machine Receipt::ack(self) {}

    machine choose(flag: bool, left: Receipt, right: Receipt) -> Receipt {
        transition flag {
            true -> first(left, right)
            _ -> second(left, right)
        }
        state first(left: Receipt, right: Receipt) -> Receipt {
            Receipt::ack(right);
            left
        }
        state second(left: Receipt, right: Receipt) -> Receipt {
            Receipt::ack(left);
            right
        }
    }

    machine consume(flag: bool) -> i32 {
        let left: Receipt = Receipt { code: 1 };
        let right: Receipt = Receipt { code: 2 };
        let selected: Receipt = choose(flag, left, right);
        Receipt::ack(selected);
        0
    }

    machine identity(value: u64) -> u64 { value }
"#;

#[test]
fn conditional_result_custody_requires_terminal_correspondence_only_when_demanded() {
    let checked = checked_source(SOURCE);
    assert!(!checked.facts.flow.ownership.claim_join_receipts.is_empty());
    assert_eq!(
        lower_machine(&checked, "consume").expect_err("Terminal must retain both return origins"),
        LoweringError::Unsupported(
            "conditional result custody requires Terminal exit-alternative correspondence"
        ),
    );
    lower_machine(&checked, "identity").expect("unused checked joins do not block another product");
}

#[test]
fn removing_the_join_receipt_does_not_make_joined_provenance_executable() {
    let mut checked = checked_source(SOURCE);
    checked.facts.flow.ownership.claim_join_receipts = arena::Arena::default();
    assert_eq!(
        lower_machine(&checked, "consume").expect_err("missing correspondence is not a root"),
        LoweringError::Unsupported(
            "conditional result custody requires Terminal exit-alternative correspondence"
        ),
    );
}
