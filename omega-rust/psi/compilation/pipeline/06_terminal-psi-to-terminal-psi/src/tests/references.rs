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
use typed_trees_to_terminal_psi::{Borrowed, RecordField, RecordType};

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
    ValueType::record(id(1))
}

fn records() -> IdTable<StructuralTypeId, RecordType> {
    let field = |raw: u64, spelled: &str| RecordField {
        id: StructuralFieldId::new(raw).unwrap(),
        identity: spelled.to_owned(),
        value_type: ValueType::plain(u32_type()),
        byte_predicates: semantic_vocabulary::ByteSequencePredicateSet::EMPTY,
    };
    let mut records = IdTable::default();
    records.push(RecordType {
        authored_layout: false,
        owns_linear_claim: false,
        copyable: true,
        cleanup: None,
        identity: String::from("Pair"),
        shape: typed_trees_to_terminal_psi::StorageShape::Record(vec![
            field(1, "a"),
            field(2, "b"),
        ]),
        requires: Vec::new(),
    });
    records
}

pub(super) fn lower_lending(
    machines: Vec<Lending>,
) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
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
                referent: typed_trees_to_terminal_psi::BorrowedReferent::Value,
                position: *position,
                access: *access,
                regions: Default::default(),
                home: matches!(machine.parameters[*position], ValueType::Scalar(..))
                    .then(|| String::from("u32")),
            })
            .collect();
        signatures.push(Signature {
            operational: Default::default(),
            parameters: machine.parameters,
            erased: Vec::new(),
            borrowed,
            result: None,
            requires: machine.requires,
            ensures: Vec::new(),
            reaches: typed_trees_to_terminal_psi::Ceiling::Inferred,
            invokes: typed_trees_to_terminal_psi::Ceiling::Inferred,
            crashes: CrashCeiling::Authored(Vec::new()),
            termination: Termination::Silent,
            ranking: None,
        });
    }
    let mut elaboration =
        typed_trees_to_terminal_psi::lower_bodies(id(1), &bodies, &signatures, &records())?;
    declare_body_models(&mut elaboration, &signatures);
    Ok(elaboration)
}

fn of(kind: ExpressionKind, result: ValueType) -> Expression {
    Expression {
        call_context: None,
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
    Statement::discard(Expression {
        call_context: None,
        kind: ExpressionKind::Call {
            required_contract: None,
            static_reach_binding: None,
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
        call_context: None,
        kind: ExpressionKind::Slot(slot),
        result: None,
        source: nowhere(),
    };
    for field in path {
        destination = Expression {
            call_context: None,
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
            version: semantic_vocabulary::ContentPlaceVersion::Current,
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
fn a_build_dispatch_captures_a_reference_without_copying_its_referent() {
    let program = |body: &str| {
        format!(
            r#"
data Source<'path> {{
    case Path(location: &'path [u8]);
    case Empty;
}}
machine build_entry(source: &Source) {{ {body} }}
machine consume(location: &[u8]) {{ }}
machine done() {{ }}
"#
        )
    };
    let selected = program(
        "transition source { Source::Path { location } -> consume(location) Source::Empty -> done() }",
    );
    let verdict = super::build_purpose::judge_build(&selected).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("the selected payload view must pass through the Build helper: {verdict:?}");
    };
    accepted.publish_invocation().unwrap();

    let unselected = program("consume(source.location);");
    let verdict = super::build_purpose::judge_build(&unselected).unwrap();
    assert!(
        matches!(&verdict, RouteVerdict::Rejected { unestablished, .. }
            if unestablished.iter().any(|failure| failure.detail.contains("cannot prove selected case for payload access"))),
        "the same source call without a selected case must reject: {verdict:?}"
    );
}

#[test]
fn a_captured_reference_keeps_the_original_storage_for_overlap_checks() {
    let caller = Lending {
        borrowed: vec![(0, ReferenceAccess::Mutable)],
        locals: vec![pair()],
        ..unit(
            vec![pair()],
            vec![
                Statement::CaptureReference {
                    slot: 1,
                    original: 0,
                },
                invoke(
                    2,
                    vec![
                        lend(ReferenceAccess::Mutable, named(0, pair()), false),
                        lend(ReferenceAccess::Shared, named(1, pair()), false),
                    ],
                ),
            ],
        )
    };
    let callee = Lending {
        borrowed: vec![(0, ReferenceAccess::Mutable), (1, ReferenceAccess::Shared)],
        ..unit(vec![pair(), pair()], Vec::new())
    };
    let found = conflicts(vec![caller, callee]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].conflict, LoanConflict::ExclusiveAndShared);
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
                Statement::discard(strict(
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
                        call_context: None,
                        kind: ExpressionKind::Call {
                            required_contract: None,
                            static_reach_binding: None,
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
                Statement::discard(strict(
                    Strict::Add,
                    named(1, u32_type().into()),
                    unsigned(1),
                )),
            ],
        )
    };
    accepted(vec![main, set]);
}

#[test]
fn a_selected_reference_result_needs_its_public_requirement_contract() {
    // `hand` elides its lifetimes, so its requirement relates the result to
    // `source` by lifetimes.md's elision rule, and the selected call in
    // `relay` reads that public relation. Erasing the call's public header
    // would leave only the implementation's private result origin, and the
    // checker refuses the call rather than adopt it.
    use terminal_check_tables::compilation::TerminalCompilation;
    use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus, TerminalProgram};
    let text = r#"
trait Handoff {
    machine hand(source: &mut i64) -> &mut i64 crashes none;
}
Direct satisfies Handoff {
    machine hand(source: &mut i64) -> &mut i64 crashes none { source }
}
machine relay<Chosen satisfies Handoff>(source: &mut i64) crashes none {
    let child: &mut i64 = Chosen::hand(source);
    child = 41;
}
machine entry() crashes none {
    let mut backing: i64 = 17;
    relay<Direct>(&mut backing);
}
"#;
    let (typed, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        super::boundary_calls::typed(text),
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap();
    let entry = super::boundary_calls::entry(&typed);
    let prepare = || {
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .program
    };
    // The product closure is accepted and runs. Its generic check form is
    // `a_rigid_requirement_call_borrows_its_declared_input_region`.
    let RouteVerdict::Accepted(accepted) = crate::judge_compilation(prepare()).remove(0).unwrap()
    else {
        panic!("the selected call must read its public requirement contract");
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    assert_eq!(
        instance
            .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    let mut erased = prepare();
    let TerminalCompilation::Prepared(prepared) = &mut erased else {
        panic!("the selected program prepares");
    };
    let mut headers = 0;
    for operation in prepared
        .closure
        .as_mut()
        .unwrap()
        .module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
    {
        headers += usize::from(operation.required_contract.take().is_some());
    }
    assert_eq!(
        headers, 1,
        "exactly the selected call carries a public header"
    );
    let Err(crate::CompilationRefusal::Elaboration(refused)) =
        crate::judge_compilation(erased).remove(0)
    else {
        panic!("a selected call without its public header must be refused");
    };
    assert_eq!(refused.reason, RefusalReason::ForeignIdentity);
    assert_eq!(
        refused.detail,
        "invocation row and lowered call name different targets"
    );

    let mut missing_lifetimes = prepare();
    let TerminalCompilation::Prepared(prepared) = &mut missing_lifetimes else {
        panic!("the selected program prepares");
    };
    let module = &mut prepared.closure.as_mut().unwrap().module;
    let (caller, operation, callee, required) = module
        .machines
        .iter()
        .find_map(|machine| {
            machine.operations().find_map(|operation| {
                let callee = operation.kind.declared_invocation()?.callee;
                let semantic_vocabulary::TargetMachineCallable::Machine(target) = callee else {
                    return None;
                };
                Some((
                    machine.id,
                    operation.id,
                    target,
                    operation.required_contract?,
                ))
            })
        })
        .expect("the selected call has its exact public header and callee");
    assert!(
        terminal_semantics::selected_reference_result(module, caller, callee, required).is_ok()
    );
    let header = module
        .declaration_contracts
        .iter_mut()
        .find(|contract| contract.id == required)
        .expect("the selected call has its public header");
    assert!(
        header.result_lifetimes.take().is_some(),
        "erase only the selected public header's lifetime groups"
    );
    let reason = "public reference result has an unresolved declared lifetime frontier";
    assert_eq!(
        terminal_semantics::selected_reference_result(module, caller, callee, required).err(),
        Some(reason),
    );
    let Err(graph_refusal) =
        terminal_semantics::descriptor_results::terminal_borrowed_storage_graph_from(
            module,
            [caller],
            &[],
        )
    else {
        panic!("the descriptor graph must refuse the missing lifetimes");
    };
    assert_eq!(graph_refusal.machine(), caller);
    assert_eq!(graph_refusal.operation(module), Some(operation));
    let detail = graph_refusal.in_module(module);
    assert!(detail.starts_with(reason), "{detail}");
    let rejected = crate::judge_compilation(missing_lifetimes).remove(0);
    let Ok(RouteVerdict::FallBack(refused)) = rejected else {
        panic!("missing lifetime evidence must refuse the selected call: {rejected:?}");
    };
    assert_eq!(refused.reason, RefusalReason::Unsupported);
    assert_eq!(refused.machine, caller);
    assert_eq!(
        refused.concerns,
        terminal_check_tables::Construct::Operation(operation)
    );
    assert_eq!(refused.detail, format!("{detail} in machine `relay`"));
}

#[test]
fn an_original_returned_reference_keeps_its_backing_and_excludes_its_parent() {
    use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus, TerminalProgram};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../../samples/cli/facts/reference_arrival_handoff");
    let prepared = |path: &std::path::Path| {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("reading reference source {}: {error}", path.display()));
        let typed = super::boundary_calls::typed(&text);
        let entry = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "main")
            .unwrap()
            .symbol;
        typed_trees_to_terminal_psi::elaborate_for(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
    };
    // These are the retained source programs, including the original 17/41
    // values and the invalid write to the parent. No test rewrites the body.
    let positive = prepared(&root.join("call_result/main.omg")).unwrap();
    // A foreign result header cannot substitute for the body's actual
    // borrowed source, even at the same caller/callee operation coordinates.
    let mut changed = positive.module.clone();
    let (caller, operation, callee) = changed
        .machines
        .iter()
        .find_map(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|operation| {
                    let call = operation.kind.declared_invocation()?;
                    let terminal_psi::TargetMachineCallable::Machine(callee) = call.callee else {
                        return None;
                    };
                    changed
                        .machine(callee)?
                        .result
                        .structural()
                        .filter(|result| !result.reference_sources.is_empty())?;
                    Some((machine.id, operation.id, callee))
                })
        })
        .expect("the original source calls a reference-returning helper");
    let terminal_psi::TerminalMachineResult::Structural(changed_result) = &mut changed
        .machines
        .iter_mut()
        .find(|machine| machine.id == callee)
        .unwrap()
        .contract
        .result
    else {
        panic!("the helper has a structural reference result")
    };
    changed_result.reference_sources[0].sources.clear();
    let graph = terminal_semantics::descriptor_results::terminal_borrowed_storage_graph_from(
        &changed,
        [caller],
        &[],
    )
    .unwrap();
    let mut analysis =
        terminal_semantics::descriptor_results::DescriptorAnalysis::with_reference_slots(
            &changed.structural_types,
            &graph,
        );
    let caller = changed.machine(caller).unwrap();
    let refusal = match terminal_semantics::reference_call_lifetimes(
        &changed,
        caller,
        caller.operation(operation).unwrap(),
        &mut analysis,
    ) {
        Err(reason) => reason,
        Ok(_) => panic!("changed result origins must not supply a retained loan"),
    };
    assert_eq!(
        refusal.detail(),
        Some("returned reference header differs from its actual body origins")
    );
    assert_eq!(refusal.machine(), caller.id);
    assert_eq!(refusal.operation(&changed), Some(operation));

    // A changed instruction cannot borrow the source site of the original.
    let actual_operation = caller.operation(operation).unwrap().clone();
    let mut different = actual_operation.clone();
    different.kind = terminal_psi::OperationKind::EstablishStructuralLocal {};
    let reason = refusal.detail().expect("the original header refusal");
    let unattributed = terminal_semantics::descriptor_results::DescriptorGraphError::at_operation(
        &changed, caller.id, &different, reason,
    );
    assert_eq!(unattributed.detail(), Some(reason));
    assert_eq!(unattributed.machine(), caller.id);
    assert_eq!(unattributed.operation(&changed), None);

    // The ID is ambiguous even with exactly one fully matching instruction.
    let mut duplicated = changed.clone();
    let body = duplicated
        .machines
        .iter_mut()
        .find(|body| body.id == caller.id)
        .unwrap();
    let block = body
        .blocks
        .iter_mut()
        .find(|block| block.operations.iter().any(|row| row.id == operation))
        .unwrap();
    block.operations.push(different);
    let body = duplicated.machine(caller.id).unwrap();
    assert_eq!(
        body.operations().filter(|row| row.id == operation).count(),
        2
    );
    assert_eq!(
        body.operations()
            .filter(|row| *row == &actual_operation)
            .count(),
        1
    );
    let unattributed = terminal_semantics::descriptor_results::DescriptorGraphError::at_operation(
        &duplicated,
        caller.id,
        &actual_operation,
        reason,
    );
    assert_eq!(unattributed.detail(), Some(reason));
    assert_eq!(unattributed.machine(), caller.id);
    assert_eq!(unattributed.operation(&duplicated), None);

    let positive = crate::judge(Ok(positive)).unwrap();
    let RouteVerdict::Accepted(accepted) = positive else {
        panic!("{positive:?}");
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    assert_eq!(
        instance
            .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    let negative = crate::judge(prepared(&root.join("invalid_call_result/main.omg"))).unwrap();
    let RouteVerdict::Rejected {
        formations,
        violations,
        unproved,
        unestablished,
        conflicts,
        exceeded,
    } = negative
    else {
        panic!("the invalid parent write must be a program rejection: {negative:?}");
    };
    assert!(
        formations.is_empty()
            && violations.is_empty()
            && unproved.is_empty()
            && unestablished.is_empty()
            && exceeded.is_empty()
    );
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].conflict, LoanConflict::ChangeWhileBorrowed);
    assert!(matches!(
        conflicts[0].site,
        terminal_check_tables::program::SourceOrigin::Authored(_)
    ));
    assert_eq!(
        conflicts[0].statement,
        "storage is changed while a conflicting reference remains live"
    );
}

// The permission created by a reference formation has its own lifecycle,
// independently of whether its shared reference type admits later copies.
const SHARED_REFERENCE_FORMATION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../tests/fixtures/run/ownership/shared_reference_formation/main.omg"
));

fn shared_reference_source() -> String {
    SHARED_REFERENCE_FORMATION.replace("machine main()", "machine build_entry()")
}

#[test]
fn a_shared_reference_formation_owns_its_fresh_permission_carrier() {
    let source = shared_reference_source();
    let verdict = super::build_purpose::judge_build(&source).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("the source's shared field reference must be checked: {verdict:?}");
    };
    let formed = accepted
        .elaboration
        .module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::EstablishReference { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        formed.len(),
        1,
        "the source forms exactly one persistent reference"
    );
    let result = formed[0].result.structural().unwrap();
    assert!(
        accepted
            .elaboration
            .module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == result.structural_type)
            .unwrap()
            .copyable,
        "the shared reference type retains copy permission"
    );
    assert_eq!(
        result.multiplicity,
        terminal_psi::StructuralMultiplicity::Affine,
        "the formation must own its fresh permission carrier: {:?}",
        formed[0]
    );
    terminal_semantics::structural_effect_leaf_observation(formed[0]).unwrap();
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
        .unwrap();
    assert!(
        matches!(
            status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Unit
            )
        ),
        "forming the reference must preserve the caller's seven-value oracle: {status:?}"
    );
}

#[test]
fn a_shared_reference_copy_keeps_the_original_formation_owned() {
    let source = shared_reference_source().replace(
        "observe(held);",
        "let copied: &u32 = held; observe(held); observe(copied);",
    );
    let verdict = super::build_purpose::judge_build(&source).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("shared reference copying must preserve both loans: {verdict:?}");
    };
    let operations = accepted
        .elaboration
        .module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let formation = operations
        .iter()
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::EstablishReference { .. }
            )
        })
        .unwrap()
        .result
        .structural()
        .unwrap();
    assert_eq!(
        formation.multiplicity,
        terminal_psi::StructuralMultiplicity::Affine
    );
    assert!(
        operations.iter().any(|operation| matches!(
            operation.kind,
            terminal_psi::OperationKind::StructuralLeafCopy { .. }
        ) && operation
            .result
            .structural()
            .is_some_and(|result| result.structural_type == formation.structural_type
                && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted)),
        "a later descriptor copy keeps the shared type's unrestricted copy permission"
    );
    // Ordinary copies still have their type's unrestricted custody. The
    // creation rule cannot authorize an affine retagging of that copy.
    let (machine_id, copy_id) = accepted
        .elaboration
        .module
        .machines
        .iter()
        .find_map(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::StructuralLeafCopy { .. }
                    ) && operation
                        .result
                        .structural()
                        .is_some_and(|result| result.structural_type == formation.structural_type)
                })
                .map(|operation| (machine.id, operation.id))
        })
        .unwrap();
    let mut retagged = accepted
        .elaboration
        .module
        .machine(machine_id)
        .unwrap()
        .clone();
    let result = retagged
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| operation.id == copy_id)
        .unwrap();
    let terminal_psi::OperationResult::Structural(result) = &mut result.result else {
        unreachable!()
    };
    result.multiplicity = terminal_psi::StructuralMultiplicity::Affine;
    assert!(matches!(terminal_semantics::plain_affine::plan(&retagged,
        &accepted.elaboration.module.structural_types),
        Err(terminal_semantics::plain_affine::Error::Malformed { detail, .. })
            if detail == "place custody disagrees with its exact type declaration"));
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
        .unwrap();
    assert!(
        matches!(
            status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Unit
            )
        ),
        "both shared copies must close before the root replacement: {status:?}"
    );
}

#[test]
fn a_shared_reference_still_excludes_replacement_until_its_last_use() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/fixtures/fail/ownership/reference_live_across_replacement/main.omg"
    ))
    .replace("machine main()", "machine build_entry()");
    let verdict = super::build_purpose::judge_build(&source).unwrap();
    let RouteVerdict::Rejected { conflicts, .. } = verdict else {
        panic!("the captured field loan must exclude its overlapping store: {verdict:?}");
    };
    assert!(
        !conflicts.is_empty(),
        "the live reference must retain its loan conflict"
    );
}

/// `cell = Cell { .. }` replaces the record a live `&cell` observes, so the
/// checker must see that write. The positive twin uses the reference before
/// the assignment and also reassigns a local, a declared local and an owned
/// parameter after a whole move, a `[copy]` record, and a record whose
/// replacement reads its old contents.
#[test]
fn a_whole_record_assignment_writes_the_storage_its_references_observe() {
    let negative = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/fixtures/fail/ownership/reference_live_across_whole_record_assignment/main.omg"
    ))
    .replace("machine main()", "machine build_entry()");
    let verdict = super::build_purpose::judge_build(&negative).unwrap();
    let RouteVerdict::Rejected { conflicts, .. } = verdict else {
        panic!("the live reference must exclude the whole assignment: {verdict:?}");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].conflict, LoanConflict::ChangeWhileBorrowed);
    let positive = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/fixtures/run/ownership/whole_record_assignment/main.omg"
    ))
    .replace("machine main()", "machine build_entry()");
    let verdict = super::build_purpose::judge_build(&positive).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("assignments after the last use must be accepted: {verdict:?}");
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    assert_eq!(
        instance
            .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
            .unwrap(),
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        )
    );
}

#[test]
fn canonical_reference_creation_rejects_an_unowned_carrier() {
    // The direct producer lookup and supplied-row validation share one
    // lifecycle catalog. Only the creation owns a new permission carrier.
    for row in terminal_semantics::StructuralEffectSemanticRow::ALL {
        assert_eq!(
            terminal_semantics::StructuralEffectSemanticRow::for_tag(row.tag()),
            Some(row)
        );
    }
    assert!(
        terminal_semantics::StructuralEffectSemanticRow::for_tag(
            terminal_semantics::OperationSemanticTag::BooleanConstant
        )
        .is_none()
    );
    let verdict = super::build_purpose::judge_build(&shared_reference_source()).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("{verdict:?}");
    };
    // A receiving type cannot stand in for the field's actual primitive
    // declaration. Keep the exact producer-published carrier independently.
    let mut wrong_carrier_types = accepted.elaboration.module.structural_types.clone();
    let owner = wrong_carrier_types
        .iter()
        .position(|declaration| {
            declaration.shape.direct_fields().is_some_and(|fields| {
                fields.iter().any(|field| {
                    matches!(
                        field.field_type,
                        terminal_psi::StructuralFieldType::Scalar(_)
                    ) && field.inline_carrier.is_some()
                })
            })
        })
        .unwrap();
    let reference_type = wrong_carrier_types
        .iter()
        .find(|declaration| {
            matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::Reference { .. }
            )
        })
        .unwrap()
        .id;
    let terminal_psi::StructuralTypeShape::Record { fields, .. } =
        &mut wrong_carrier_types[owner].shape
    else {
        panic!("the source's Item field must remain a record")
    };
    fields[0].inline_carrier = Some(reference_type);
    assert!(
        terminal_semantics::validate_inline_carriers(
            &wrong_carrier_types,
            &wrong_carrier_types[owner]
        )
        .is_err()
    );
    let mut foreign_producer = accepted
        .elaboration
        .module
        .machines
        .iter()
        .find(|machine| {
            machine.operations().any(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::EstablishReference { .. }
                )
            })
        })
        .unwrap()
        .clone();
    let formed = foreign_producer
        .operations()
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::EstablishReference { .. }
            )
        })
        .unwrap();
    let result_place = formed.result.structural().unwrap().place;
    let wrong_producer = semantic_vocabulary::OperationId::new(formed.id.get() + 1).unwrap();
    let declaration = foreign_producer
        .structural_places
        .iter_mut()
        .find(|place| place.id == result_place)
        .unwrap();
    let semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. } =
        &mut declaration.kind
    else {
        unreachable!()
    };
    *producer = wrong_producer;
    assert!(
        matches!(terminal_semantics::plain_affine::plan(&foreign_producer,
        &accepted.elaboration.module.structural_types),
        Err(terminal_semantics::plain_affine::Error::Malformed { detail, .. })
            if detail == "permission result has no exact reference producer declaration")
    );
    let mut formed = accepted
        .elaboration
        .module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::EstablishReference { .. }
            )
        })
        .unwrap()
        .clone();
    let terminal_psi::OperationResult::Structural(result) = &mut formed.result else {
        unreachable!()
    };
    result.multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted;
    assert_eq!(
        terminal_semantics::structural_effect_leaf_observation(&formed),
        Err(
            terminal_semantics::OperationSemanticError::StructuralEffectActionShapeMismatch(
                terminal_semantics::OperationSemanticTag::EstablishReference
            )
        )
    );
}

/// loans.md, "Reborrow lineage and access": a Mutable parent may form a
/// Shared child ("Freeze mutation until the complete shared cohort ends;
/// restore once"). An exclusive loan the program writes for a parameter that
/// only observes is that: the place must admit the exclusive loan, and the
/// callee receives observation. Each refused or rejected twin differs from
/// an accepted program in the one thing named beside it.
#[test]
fn an_exclusive_loan_is_lent_on_to_a_parameter_that_only_observes() {
    use super::build_purpose::judge_build;
    let verdict = |build: &str| match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
            "accepted".to_owned()
        }
        Ok(RouteVerdict::Rejected { violations, .. }) => violations
            .first()
            .map_or("rejected".to_owned(), |first| first.statement.clone()),
        other => panic!("{other:?}"),
    };
    let program = |binding: &str, call: &str| {
        format!(
            "data Pair {{ left: u32; right: u32; }}
machine see(pair: &Pair) -> u32 {{ pair.left }}
machine both(first: &Pair, second: &Pair) -> u32 {{ first.left }}
machine relay(pair: &mut Pair) -> u32 {{ see(&mut pair) }}
machine watch(pair: &Pair) -> u32 {{ see(&pair) }}
machine build_entry() {{
    let {binding}: Pair = Pair {{ left: 1, right: 2 }};
    let mut other: Pair = Pair {{ left: 3, right: 4 }};
    use({call});
    pair_left(pair.left);
}}
machine use(seen: u32) {{ }}
machine pair_left(left: u32) {{ }}"
        )
    };
    // The loan as written, through a reference the machine holds, and beside
    // a loan of other storage.
    for call in [
        "see(&mut pair)",
        "relay(&mut pair)",
        "watch(&pair)",
        "both(&mut pair, &other)",
        "both(&mut pair, &mut other)",
    ] {
        assert_eq!(verdict(&program("mut pair", call)), "accepted", "{call}");
    }
    // The place does not admit an exclusive loan.
    assert_eq!(
        verdict(&program("pair", "see(&mut pair)")),
        "call exclusive loan of an immutable binding while reading the body of `build_entry`"
    );
    // The exclusive loan overlaps another loan of its storage.
    for call in ["both(&mut pair, &pair)", "both(&pair, &mut pair)"] {
        let judged = judge_build(&program("mut pair", call));
        assert!(
            matches!(&judged,
                Ok(RouteVerdict::Rejected { conflicts, .. }) if conflicts.iter().any(|conflict|
                    conflict.conflict == LoanConflict::ExclusiveAndShared
                        && conflict.message().contains("conflicting loans"))
            ),
            "the actual exclusive formation must conflict: {call}: {judged:?}"
        );
    }
    // The other direction has no row: observation does not become mutation.
    assert!(
        verdict(&program("mut pair", "relay(&pair)")).contains("lends only immutable access"),
        "{}",
        verdict(&program("mut pair", "relay(&pair)"))
    );
}

/// `&mut item` written for a `&Item` formal is an exclusive formation that
/// lends shared access (loans.md, "Reborrow lineage and access"). `cohort`
/// places the shared reference `view`'s uses around that call.
fn exclusive_formation_beside(cohort: &str) -> String {
    format!(
        "data Item {{ value: u32; }}
machine observe(value: &Item) {{ }}
machine build_entry() {{
    let mut item: Item = Item {{ value: 3 }};
    let view: &Item = &item;
{cohort}
}}"
    )
}

#[test]
fn an_exclusive_formation_for_a_shared_formal_excludes_a_live_shared_cohort() {
    let verdict = super::build_purpose::judge_build(&exclusive_formation_beside(
        "    observe(&mut item);\n    observe(view);",
    ))
    .unwrap();
    let RouteVerdict::Rejected { conflicts, .. } = verdict else {
        panic!("the written exclusive loan overlaps the live shared view: {verdict:?}");
    };
    assert!(
        !conflicts.is_empty(),
        "the exclusive formation must conflict with the shared cohort"
    );
}

#[test]
fn an_exclusive_formation_for_a_shared_formal_follows_an_ended_cohort() {
    for cohort in [
        // The view's last use precedes the exclusive formation.
        "    observe(view);\n    observe(&mut item);",
        // An ordinary shared loan joins the live cohort.
        "    observe(&item);\n    observe(view);",
    ] {
        let verdict =
            super::build_purpose::judge_build(&exclusive_formation_beside(cohort)).unwrap();
        assert!(
            matches!(verdict, RouteVerdict::Accepted(_)),
            "{cohort}: {verdict:?}"
        );
    }
}

#[test]
fn a_shared_reference_formed_from_an_exclusive_loan_holds_it_exclusively() {
    let source = |uses: &str| {
        format!(
            "data Item {{ value: u32; }}
machine observe(value: &Item) {{ }}
machine build_entry() {{
    let mut item: Item = Item {{ value: 3 }};
    let view: &Item = &mut item;
{uses}
}}"
        )
    };
    for uses in [
        "observe(&item); observe(view);",
        "let child: &Item = &view; observe(&item); observe(child);",
        "let child: &Item = &view; let leaf: &Item = &child; observe(&item); observe(leaf);",
    ] {
        let rejected = super::build_purpose::judge_build(&source(uses)).unwrap();
        assert!(
            matches!(rejected, RouteVerdict::Rejected { ref conflicts, .. }
                if conflicts.iter().any(|row| matches!(row.conflict,
                    LoanConflict::ExclusiveAndShared | LoanConflict::ObserveWhileExclusive))),
            "a shared loan overlaps the live exclusive ancestor: {uses}: {rejected:?}"
        );
    }
    for uses in [
        "observe(view); observe(&item);",
        "let child: &Item = &view; observe(child); observe(&item);",
        "let child: &Item = &view; let leaf: &Item = &child; observe(leaf); observe(&item);",
    ] {
        let accepted = super::build_purpose::judge_build(&source(uses)).unwrap();
        assert!(
            matches!(accepted, RouteVerdict::Accepted(_)),
            "{uses}: {accepted:?}"
        );
    }
}

const HANDOFF: &str = "
trait Handoff {
    machine hand(source: &mut i64) -> &mut i64 crashes none;
}
Direct satisfies Handoff {
    machine hand(source: &mut i64) -> &mut i64 crashes none { source }
}
";

/// The generic check form of `relay`, whose `Chosen::hand` call reaches the
/// bodiless requirement, judged against that requirement alone. `edit` may
/// change the elaborated form before it is judged.
fn rigid_relay_with(
    relay: &str,
    edit: impl FnOnce(&mut typed_trees_to_terminal_psi::Elaboration),
) -> (
    typed_trees_to_terminal_psi::Elaboration,
    Result<crate::Discharge, String>,
) {
    let typed = super::boundary_calls::typed(&format!("{HANDOFF}{relay}"));
    let symbol = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "relay")
        .unwrap()
        .symbol;
    let mut form = typed_trees_to_terminal_psi::elaborate_generic(&typed, symbol).unwrap();
    edit(&mut form);
    let mut activity = crate::ProofSearchActivity::default();
    let discharged =
        crate::discharge(&mut form, &mut activity).map_err(|refusal| format!("{refusal:?}"));
    (form, discharged)
}

fn rigid_relay(relay: &str) -> (typed_trees_to_terminal_psi::Elaboration, crate::Discharge) {
    let (form, discharged) = rigid_relay_with(relay, |_| {});
    (form, discharged.unwrap())
}

fn within_source(form: &typed_trees_to_terminal_psi::Elaboration, machine: usize) -> bool {
    let machine = &form.module.machines[machine];
    let source = machine.structural_parameters[0].place;
    machine.result.structural().unwrap().reference_sources
        == vec![terminal_psi::StructuralReferenceResultSource {
            path: Vec::new(),
            sources: vec![terminal_psi::StructuralReferenceOrigin::Parameter(
                terminal_psi::ReferenceOriginSource {
                    place: source,
                    path: Vec::new(),
                    extent: semantic_vocabulary::ReferenceOriginExtent::WithinRegion,
                    access: terminal_psi::StructuralAccess::MutableBorrow,
                },
            )],
        }]
}

#[test]
fn a_rigid_requirement_call_borrows_its_declared_input_region() {
    // lifetimes.md, "Returned views": `hand` elides its lifetimes, so the
    // result may be anywhere `source` lends. The requirement has no body;
    // its contract carries that declared lifetime, the same account a
    // selected call reads from its public header, and no body-origin row.
    // `relay`'s generic form keeps `source` exclusively lent while `child`
    // is live.
    const ASSIGN: &str = "machine relay<Chosen satisfies Handoff>(source: &mut i64) crashes none {
    let child: &mut i64 = Chosen::hand(source);
    child = 41;
}";
    let (form, discharged) = rigid_relay(ASSIGN);
    assert_eq!(form.requirements, vec![id(2)]);
    let requirement = &form.module.machines[1];
    assert!(requirement.blocks.is_empty());
    assert!(
        requirement
            .result
            .structural()
            .unwrap()
            .reference_sources
            .is_empty()
    );
    assert_eq!(
        requirement.result_lifetimes,
        Some(vec![terminal_psi::DeclaredResultLifetime {
            inputs: vec![terminal_psi::BoundaryLifetimeDependency::Referent {
                argument_index: 0,
                access: terminal_psi::StructuralAccess::MutableBorrow,
            }],
            results: vec![Vec::new()],
        }]),
    );
    let relay = discharged.machines.get(id(1)).unwrap();
    assert!(relay.conflicts.is_empty(), "{:?}", relay.conflicts);
    assert!(
        relay.verdicts.values().all(Result::is_ok),
        "{:?}",
        relay.verdicts
    );

    // The negative twin writes the lent input while `child` is still used.
    let (_, discharged) = rigid_relay(
        "machine relay<Chosen satisfies Handoff>(source: &mut i64) crashes none {
    let child: &mut i64 = Chosen::hand(source);
    source = 5;
    child = 41;
}",
    );
    assert!(!discharged.machines.get(id(1)).unwrap().conflicts.is_empty());

    // Without its declared account the rigid call has no origins at all; it
    // is refused rather than read as an unborrowed result.
    let (_, discharged) = rigid_relay_with(ASSIGN, |form| {
        form.module.machines[1].contract.result_lifetimes = None;
    });
    let refusal = discharged.expect_err("a rigid call without its account is refused");
    assert!(
        refusal.contains("public reference result has an unresolved declared lifetime frontier"),
        "{refusal}"
    );
}

#[test]
fn a_returned_requirement_result_publishes_its_input_region() {
    // Returning the rigid call's result returns storage known only within
    // `source`'s region. The generic form publishes that region rather than
    // inventing an exact projection.
    let (form, discharged) = rigid_relay(
        "machine relay<Chosen satisfies Handoff>(source: &mut i64) -> &mut i64 crashes none {
    Chosen::hand(source)
}",
    );
    assert!(
        within_source(&form, 0),
        "{:?}",
        form.module.machines[0].result
    );
    let relay = discharged.machines.get(id(1)).unwrap();
    assert!(relay.conflicts.is_empty(), "{:?}", relay.conflicts);
    assert!(
        relay.verdicts.values().all(Result::is_ok),
        "{:?}",
        relay.verdicts
    );
}

/// The product closure of `entry`, with `relay<Direct>` returning the
/// selected call's result.
fn returned_relay(entry: &str) -> crate::RouteVerdict {
    let text = format!(
        "{HANDOFF}machine relay<Chosen satisfies Handoff>(source: &mut i64) -> &mut i64 crashes none {{
    Chosen::hand(source)
}}
{entry}"
    );
    let (typed, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        super::boundary_calls::typed(&text),
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap();
    let entry = super::boundary_calls::entry(&typed);
    let prepared = typed_trees_to_terminal_psi::prepare_compilation(
        &typed,
        typed_trees_to_terminal_psi::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .program;
    crate::judge_compilation(prepared).remove(0).unwrap()
}

#[test]
fn a_returned_selected_result_keeps_its_input_lent_and_runs() {
    use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus, TerminalProgram};
    let RouteVerdict::Accepted(accepted) = returned_relay(
        "machine entry() crashes none {
    let mut backing: i64 = 17;
    let child: &mut i64 = relay<Direct>(&mut backing);
    child = 41;
}",
    ) else {
        panic!("a returned selected result keeps its declared input region");
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    assert_eq!(
        instance
            .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );

    // The negative twin writes `backing` while the returned `child` is live.
    let verdict = returned_relay(
        "machine entry() crashes none {
    let mut backing: i64 = 17;
    let child: &mut i64 = relay<Direct>(&mut backing);
    backing = 5;
    child = 41;
}",
    );
    assert!(
        matches!(&verdict, RouteVerdict::Rejected { conflicts, .. } if !conflicts.is_empty()),
        "{verdict:?}"
    );
}
