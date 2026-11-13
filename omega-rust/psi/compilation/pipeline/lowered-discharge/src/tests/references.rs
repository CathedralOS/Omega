// Formals passed by reference and the loans a call forms for them.
//
//     machine bump(x: &mut u32) requires x < 10 { x = x + 1; }
//     machine main() { let mut y: u32 = 3; bump(&mut y); }
//
// The callee reads and stores the caller's storage in place, its `requires`
// names what that storage holds at entry, and the caller lends a home of its
// local for the call: one `Loan` row, judged by structure. Two loans of one
// call that may share storage conflict when either is exclusive; two
// different fields of one record do not.

use super::*;
use crate::LoanConflict;
use language_core::ReferenceAccess;
use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralTypeId};
use structure_elaboration::{Borrowed, RecordField, RecordType};

/// One machine with formals that may be borrowed. `borrowed` lists each
/// borrowed formal's position and access; its `parameters` entry is the
/// referent type.
pub(super) struct Lending {
    pub(super) parameters: Vec<ValueType>,
    pub(super) borrowed: Vec<(usize, ReferenceAccess)>,
    pub(super) locals: Vec<ValueType>,
    pub(super) requires: Vec<Proposition>,
    pub(super) statements: Vec<Statement>,
}

pub(super) fn unit(parameters: Vec<ValueType>, statements: Vec<Statement>) -> Lending {
    Lending {
        parameters,
        borrowed: Vec::new(),
        locals: Vec::new(),
        requires: Vec::new(),
        statements,
    }
}

pub(super) fn pair() -> ValueType {
    ValueType::Record(id(1))
}

fn records() -> IdTable<StructuralTypeId, RecordType> {
    let field = |raw: u64, spelled: &str| RecordField {
        id: StructuralFieldId::new(raw).unwrap(),
        identity: spelled.to_owned(),
        value_type: ValueType::Scalar(u32_type()),
    };
    let mut records = IdTable::default();
    records.push(RecordType {
        copyable: true,
        identity: String::from("Pair"),
        shape: structure_elaboration::StorageShape::Record(vec![field(1, "a"), field(2, "b")]),
        requires: Vec::new(),
    });
    records
}

pub(super) fn lower_lending(machines: Vec<Lending>) -> Checked<structure_elaboration::Elaboration> {
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
    for machine in machines {
        let mut slots = machine.parameters.clone();
        slots.extend(machine.locals);
        bodies.push(Body {
            slots,
            statements: machine.statements,
        });
        let borrowed = machine
            .borrowed
            .iter()
            .map(|(position, access)| Borrowed {
                position: *position,
                access: *access,
                home: matches!(machine.parameters[*position], ValueType::Scalar(_))
                    .then(|| String::from("u32")),
            })
            .collect();
        signatures.push(Signature {
            parameters: machine.parameters,
            erased: Vec::new(),
            borrowed,
            result: None,
            requires: machine.requires,
            ensures: Vec::new(),
            reaches: structure_elaboration::Ceiling::Inferred,
            invokes: structure_elaboration::Ceiling::Inferred,
            crashes: CrashCeiling::Authored(Vec::new()),
            termination: Termination::Silent,
        });
    }
    structure_elaboration::lower_bodies(id(1), &bodies, &signatures, &records())
}

fn of(kind: ExpressionKind, result: ValueType) -> Expression {
    Expression {
        kind,
        result: Some(result),
        source: nowhere(),
    }
}

pub(super) fn named(slot: usize, value_type: ValueType) -> Expression {
    of(ExpressionKind::Slot(slot), value_type)
}

pub(super) fn field(base: Expression, field: usize) -> Expression {
    of(
        ExpressionKind::Field {
            base: Box::new(base),
            field,
        },
        u32_type().into(),
    )
}

/// `&place` or `&mut place`; `home` when the place is an SSA scalar local.
pub(super) fn lend(access: ReferenceAccess, place: Expression, home: bool) -> Expression {
    let result = place.result.unwrap();
    of(
        ExpressionKind::Borrow {
            access,
            place: Box::new(place),
            home: home.then(|| String::from("u32")),
        },
        result,
    )
}

pub(super) fn invoke(callee: u64, arguments: Vec<Expression>) -> Statement {
    Statement::Evaluate(Expression {
        kind: ExpressionKind::Call {
            callee: id::<MachineId>(callee).into(),
            arguments,
            designated: None,
        },
        result: None,
        source: nowhere(),
    })
}

pub(super) fn store(slot: usize, path: Vec<usize>, value: Expression) -> Statement {
    let mut destination = Expression {
        kind: ExpressionKind::Slot(slot),
        result: None,
        source: nowhere(),
    };
    for field in path {
        destination = Expression {
            kind: ExpressionKind::Field {
                base: Box::new(destination),
                field,
            },
            result: None,
            source: nowhere(),
        };
    }
    Statement::Store {
        destination,
        value,
        source: nowhere(),
    }
}

pub(super) fn pair_of(a: u128, b: u128) -> Expression {
    of(
        ExpressionKind::Record {
            fields: vec![(0, unsigned(a)), (1, unsigned(b))],
        },
        pair(),
    )
}

/// `x < 10` over the referent of the first place formal.
fn referent_below_ten() -> Proposition {
    let ScalarType::Integer(integer) = u32_type() else {
        unreachable!()
    };
    Proposition::LessThan(
        ScalarTerm::IntegerField {
            root: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            scalar_type: integer,
        },
        bound(u32_type(), IntegerValue::Unsigned(10)),
    )
}

/// `machine bump(x: &mut u32) requires x < 10 { x = x + 1; }`
fn bump() -> Lending {
    Lending {
        borrowed: vec![(0, ReferenceAccess::Mutable)],
        requires: vec![referent_below_ten()],
        ..unit(
            vec![u32_type().into()],
            vec![store(
                0,
                Vec::new(),
                strict(Strict::Add, named(0, u32_type().into()), unsigned(1)),
            )],
        )
    }
}

/// `machine two(a: &<access> u32, b: &<access> u32) {}`
fn two(first: ReferenceAccess, second: ReferenceAccess) -> Lending {
    Lending {
        borrowed: vec![(0, first), (1, second)],
        ..unit(vec![u32_type().into(), u32_type().into()], Vec::new())
    }
}

pub(super) fn accepted(machines: Vec<Lending>) -> Box<crate::AcceptedClosure> {
    match judge(lower_lending(machines)).unwrap() {
        RouteVerdict::Accepted(closure) => closure,
        verdict => panic!("the program must be accepted: {verdict:?}"),
    }
}

fn conflicts(machines: Vec<Lending>) -> Vec<crate::ConflictingUse> {
    match judge(lower_lending(machines)).unwrap() {
        RouteVerdict::Rejected {
            unproved,
            unestablished,
            conflicts,
            ..
        } if unproved.is_empty() && unestablished.is_empty() => conflicts,
        verdict => panic!("the program must be rejected for its loans: {verdict:?}"),
    }
}

#[test]
fn a_loan_of_a_local_is_formed_judged_and_published() {
    let main = Lending {
        locals: vec![u32_type().into()],
        ..unit(
            Vec::new(),
            vec![
                Statement::Bind {
                    slot: 0,
                    value: unsigned(3),
                },
                invoke(
                    2,
                    vec![lend(
                        ReferenceAccess::Mutable,
                        named(0, u32_type().into()),
                        true,
                    )],
                ),
            ],
        )
    };
    let closure = accepted(vec![main, bump()]);
    let tables = closure.elaboration.tables.machines.get(id(1)).unwrap();
    let loans: Vec<_> = tables
        .obligations
        .iter()
        .filter(|(_, row)| matches!(row.kind, ObligationKind::Loan { .. }))
        .collect();
    assert_eq!(loans.len(), 1, "one loan for one borrowed argument");
    let (loan, row) = loans[0];
    assert!(matches!(
        row.kind,
        ObligationKind::Loan {
            access: ReferenceAccess::Mutable,
            lender: terminal_check_tables::obligations::Lender::Owner,
            ..
        }
    ));
    let verdict = closure
        .discharge
        .machines
        .get(id(1))
        .unwrap()
        .verdicts
        .get(loan);
    assert_eq!(verdict, Some(&Ok(Discharged::Lent)));
    // The callee's requirement is proved over what the home holds at the
    // call, and its addition from the entry content of its own place.
    for (_, result) in closure.discharge.machines.iter() {
        for (_, verdict) in result.verdicts.iter() {
            assert!(verdict.is_ok(), "{verdict:?}");
        }
    }
    let callee = closure.elaboration.tables.machines.get(id(2)).unwrap();
    assert!(
        callee
            .obligations
            .iter()
            .any(|(_, row)| matches!(row.kind, ObligationKind::OperationSafety { .. }))
    );
    // The module the route checked is valid Terminal Psi.
    closure.publish().unwrap();
}

#[test]
fn a_call_that_breaks_the_requirement_of_a_borrowed_formal_is_rejected() {
    let main = Lending {
        locals: vec![u32_type().into()],
        ..unit(
            Vec::new(),
            vec![
                Statement::Bind {
                    slot: 0,
                    value: unsigned(10),
                },
                invoke(
                    2,
                    vec![lend(
                        ReferenceAccess::Mutable,
                        named(0, u32_type().into()),
                        true,
                    )],
                ),
            ],
        )
    };
    let lowered = lower_lending(vec![main, bump()]);
    match judge(lowered).unwrap() {
        RouteVerdict::Rejected { unproved, .. } => {
            assert!(
                unproved
                    .iter()
                    .any(|entry| matches!(entry.kind, ObligationKind::CallRequirement { .. })),
                "{unproved:?}"
            );
        }
        verdict => panic!("10 < 10 does not hold: {verdict:?}"),
    }
}

#[test]
fn two_exclusive_loans_of_one_local_conflict() {
    let main = Lending {
        locals: vec![u32_type().into()],
        ..unit(
            Vec::new(),
            vec![
                Statement::Bind {
                    slot: 0,
                    value: unsigned(3),
                },
                invoke(
                    2,
                    vec![
                        lend(ReferenceAccess::Mutable, named(0, u32_type().into()), true),
                        lend(ReferenceAccess::Mutable, named(0, u32_type().into()), true),
                    ],
                ),
            ],
        )
    };
    let found = conflicts(vec![
        main,
        two(ReferenceAccess::Mutable, ReferenceAccess::Mutable),
    ]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].conflict, LoanConflict::TwiceExclusive);
    assert!(found[0].message().contains("as mutable more than once"));
}

#[test]
fn loans_of_two_fields_of_one_record_do_not_conflict_but_of_a_field_and_its_record_do() {
    let main = |second: Expression, access: ReferenceAccess| Lending {
        locals: vec![pair()],
        ..unit(
            Vec::new(),
            vec![
                Statement::Bind {
                    slot: 0,
                    value: pair_of(1, 2),
                },
                invoke(
                    2,
                    vec![
                        lend(ReferenceAccess::Mutable, field(named(0, pair()), 0), false),
                        lend(access, second, false),
                    ],
                ),
            ],
        )
    };
    let disjoint = main(field(named(0, pair()), 1), ReferenceAccess::Mutable);
    accepted(vec![
        disjoint,
        two(ReferenceAccess::Mutable, ReferenceAccess::Mutable),
    ]);
    let same_field = main(field(named(0, pair()), 0), ReferenceAccess::Shared);
    let found = conflicts(vec![
        same_field,
        two(ReferenceAccess::Mutable, ReferenceAccess::Shared),
    ]);
    assert_eq!(found[0].conflict, LoanConflict::ExclusiveAndShared);
    // The record and one of its fields share storage.
    let whole = Lending {
        locals: vec![pair()],
        ..unit(
            Vec::new(),
            vec![
                Statement::Bind {
                    slot: 0,
                    value: pair_of(1, 2),
                },
                invoke(
                    2,
                    vec![
                        lend(ReferenceAccess::Mutable, field(named(0, pair()), 0), false),
                        lend(ReferenceAccess::Shared, named(0, pair()), false),
                    ],
                ),
            ],
        )
    };
    let holder = Lending {
        borrowed: vec![(0, ReferenceAccess::Mutable), (1, ReferenceAccess::Shared)],
        ..unit(vec![u32_type().into(), pair()], Vec::new())
    };
    assert_eq!(
        conflicts(vec![whole, holder])[0].conflict,
        LoanConflict::ExclusiveAndShared
    );
}

#[test]
fn an_exclusive_loan_changes_only_the_lent_field() {
    // `setb(&mut q.a); q.b + 1` and `q.a + 1` after `q = Pair { a: MAX, b: 2 }`:
    // the sibling still holds 2, the lent field holds what the callee left.
    let after = |read: usize| Lending {
        locals: vec![pair()],
        ..unit(
            Vec::new(),
            vec![
                Statement::Bind {
                    slot: 0,
                    value: pair_of(u128::from(u32::MAX), 2),
                },
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
                    unsigned(1),
                )),
            ],
        )
    };
    let set = || Lending {
        borrowed: vec![(0, ReferenceAccess::Mutable)],
        ..unit(
            vec![u32_type().into()],
            vec![store(0, Vec::new(), unsigned(0))],
        )
    };
    accepted(vec![after(1), set()]);
    match judge(lower_lending(vec![after(0), set()])).unwrap() {
        RouteVerdict::Rejected { unproved, .. } => assert!(
            unproved
                .iter()
                .all(|entry| matches!(entry.kind, ObligationKind::OperationSafety { .. })),
            "{unproved:?}"
        ),
        verdict => panic!("the lent field holds nothing known: {verdict:?}"),
    }
}

#[test]
fn a_borrowed_formal_passed_on_reborrows_its_incoming_loan() {
    // `forward(v: &mut u32) { set(v) }` and `peek(v: &u32) { set(v) }`.
    let forward = |access: ReferenceAccess| Lending {
        borrowed: vec![(0, access)],
        ..unit(
            vec![u32_type().into()],
            vec![invoke(
                2,
                vec![lend(
                    ReferenceAccess::Mutable,
                    named(0, u32_type().into()),
                    false,
                )],
            )],
        )
    };
    let set = Lending {
        borrowed: vec![(0, ReferenceAccess::Mutable)],
        ..unit(
            vec![u32_type().into()],
            vec![store(0, Vec::new(), unsigned(7))],
        )
    };
    let closure = accepted(vec![forward(ReferenceAccess::Mutable), set]);
    let tables = closure.elaboration.tables.machines.get(id(1)).unwrap();
    assert!(tables.obligations.iter().any(|(_, row)| matches!(
        row.kind,
        ObligationKind::Loan {
            lender: terminal_check_tables::obligations::Lender::Incoming,
            ..
        }
    )));
    // A shared reference grants no exclusive child.
    let set = Lending {
        borrowed: vec![(0, ReferenceAccess::Mutable)],
        ..unit(
            vec![u32_type().into()],
            vec![store(0, Vec::new(), unsigned(7))],
        )
    };
    let found = conflicts(vec![forward(ReferenceAccess::Shared), set]);
    assert_eq!(found[0].conflict, LoanConflict::ExclusiveOfShared);
}

#[test]
fn a_loan_row_the_lowered_form_does_not_owe_is_a_defect() {
    let main = Lending {
        locals: vec![u32_type().into()],
        ..unit(
            Vec::new(),
            vec![
                Statement::Bind {
                    slot: 0,
                    value: unsigned(3),
                },
                invoke(
                    2,
                    vec![lend(
                        ReferenceAccess::Mutable,
                        named(0, u32_type().into()),
                        true,
                    )],
                ),
            ],
        )
    };
    let mut elaboration = lower_lending(vec![main, bump()]).unwrap();
    let tables = elaboration.tables.machines.get_mut(id(1)).unwrap();
    let loans: Vec<ObligationId> = tables
        .obligations
        .iter()
        .filter(|(_, row)| matches!(row.kind, ObligationKind::Loan { .. }))
        .map(|(loan, _)| loan)
        .collect();
    for loan in loans {
        if let ObligationKind::Loan { access, .. } =
            &mut tables.obligations.get_mut(loan).unwrap().kind
        {
            *access = ReferenceAccess::Shared;
        }
    }
    let refused = judge(Ok(elaboration)).unwrap_err();
    assert_eq!(refused.reason, RefusalReason::ForeignIdentity);
}

#[test]
fn a_home_a_loan_gives_a_local_in_an_arm_is_not_the_continuations() {
    // `machine main(c: bool) { let mut y: u32 = 3;
    //   transition c { true -> set(&mut y) } y + 1; }`: the arm completes
    // the machine, so the continuation still reads the value `y` was bound
    // to, which every path establishes.
    let set = Lending {
        borrowed: vec![(0, ReferenceAccess::Mutable)],
        ..unit(
            vec![u32_type().into()],
            vec![store(0, Vec::new(), unsigned(7))],
        )
    };
    let main = Lending {
        locals: vec![u32_type().into()],
        ..unit(
            vec![ScalarType::Boolean.into()],
            vec![
                Statement::Bind {
                    slot: 1,
                    value: unsigned(3),
                },
                Statement::Arm {
                    guard: Some(named(0, ScalarType::Boolean.into())),
                    target: Target::Value(Expression {
                        kind: ExpressionKind::Call {
                            callee: id::<MachineId>(2).into(),
                            arguments: vec![lend(
                                ReferenceAccess::Mutable,
                                named(1, u32_type().into()),
                                true,
                            )],
                            designated: None,
                        },
                        result: None,
                        source: nowhere(),
                    }),
                    source: nowhere(),
                },
                Statement::Evaluate(strict(
                    Strict::Add,
                    named(1, u32_type().into()),
                    unsigned(1),
                )),
            ],
        )
    };
    accepted(vec![main, set]);
}
