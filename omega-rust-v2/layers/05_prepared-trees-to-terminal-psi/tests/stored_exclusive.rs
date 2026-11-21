//! A `@mut` reference stored in a record is affine. A by-value use transfers
//! the loan carrier; an explicit borrow or a call operand reborrows its
//! referent. Lowering publishes which one happened and from which formal and
//! field, and leaves the verdict to the lowered-form checker.
#[path = "support/source.rs"]
mod source;

use omega_lowering::{LoweredPackage, OperationKind, PlaceProjection, ReferenceAccess};
use psi_handoff_consumer::{ExclusiveOrigin, ReturnedExclusive, returned_exclusive, returned_stored_loan};

const DATA: &str = "data Leaf{v:u8;} data Node{next:@mut Leaf;}";

fn lower(body: &str) -> LoweredPackage {
    source::lower(&[("case.omg", &format!("{DATA} {body}"))]).unwrap()
}

fn read(body: &str) -> Result<ReturnedExclusive, &'static str> {
    returned_exclusive(&lower(body), "read")
}

fn typer_refusal(text: &str) -> String {
    let package = source::resolve(&[("case.omg", text)]);
    match omega_preparation::prepare_source_package_diagnosed(&package) {
        Err(omega_preparation::PreparationFailure::Typing(error)) => error.message,
        other => panic!("expected a typing refusal, got {:?}", other.map(|_| ())),
    }
}

fn transfer(
    formal: usize,
    path: &[&str],
    moves: usize,
    cleanup: &[&str],
) -> ReturnedExclusive {
    ReturnedExclusive {
        origin: ExclusiveOrigin::Transfer,
        formal,
        path: path.iter().map(|s| s.to_string()).collect(),
        moves_of_source: moves,
        cleanup: cleanup.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn the_shared_control_still_reborrows_through_its_stored_loan() {
    let text = "data Leaf{v:u8;} data Node{next:@Leaf;} machine read(x:Node)->@Leaf{x.next}";
    let package = source::lower(&[("case.omg", text)]).unwrap();
    assert_eq!(
        returned_stored_loan(&package, "read").map(|loan| loan.fields),
        Ok(vec!["Node::next".to_string()])
    );
    assert_eq!(returned_exclusive(&package, "read"), Err("the result type must be an exclusive reference"));
}

#[test]
fn implicit_and_explicit_moves_of_the_stored_loan_publish_the_same_transfer() {
    let expected = Ok(transfer(0, &["Node::next"], 1, &[]));
    assert_eq!(read("machine read(x:Node)->@mut Leaf{x.next}"), expected);
    assert_eq!(read("machine read(x:Node)->@mut Leaf{move x.next}"), expected);
}

#[test]
fn a_same_typed_sibling_is_disposed_and_never_selected() {
    let pair = "data Leaf{v:u8;} data Node{next:@mut Leaf; other:@mut Leaf;}";
    let take = |field: &str| {
        let package = source::lower(&[("case.omg", &format!("{pair} machine read(x:Node)->@mut Leaf{{x.{field}}}"))]).unwrap();
        returned_exclusive(&package, "read")
    };
    assert_eq!(take("next"), Ok(transfer(0, &["Node::next"], 1, &["Node::other"])));
    assert_eq!(take("other"), Ok(transfer(0, &["Node::other"], 1, &["Node::next"])));
}

#[test]
fn a_nested_record_keeps_the_whole_path_to_the_stored_loan() {
    let body = "data Outer{inner:Node;} machine read(o:Outer)->@mut Leaf{o.inner.next}";
    assert_eq!(read(body), Ok(transfer(0, &["Outer::inner", "Node::next"], 1, &[])));
}

#[test]
fn local_bindings_preserve_the_incoming_origin() {
    // The local takes the moved loan; returning it returns that same place.
    assert_eq!(
        read("machine read(x:Node)->@mut Leaf{let a = x.next; a}"),
        Ok(transfer(0, &["Node::next"], 1, &[]))
    );
    // Moving the whole record into a local first keeps the formal as origin.
    let moved = read("machine read(x:Node)->@mut Leaf{let y = x; y.next}").unwrap();
    assert_eq!(
        (moved.origin, moved.formal, moved.path),
        (ExclusiveOrigin::Transfer, 0, vec!["Node::next".to_string()])
    );
    // The second formal of the same type is a different origin.
    let second = "machine read(w:Node, x:Node)->@mut Leaf{x.next}";
    assert_eq!(read(second).map(|r| (r.formal, r.cleanup)), Ok((1, vec!["root".to_string()])));
}

#[test]
fn a_use_after_the_move_stays_in_psi_for_the_checker() {
    // Normal return moves the stored loan once; the witness moves it twice.
    // A second move of a moved path is the checker's refusal
    // (terminal-psi/ownership.md#partial-ownership-and-residuals), so L5 keeps both.
    assert_eq!(read("machine read(x:Node)->@mut Leaf{x.next}").map(|r| r.moves_of_source), Ok(1));
    assert_eq!(
        read("machine read(x:Node)->@mut Leaf{let a = x.next; x.next}").map(|r| r.moves_of_source),
        Ok(2)
    );
}

#[test]
fn a_mutable_parent_moves_out_under_its_window_and_a_shared_parent_is_refused() {
    // Through `@mut Node`, L3 records a move out of borrowed storage. Returning
    // leaves the hole open, which the checker refuses
    // (language/ownership.md#borrowed-storage-invariant-windows); L5 publishes it.
    let moved = read("machine read(x:@mut Node)->@mut Leaf{x.next}").unwrap();
    assert_eq!(
        (moved.origin, moved.path),
        (ExclusiveOrigin::BorrowedMove, vec!["*".to_string(), "Node::next".to_string()])
    );
    // The lawful spelling reborrows below both exclusive loans.
    let reborrowed = read("machine read(x:@mut Node)->@mut Leaf{@mut $(x.next)}").unwrap();
    assert_eq!(
        (reborrowed.origin, reborrowed.path),
        (ExclusiveOrigin::Reborrow, ["*", "Node::next", "*"].map(String::from).to_vec())
    );
    // A shared parent cannot lend a mutable child (terminal-psi/loans.md).
    assert_eq!(
        typer_refusal(&format!("{DATA} machine read(x:@Node)->@mut Leaf{{x.next}}")),
        "reading @mut app\\Leaf out of borrowed storage requires a copy type; borrow it with @ instead"
    );
}

#[test]
fn an_explicit_reborrow_from_an_owned_record_is_a_child_of_the_stored_loan() {
    // No `mut x` is needed (language/ownership.md#owned-local-mutability). The
    // record is discarded at return while the child is returned; the checker
    // judges that lifetime from the published lineage.
    let reborrowed = read("machine read(x:Node)->@mut Leaf{@mut $(x.next)}").unwrap();
    assert_eq!(
        reborrowed,
        ReturnedExclusive {
            origin: ExclusiveOrigin::Reborrow,
            formal: 0,
            path: ["Node::next", "*"].map(String::from).to_vec(),
            moves_of_source: 0,
            cleanup: vec!["root".into()]
        }
    );
}

#[test]
fn a_call_operand_reborrows_the_stored_loan_without_consuming_it() {
    let package = lower("machine touch(l:@mut Leaf){} machine f(x:Node){touch(x.next); touch(x.next)}");
    let f = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "f")
        .unwrap();
    let borrows: Vec<_> = f
        .body
        .operations
        .iter()
        .map(|op| match &op.kind {
            OperationKind::Call { arguments, .. } => match arguments.as_slice() {
                [
                    omega_lowering::CallArgument::Borrow {
                        place,
                        access: ReferenceAccess::Mutable,
                    },
                ] => Some(place.path.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    let below =
        |path: &Option<Vec<PlaceProjection>>| matches!(path.as_deref(), Some([PlaceProjection::Field(f), PlaceProjection::Referent]) if f.path == "Node::next");
    assert_eq!(borrows.len(), 2);
    assert!(borrows.iter().all(below), "{borrows:?}");
    assert!(
        !f.body
            .operations
            .iter()
            .any(|op| matches!(op.kind, OperationKind::MoveStructural { .. } | OperationKind::CopyStructural { .. }))
    );
}

#[test]
fn a_write_through_the_stored_loan_needs_its_access_not_a_mutable_owner() {
    let package = lower("machine f(x:Node){x.next.v = 1u8;}");
    let store = package.machines[0]
        .body
        .operations
        .iter()
        .find_map(|op| match &op.kind {
            OperationKind::StructuralScalarFieldStore { destination, field, .. } => Some((destination.path.clone(), field.path.clone())),
            _ => None,
        });
    assert!(
        matches!(store, Some((ref path, ref field)) if matches!(path.as_slice(), [PlaceProjection::Field(f), PlaceProjection::Referent] if f.path == "Node::next") && field == "Leaf::v")
    );
    // A stored shared reference grants no write, whoever owns the record, and
    // neither does a shared parent of a stored `@mut`. The typer refuses both,
    // so neither reaches the L5 authority check.
    assert_eq!(
        typer_refusal("data Leaf{v:u8;} data Node{next:@Leaf;} machine f(x:Node){x.next.v = 1u8;}"),
        "place requires writable authority"
    );
    assert_eq!(
        typer_refusal(&format!("{DATA} machine f(x:@Node){{x.next.v = 1u8;}}")),
        "place requires writable authority"
    );
}

#[test]
fn a_copyable_record_cannot_hold_an_exclusive_loan() {
    assert_eq!(
        typer_refusal("data Leaf{v:u8;} data Node [copy] {next:@mut Leaf;}"),
        "copy data contains non-copy field next"
    );
}
