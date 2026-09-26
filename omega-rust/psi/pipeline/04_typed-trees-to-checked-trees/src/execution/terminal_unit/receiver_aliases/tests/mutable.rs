use super::{BorrowAccessKind, FlowBorrowWeakeningReason};
use crate::execution::terminal_unit::receiver_aliases::tests::fixture;
use crate::execution::terminal_unit::receiver_aliases::tests::forward_plan;

fn chain() -> checked_trees::CheckedTrees {
    fixture(
        "mut",
        "let parent: &mut [Record; 2] = &mut records;
         let middle: &mut [Record; 2] = &mut parent;
         let child: &write Record = &write middle[1];",
        "child.replace(17); child.replace(value);",
    )
}

#[test]
fn mutable_prefix_replay_rejects_access_capture_and_lifecycle_drift() {
    let original = chain();
    let parent = original
        .facts
        .borrow
        .direct_loan_resources
        .iter()
        .find(|(_, row)| original.symbols.name(row.owner_symbol) == "parent")
        .unwrap()
        .0;
    let child = original
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .find(|(_, row)| original.symbols.name(row.owner_symbol) == "child")
        .unwrap()
        .0;
    let middle = original
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .find(|(_, row)| original.symbols.name(row.owner_symbol) == "middle")
        .unwrap()
        .0;
    for mutation in 0..8 {
        let mut checked = original.clone();
        match mutation {
            0 => {
                checked
                    .facts
                    .borrow
                    .direct_loan_resources
                    .get_mut(parent)
                    .access = BorrowAccessKind::WriteOnly
            }
            1 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .get_mut(middle)
                    .access = BorrowAccessKind::WriteOnly
            }
            2 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .get_mut(child)
                    .parent_access = BorrowAccessKind::WriteOnly
            }
            3 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .get_mut(child)
                    .access = BorrowAccessKind::Mutable
            }
            4 => checked
                .facts
                .borrow
                .reborrow_loan_resources
                .get_mut(child)
                .captured_place
                .segments
                .clear(),
            5 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .get_mut(child)
                    .parent_loan = checked.facts.borrow.direct_loan_resources.get(parent).loan
            }
            6 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .get_mut(child)
                    .weakening_reason = FlowBorrowWeakeningReason::LastUseExpired
            }
            _ => checked.facts.borrow.reborrow_disposition_events.clear(),
        }
        assert!(forward_plan(&checked).is_none(), "mutation {mutation}");
    }
}
