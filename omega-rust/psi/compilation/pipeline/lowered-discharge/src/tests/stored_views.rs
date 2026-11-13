//! A byte view stored in a record is a byte field of that record, not a
//! nested value of a view type. structural_access.md, "Store vocabulary":
//! `StructuralByteViewFieldStore` replaces "the descriptor of one shared
//! borrowed-view field", and "Zero-initialized storage holds the empty shared
//! view, which borrows nothing". Read from source, as a Build program: the
//! toolchain's `Build` stores `identifier: &'storage [u8]`.

use super::build_purpose::judge_build;
use super::*;
use terminal_psi::{
    ByteSequenceCarrier, OperationKind, StructuralAccess, StructuralFieldType, StructuralTypeShape,
};

fn accepted(build: &str) -> crate::AcceptedClosure {
    match judge_build(build).unwrap() {
        RouteVerdict::Accepted(accepted) => *accepted,
        other => panic!("{other:?}"),
    }
}

fn refusal(build: &str) -> String {
    match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        other => panic!("{other:?}"),
    }
}

/// How each record field of the lowered module that is no scalar is
/// declared: `true` for a byte field holding a shared view.
fn stored_fields(accepted: &crate::AcceptedClosure) -> Vec<bool> {
    let shared = ByteSequenceCarrier::BorrowedView {
        access: Some(StructuralAccess::SharedBorrow),
    };
    accepted
        .elaboration
        .module
        .structural_types
        .iter()
        .filter_map(|declared| match &declared.shape {
            StructuralTypeShape::Record { fields, .. } => Some(fields),
            _ => None,
        })
        .flatten()
        .filter(|field| !matches!(field.field_type, StructuralFieldType::Scalar(_)))
        .map(|field| field.field_type == StructuralFieldType::ByteSequence(shared))
        .collect()
}

fn operations(accepted: &crate::AcceptedClosure) -> Vec<&OperationKind> {
    accepted
        .elaboration
        .module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .map(|operation| &operation.kind)
        .collect()
}

/// The field is declared as the record's own byte field, and an assignment
/// to it replaces the descriptor with the view store, never with the
/// replacement of a nested structural value.
#[test]
fn an_assigned_view_replaces_the_descriptor_of_its_byte_field() {
    let accepted = accepted(
        r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(session: &mut Session, other: &Session) {
    session.name = other.name;
    session.ready = true;
}
"#,
    );
    assert_eq!(stored_fields(&accepted), [true]);
    let operations = operations(&accepted);
    assert_eq!(
        operations
            .iter()
            .filter(|kind| matches!(
                kind,
                OperationKind::StructuralByteViewFieldStore {
                    path,
                    field: Some(_),
                    predicate_obligation: None,
                    ..
                } if path.is_empty()
            ))
            .count(),
        1
    );
    assert!(
        !operations
            .iter()
            .any(|kind| matches!(kind, OperationKind::StoreStructuralField { .. }))
    );
    accepted.publish_invocation().unwrap();
}

/// The twin: the same store through a formal the machine holds shared. The
/// view store needs the exclusive custody a scalar field store needs.
#[test]
fn a_view_is_not_stored_through_a_shared_formal() {
    assert_eq!(
        refusal(
            r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(session: &Session, other: &Session) {
    session.name = other.name;
}
"#
        ),
        "statement store into an immutable binding"
    );
}

/// A record literal takes the view as its field's operand, a local declared
/// without an initializer holds the empty shared view, a view nested in a
/// member record is replaced beneath that member's path, and the record that
/// holds a view is still replaced whole as a record.
#[test]
fn a_stored_view_is_established_by_a_literal_and_by_zero() {
    let accepted = accepted(
        r#"
data Inner<'held> { count: u32; name: &'held [u8]; }
data Session<'storage> { ready: bool; inner: Inner<'storage>; }
machine build_entry(session: &mut Session) {
    let mut zeroed: Session;
    let made: Inner = Inner { count: 1, name: "made" };
    zeroed.inner.name = made.name;
    session.inner.name = made.name;
    session.inner = zeroed.inner;
}
"#,
    );
    // `Inner.name` is the byte field; `Session.inner` stays a nested record.
    assert_eq!(stored_fields(&accepted), [true, false]);
    let operations = operations(&accepted);
    assert_eq!(
        operations
            .iter()
            .filter(|kind| matches!(
                kind,
                OperationKind::StructuralByteViewFieldStore { path, .. } if path.len() == 1
            ))
            .count(),
        2
    );
    assert_eq!(
        operations
            .iter()
            .filter(|kind| matches!(kind, OperationKind::EstablishRecord { .. }))
            .count(),
        1
    );
    accepted.publish_invocation().unwrap();
}

/// Only the shared view is a storable descriptor: "Qualified mutable,
/// write-only, and access-unspecified view fields remain unsupported".
#[test]
fn an_exclusive_view_is_not_a_field() {
    assert_eq!(
        refusal(
            r#"
data Session<'storage> { ready: bool; name: &'storage mut [u8]; }
machine build_entry(session: &mut Session) { session.ready = true; }
"#
        ),
        "type stored view that is not a shared byte view"
    );
}
