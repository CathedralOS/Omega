// Data with no field and no case: `data Token {}`.
//
// It is a record whose roster is empty, not a special kind: a value of it is
// established by a literal or by declaring a local, copied when the
// declaration is `[copy]` and moved once when it is not. It has no leaf, so
// nothing can be read from it, and these tests pin that having no leaf does
// not make a value established by default or movable twice.

use super::*;
use structure_elaboration::{RecordField, RecordType};

fn token() -> ValueType {
    ValueType::Record(id(1))
}

/// `data Token {}`, `[copy]` or not, and `data Holder { token: Token; }`.
fn tokens(copyable: bool) -> IdTable<StructuralTypeId, RecordType> {
    let mut records = IdTable::default();
    records.push(RecordType {
        copyable,
        identity: "Token".into(),
        shape: structure_elaboration::StorageShape::Record(Vec::new()),
        requires: Vec::new(),
    });
    records.push(RecordType {
        copyable,
        identity: "Holder".into(),
        shape: structure_elaboration::StorageShape::Record(vec![RecordField {
            id: id(1),
            identity: "token".into(),
            value_type: token(),
        }]),
        requires: Vec::new(),
    });
    records
}

fn holder() -> ValueType {
    ValueType::Record(id(2))
}

/// `Token {}`
fn minted() -> Expression {
    of_type(ExpressionKind::Record { fields: Vec::new() }, token())
}

fn verdict(
    machines: Vec<Shaped>,
    records: &IdTable<StructuralTypeId, RecordType>,
) -> Checked<RouteVerdict> {
    judge(Ok(elaborate_over(machines, records)))
}

fn accepted(machines: Vec<Shaped>, records: &IdTable<StructuralTypeId, RecordType>) {
    let verdict = verdict(machines, records);
    assert!(
        matches!(verdict, Ok(RouteVerdict::Accepted(_))),
        "the program must be accepted: {verdict:?}"
    );
}

/// The program must be rejected for using storage that holds no value.
fn unestablished(machines: Vec<Shaped>, records: &IdTable<StructuralTypeId, RecordType>) {
    let verdict = verdict(machines, records);
    assert!(
        matches!(
            &verdict,
            Ok(RouteVerdict::Rejected { unestablished, .. }) if !unestablished.is_empty()
        ),
        "the program must be rejected for a use of storage that holds no value: {verdict:?}"
    );
}

/// One machine with a `Token` formal, two `Token` locals and a `u32` result.
fn over_a_token(statements: Vec<Statement>) -> Shaped {
    Shaped {
        parameters: vec![token()],
        locals: vec![token(), token()],
        result: u32_type().into(),
        requires: Vec::new(),
        statements,
    }
}

#[test]
fn an_empty_record_is_minted_copied_and_declared() {
    for copyable in [true, false] {
        // `let a = Token {}; 0`
        accepted(
            vec![over_a_token(vec![bind(1, minted()), returns(unsigned(0))])],
            &tokens(copyable),
        );
        // `let a: Token; let b = a; 0`: a local declared without an
        // initializer holds the zero value, which for no field is the value.
        accepted(
            vec![over_a_token(vec![
                declare(1, token()),
                bind(2, place(1, token())),
                returns(unsigned(0)),
            ])],
            &tokens(copyable),
        );
    }
}

#[test]
fn an_affine_empty_record_moves_once() {
    let twice = || {
        vec![over_a_token(vec![
            bind(1, place(0, token())),
            bind(2, place(0, token())),
            returns(unsigned(0)),
        ])]
    };
    // `[copy]`: two copies of the formal.
    accepted(twice(), &tokens(true));
    // Not `[copy]`: the second names a formal whose value was moved out.
    // There is no leaf to find unestablished, so the custody replay is what
    // rejects it.
    unestablished(twice(), &tokens(false));
    accepted(
        vec![over_a_token(vec![
            bind(1, place(0, token())),
            returns(unsigned(0)),
        ])],
        &tokens(false),
    );
}

#[test]
fn an_empty_record_is_a_field_and_an_argument() {
    for copyable in [true, false] {
        // `machine take(token: Token) -> u32 { 0 }` and a caller that
        // builds `Holder { token: Token {} }` and passes a fresh `Token {}`.
        let caller = Shaped {
            parameters: Vec::new(),
            locals: vec![holder()],
            result: u32_type().into(),
            requires: Vec::new(),
            statements: vec![
                bind(
                    0,
                    of_type(
                        ExpressionKind::Record {
                            fields: vec![(0, minted())],
                        },
                        holder(),
                    ),
                ),
                returns(Expression {
                    kind: ExpressionKind::Call {
                        callee: id(2),
                        arguments: vec![minted()],
                    },
                    result: Some(u32_type().into()),
                    source: nowhere(),
                }),
            ],
        };
        let take = Shaped {
            parameters: vec![token()],
            locals: Vec::new(),
            result: u32_type().into(),
            requires: Vec::new(),
            statements: vec![returns(unsigned(0))],
        };
        accepted(vec![caller, take], &tokens(copyable));
    }
}
