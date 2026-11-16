//! Direct calls preserve actual sibling obligations at their current paths.
//! The source publisher supplies rows, and receiving must reconstruct them
//! rather than accepting any declared claim ID or refusing all claimed calls.

use super::*;
use terminal_check_tables::program::Elaboration;
use terminal_psi::{ClaimTransfer, OperationKind};

fn elaborated() -> Elaboration {
    let typed = super::build_purpose::frontier_of(
        "machine product_entry() {}",
        r#"
data Token [linear] { value: u32; }
data Pair { left: Token; right: Token; }
machine forward(value: Pair) -> Pair { value }
machine build_entry(value: Pair) -> Pair { forward(value) }
"#,
    );
    let entry = super::build_purpose::symbol(&typed, "build_entry");
    typed_trees_to_terminal_psi::elaborate_for(
        &typed,
        super::build_purpose::roster(
            &[entry],
            &[Some(super::build_purpose::package())],
            source::DependencyScope::Build,
        ),
        terminal_codec::TerminalPublication::Invocation,
    )
    .expect("ordinary linear siblings publish their real source claims")
}

fn transfers(elaboration: &mut Elaboration) -> &mut Vec<ClaimTransfer> {
    elaboration
        .module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::CallStructural {
                claim_transfers, ..
            }
            | OperationKind::CallStructuralWithScalarArguments {
                claim_transfers, ..
            } if !claim_transfers.is_empty() => Some(claim_transfers),
            _ => None,
        })
        .expect("the forwarding call has explicit claim transfers")
}

fn refuses(elaboration: Elaboration) {
    match judge(Ok(elaboration)) {
        Err(refusal) => assert_eq!(refusal.reason, RefusalReason::ForeignIdentity),
        other => panic!("a forged current claim frontier must fail receiving: {other:?}"),
    }
}

#[test]
fn ordinary_call_receiving_preserves_exact_sibling_claims() {
    let mut original = elaborated();
    assert_eq!(transfers(&mut original).len(), 2);
    let accepted = match judge(Ok(original.clone())) {
        Ok(RouteVerdict::Accepted(accepted)) => accepted,
        other => panic!("a complete ordinary claim transfer is accepted: {other:?}"),
    };
    accepted.publish_invocation().unwrap();

    let mut missing = original.clone();
    transfers(&mut missing).pop();
    refuses(missing);

    let mut duplicate = original.clone();
    let rows = transfers(&mut duplicate);
    rows[1] = rows[0].clone();
    refuses(duplicate);

    let mut foreign = original.clone();
    transfers(&mut foreign)[0].claim = id(u64::MAX);
    refuses(foreign);

    let mut sibling = original.clone();
    let rows = transfers(&mut sibling);
    let path = rows[0].path.clone();
    rows[0].path = rows[1].path.clone();
    rows[1].path = path;
    refuses(sibling);

    let mut wrong_argument = original;
    transfers(&mut wrong_argument)[0].argument_index = 1;
    refuses(wrong_argument);
}
