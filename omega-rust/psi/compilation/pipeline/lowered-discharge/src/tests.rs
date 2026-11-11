// Each test states a scalar program as closed bodies, lowers it with
// structure-elaboration's own lowering, discharges it, and checks the verdict
// of each obligation. A discharged verdict is re-decided here by the kernel
// from the evidence alone, and a tampered certificate must be refused.

use proof_admission::{
    AcceptedFactRoute, EvidenceRoute, MathematicalCoreDecision, Obligation, ObligationClass,
    verify_obligation,
};
use semantic_vocabulary::{
    IdTable, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId, Proposition,
    PropositionContext, PsiSemanticId, ScalarTerm, ScalarType,
};
use structure_elaboration::{
    Body, CrashCeiling, Expression, ExpressionKind, Signature, SourceOrigin, Statement, Strict,
    Target, Unpositioned,
};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::values::Justification;
use terminal_check_tables::{Checked, ProgramPoint, RefusalReason};

use crate::{Discharge, Discharged, Evidence, discharge};

fn integer_type(sign: IntegerSign) -> IntegerType {
    IntegerType::new(sign, 32).unwrap()
}

fn u32_type() -> ScalarType {
    ScalarType::Integer(integer_type(IntegerSign::Unsigned))
}

fn i32_type() -> ScalarType {
    ScalarType::Integer(integer_type(IntegerSign::Signed))
}

fn id<T: PsiSemanticId>(raw: u64) -> T {
    T::new(raw).unwrap()
}

fn nowhere() -> SourceOrigin {
    SourceOrigin::Unpositioned(Unpositioned::SpanAbsent)
}

fn expression(kind: ExpressionKind, result: Option<ScalarType>) -> Expression {
    Expression {
        kind,
        result,
        source: nowhere(),
    }
}

fn literal(scalar_type: ScalarType, value: IntegerValue) -> Expression {
    expression(ExpressionKind::Integer(value), Some(scalar_type))
}

fn unsigned(value: u128) -> Expression {
    literal(u32_type(), IntegerValue::Unsigned(value))
}

fn slot(slot: usize, scalar_type: ScalarType) -> Expression {
    expression(ExpressionKind::Slot(slot), Some(scalar_type))
}

fn strict(operation: Strict, left: Expression, right: Expression) -> Expression {
    let result = left.result;
    expression(
        ExpressionKind::Strict {
            operation,
            left: Box::new(left),
            right: Box::new(right),
        },
        result,
    )
}

fn call(callee: u64, arguments: Vec<Expression>) -> Expression {
    expression(
        ExpressionKind::Call {
            callee: id(callee),
            arguments,
        },
        Some(u32_type()),
    )
}

fn returns(value: Expression) -> Statement {
    Statement::Arm {
        guard: None,
        target: Target::Value(value),
        source: nowhere(),
    }
}

fn formal(position: u64, scalar_type: ScalarType) -> ScalarTerm {
    ScalarTerm::value(id(position), scalar_type)
}

fn bound(scalar_type: ScalarType, value: IntegerValue) -> ScalarTerm {
    let ScalarType::Integer(integer) = scalar_type else {
        panic!("integer bound");
    };
    ScalarTerm::integer(integer, value).unwrap()
}

fn at_most(position: u64, value: u128) -> Proposition {
    Proposition::LessOrEqual(
        formal(position, u32_type()),
        bound(u32_type(), IntegerValue::Unsigned(value)),
    )
}

fn at_least(position: u64, value: u128) -> Proposition {
    Proposition::LessOrEqual(
        bound(u32_type(), IntegerValue::Unsigned(value)),
        formal(position, u32_type()),
    )
}

struct Machine {
    parameters: Vec<ScalarType>,
    result: ScalarType,
    locals: Vec<ScalarType>,
    requires: Vec<Proposition>,
    ensures: Vec<Proposition>,
    statements: Vec<Statement>,
}

fn machine(parameters: Vec<ScalarType>, statements: Vec<Statement>) -> Machine {
    Machine {
        parameters,
        result: u32_type(),
        locals: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        statements,
    }
}

/// Lowers `machines` (machine 1 is the entry) and discharges them.
fn run(machines: Vec<Machine>) -> (structure_elaboration::Elaboration, Discharge) {
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
    for machine in machines {
        let mut slots = machine.parameters.clone();
        slots.extend(machine.locals);
        bodies.push(Body {
            slots,
            statements: machine.statements,
        });
        signatures.push(Signature {
            parameters: machine.parameters,
            result: Some(machine.result),
            requires: machine.requires,
            ensures: machine.ensures,
            crashes: CrashCeiling::Authored(Vec::new()),
        });
    }
    let mut elaboration = structure_elaboration::lower_bodies(id(1), &bodies, &signatures).unwrap();
    let result = discharge(&mut elaboration).unwrap();
    (elaboration, result)
}

/// Every verdict of `machine` whose obligation has the kind `select` picks.
fn verdicts<'a>(
    elaboration: &structure_elaboration::Elaboration,
    result: &'a Discharge,
    machine: u64,
    select: impl Fn(&ObligationKind) -> bool,
) -> Vec<(ObligationId, &'a Checked<Discharged>)> {
    let tables = elaboration.tables.machines.get(id(machine)).unwrap();
    let verdicts = &result.machines.get(id(machine)).unwrap().verdicts;
    tables
        .obligations
        .iter()
        .filter(|(_, row)| select(&row.kind))
        .map(|(obligation, _)| (obligation, verdicts.get(obligation).unwrap()))
        .collect()
}

fn safety(kind: &ObligationKind) -> bool {
    matches!(kind, ObligationKind::OperationSafety { .. })
}

fn requirement(kind: &ObligationKind) -> bool {
    matches!(kind, ObligationKind::CallRequirement { .. })
}

fn guarantee(kind: &ObligationKind) -> bool {
    matches!(kind, ObligationKind::NormalGuarantee { .. })
}

/// The kernel decides the evidence again from its parts, its mathematical
/// core judged it too, and the same certificate is refused once its
/// conclusion is tampered with.
fn assert_kernel_checked(
    elaboration: &structure_elaboration::Elaboration,
    machine: u64,
    obligation: ObligationId,
    evidence: &Evidence,
) {
    let core = assert_kernel_accepted(elaboration, machine, obligation, evidence);
    assert!(
        matches!(core, MathematicalCoreDecision::Judged(_)),
        "{core:?}"
    );
}

/// As `assert_kernel_checked`, returning the mathematical core's decision
/// instead of requiring `Judged`: `verify_obligation` accepts a certificate
/// whose rule families the core does not cover.
fn assert_kernel_accepted(
    elaboration: &structure_elaboration::Elaboration,
    machine: u64,
    obligation: ObligationId,
    evidence: &Evidence,
) -> MathematicalCoreDecision {
    assert!(matches!(
        evidence.fact.route,
        AcceptedFactRoute::CertificateDerived { .. }
    ));
    let tables = elaboration.tables.machines.get(id(machine)).unwrap();
    let resolve = |rows: &[terminal_check_tables::KnowledgeId]| {
        rows.iter()
            .map(|row| {
                let row = tables.knowledge.get(*row).unwrap();
                tables.propositions.get(row.proposition).unwrap().clone()
            })
            .collect::<Vec<_>>()
    };
    let context = PropositionContext::from_value_types(
        tables
            .values
            .iter()
            .map(|(value, row)| (value, row.scalar_type)),
    )
    .unwrap();
    let decide = |goal: &Proposition, certificate| {
        verify_obligation(
            &context,
            &Obligation {
                id: obligation,
                proposition: goal.clone(),
                class: ObligationClass::Derivable,
            },
            &resolve(&evidence.assumptions),
            &resolve(&evidence.axioms),
            EvidenceRoute::CertificateDerived(certificate),
        )
    };
    // The mathematical core judged it too; a `Refused` core decision would
    // mean only the rule checker saw this certificate.
    let fact = decide(&evidence.goal, evidence.certificate.clone()).unwrap();
    let AcceptedFactRoute::CertificateDerived { acceptance, .. } = &fact.route else {
        panic!("certificate route");
    };
    let core = acceptance.mathematical_core.clone();
    let mut tampered = evidence.certificate.clone();
    tampered.proof.conclusion = Proposition::Falsehood;
    assert!(decide(&evidence.goal, tampered).is_err());
    core
}

fn assert_unproved(verdict: &Checked<Discharged>) {
    let refusal = verdict
        .as_ref()
        .expect_err("the obligation must be refused");
    assert_eq!(
        refusal.reason,
        RefusalReason::Unproved,
        "{}",
        refusal.detail
    );
}

fn add_of_formals(requires: Vec<Proposition>) -> Machine {
    Machine {
        requires,
        ..machine(
            vec![u32_type(), u32_type()],
            vec![returns(strict(
                Strict::Add,
                slot(0, u32_type()),
                slot(1, u32_type()),
            ))],
        )
    }
}

#[test]
fn bounded_operands_discharge_an_exact_add_with_kernel_evidence() {
    let (elaboration, result) = run(vec![add_of_formals(vec![at_most(1, 100), at_most(2, 100)])]);
    let verdicts = verdicts(&elaboration, &result, 1, safety);
    assert_eq!(verdicts.len(), 1);
    let (obligation, verdict) = verdicts[0];
    let evidence = certificate(verdict);
    assert_kernel_checked(&elaboration, 1, obligation, evidence);
}

#[test]
fn an_exact_add_of_unbounded_operands_is_unproved() {
    let (elaboration, result) = run(vec![add_of_formals(Vec::new())]);
    let verdicts = verdicts(&elaboration, &result, 1, safety);
    assert_unproved(verdicts[0].1);
}

#[test]
fn an_add_that_overflows_is_refused() {
    let overflow = machine(
        Vec::new(),
        vec![returns(strict(
            Strict::Add,
            unsigned(u128::from(u32::MAX)),
            unsigned(1),
        ))],
    );
    let (elaboration, result) = run(vec![overflow]);
    assert_unproved(verdicts(&elaboration, &result, 1, safety)[0].1);
}

#[test]
fn a_bound_follows_an_earlier_exact_result() {
    // local = a + 1; return local + 1, with a <= 10: the second add needs
    // the first add's result equation as a definition.
    let chained = Machine {
        locals: vec![u32_type()],
        requires: vec![at_most(1, 10)],
        ..machine(
            vec![u32_type()],
            vec![
                Statement::Bind {
                    slot: 1,
                    value: strict(Strict::Add, slot(0, u32_type()), unsigned(1)),
                },
                returns(strict(
                    Strict::Add,
                    slot(1, u32_type()),
                    unsigned(u128::from(u32::MAX) - 11),
                )),
            ],
        )
    };
    let (elaboration, result) = run(vec![chained]);
    for (obligation, verdict) in verdicts(&elaboration, &result, 1, safety) {
        assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
    }
}

#[test]
fn a_signed_add_needs_both_bounds() {
    let signed = |value: i128| bound(i32_type(), IntegerValue::Signed(value));
    let range = |position| {
        vec![
            Proposition::LessOrEqual(signed(-100), formal(position, i32_type())),
            Proposition::LessOrEqual(formal(position, i32_type()), signed(100)),
        ]
    };
    let add = |requires| Machine {
        requires,
        result: i32_type(),
        ..machine(
            vec![i32_type(), i32_type()],
            vec![returns(strict(
                Strict::Add,
                slot(0, i32_type()),
                slot(1, i32_type()),
            ))],
        )
    };
    let mut both = range(1);
    both.extend(range(2));
    let (elaboration, result) = run(vec![add(both)]);
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, safety)[0];
    assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
    // Upper bounds alone leave the sum's lower end at the carrier minimum.
    let upper_only = vec![
        Proposition::LessOrEqual(formal(1, i32_type()), signed(100)),
        Proposition::LessOrEqual(formal(2, i32_type()), signed(100)),
    ];
    let (elaboration, result) = run(vec![add(upper_only)]);
    assert_unproved(verdicts(&elaboration, &result, 1, safety)[0].1);
}

#[test]
fn a_divisor_known_nonzero_discharges_and_a_zero_divisor_is_refused() {
    let divide = |requires, divisor| Machine {
        requires,
        ..machine(
            vec![u32_type(), u32_type()],
            vec![returns(strict(
                Strict::Divide,
                slot(0, u32_type()),
                divisor,
            ))],
        )
    };
    let (elaboration, result) = run(vec![divide(vec![at_least(2, 1)], slot(1, u32_type()))]);
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, safety)[0];
    assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));

    let (elaboration, result) = run(vec![divide(Vec::new(), slot(1, u32_type()))]);
    assert_unproved(verdicts(&elaboration, &result, 1, safety)[0].1);

    let (elaboration, result) = run(vec![divide(Vec::new(), unsigned(0))]);
    assert_unproved(verdicts(&elaboration, &result, 1, safety)[0].1);
}

#[test]
fn a_multiply_of_bounded_operands_discharges() {
    let multiply = Machine {
        requires: vec![at_most(1, 1000), at_most(2, 1000)],
        ..machine(
            vec![u32_type(), u32_type()],
            vec![returns(strict(
                Strict::Multiply,
                slot(0, u32_type()),
                slot(1, u32_type()),
            ))],
        )
    };
    let (elaboration, result) = run(vec![multiply]);
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, safety)[0];
    assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
}

/// Machine 1 calls machine 2, whose `requires` is `x <= 10`.
fn caller(requires: Vec<Proposition>, argument: Expression) -> Vec<Machine> {
    let callee = Machine {
        requires: vec![at_most(1, 10)],
        ..machine(vec![u32_type()], vec![returns(slot(0, u32_type()))])
    };
    let caller = Machine {
        requires,
        ..machine(vec![u32_type()], vec![returns(call(2, vec![argument]))])
    };
    vec![caller, callee]
}

#[test]
fn a_call_requirement_discharges_from_the_callers_requires() {
    let (elaboration, result) = run(caller(vec![at_most(1, 5)], slot(0, u32_type())));
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, requirement)[0];
    assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
}

#[test]
fn a_call_requirement_without_knowledge_is_unproved() {
    let (elaboration, result) = run(caller(Vec::new(), slot(0, u32_type())));
    assert_unproved(verdicts(&elaboration, &result, 1, requirement)[0].1);
}

#[test]
fn a_call_requirement_broken_by_a_constant_is_refused() {
    let (elaboration, result) = run(caller(Vec::new(), unsigned(20)));
    assert_unproved(verdicts(&elaboration, &result, 1, requirement)[0].1);
    // The same call with 7 holds by the constant's own result equation.
    let (elaboration, result) = run(caller(Vec::new(), unsigned(7)));
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, requirement)[0];
    assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
}

fn guaranteeing(requires: Vec<Proposition>, ensures: Proposition) -> Machine {
    Machine {
        requires,
        ensures: vec![ensures],
        ..machine(vec![u32_type()], vec![returns(slot(0, u32_type()))])
    }
}

#[test]
fn a_normal_guarantee_discharges_from_the_entry_requires() {
    let (elaboration, result) = run(vec![guaranteeing(vec![at_most(1, 3)], at_most(1, 10))]);
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, guarantee)[0];
    assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
}

#[test]
fn a_normal_guarantee_without_knowledge_is_unproved() {
    let (elaboration, result) = run(vec![guaranteeing(Vec::new(), at_most(1, 10))]);
    assert_unproved(verdicts(&elaboration, &result, 1, guarantee)[0].1);
}

#[test]
fn a_false_normal_guarantee_is_refused() {
    let (elaboration, result) = run(vec![guaranteeing(vec![at_most(1, 3)], at_least(1, 5))]);
    assert_unproved(verdicts(&elaboration, &result, 1, guarantee)[0].1);
}

#[test]
fn knowledge_rows_carry_their_justification_and_point() {
    let (elaboration, _) = run(caller(vec![at_most(1, 5)], slot(0, u32_type())));
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let mut entry_requirements = 0;
    let mut equations = 0;
    for (_, row) in tables.knowledge.iter() {
        assert!(row.premises.is_empty());
        match row.justification {
            Justification::EntryRequirement { clause } => {
                assert_eq!(clause, 0);
                assert!(matches!(row.available_from, ProgramPoint::BlockEntry(_)));
                entry_requirements += 1;
            }
            Justification::OperationSemantics(operation) => {
                assert_eq!(row.available_from, ProgramPoint::AfterOperation(operation));
                equations += 1;
            }
            other => panic!("unexpected justification {other:?}"),
        }
    }
    assert_eq!(entry_requirements, 1);
    // The only operation is the call, which has no result equation.
    assert_eq!(equations, 0);
}

#[test]
fn knowledge_after_an_arm_operation_is_not_available_after_the_guard() {
    // when a == 0 -> 1 + a; return a + 1. The arm's operations do not
    // dominate the continuation.
    let guarded = Machine {
        requires: vec![at_most(1, 10)],
        ..machine(
            vec![u32_type()],
            vec![
                Statement::Arm {
                    guard: Some(expression(
                        ExpressionKind::Strict {
                            operation: Strict::Equal,
                            left: Box::new(slot(0, u32_type())),
                            right: Box::new(unsigned(0)),
                        },
                        Some(ScalarType::Boolean),
                    )),
                    target: Target::Value(strict(Strict::Add, unsigned(1), slot(0, u32_type()))),
                    source: nowhere(),
                },
                returns(strict(Strict::Add, slot(0, u32_type()), unsigned(1))),
            ],
        )
    };
    let (elaboration, result) = run(vec![guarded]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let lowered = &elaboration.module.machines[0];
    let graph = crate::graph::Graph::new(id(1), lowered, tables).unwrap();
    let arm = &lowered.blocks[1];
    let rest = &lowered.blocks[2];
    let in_arm = ProgramPoint::AfterOperation(arm.operations[0].id);
    let in_rest = ProgramPoint::AfterOperation(rest.operations[0].id);
    assert!(!graph.dominates(in_arm, in_rest).unwrap());
    assert!(!graph.dominates(in_rest, in_arm).unwrap());
    let entry = ProgramPoint::BlockEntry(lowered.entry);
    assert!(graph.dominates(entry, in_arm).unwrap() && graph.dominates(entry, in_rest).unwrap());
    // Both adds discharge from `a <= 10` and their own constants.
    for (obligation, verdict) in verdicts(&elaboration, &result, 1, safety) {
        assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
    }
}

/// `local = a * b; return local <operation> operand`: the second operation
/// is representable only through a bound on the product.
fn after_a_product(
    scalar_type: ScalarType,
    requires: Vec<Proposition>,
    operation: Strict,
    operand: IntegerValue,
) -> Machine {
    Machine {
        locals: vec![scalar_type],
        requires,
        result: scalar_type,
        ..machine(
            vec![scalar_type, scalar_type],
            vec![
                Statement::Bind {
                    slot: 2,
                    value: strict(Strict::Multiply, slot(0, scalar_type), slot(1, scalar_type)),
                },
                returns(strict(
                    operation,
                    slot(2, scalar_type),
                    literal(scalar_type, operand),
                )),
            ],
        )
    }
}

fn assert_all_kernel_checked(machine: Machine, expected: usize) {
    let (elaboration, result) = run(vec![machine]);
    let verdicts = verdicts(&elaboration, &result, 1, safety);
    assert_eq!(verdicts.len(), expected);
    for (obligation, verdict) in verdicts {
        assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
    }
}

fn compare(operation: Strict, left: Expression, right: Expression) -> Expression {
    expression(
        ExpressionKind::Strict {
            operation,
            left: Box::new(left),
            right: Box::new(right),
        },
        Some(ScalarType::Boolean),
    )
}

fn guarded(guard: Expression, target: Expression) -> Statement {
    Statement::Arm {
        guard: Some(guard),
        target: Target::Value(target),
        source: nowhere(),
    }
}

#[test]
fn a_true_branch_bounds_its_arm() {
    // when a < 10 -> a + (MAX - 9); return 0. Only the guard bounds `a`.
    let arm = |guard: Option<Expression>| {
        let sum = strict(
            Strict::Add,
            slot(0, u32_type()),
            unsigned(u128::from(u32::MAX) - 9),
        );
        let mut statements = Vec::new();
        match guard {
            Some(guard) => statements.push(guarded(guard, sum)),
            None => statements.push(returns(sum)),
        }
        statements.push(returns(unsigned(0)));
        machine(vec![u32_type()], statements)
    };
    let (elaboration, result) = run(vec![arm(Some(compare(
        Strict::Less,
        slot(0, u32_type()),
        unsigned(10),
    )))]);
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, safety)[0];
    let core = assert_kernel_accepted(&elaboration, 1, obligation, certificate(verdict));
    // The rule checker accepts it; the mathematical core does not cover the
    // discreteness step from the transported `a < 10` to `a <= 9`, and says
    // so instead of judging.
    assert_eq!(
        core,
        MathematicalCoreDecision::Refused("checked discreteness has integer order endpoints"),
        "{core:?}"
    );
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    assert_eq!(
        tables
            .knowledge
            .values()
            .filter(|row| matches!(row.justification, Justification::BranchCondition { .. }))
            .count(),
        2
    );
    // The same add with the guard on `a < 11` overflows at `a = 10`.
    let (elaboration, result) = run(vec![arm(Some(compare(
        Strict::Less,
        slot(0, u32_type()),
        unsigned(11),
    )))]);
    assert_unproved(verdicts(&elaboration, &result, 1, safety)[0].1);
}

#[test]
fn a_false_branch_bounds_the_continuation() {
    // when a < 10 -> 0; return a - 10. Past the guard, `10 <= a`.
    let program = machine(
        vec![u32_type()],
        vec![
            guarded(
                compare(Strict::Less, slot(0, u32_type()), unsigned(10)),
                unsigned(0),
            ),
            returns(strict(Strict::Subtract, slot(0, u32_type()), unsigned(10))),
        ],
    );
    let (elaboration, result) = run(vec![program]);
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, safety)[0];
    assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
}

#[test]
fn every_obligation_has_exactly_one_verdict() {
    // Calls, guards, every partial operation and guarantees in one closure;
    // some discharge and some do not. No obligation may lack a verdict and
    // none may have two.
    let callee = Machine {
        requires: vec![at_most(1, 10)],
        ensures: vec![at_most(1, 10)],
        ..machine(
            vec![u32_type(), u32_type()],
            vec![
                guarded(
                    compare(Strict::Equal, slot(1, u32_type()), unsigned(0)),
                    unsigned(0),
                ),
                returns(strict(
                    Strict::ShiftLeft,
                    strict(Strict::Multiply, slot(0, u32_type()), slot(1, u32_type())),
                    strict(Strict::Remainder, slot(0, u32_type()), slot(1, u32_type())),
                )),
            ],
        )
    };
    let caller = Machine {
        locals: vec![u32_type()],
        requires: vec![at_most(1, 5)],
        ..machine(
            vec![u32_type()],
            vec![
                Statement::Bind {
                    slot: 1,
                    value: call(2, vec![slot(0, u32_type()), unsigned(3)]),
                },
                returns(strict(
                    Strict::Divide,
                    strict(Strict::Subtract, slot(1, u32_type()), slot(0, u32_type())),
                    call(2, vec![unsigned(11), slot(1, u32_type())]),
                )),
            ],
        )
    };
    let (elaboration, result) = run(vec![caller, callee]);
    let mut discharged = 0;
    let mut refused = 0;
    for (machine, tables) in elaboration.tables.machines.iter() {
        let verdicts = &result.machines.get(machine).unwrap().verdicts;
        assert_eq!(verdicts.len(), tables.obligations.len());
        assert!(
            tables.obligations.keys().eq(verdicts.keys()),
            "verdicts are indexed by exactly the obligation identities"
        );
        for verdict in verdicts.values() {
            match verdict {
                Ok(_) => discharged += 1,
                Err(_) => refused += 1,
            }
        }
    }
    assert!(discharged > 0 && refused > 0);
}

/// `formal(left) R formal(right)` over u32 formals, as a contract clause.
fn relate(strict: bool, left: u64, right: u64) -> Proposition {
    let (left, right) = (formal(left, u32_type()), formal(right, u32_type()));
    if strict {
        Proposition::LessThan(left, right)
    } else {
        Proposition::LessOrEqual(left, right)
    }
}

#[test]
fn a_bound_follows_an_earlier_exact_multiply_result() {
    // a <= 1000 and b <= 1000 give local <= 1_000_000, which is exactly
    // what the add needs. One more and the add can overflow.
    let requires = || vec![at_most(1, 1000), at_most(2, 1000)];
    let headroom = u128::from(u32::MAX) - 1_000_000;
    assert_all_kernel_checked(
        after_a_product(
            u32_type(),
            requires(),
            Strict::Add,
            IntegerValue::Unsigned(headroom),
        ),
        2,
    );
    let (elaboration, result) = run(vec![after_a_product(
        u32_type(),
        requires(),
        Strict::Add,
        IntegerValue::Unsigned(headroom + 1),
    )]);
    let verdicts = verdicts(&elaboration, &result, 1, safety);
    assert!(verdicts[0].1.is_ok(), "the multiply itself still fits");
    assert_unproved(verdicts[1].1);
}

#[test]
fn a_signed_multiply_result_is_bounded_on_both_sides() {
    // a in [-10, 10] and b in [-20, 5] give local in [-200, 200]: the least
    // product is 10 * -20 and the greatest -10 * -20, neither from the
    // operands' lower bounds alone.
    let signed = |value: i128| bound(i32_type(), IntegerValue::Signed(value));
    let range = |position, low, high| {
        vec![
            Proposition::LessOrEqual(signed(low), formal(position, i32_type())),
            Proposition::LessOrEqual(formal(position, i32_type()), signed(high)),
        ]
    };
    let requires = || {
        let mut requires = range(1, -10, 10);
        requires.extend(range(2, -20, 5));
        requires
    };
    let maximum = i128::from(i32::MAX);
    for (operation, fits) in [
        (Strict::Add, maximum - 200),
        (Strict::Subtract, maximum - 199),
    ] {
        assert_all_kernel_checked(
            after_a_product(
                i32_type(),
                requires(),
                operation,
                IntegerValue::Signed(fits),
            ),
            2,
        );
        let (elaboration, result) = run(vec![after_a_product(
            i32_type(),
            requires(),
            operation,
            IntegerValue::Signed(fits + 1),
        )]);
        assert_unproved(verdicts(&elaboration, &result, 1, safety)[1].1);
    }
}

#[test]
fn a_multiply_result_past_the_carrier_keeps_only_its_carrier_bound() {
    // a, b <= 100_000 reach 10^10, beyond u32. The multiply's own
    // obligation is unproved, and its result gains no bound that would let
    // the add through.
    let (elaboration, result) = run(vec![after_a_product(
        u32_type(),
        vec![at_most(1, 100_000), at_most(2, 100_000)],
        Strict::Add,
        IntegerValue::Unsigned(1),
    )]);
    let verdicts = verdicts(&elaboration, &result, 1, safety);
    assert_unproved(verdicts[0].1);
    assert_unproved(verdicts[1].1);
}

#[test]
fn an_index_below_a_length_passes_to_a_callee_with_no_literal_bound() {
    // check(i, len) requires i < len calls at(i, len), which requires i < len
    // and guarantees i <= len: both are relations between two values.
    let at = Machine {
        requires: vec![relate(true, 1, 2)],
        ensures: vec![relate(false, 1, 2)],
        ..machine(
            vec![u32_type(), u32_type()],
            vec![returns(slot(0, u32_type()))],
        )
    };
    let check = Machine {
        requires: vec![relate(true, 1, 2)],
        ..machine(
            vec![u32_type(), u32_type()],
            vec![returns(call(
                2,
                vec![slot(0, u32_type()), slot(1, u32_type())],
            ))],
        )
    };
    let (elaboration, result) = run(vec![check, at]);
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, requirement)[0];
    assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
    // `i <= len` from `i < len`: a strict relation, weakened.
    let (obligation, verdict) = verdicts(&elaboration, &result, 2, guarantee)[0];
    assert_kernel_checked(&elaboration, 2, obligation, certificate(verdict));
}

#[test]
fn an_unsigned_difference_is_safe_when_the_subtrahend_is_no_larger() {
    // a - b with b <= a: no literal bound on either operand.
    let subtract = |requires| Machine {
        requires,
        ..machine(
            vec![u32_type(), u32_type()],
            vec![returns(strict(
                Strict::Subtract,
                slot(0, u32_type()),
                slot(1, u32_type()),
            ))],
        )
    };
    let (elaboration, result) = run(vec![subtract(vec![relate(false, 2, 1)])]);
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, safety)[0];
    assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
    // a <= b says nothing about a - b staying above zero.
    let (elaboration, result) = run(vec![subtract(vec![relate(false, 1, 2)])]);
    assert_unproved(verdicts(&elaboration, &result, 1, safety)[0].1);
}

#[test]
fn order_between_values_is_transitive() {
    // requires a <= b, b <= c ensures a <= c; and not c <= a.
    let chain = |ensures| Machine {
        requires: vec![relate(false, 1, 2), relate(false, 2, 3)],
        ensures: vec![ensures],
        ..machine(
            vec![u32_type(), u32_type(), u32_type()],
            vec![returns(slot(0, u32_type()))],
        )
    };
    let (elaboration, result) = run(vec![chain(relate(false, 1, 3))]);
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, guarantee)[0];
    assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
    let (elaboration, result) = run(vec![chain(relate(false, 3, 1))]);
    assert_unproved(verdicts(&elaboration, &result, 1, guarantee)[0].1);
}

#[test]
fn a_branch_relation_makes_a_difference_safe() {
    // when b <= a -> a - b; return 0. Only the guard relates a and b.
    let program = machine(
        vec![u32_type(), u32_type()],
        vec![
            guarded(
                compare(
                    Strict::LessOrEqual,
                    slot(1, u32_type()),
                    slot(0, u32_type()),
                ),
                strict(Strict::Subtract, slot(0, u32_type()), slot(1, u32_type())),
            ),
            returns(unsigned(0)),
        ],
    );
    let (elaboration, result) = run(vec![program]);
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, safety)[0];
    assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
}

/// The certificate of a discharged verdict.
fn certificate(verdict: &Checked<Discharged>) -> &Evidence {
    verdict
        .as_ref()
        .expect("discharged")
        .certificate()
        .expect("a certificate")
}

fn published(kind: &ObligationKind) -> bool {
    matches!(kind, ObligationKind::PublishedGuarantee { .. })
}

#[test]
fn a_published_clause_holds_only_when_every_return_establishes_it() {
    // f(a) ensures a <= 10 { when a <= 10 -> 0; return 1 }: two normal
    // returns. Without a requires, the arm's return establishes the clause
    // from its guard and the fall-through return does not.
    let program = |requires| Machine {
        requires,
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
    };
    let (elaboration, result) = run(vec![program(Vec::new())]);
    let exits = verdicts(&elaboration, &result, 1, guarantee);
    assert_eq!(exits.len(), 2);
    assert!(exits.iter().filter(|(_, verdict)| verdict.is_ok()).count() == 1);
    let (clause, verdict) = verdicts(&elaboration, &result, 1, published)[0];
    assert_unproved(verdict);
    // The module's clause carries the published identity, not an exit's.
    let lowered = &elaboration.module.machines[0];
    assert_eq!(lowered.contract.ensures[0].obligation, clause);

    let (elaboration, result) = run(vec![program(vec![at_most(1, 5)])]);
    let (_, verdict) = verdicts(&elaboration, &result, 1, published)[0];
    let Ok(Discharged::EveryExit(exits)) = verdict else {
        panic!("both returns establish the clause: {verdict:?}");
    };
    assert_eq!(exits.len(), 2);
}

#[test]
fn a_published_clause_with_no_normal_return_holds_vacuously() {
    // The subset cannot write a machine with no normal return, so the
    // tables state one directly: a published clause and no return edge.
    let mut tables = terminal_check_tables::MachineTables::default();
    let clause = tables
        .obligations
        .push(terminal_check_tables::obligations::ObligationRow {
            kind: ObligationKind::PublishedGuarantee { clause: 0 },
            created_at: ProgramPoint::BlockEntry(id(1)),
        });
    let verdict = crate::published(&tables, id(1), clause, 0, &[None]);
    assert_eq!(verdict, Ok(Discharged::EveryExit(Vec::new())));
}

#[test]
fn a_disequality_guard_splits_into_cases() {
    // when 0 != a -> a - 1; return 0. The guard transports to `a < 0 or
    // 0 < a`; the first alternative contradicts the carrier, the second
    // bounds `a` from below.
    let program = machine(
        vec![u32_type()],
        vec![
            guarded(
                compare(Strict::NotEqual, unsigned(0), slot(0, u32_type())),
                strict(Strict::Subtract, slot(0, u32_type()), unsigned(1)),
            ),
            returns(unsigned(0)),
        ],
    );
    let (elaboration, result) = run(vec![program]);
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, safety)[0];
    let core = assert_kernel_accepted(&elaboration, 1, obligation, certificate(verdict));
    // The case split and the contradiction are judged; the discreteness step
    // from `0 < a` to `1 <= a` is the one the core does not cover.
    assert_eq!(
        core,
        MathematicalCoreDecision::Refused("checked discreteness has integer order endpoints")
    );
}
