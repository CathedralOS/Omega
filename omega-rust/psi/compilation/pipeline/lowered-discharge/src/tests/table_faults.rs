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

use semantic_vocabulary::{EdgeId, InvocationSite};
use structure_elaboration::Elaboration;
use terminal_check_tables::edges::{Actual, Callee, InvocationRow};
use terminal_check_tables::obligations::ObligationRow;
use terminal_check_tables::values::KnowledgeRow;
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
