// A `mut` formal that holds its own value: the callee's own storage,
// initialised from the argument, which the body assigns and stores into as
// it does a `mut` local.
//
//     machine step(mut x: u32) -> u32 requires x <= 10 { x = x + 1; x + 1 }
//     machine set(mut p: Pair) requires p.a <= 10 { p.a = 5; }
//
// A scalar is rebound to the assigned value, so the entry value keeps its
// identity and an entry `requires` stays a fact about that value only. A
// record is stored into by field in the formal's own parameter place, and
// the store replaces what that field held at entry. Either way a fact from
// `requires` must not prove anything about the formal after a write to it,
// and the caller's argument is untouched. The reader decides which formals
// may be written (`typed-trees-to-terminal-psi/src/read/machine.rs`); these bodies
// are what it produces for them.

use super::references::{
    Lending, accepted, field, invoke, lend, lower_lending, named, pair, pair_of, store, unit,
};
use super::*;
use language_core::ReferenceAccess;
use semantic_vocabulary::CanonicalStructuralPathSegment;

/// The largest addend that keeps `value + addend` inside `u32` when
/// `value <= limit`.
fn headroom(limit: u128) -> Expression {
    unsigned(u128::from(u32::MAX) - limit)
}

fn route_accepts(machines: Vec<Machine>) {
    match judge(lower(machines)).unwrap() {
        RouteVerdict::Accepted(_) => {}
        verdict => panic!("the program must be accepted: {verdict:?}"),
    }
}

/// How many obligations of a program the route must reject are unproved,
/// each of which must be an operation's safety.
pub(super) fn unsafe_operations(verdict: RouteVerdict) -> usize {
    match verdict {
        RouteVerdict::Rejected {
            unproved,
            unestablished,
            conflicts,
            ..
        } if unestablished.is_empty() && conflicts.is_empty() => {
            assert!(
                unproved
                    .iter()
                    .all(|entry| matches!(entry.kind, ObligationKind::OperationSafety { .. })),
                "{unproved:?}"
            );
            unproved.len()
        }
        verdict => panic!("the program must be rejected for an operation: {verdict:?}"),
    }
}

#[test]
fn an_assigned_scalar_formal_is_judged_by_the_value_it_was_assigned() {
    // `step(mut x: u32) requires x <= 10 { x = x + 1; x + (MAX - 11) }`:
    // the second add reads the assigned value, which is at most 11.
    let step = |addend: Expression| Machine {
        requires: vec![at_most(1, 10)],
        ..machine(
            vec![u32_type()],
            vec![
                Statement::Bind {
                    slot: 0,
                    value: strict(Strict::Add, slot(0, u32_type()), unsigned(1)),
                },
                returns(strict(Strict::Add, slot(0, u32_type()), addend)),
            ],
        )
    };
    let (elaboration, result) = run(vec![step(headroom(11))]);
    let safeties = verdicts(&elaboration, &result, 1, safety);
    assert_eq!(safeties.len(), 2);
    for (obligation, verdict) in safeties {
        assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
    }
    // The entry bound is no longer the formal's: the addend that fits a
    // value of at most 10 does not fit the assigned one.
    assert_eq!(
        unsafe_operations(judge(lower(vec![step(headroom(10))])).unwrap()),
        1
    );
}

#[test]
fn an_entry_fact_does_not_follow_a_scalar_formal_through_an_assignment() {
    // `f(mut x: u32, y: u32) requires x <= 10 { x = y; x + (MAX - 10) }`:
    // `x <= 10` was true of the argument, and `x` now holds `y`. Without
    // the assignment the same add is safe.
    let body = |assigned: bool| {
        let mut statements = Vec::new();
        if assigned {
            statements.push(Statement::Bind {
                slot: 0,
                value: slot(1, u32_type()),
            });
        }
        statements.push(returns(strict(
            Strict::Add,
            slot(0, u32_type()),
            headroom(10),
        )));
        Machine {
            requires: vec![at_most(1, 10)],
            ..machine(vec![u32_type(), u32_type()], statements)
        }
    };
    route_accepts(vec![body(false)]);
    assert_eq!(
        unsafe_operations(judge(lower(vec![body(true)])).unwrap()),
        1
    );
}

/// `p.<field> <= 10` at entry, over the first place formal.
fn field_at_most_ten(field: u64) -> Proposition {
    let ScalarType::Integer(integer) = u32_type() else {
        unreachable!()
    };
    Proposition::LessOrEqual(
        ScalarTerm::IntegerField {
            root: id(1),
            path: vec![CanonicalStructuralPathSegment::Field(id(field))],
            scalar_type: integer,
        },
        bound(u32_type(), IntegerValue::Unsigned(10)),
    )
}

/// `f(mut p: Pair) requires p.<bounded> <= 10 { [p.a = <stored>;] p.<read> + (MAX - 10); }`
fn stored_then_read(bounded: u64, stored: Option<u128>, read: usize) -> Lending {
    let mut statements = Vec::new();
    if let Some(stored) = stored {
        statements.push(store(0, vec![0], unsigned(stored)));
    }
    statements.push(Statement::Evaluate(strict(
        Strict::Add,
        field(named(0, pair()), read),
        headroom(10),
    )));
    Lending {
        requires: vec![field_at_most_ten(bounded)],
        ..unit(vec![pair()], statements)
    }
}

#[test]
fn a_store_into_a_record_formal_replaces_what_its_field_held_at_entry() {
    // With no store the entry fact bounds the read.
    accepted(vec![stored_then_read(1, None, 0)]);
    // `p.a = 5;` then the read is of 5, which fits.
    accepted(vec![stored_then_read(1, Some(5), 0)]);
    // `p.a = MAX;` then `p.a + (MAX - 10)`: the entry fact `p.a <= 10` is
    // about what the field held before the store and proves nothing now.
    let overwritten = stored_then_read(1, Some(u128::from(u32::MAX)), 0);
    assert_eq!(
        unsafe_operations(judge(lower_lending(vec![overwritten])).unwrap()),
        1
    );
    // The sibling field was not written and keeps its entry fact.
    accepted(vec![stored_then_read(2, Some(u128::from(u32::MAX)), 1)]);
}

#[test]
fn a_store_into_a_record_formal_leaves_the_callers_argument_alone() {
    // `main() { let q = Pair { a: 1, b: 2 }; set(q); q.a + (MAX - 1); }`
    // with `set(mut p: Pair) { p.a = MAX; }`: the call passes a copy, so
    // the caller's field still holds 1.
    let main = Lending {
        locals: vec![pair()],
        ..unit(
            Vec::new(),
            vec![
                Statement::Bind {
                    slot: 0,
                    value: pair_of(1, 2),
                },
                invoke(2, vec![named(0, pair())]),
                Statement::Evaluate(strict(Strict::Add, field(named(0, pair()), 0), headroom(1))),
            ],
        )
    };
    let set = unit(
        vec![pair()],
        vec![store(0, vec![0], unsigned(u128::from(u32::MAX)))],
    );
    accepted(vec![main, set]);
}

#[test]
fn a_formal_lent_exclusively_holds_what_the_callee_left() {
    // `f(mut p: Pair) requires p.<bounded> <= 10 { clear(&mut p.a); p.<read> + (MAX - 10); }`:
    // the formal's storage is the machine's own to lend, the lent field
    // holds whatever `clear` stored, and its sibling is untouched.
    let lending = |bounded: u64, read: usize| Lending {
        requires: vec![field_at_most_ten(bounded)],
        ..unit(
            vec![pair()],
            vec![
                invoke(
                    2,
                    vec![lend(
                        ReferenceAccess::Mutable,
                        field(named(0, pair()), 0),
                        false,
                    )],
                ),
                Statement::Evaluate(strict(
                    Strict::Add,
                    field(named(0, pair()), read),
                    headroom(10),
                )),
            ],
        )
    };
    let clear = || Lending {
        borrowed: vec![(0, ReferenceAccess::Mutable)],
        ..unit(
            vec![u32_type().into()],
            vec![store(0, Vec::new(), unsigned(0))],
        )
    };
    accepted(vec![lending(2, 1), clear()]);
    assert_eq!(
        unsafe_operations(judge(lower_lending(vec![lending(1, 0), clear()])).unwrap()),
        1
    );
}
