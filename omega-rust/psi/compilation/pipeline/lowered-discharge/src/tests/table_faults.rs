// The discharge decides the rows elaboration hands it, so these tests ask the
// other question: what if the rows themselves are wrong? Each one lowers a
// program the route rejects, corrupts one thing in elaboration's tables the
// way a defective elaborator could, and requires that the route still does
// not accept the program. Before `minted` existed most of these corruptions
// turned the rejection into an acceptance, because every remaining row had a
// certificate.
//
// Four kinds of fault: an obligation that is missing, a call that owes fewer
// clauses than its callee requires, an obligation or call stated over the
// wrong subject, and an obligation or premise attached at a point where
// facts hold that do not hold where it belongs.
//
// The last three groups do the same to the tables that say what storage
// holds: the write log, the place table and the value table. A goal over a
// field is stated over the value its leaf holds, so a log, a place or a
// value that is wrong changes what is proved without touching an obligation.

use semantic_vocabulary::{EdgeId, InvocationSite};
use structure_elaboration::Elaboration;
use terminal_check_tables::edges::{Actual, Callee, InvocationRow};
use terminal_check_tables::obligations::ObligationRow;
use terminal_check_tables::values::{KnowledgeRow, ValueRow};
use terminal_psi::OperationKind;

use super::*;

fn entry_tables(elaboration: &mut Elaboration) -> &mut MachineTables {
    elaboration.tables.machines.get_mut(id(1)).unwrap()
}

/// The route's verdict on `program` once `fault` has corrupted its tables.
/// As lowered, the program must be rejected for an unproved obligation, so
/// an acceptance afterwards is the fault's doing.
fn with_fault(
    program: impl Fn() -> Elaboration,
    fault: impl FnOnce(&mut Elaboration),
) -> Checked<RouteVerdict> {
    let lowered = judge(Ok(program())).unwrap();
    assert!(
        matches!(&lowered, RouteVerdict::Rejected { unestablished, .. } if unestablished.is_empty()),
        "the program as lowered must be rejected: {lowered:?}"
    );
    let mut elaboration = program();
    fault(&mut elaboration);
    judge(Ok(elaboration))
}

/// The tables disagree with the lowered form: a defect of the route, which
/// `judge` returns as `Err` so that no router accepts the program on it.
fn assert_refused(verdict: Checked<RouteVerdict>, detail: &str) {
    let refusal = match verdict {
        Err(refusal) => refusal,
        Ok(verdict) => panic!("the corrupted tables must be refused: {verdict:?}"),
    };
    assert_eq!(
        refusal.reason,
        RefusalReason::ForeignIdentity,
        "{refusal:?}"
    );
    assert_eq!(refusal.detail, detail);
}

/// Removes the last `count` obligation rows, as if they were never minted.
fn drop_last_obligations(tables: &mut MachineTables, count: usize) {
    let keep = tables.obligations.len() - count;
    let mut kept = IdTable::default();
    for row in tables.obligations.values().take(keep) {
        kept.push(*row);
    }
    tables.obligations = kept;
}

/// The `nth` obligation row whose kind `select` picks.
fn row_mut(
    tables: &mut MachineTables,
    select: impl Fn(&ObligationKind) -> bool,
    nth: usize,
) -> &mut ObligationRow {
    let found = tables
        .obligations
        .iter()
        .filter(|(_, row)| select(&row.kind))
        .map(|(obligation, _)| obligation)
        .nth(nth)
        .unwrap();
    tables.obligations.get_mut(found).unwrap()
}

/// The true and the false edge of the entry block's conditional.
fn guard_edges(elaboration: &Elaboration) -> (EdgeId, EdgeId) {
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &elaboration.module.machines[0].blocks[0].terminator
    else {
        panic!("the entry block ends in a conditional");
    };
    (when_true.edge, when_false.edge)
}

// A missing obligation.

#[test]
fn an_operation_whose_safety_obligation_has_no_row_is_refused() {
    // f(a, b) { a + b }: the add may overflow. With its one row gone the
    // table is empty and nothing is left unproved.
    let verdict = with_fault(
        || lower(vec![add_of_formals(Vec::new())]).unwrap(),
        |elaboration| drop_last_obligations(entry_tables(elaboration), 1),
    );
    assert_refused(verdict, "the lowered form owes an obligation with no row");
}

#[test]
fn a_return_whose_guarantee_has_no_row_is_refused() {
    // f(a) ensures a <= 10 { a }: the return does not establish the clause.
    let program = || lower(vec![guaranteeing(Vec::new(), at_most(1, 10))]).unwrap();
    let verdict = with_fault(program, |elaboration| {
        drop_last_obligations(entry_tables(elaboration), 1);
    });
    assert_refused(verdict, "a normal return owes a guarantee with no row");
    // Without the published row either, no row mentions the clause at all,
    // while every caller would still be given it as a callee guarantee.
    let verdict = with_fault(program, |elaboration| {
        drop_last_obligations(entry_tables(elaboration), 2);
    });
    assert_refused(verdict, "the lowered form owes an obligation with no row");
}

/// `make(v) requires v <= 255 -> Paired { neighbor: 17, value: v }`: the
/// clause `value <= 255` holds and `1 <= value` is unproved.
fn paired() -> Elaboration {
    elaborate_over(
        vec![Shaped {
            parameters: vec![u32_type().into()],
            locals: Vec::new(),
            result: data(PAIRED),
            requires: vec![at_most(1, 255)],
            statements: vec![returns(literal_of(
                PAIRED,
                vec![unsigned(17), slot(0, u32_type())],
            ))],
        }],
        &constrained(),
    )
}

#[test]
fn a_construction_whose_data_requires_clause_has_no_row_is_refused() {
    let verdict = with_fault(paired, |elaboration| {
        drop_last_obligations(entry_tables(elaboration), 1);
    });
    assert_refused(
        verdict,
        "an operation owes a data requires clause no obligation names",
    );
}

// A missing callee clause.

#[test]
fn a_call_whose_requirement_has_no_row_is_refused() {
    // f(a) { g(20) } where g requires x <= 10.
    let verdict = with_fault(
        || lower(caller(Vec::new(), unsigned(20))).unwrap(),
        |elaboration| drop_last_obligations(entry_tables(elaboration), 1),
    );
    assert_refused(verdict, "the lowered form owes an obligation with no row");
}

#[test]
fn a_call_that_owes_fewer_requirements_than_its_callee_has_clauses_is_refused() {
    // The same call, with the clause dropped everywhere elaboration writes
    // it: the row, the invocation row and the call operation's own list.
    // Only the callee's contract still says a clause is owed.
    let verdict = with_fault(
        || lower(caller(Vec::new(), unsigned(20))).unwrap(),
        |elaboration| {
            let call = elaboration.module.machines[0].blocks[0]
                .operations
                .iter_mut()
                .find_map(|operation| match &mut operation.kind {
                    OperationKind::Call {
                        requirement_obligations,
                        ..
                    } => Some(requirement_obligations),
                    _ => None,
                })
                .unwrap();
            call.clear();
            let tables = entry_tables(elaboration);
            drop_last_obligations(tables, 1);
            let invocation = tables.invocations.keys().next().unwrap();
            tables
                .invocations
                .get_mut(invocation)
                .unwrap()
                .requirements
                .clear();
        },
    );
    assert_refused(
        verdict,
        "a call does not owe one requirement per requires clause of its callee",
    );
}

#[test]
fn a_requirement_row_stated_for_another_clause_is_refused() {
    // f() { g(3) } where g requires x <= 10, 5 <= x. The second clause
    // fails; restating its row as the first clause proves `3 <= 10` twice.
    let program = || {
        let callee = Machine {
            requires: vec![at_most(1, 10), at_least(1, 5)],
            ..machine(vec![u32_type()], vec![returns(slot(0, u32_type()))])
        };
        let caller = machine(Vec::new(), vec![returns(call(2, vec![unsigned(3)]))]);
        lower(vec![caller, callee]).unwrap()
    };
    let verdict = with_fault(program, |elaboration| {
        let ObligationKind::CallRequirement { clause, .. } =
            &mut row_mut(entry_tables(elaboration), requirement, 1).kind
        else {
            panic!("a call requirement");
        };
        *clause = 0;
    });
    assert_refused(
        verdict,
        "an obligation row is not what the lowered form owes under its identity",
    );
}

// A wrong subject identity.

#[test]
fn a_safety_row_that_names_another_operation_is_refused() {
    // f(a, b) { when a < 10 -> 1 + 2; a + b }: the second add is unproved,
    // and its row is restated as the first add's.
    let program = || {
        lower(vec![machine(
            vec![u32_type(), u32_type()],
            vec![
                guarded(
                    compare(Strict::Less, slot(0, u32_type()), unsigned(10)),
                    strict(Strict::Add, unsigned(1), unsigned(2)),
                ),
                returns(strict(
                    Strict::Add,
                    slot(0, u32_type()),
                    slot(1, u32_type()),
                )),
            ],
        )])
        .unwrap()
    };
    let verdict = with_fault(program, |elaboration| {
        let tables = entry_tables(elaboration);
        let first = row_mut(tables, safety, 0).kind;
        row_mut(tables, safety, 1).kind = first;
    });
    assert_refused(
        verdict,
        "an obligation row is not what the lowered form owes under its identity",
    );
}

#[test]
fn a_requirement_row_that_names_another_call_is_refused() {
    // f() { g(g(7)) } where g requires x <= 10: the outer call's argument
    // is what the inner call returned, which nothing bounds. Restated over
    // the inner call, its requirement is `7 <= 10`.
    let program = || {
        let callee = Machine {
            requires: vec![at_most(1, 10)],
            ..machine(vec![u32_type()], vec![returns(slot(0, u32_type()))])
        };
        let caller = machine(
            Vec::new(),
            vec![returns(call(2, vec![call(2, vec![unsigned(7)])]))],
        );
        lower(vec![caller, callee]).unwrap()
    };
    let verdict = with_fault(program, |elaboration| {
        let tables = entry_tables(elaboration);
        let inner = row_mut(tables, requirement, 0).kind;
        row_mut(tables, requirement, 1).kind = inner;
    });
    assert_refused(
        verdict,
        "an obligation row is not what the lowered form owes under its identity",
    );
}

#[test]
fn an_invocation_row_with_another_actual_is_refused() {
    // f(a) requires a <= 5 { g(20) }: the invocation row says the actual
    // is `a`, so the requirement would be stated over `a`.
    let verdict = with_fault(
        || lower(caller(vec![at_most(1, 5)], unsigned(20))).unwrap(),
        |elaboration| {
            let tables = entry_tables(elaboration);
            let invocation = tables.invocations.keys().next().unwrap();
            tables.invocations.get_mut(invocation).unwrap().actuals = vec![Actual::Value(id(1))];
        },
    );
    assert_refused(
        verdict,
        "an invocation row's actuals are not the call's arguments",
    );
}

#[test]
fn an_invocation_row_with_another_callee_is_refused() {
    // f() { g(20) } where g requires x <= 10 and h requires x <= 100: the
    // invocation row says the callee is h.
    let program = || {
        let callee = |limit| Machine {
            requires: vec![at_most(1, limit)],
            ..machine(vec![u32_type()], vec![returns(slot(0, u32_type()))])
        };
        let caller = machine(Vec::new(), vec![returns(call(2, vec![unsigned(20)]))]);
        lower(vec![caller, callee(10), callee(100)]).unwrap()
    };
    let verdict = with_fault(program, |elaboration| {
        let tables = entry_tables(elaboration);
        let invocation = tables.invocations.keys().next().unwrap();
        tables.invocations.get_mut(invocation).unwrap().callee = Callee::Machine(id(3));
    });
    assert_refused(verdict, "an invocation row names another callee");
}

#[test]
fn a_guarantee_row_stated_for_another_clause_is_refused() {
    // f(a) requires a <= 5, ensures a <= 10, 7 <= a { a }: the second
    // clause fails and its return's row is restated as the first clause's.
    let program = || {
        lower(vec![Machine {
            requires: vec![at_most(1, 5)],
            ensures: vec![at_most(1, 10), at_least(1, 7)],
            ..machine(vec![u32_type()], vec![returns(slot(0, u32_type()))])
        }])
        .unwrap()
    };
    let verdict = with_fault(program, |elaboration| {
        let tables = entry_tables(elaboration);
        let first = row_mut(tables, guarantee, 0).kind;
        row_mut(tables, guarantee, 1).kind = first;
    });
    assert_refused(verdict, "a normal return owes a guarantee with no row");
}

/// `f(a) ensures a <= 10 { when a <= 10 -> 0; 1 }`: the arm's return
/// establishes the clause from its guard and the fall-through return does
/// not.
fn two_returns() -> Elaboration {
    lower(vec![Machine {
        ensures: vec![at_most(1, 10)],
        ..machine(
            vec![u32_type()],
            vec![
                guarded(
                    compare(Strict::LessOrEqual, slot(0, u32_type()), unsigned(10)),
                    unsigned(0),
                ),
                returns(unsigned(1)),
            ],
        )
    }])
    .unwrap()
}

#[test]
fn a_guarantee_row_that_names_another_return_is_refused() {
    let verdict = with_fault(two_returns, |elaboration| {
        let tables = entry_tables(elaboration);
        let arm = *row_mut(tables, guarantee, 0);
        *row_mut(tables, guarantee, 1) = arm;
    });
    assert_refused(verdict, "a normal return owes a guarantee with no row");
}

// A premise attached at the wrong point.

#[test]
fn a_safety_obligation_created_on_the_other_side_of_its_guard_is_refused() {
    // f(a, b) { when a < b -> a - b; 0 }: the difference underflows in its
    // arm. Created on the false edge instead, `b <= a` would prove it.
    let program = || {
        lower(vec![machine(
            vec![u32_type(), u32_type()],
            vec![
                guarded(
                    compare(Strict::Less, slot(0, u32_type()), slot(1, u32_type())),
                    strict(Strict::Subtract, slot(0, u32_type()), slot(1, u32_type())),
                ),
                returns(unsigned(0)),
            ],
        )])
        .unwrap()
    };
    let verdict = with_fault(program, |elaboration| {
        let (_, when_false) = guard_edges(elaboration);
        row_mut(entry_tables(elaboration), safety, 0).created_at = ProgramPoint::Edge(when_false);
    });
    assert_refused(
        verdict,
        "an obligation is not created at the point of what owes it",
    );
}

#[test]
fn a_call_requirement_created_after_its_own_call_is_refused() {
    // f(a) { g(a) } where g requires x <= 10 and ensures x <= 10. After the
    // call the callee's guarantee is knowledge, and it is the requirement.
    let program = || {
        let callee = Machine {
            requires: vec![at_most(1, 10)],
            ensures: vec![at_most(1, 10)],
            ..machine(vec![u32_type()], vec![returns(slot(0, u32_type()))])
        };
        let caller = machine(
            vec![u32_type()],
            vec![returns(call(2, vec![slot(0, u32_type())]))],
        );
        lower(vec![caller, callee]).unwrap()
    };
    let verdict = with_fault(program, |elaboration| {
        let call = elaboration.module.machines[0].blocks[0]
            .operations
            .last()
            .unwrap()
            .id;
        row_mut(entry_tables(elaboration), requirement, 0).created_at =
            ProgramPoint::AfterOperation(call);
    });
    assert_refused(
        verdict,
        "an obligation is not created at the point of what owes it",
    );
}

#[test]
fn a_guarantee_created_on_a_guarded_edge_is_refused() {
    // The fall-through return's guarantee, created on the true edge of the
    // guard it did not pass.
    let verdict = with_fault(two_returns, |elaboration| {
        let (when_true, _) = guard_edges(elaboration);
        row_mut(entry_tables(elaboration), guarantee, 1).created_at = ProgramPoint::Edge(when_true);
    });
    assert_refused(
        verdict,
        "an obligation is not created at the point of what owes it",
    );
}

#[test]
fn a_data_requires_obligation_created_on_a_guarded_edge_is_refused() {
    // f(v) -> Bounded { when v <= 5 -> Bounded { value: v };
    //                   Bounded { value: v } }
    // The second construction is past the guard, where `v <= 5` is refuted.
    let program = || {
        let built = || literal_of(BOUNDED, vec![slot(0, u32_type())]);
        elaborate_over(
            vec![Shaped {
                parameters: vec![u32_type().into()],
                locals: Vec::new(),
                result: data(BOUNDED),
                requires: Vec::new(),
                statements: vec![
                    guarded(
                        compare(Strict::LessOrEqual, slot(0, u32_type()), unsigned(5)),
                        built(),
                    ),
                    returns(built()),
                ],
            }],
            &constrained(),
        )
    };
    let verdict = with_fault(program, |elaboration| {
        let (when_true, _) = guard_edges(elaboration);
        row_mut(entry_tables(elaboration), domain, 1).created_at = ProgramPoint::Edge(when_true);
    });
    assert_refused(
        verdict,
        "an obligation is not created at the point of what owes it",
    );
}

/// `f(a) { when a < 10 -> 0; a + (MAX - 9) }`: past the guard `10 <= a`, so
/// the add overflows.
fn add_past_a_guard() -> Elaboration {
    lower(vec![machine(
        vec![u32_type()],
        vec![
            guarded(
                compare(Strict::Less, slot(0, u32_type()), unsigned(10)),
                unsigned(0),
            ),
            returns(strict(
                Strict::Add,
                slot(0, u32_type()),
                unsigned(u128::from(u32::MAX) - 9),
            )),
        ],
    )])
    .unwrap()
}

#[test]
fn an_edge_table_that_exchanges_a_guards_destinations_is_refused() {
    // Dominance is computed from the edge table. With the two destinations
    // exchanged, `a < 10` holds on the way to the add.
    let verdict = with_fault(add_past_a_guard, |elaboration| {
        let (when_true, when_false) = guard_edges(elaboration);
        let tables = entry_tables(elaboration);
        let taken = tables.edges.get(when_true).unwrap().destination;
        let other = tables.edges.get(when_false).unwrap().destination;
        tables.edges.get_mut(when_true).unwrap().destination = other;
        tables.edges.get_mut(when_false).unwrap().destination = taken;
    });
    assert_refused(verdict, "an edge row does not go where its terminator does");
}

#[test]
fn an_invocation_row_for_a_call_the_machine_does_not_make_is_refused() {
    // f(a) { a + (MAX - 9) } never calls g, which requires and ensures
    // x <= 9. An invocation row at the constant would put g's guarantee
    // about `a` before the add.
    let program = || {
        let bounded = Machine {
            requires: vec![at_most(1, 9)],
            ensures: vec![at_most(1, 9)],
            ..machine(vec![u32_type()], vec![returns(slot(0, u32_type()))])
        };
        let adder = machine(
            vec![u32_type()],
            vec![returns(strict(
                Strict::Add,
                slot(0, u32_type()),
                unsigned(u128::from(u32::MAX) - 9),
            ))],
        );
        lower(vec![adder, bounded]).unwrap()
    };
    let verdict = with_fault(program, |elaboration| {
        let constant = elaboration.module.machines[0].blocks[0].operations[0].id;
        entry_tables(elaboration).invocations.push(InvocationRow {
            site: InvocationSite::Operation(constant),
            callee: Callee::Machine(id(2)),
            actuals: vec![Actual::Value(id(1))],
            requirements: Vec::new(),
        });
    });
    assert_refused(
        verdict,
        "an invocation row restates no call of this machine",
    );
}

#[test]
fn knowledge_the_elaborator_supplies_is_never_used() {
    // The knowledge table is this crate's to fill. Rows already in it would
    // be premises nothing here established, so the closure is handed back.
    let verdict = with_fault(
        || lower(vec![add_of_formals(Vec::new())]).unwrap(),
        |elaboration| {
            let entry = elaboration.module.machines[0].entry;
            let tables = entry_tables(elaboration);
            for formal in [1, 2] {
                let proposition = tables.propositions.push(at_most(formal, 100));
                tables.knowledge.push(KnowledgeRow {
                    proposition,
                    available_from: ProgramPoint::BlockEntry(entry),
                    justification: Justification::EntryRequirement { clause: 0 },
                    premises: Vec::new(),
                });
            }
        },
    );
    let Ok(RouteVerdict::FallBack(refusal)) = verdict else {
        panic!("the closure must be handed back: {verdict:?}");
    };
    assert_eq!(refusal.detail, "knowledge rows already present");
}

// A write log that is not what the operations store.

/// Replaces the write log with `rows` and rebuilds each place's `writes`
/// column from them, so the fault is in what the log says and nowhere else.
fn relog(tables: &mut MachineTables, rows: Vec<WriteRow>) {
    for place in tables.places.keys().collect::<Vec<_>>() {
        tables.places.get_mut(place).unwrap().writes.clear();
    }
    tables.writes = IdTable::default();
    for row in rows {
        let write = tables.writes.push(row);
        tables.places.get_mut(row.place).unwrap().writes.push(write);
    }
}

fn logged(tables: &MachineTables) -> Vec<WriteRow> {
    tables.writes.values().copied().collect()
}

#[test]
fn a_write_row_that_logs_another_value_is_refused() {
    // `paired`: the construction stores `v` into `value`, and `1 <= value`
    // is unproved of it. Logged as holding the `17` stored into `neighbor`,
    // the clause is `1 <= 17`.
    let verdict = with_fault(paired, |elaboration| {
        let seventeen = constants(elaboration)[0];
        let mut rows = logged(entry_tables(elaboration));
        let stored = rows
            .iter_mut()
            .find(|row| row.content == Content::Value(id(1)))
            .unwrap();
        stored.content = Content::Value(seventeen);
        relog(entry_tables(elaboration), rows);
    });
    assert_refused(verdict, "a write row is not what its operation stores");
}

/// `f(v) { let mut p = Point { x: 7, y: 9 }; p.x = v; peek(p) }` where
/// `peek` requires `p.x == 7`: at the call `p.x` holds `v`, not the `7` it
/// was constructed with.
fn stored_over() -> Elaboration {
    elaborate_places(vec![
        Shaped {
            parameters: vec![u32_type().into()],
            locals: vec![point()],
            result: u32_type().into(),
            requires: Vec::new(),
            statements: vec![
                bind(1, point_of(unsigned(7), unsigned(9))),
                store(1, &[0], slot(0, u32_type())),
                returns(peeking(place(1, point()))),
            ],
        },
        peek(),
    ])
}

/// The one scalar field store of machine 1, as its position in the write
/// log: the row at its normal continuation.
fn store_row(elaboration: &Elaboration) -> usize {
    let store = elaboration.module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::StructuralScalarFieldStore { .. }
            )
        })
        .unwrap()
        .id;
    elaboration
        .tables
        .machines
        .get(id(1))
        .unwrap()
        .writes
        .values()
        .position(|row| row.at == ProgramPoint::AfterOperation(store))
        .unwrap()
}

#[test]
fn a_store_whose_write_row_is_missing_is_refused() {
    // Without the store's row the last write to `p.x` is the construction.
    let verdict = with_fault(stored_over, |elaboration| {
        let row = store_row(elaboration);
        let mut rows = logged(entry_tables(elaboration));
        rows.remove(row);
        relog(entry_tables(elaboration), rows);
    });
    assert_refused(verdict, "an operation's store has no write row");
}

#[test]
fn a_write_row_moved_before_the_construction_it_follows_is_refused() {
    // The store's row, stated at the first operation of the block: the
    // construction then comes after it and is the write that reaches the
    // call.
    let verdict = with_fault(stored_over, |elaboration| {
        let first = elaboration.module.machines[0].blocks[0].operations[0].id;
        let row = store_row(elaboration);
        let mut rows = logged(entry_tables(elaboration));
        rows[row].at = ProgramPoint::AfterOperation(first);
        relog(entry_tables(elaboration), rows);
    });
    assert_refused(verdict, "an operation's store has no write row");
}

/// `f(p: Point) { let c = 3; bounded(p.x) }` where `bounded` requires
/// `v <= 10`: nothing is known of what `p.x` held at entry.
fn passes_a_formal_field() -> Elaboration {
    elaborate_places(vec![
        Shaped {
            parameters: vec![point()],
            locals: vec![u32_type().into()],
            result: u32_type().into(),
            requires: Vec::new(),
            statements: vec![
                bind(1, unsigned(3)),
                returns(call(2, vec![read(place(0, point()), 0)])),
            ],
        },
        bounded(),
    ])
}

/// The first integer constant of machine 1: its operation and its value.
fn first_constant(elaboration: &Elaboration) -> (OperationId, ValueId) {
    elaboration.module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::IntegerConstant { .. }))
        .map(|operation| (operation.id, operation.result.scalar_ref().unwrap().id))
        .unwrap()
}

#[test]
fn a_write_row_for_a_store_no_operation_makes_is_refused() {
    // A row that says the constant `3` was stored into `p.x`. No operation
    // stores into the formal, and with the row its read equals `3`.
    let verdict = with_fault(passes_a_formal_field, |elaboration| {
        let (constant, three) = first_constant(elaboration);
        let tables = entry_tables(elaboration);
        let mut rows = logged(tables);
        rows.push(WriteRow {
            place: projected(tables, id(1), &[1]),
            at: ProgramPoint::AfterOperation(constant),
            content: Content::Value(three),
        });
        relog(tables, rows);
    });
    assert_refused(
        verdict,
        "a write row states a store its operation does not make",
    );
}

#[test]
fn a_zeroed_leaf_logged_as_another_constant_is_refused() {
    // f() { let c = 3; let b: Bounded; positive(b.value) } where `positive`
    // requires `1 <= v`: the zero value's `value` is `0`. Logged as holding
    // the constant `3`, the requirement is `1 <= 3`.
    let program = || {
        elaborate_over(
            vec![
                Shaped {
                    parameters: Vec::new(),
                    locals: vec![u32_type().into(), data(BOUNDED)],
                    result: u32_type().into(),
                    requires: Vec::new(),
                    statements: vec![
                        bind(0, unsigned(3)),
                        declare(1, data(BOUNDED)),
                        returns(call(2, vec![read(place(1, data(BOUNDED)), 0)])),
                    ],
                },
                Shaped {
                    requires: vec![at_least(1, 1)],
                    ..bounded()
                },
            ],
            &constrained(),
        )
    };
    let verdict = with_fault(program, |elaboration| {
        let (_, three) = first_constant(elaboration);
        let mut rows = logged(entry_tables(elaboration));
        let zeroed = rows
            .iter_mut()
            .find(|row| matches!(row.content, Content::Value(_)))
            .unwrap();
        zeroed.content = Content::Value(three);
        relog(entry_tables(elaboration), rows);
    });
    assert_refused(verdict, "a write row is not what its operation stores");
}

// A place table that is not one place per path.

#[test]
fn a_place_table_that_holds_two_fields_as_one_place_is_refused() {
    // f(p: Point) requires p.x <= 10 { p.x & bounded(p.y) }: the clause is
    // about what `p.x` held at entry and says nothing of `p.y`. With `y`
    // listed as the place of `x`, the read of `p.y` equals the entry
    // content of `p.x`.
    let program = || {
        elaborate_places(vec![
            Shaped {
                parameters: vec![point()],
                locals: Vec::new(),
                result: u32_type().into(),
                requires: vec![Proposition::LessOrEqual(formal_field(1), number(10))],
                statements: vec![returns(and(
                    read(place(0, point()), 0),
                    call(2, vec![read(place(0, point()), 1)]),
                ))],
            },
            bounded(),
        ])
    };
    let verdict = with_fault(program, |elaboration| {
        let tables = entry_tables(elaboration);
        let x = projected(tables, id(1), &[1]);
        let root = *tables.roots.get(id::<PlaceId>(1)).unwrap();
        tables.places.get_mut(root).unwrap().children[1].1 = x;
    });
    assert_refused(
        verdict,
        "a place row is not the projection its parent lists",
    );
}

#[test]
fn a_place_row_restated_as_a_field_of_another_formal_is_refused() {
    // f(b: Bounded, s: Span) { bounded(s.high) }: `Span` says nothing of
    // `high` alone, so the read is unbounded. Its place restated as
    // `b.value`, the read learns `value <= 5` from `Bounded`.
    let program = || {
        elaborate_over(
            vec![
                Shaped {
                    parameters: vec![data(BOUNDED), data(SPAN)],
                    locals: Vec::new(),
                    result: u32_type().into(),
                    requires: Vec::new(),
                    statements: vec![returns(call(2, vec![read(place(1, data(SPAN)), 1)]))],
                },
                bounded(),
            ],
            &constrained(),
        )
    };
    let verdict = with_fault(program, |elaboration| {
        let tables = entry_tables(elaboration);
        let high = projected(tables, id(2), &[3]);
        let bounded = *tables.roots.get(id::<PlaceId>(1)).unwrap();
        tables.places.get_mut(high).unwrap().origin = PlaceOrigin::Projection {
            parent: bounded,
            segment: Segment::Field(id(1)),
        };
    });
    assert_refused(
        verdict,
        "a place row is not the projection its parent lists",
    );
}

// A value table that is not the values the lowered form declares.

fn value_mut(tables: &mut MachineTables, value: ValueId) -> &mut ValueRow {
    tables.values.get_mut(value).unwrap()
}

/// The entry content values of machine 1, in identity order.
fn entry_values(tables: &MachineTables) -> Vec<ValueId> {
    tables
        .values
        .iter()
        .filter(|(_, row)| matches!(row.origin, ValueOrigin::EntryContent { .. }))
        .map(|(value, _)| value)
        .collect()
}

#[test]
fn a_constant_restated_as_what_a_formal_held_at_entry_is_refused() {
    // `passes_a_formal_field`, with the origins of the constant `3` and of
    // the entry content of `p.x` exchanged: the read of `p.x` then equals a
    // value whose own equation says it is `3`.
    let verdict = with_fault(passes_a_formal_field, |elaboration| {
        let (_, three) = first_constant(elaboration);
        let tables = entry_tables(elaboration);
        let held = entry_values(tables)[0];
        let constant = value_mut(tables, three).origin;
        let entry = std::mem::replace(&mut value_mut(tables, held).origin, constant);
        value_mut(tables, three).origin = entry;
    });
    assert_refused(
        verdict,
        "a value row is not the value the lowered form declares under its identity",
    );
}

#[test]
fn a_value_row_typed_wider_than_its_declaration_is_refused() {
    // f(a, b) { a + b }, with both formals typed `u64` in the value table.
    let verdict = with_fault(
        || lower(vec![add_of_formals(Vec::new())]).unwrap(),
        |elaboration| {
            let wide = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
            let tables = entry_tables(elaboration);
            for formal in [1, 2] {
                value_mut(tables, id(formal)).scalar_type = wide;
            }
        },
    );
    assert_refused(
        verdict,
        "a value row is not the value the lowered form declares under its identity",
    );
}

#[test]
fn an_entry_content_of_storage_that_is_no_formal_is_refused() {
    // f(p: Point) { let q = Point { x: 20, y: 2 }; bounded(p.x) }: the
    // entry content of `p.x`, restated as what the local `q.x` held at
    // entry, where a local holds nothing.
    let program = || {
        elaborate_places(vec![
            Shaped {
                parameters: vec![point()],
                locals: vec![point()],
                result: u32_type().into(),
                requires: Vec::new(),
                statements: vec![
                    bind(1, point_of(unsigned(20), unsigned(2))),
                    returns(call(2, vec![read(place(0, point()), 0)])),
                ],
            },
            bounded(),
        ])
    };
    let verdict = with_fault(program, |elaboration| {
        let local = elaboration.module.machines[0]
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| matches!(operation.kind, OperationKind::EstablishRecord { .. }))
            .and_then(|operation| operation.result.as_structural())
            .unwrap()
            .place;
        let tables = entry_tables(elaboration);
        let place = projected(tables, local, &[1]);
        let held = entry_values(tables)[0];
        value_mut(tables, held).origin = ValueOrigin::EntryContent { place };
    });
    assert_refused(
        verdict,
        "an entry content value of storage that is no scalar leaf of a formal",
    );
}

#[test]
fn entry_content_values_that_exchange_their_leaves_are_refused() {
    // f(p: Point) { bounded(p.x + p.y) }: the entry contents of `p.x` and
    // `p.y` are both `u32`, so exchanging the leaves their rows name keeps
    // every type and origin check satisfied. The lowered form declares
    // neither value, so only their order says which leaf each belongs to.
    let program = || {
        elaborate_places(vec![
            Shaped {
                parameters: vec![point()],
                locals: Vec::new(),
                result: u32_type().into(),
                requires: Vec::new(),
                statements: vec![returns(call(
                    2,
                    vec![strict(
                        Strict::Add,
                        read(place(0, point()), 0),
                        read(place(0, point()), 1),
                    )],
                ))],
            },
            bounded(),
        ])
    };
    let verdict = with_fault(program, |elaboration| {
        let tables = entry_tables(elaboration);
        let [x, y] = entry_values(tables)[..] else {
            panic!("p.x and p.y each have an entry content value");
        };
        let held_by_x = value_mut(tables, x).origin;
        let held_by_y = std::mem::replace(&mut value_mut(tables, y).origin, held_by_x);
        value_mut(tables, x).origin = held_by_y;
    });
    assert_refused(
        verdict,
        "entry content values are not in the order of the leaves they belong to",
    );
}

#[test]
fn a_condition_restated_as_a_case_test_is_refused() {
    // f(s: Shape, flag: bool) { let t = s in Dot; when flag -> s.at; 0 }:
    // the arm names the payload of `Dot` under a guard that says nothing of
    // the case. The payload judgment asks which operation produced the
    // condition, so `flag` restated as the result of the test passes it.
    let program = || {
        elaborate_over(
            vec![over_sums(
                vec![shape(), ScalarType::Boolean.into()],
                vec![ScalarType::Boolean.into()],
                vec![
                    bind(2, is_case(place(0, shape()), DOT)),
                    guarded(
                        slot(1, ScalarType::Boolean),
                        payload(place(0, shape()), DOT, 0),
                    ),
                    returns(unsigned(0)),
                ],
            )],
            &sums(),
        )
    };
    let lowered = judge(Ok(program())).unwrap();
    assert!(
        matches!(&lowered, RouteVerdict::Rejected { unestablished, .. } if !unestablished.is_empty()),
        "the payload access as lowered must be rejected: {lowered:?}"
    );
    let mut elaboration = program();
    let test = elaboration.module.machines[0].blocks[0]
        .operations
        .iter()
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::StructuralCaseMembership { .. }
            )
        })
        .unwrap()
        .id;
    value_mut(entry_tables(&mut elaboration), id(1)).origin = ValueOrigin::Operation(test);
    assert_refused(
        judge(Ok(elaboration)),
        "a value row is not the value the lowered form declares under its identity",
    );
}

// The data `requires` clauses. They are the goal of every `DataDomain`
// obligation and the premise a read of an arrived value learns from its
// type, and they reach the discharge in a table beside the module
// (`Elaboration::domains`). The module's own type declarations state them,
// so the table must restate those exactly.

/// `Elaboration::domains`' row for `structural_type`.
fn clauses_mut(elaboration: &mut Elaboration, structural_type: u64) -> &mut Vec<Proposition> {
    elaboration.domains.get_mut(id(structural_type)).unwrap()
}

const DOMAINS_DIFFER: &str =
    "a data type's requires clauses are not the ones its declaration states";

#[test]
fn a_data_requires_clause_weakened_beside_the_module_is_refused() {
    // `paired`'s construction leaves `1 <= value` unproved. With that clause
    // replaced by one that holds of what is stored, every row has a
    // certificate.
    let verdict = with_fault(paired, |elaboration| {
        clauses_mut(elaboration, PAIRED)[1] =
            Proposition::LessOrEqual(field_of_self(9), number(4_096));
    });
    assert_refused(verdict, DOMAINS_DIFFER);
}

#[test]
fn a_data_requires_clause_dropped_beside_the_module_with_its_obligation_is_refused() {
    // The unproved clause and the row that owes it both gone: the count of
    // owed rows agrees with the table, and only the declaration still states
    // the clause.
    let verdict = with_fault(paired, |elaboration| {
        clauses_mut(elaboration, PAIRED).pop();
        drop_last_obligations(entry_tables(elaboration), 1);
    });
    assert_refused(verdict, DOMAINS_DIFFER);
}

#[test]
fn a_data_requires_clause_strengthened_beside_the_module_is_refused() {
    // f(b: Bounded) -> u32 { tight(b.value) } where tight requires v <= 3.
    // `b` arrived established, so the read learns `value <= 5`, which does
    // not give `v <= 3`. A table that says `value <= 3` hands the read the
    // premise the call needs.
    let program = || {
        elaborate_over(
            vec![
                Shaped {
                    parameters: vec![data(BOUNDED)],
                    locals: Vec::new(),
                    result: u32_type().into(),
                    requires: Vec::new(),
                    statements: vec![returns(call(2, vec![read(place(0, data(BOUNDED)), 0)]))],
                },
                Shaped {
                    requires: vec![at_most(1, 3)],
                    ..bounded()
                },
            ],
            &constrained(),
        )
    };
    let verdict = with_fault(program, |elaboration| {
        clauses_mut(elaboration, BOUNDED)[0] =
            Proposition::LessOrEqual(field_of_self(1), number(3));
    });
    assert_refused(verdict, DOMAINS_DIFFER);
}

#[test]
fn data_requires_clauses_moved_to_another_type_are_refused() {
    // `Paired`'s clauses exchanged with `Gated`'s: the same number of rows,
    // each a clause of some declaration, none of its own type's.
    let verdict = with_fault(paired, |elaboration| {
        let gated = std::mem::take(clauses_mut(elaboration, GATED));
        let paired = std::mem::replace(clauses_mut(elaboration, PAIRED), gated);
        *clauses_mut(elaboration, GATED) = paired;
    });
    assert_refused(verdict, DOMAINS_DIFFER);
    // A row for a type the module does not declare.
    let verdict = with_fault(paired, |elaboration| {
        elaboration.domains.push(vec![Proposition::Truth]);
    });
    assert_refused(verdict, "a row of requires clauses names no declared type");
}
