// A formal passed as `&[u8]`: a shared byte view.
//
//     machine measure(bytes: &[u8]) -> u64 { bytes.len }
//     machine forward(bytes: &[u8]) -> u64 { measure(bytes) }
//
// The formal is a root parameter place of the module's shared byte-view
// type, declared under shared access. `bytes.len` is one `ByteSequenceLength`
// observation of that place and yields a `u64` about which nothing but its
// carrier is known: byte_views.md, "Reading length supplies no indexing
// proof". Passing the view on lends the place the formal arrived in, a
// shared reborrow of the incoming loan.

use super::mutable_formals::unsafe_operations;
use super::*;
use language_core::ReferenceAccess;
use structure_elaboration::Borrowed;
use terminal_check_tables::obligations::Lender;
use terminal_psi::{
    ByteSequenceCarrier, OperationKind, StructuralAccess, StructuralTypeShape,
    TerminalMachineResult,
};

fn u64_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

fn of(kind: ExpressionKind, result: ValueType) -> Expression {
    Expression {
        kind,
        result: Some(result),
        source: nowhere(),
    }
}

/// The view formal in `slot`, named where it is measured or lent.
fn view(slot: usize) -> Expression {
    of(ExpressionKind::Slot(slot), ValueType::Bytes)
}

/// `bytes.len`
fn length(slot: usize) -> Expression {
    of(
        ExpressionKind::Length(Box::new(view(slot))),
        u64_type().into(),
    )
}

fn operate(operation: Strict, left: Expression, right: Expression) -> Expression {
    of(
        ExpressionKind::Strict {
            operation,
            left: Box::new(left),
            right: Box::new(right),
        },
        u64_type().into(),
    )
}

fn one() -> Expression {
    of(
        ExpressionKind::Integer(IntegerValue::Unsigned(1)),
        u64_type().into(),
    )
}

/// Machines of one `&[u8]` formal and a `u64` result, each given by its
/// statements.
fn lower_views(machines: Vec<Vec<Statement>>) -> Checked<structure_elaboration::Elaboration> {
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
    for statements in machines {
        bodies.push(Body {
            slots: vec![ValueType::Bytes],
            statements,
        });
        signatures.push(Signature {
            parameters: vec![ValueType::Bytes],
            erased: Vec::new(),
            borrowed: vec![Borrowed {
                position: 0,
                access: ReferenceAccess::Shared,
                home: Some(String::from("[u8]")),
            }],
            result: Some(u64_type().into()),
            requires: Vec::new(),
            ensures: Vec::new(),
            crashes: CrashCeiling::Authored(Vec::new()),
            termination: Termination::Silent,
        });
    }
    structure_elaboration::lower_bodies(id(1), &bodies, &signatures, &IdTable::default())
}

fn accepted(machines: Vec<Vec<Statement>>) -> Box<crate::AcceptedClosure> {
    match judge(lower_views(machines)).unwrap() {
        RouteVerdict::Accepted(closure) => closure,
        verdict => panic!("the program must be accepted: {verdict:?}"),
    }
}

#[test]
fn a_view_formal_is_a_shared_view_place_and_its_length_one_observation() {
    let closure = accepted(vec![vec![returns(length(0))]]);
    let module = &closure.elaboration.module;
    // The one structural type is the shared borrowed view.
    let [view_type] = module.structural_types.as_slice() else {
        panic!("{:?}", module.structural_types);
    };
    assert_eq!(
        view_type.shape,
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow)
        })
    );
    assert!(view_type.copyable && !view_type.owns_linear_claim);
    let machine = &module.machines[0];
    let [formal] = machine.structural_parameters.as_slice() else {
        panic!("{:?}", machine.structural_parameters);
    };
    assert_eq!(formal.structural_type, view_type.id);
    assert_eq!(formal.access, StructuralAccess::SharedBorrow);
    assert!(machine.parameters.is_empty());
    assert!(matches!(machine.result, TerminalMachineResult::Scalar(_)));
    let kinds: Vec<_> = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .map(|operation| &operation.kind)
        .collect();
    let [OperationKind::ByteSequenceLength { source }] = kinds.as_slice() else {
        panic!("{kinds:?}");
    };
    assert_eq!(source.place, formal.place);
    assert!(source.path.is_empty());
    assert_eq!(source.access, StructuralAccess::SharedBorrow);
}

#[test]
fn nothing_is_known_of_a_length_but_its_carrier() {
    // `1 / bytes.len`: the view may be empty, so the divisor is unproved.
    let divided = vec![returns(operate(Strict::Divide, one(), length(0)))];
    assert_eq!(
        unsafe_operations(judge(lower_views(vec![divided])).unwrap()),
        1
    );
    // `bytes.len + 1`: a view may hold `u64::MAX` octets as far as the
    // checker knows.
    let next = vec![returns(operate(Strict::Add, length(0), one()))];
    assert_eq!(
        unsafe_operations(judge(lower_views(vec![next])).unwrap()),
        1
    );
    // `bytes.len - bytes.len`: two observations are two values, and neither
    // is known to be the other.
    let difference = vec![returns(operate(Strict::Subtract, length(0), length(0)))];
    assert_eq!(
        unsafe_operations(judge(lower_views(vec![difference])).unwrap()),
        1
    );
}

#[test]
fn a_view_passed_on_reborrows_the_incoming_loan() {
    // `forward(bytes) { measure(bytes) }`
    let forward = vec![returns(Expression {
        kind: ExpressionKind::Call {
            callee: id(2),
            arguments: vec![of(
                ExpressionKind::Borrow {
                    access: ReferenceAccess::Shared,
                    place: Box::new(view(0)),
                    home: None,
                },
                ValueType::Bytes,
            )],
        },
        result: Some(u64_type().into()),
        source: nowhere(),
    })];
    let measure = vec![returns(length(0))];
    let closure = accepted(vec![forward, measure]);
    let loans: Vec<_> = closure
        .elaboration
        .tables
        .machines
        .get(id(1))
        .unwrap()
        .obligations
        .iter()
        .filter_map(|(_, row)| match row.kind {
            ObligationKind::Loan { lender, access, .. } => Some((lender, access)),
            _ => None,
        })
        .collect();
    assert_eq!(loans, [(Lender::Incoming, ReferenceAccess::Shared)]);
}
