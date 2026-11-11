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
    Target, Unpositioned, ValueType,
};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::values::Justification;
use terminal_check_tables::{Checked, ProgramPoint, RefusalReason};

use crate::{Discharge, Discharged, Evidence, RouteVerdict, UnprovedObligation, discharge, judge};
use terminal_check_tables::{Construct, Refusal, UnprovedCause};

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
        result: result.map(ValueType::Scalar),
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
    let result = left.result.and_then(ValueType::scalar);
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

/// Lowers `machines` (machine 1 is the entry).
fn lower(machines: Vec<Machine>) -> Checked<structure_elaboration::Elaboration> {
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
    for machine in machines {
        let parameters: Vec<ValueType> = machine.parameters.into_iter().map(Into::into).collect();
        let mut slots = parameters.clone();
        slots.extend(machine.locals.into_iter().map(ValueType::Scalar));
        bodies.push(Body {
            slots,
            statements: machine.statements,
        });
        signatures.push(Signature {
            parameters,
            result: Some(machine.result.into()),
            requires: machine.requires,
            ensures: machine.ensures,
            crashes: CrashCeiling::Authored(Vec::new()),
        });
    }
    structure_elaboration::lower_bodies(id(1), &bodies, &signatures, &IdTable::default())
}

/// Lowers `machines` (machine 1 is the entry) and discharges them.
fn run(machines: Vec<Machine>) -> (structure_elaboration::Elaboration, Discharge) {
    let mut elaboration = lower(machines).unwrap();
    let result = discharge(&mut elaboration).unwrap();
    (elaboration, result)
}

/// The route's verdict on `machines`, which must reject the program.
fn rejected(machines: Vec<Machine>) -> Vec<UnprovedObligation> {
    match judge(lower(machines)).unwrap() {
        RouteVerdict::Rejected {
            unproved,
            unestablished,
        } if unestablished.is_empty() => unproved,
        verdict => panic!("the program must be rejected: {verdict:?}"),
    }
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
    assert!(
        matches!(refusal.reason, RefusalReason::Unproved(_)),
        "{:?}: {}",
        refusal.reason,
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

// The route's verdict: an obligation the prover cannot discharge rejects the
// program; the route never accepts it with a guard or hands Omega 00 a
// module with an unsealed obligation.

#[test]
fn the_route_accepts_a_closure_only_when_every_obligation_is_discharged() {
    let RouteVerdict::Accepted(accepted) = judge(lower(vec![add_of_formals(vec![
        at_most(1, 100),
        at_most(2, 100),
    ])]))
    .unwrap() else {
        panic!("every obligation is discharged");
    };
    // One seal for the one operation obligation, so Omega 00 receives
    // evidence for every operation obligation of the module.
    assert_eq!(accepted.seals.len(), 1);
    let safety_rows = verdicts(&accepted.elaboration, &accepted.discharge, 1, safety);
    assert_eq!(safety_rows.len(), 1);
    assert_eq!(accepted.seals[0].obligation, safety_rows[0].0);
}

#[test]
fn an_unproved_operation_is_a_compile_error_naming_its_kind_and_cause() {
    let unproved = rejected(vec![add_of_formals(Vec::new())]);
    assert_eq!(unproved.len(), 1);
    let entry = &unproved[0];
    assert!(safety(&entry.kind));
    assert_eq!(entry.cause, UnprovedCause::NoDerivation);
    let diagnostic = entry.reject_diagnostic();
    assert_eq!(diagnostic.severity, diagnostics::DiagnosticSeverity::Error);
    assert!(
        diagnostic.message.starts_with(
            "unproved obligation: operation safety: no proof found from the facts available here ("
        ),
        "{}",
        diagnostic.message
    );
    // The test bodies carry no authored span, and none is invented.
    assert_eq!(diagnostic.source_span, None);
}

#[test]
fn an_unproved_call_requirement_names_the_callee_clause() {
    let unproved = rejected(caller(Vec::new(), unsigned(20)));
    assert_eq!(unproved.len(), 1);
    assert!(requirement(&unproved[0].kind));
    assert!(
        unproved[0]
            .reject_diagnostic()
            .message
            .starts_with("unproved obligation: requires clause 1 of the called machine: "),
        "{}",
        unproved[0].reject_diagnostic().message
    );
}

#[test]
fn an_unestablished_ensures_is_reported_once_at_the_return_that_misses_it() {
    // f(a) ensures a <= 10 { when a <= 10 -> 0; return 1 }: the fall-through
    // return does not establish the clause. The published clause fails only
    // because of it, so it is not reported a second time.
    let program = Machine {
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
    let unproved = rejected(vec![program]);
    assert_eq!(unproved.len(), 1, "{unproved:?}");
    assert!(guarantee(&unproved[0].kind));
    assert!(
        unproved[0]
            .reject_diagnostic()
            .message
            .starts_with("unproved obligation: ensures clause 1 on a normal return: "),
        "{}",
        unproved[0].reject_diagnostic().message
    );
}

#[test]
fn an_unsupported_closure_falls_back_whole_and_a_route_defect_is_not_accepted() {
    let refused = |reason| Refusal {
        reason,
        machine: id(1),
        concerns: Construct::Machine,
        detail: String::from("test"),
    };
    assert!(matches!(
        judge(Err(refused(RefusalReason::Unsupported))),
        Ok(RouteVerdict::FallBack(_))
    ));
    assert!(judge(Err(refused(RefusalReason::ForeignIdentity))).is_err());
}

fn bool_type() -> ScalarType {
    ScalarType::Boolean
}

/// Machine 1 takes `(a: u32, flag: bool)` and calls machine 2 with `flag`;
/// machine 2 requires its `bool` formal to equal `false`, a goal form the
/// prover has no rule for. With `add`, machine 1 returns the call's result
/// plus `a`, an add of two unbounded values.
fn bool_requirement(add: bool) -> Vec<Machine> {
    let callee = Machine {
        requires: vec![Proposition::Equal(
            formal(1, bool_type()),
            ScalarTerm::boolean(false),
        )],
        ..machine(vec![bool_type()], vec![returns(unsigned(0))])
    };
    let called = call(2, vec![slot(1, bool_type())]);
    let result = if add {
        strict(Strict::Add, called, slot(0, u32_type()))
    } else {
        called
    };
    let caller = machine(vec![u32_type(), bool_type()], vec![returns(result)]);
    vec![caller, callee]
}

#[test]
fn the_prover_has_rules_for_integer_goals_and_not_for_other_forms() {
    let value = formal(1, u32_type());
    let flag = formal(1, bool_type());
    assert!(crate::has_rules_for(&at_most(1, 10)));
    assert!(crate::has_rules_for(&Proposition::Equal(
        value.clone(),
        bound(u32_type(), IntegerValue::Unsigned(3)),
    )));
    assert!(!crate::has_rules_for(&Proposition::Equal(
        flag.clone(),
        ScalarTerm::boolean(false),
    )));
    // One uncovered part leaves the whole goal without a rule.
    assert!(!crate::has_rules_for(&Proposition::Conjunction(vec![
        at_most(1, 10),
        Proposition::Equal(flag, ScalarTerm::boolean(true)),
    ])));
}

#[test]
fn a_goal_form_without_a_rule_is_incomplete_not_rejected() {
    let RouteVerdict::Incomplete(unproved) = judge(lower(bool_requirement(false))).unwrap() else {
        panic!("the prover has no rule for a bool equality");
    };
    assert_eq!(unproved.len(), 1, "{unproved:?}");
    assert!(requirement(&unproved[0].kind));
    assert_eq!(unproved[0].cause, UnprovedCause::NoRuleForGoalForm);
    assert!(
        unproved[0].message().starts_with(
            "unproved obligation: requires clause 1 of the called machine: \
             the prover has no rule for this form of goal ("
        ),
        "{}",
        unproved[0].message()
    );
}

#[test]
fn an_unproved_goal_with_rules_rejects_even_beside_one_without() {
    let unproved = rejected(bool_requirement(true));
    let causes: Vec<UnprovedCause> = unproved.iter().map(|entry| entry.cause).collect();
    assert_eq!(
        causes,
        vec![
            UnprovedCause::NoRuleForGoalForm,
            UnprovedCause::NoDerivation
        ],
        "{unproved:?}"
    );
}

// Records by structure. `Point { x: u32, y: u32 }` is record 1 with fields 1
// and 2; `Line { from: Point, to: Point }` is record 2 with fields 3 and 4.
// A field read is a fresh value; the tests below check when it is provably
// the value its place holds.

use semantic_vocabulary::{OperationId, PlaceId, StructuralFieldId, StructuralTypeId, ValueId};
use structure_elaboration::{RecordField, RecordType};
use terminal_check_tables::bindings::{Content, PlaceOrigin, PlaceRow, Segment, WriteRow};
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{ExactPlaceId, MachineTables};
use terminal_psi::OperationKind;

fn point() -> ValueType {
    ValueType::Record(id(1))
}

fn line() -> ValueType {
    ValueType::Record(id(2))
}

fn records() -> IdTable<StructuralTypeId, RecordType> {
    let field = |raw, spelled: &str, value_type| RecordField {
        id: id(raw),
        identity: spelled.to_owned(),
        value_type,
    };
    let mut records = IdTable::default();
    records.push(RecordType {
        identity: String::from("Point"),
        fields: vec![
            field(1, "x", ValueType::Scalar(u32_type())),
            field(2, "y", ValueType::Scalar(u32_type())),
        ],
    });
    records.push(RecordType {
        identity: String::from("Line"),
        fields: vec![field(3, "from", point()), field(4, "to", point())],
    });
    records
}

fn of_type(kind: ExpressionKind, result: ValueType) -> Expression {
    Expression {
        kind,
        result: Some(result),
        source: nowhere(),
    }
}

fn place(slot: usize, value_type: ValueType) -> Expression {
    of_type(ExpressionKind::Slot(slot), value_type)
}

/// `base.field`, a `u32` field.
fn read(base: Expression, field: usize) -> Expression {
    of_type(
        ExpressionKind::Field {
            base: Box::new(base),
            field,
        },
        u32_type().into(),
    )
}

fn point_of(x: Expression, y: Expression) -> Expression {
    of_type(
        ExpressionKind::Record {
            fields: vec![(0, x), (1, y)],
        },
        point(),
    )
}

fn bind(slot: usize, value: Expression) -> Statement {
    Statement::Bind { slot, value }
}

/// One machine over `Point` and `Line`: its formals, its locals and its
/// result, then its body.
struct Shaped {
    parameters: Vec<ValueType>,
    locals: Vec<ValueType>,
    result: ValueType,
    requires: Vec<Proposition>,
    statements: Vec<Statement>,
}

/// `bounded(v: u32) -> u32 requires v <= 10 { v }`.
fn bounded() -> Shaped {
    Shaped {
        parameters: vec![u32_type().into()],
        locals: Vec::new(),
        result: u32_type().into(),
        requires: vec![at_most(1, 10)],
        statements: vec![returns(slot(0, u32_type()))],
    }
}

fn elaborate_places(machines: Vec<Shaped>) -> structure_elaboration::Elaboration {
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
            ensures: Vec::new(),
            crashes: CrashCeiling::Authored(Vec::new()),
        });
    }
    structure_elaboration::lower_bodies(id(1), &bodies, &signatures, &records()).unwrap()
}

fn run_places(machines: Vec<Shaped>) -> (structure_elaboration::Elaboration, Discharge) {
    let mut elaboration = elaborate_places(machines);
    let result = discharge(&mut elaboration).unwrap();
    (elaboration, result)
}

/// Every field read of machine 1, in block and operation order, as the
/// operation and its result value.
fn field_reads(elaboration: &structure_elaboration::Elaboration) -> Vec<(OperationId, ValueId)> {
    elaboration.module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::ScalarStructuralField { .. }))
        .map(|operation| (operation.id, operation.result.scalar_ref().unwrap().id))
        .collect()
}

/// The result of each integer constant of machine 1, in block and operation
/// order.
fn constants(elaboration: &structure_elaboration::Elaboration) -> Vec<ValueId> {
    elaboration.module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::IntegerConstant { .. }))
        .map(|operation| operation.result.scalar_ref().unwrap().id)
        .collect()
}

/// The value machine 1's knowledge equates a read's result with, if any:
/// the right side of the read operation's own `OperationSemantics` row.
fn read_equals(tables: &MachineTables, read: (OperationId, ValueId)) -> Option<ValueId> {
    let mut held = tables.knowledge.iter().filter_map(|(_, row)| {
        if row.justification != Justification::OperationSemantics(read.0) {
            return None;
        }
        assert_eq!(row.available_from, ProgramPoint::AfterOperation(read.0));
        match tables.propositions.get(row.proposition).unwrap() {
            Proposition::Equal(
                ScalarTerm::Value { id: result, .. },
                ScalarTerm::Value { id: held, .. },
            ) => {
                assert_eq!(*result, read.1);
                Some(*held)
            }
            other => panic!("a read equation is between two values, found {other:?}"),
        }
    });
    let first = held.next();
    assert!(held.next().is_none(), "a read has at most one equation");
    first
}

#[test]
fn a_read_after_construction_is_provably_the_constructed_operand() {
    // f(a: u32) -> u32 {
    //     let p = Point { x: a, y: 7 };
    //     bounded(p.y);       // needs p.y <= 10: p.y is the 7 it was built with
    //     bounded(p.x)        // needs p.x <= 10: p.x is `a`, and `y`'s 7 says
    // }                       // nothing about a sibling
    let caller = Shaped {
        parameters: vec![u32_type().into()],
        locals: vec![point()],
        result: u32_type().into(),
        requires: Vec::new(),
        statements: vec![
            bind(1, point_of(slot(0, u32_type()), unsigned(7))),
            Statement::Evaluate(call(2, vec![read(place(1, point()), 1)])),
            returns(call(2, vec![read(place(1, point()), 0)])),
        ],
    };
    let (elaboration, result) = run_places(vec![caller, bounded()]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let reads = field_reads(&elaboration);
    let [read_y, read_x] = reads.as_slice() else {
        panic!("two reads");
    };
    // Each read equals exactly the operand its field was established with:
    // the constant 7 for `y`, the formal `a` (value 1) for `x`.
    let seven = constants(&elaboration)[0];
    assert_eq!(read_equals(tables, *read_y), Some(seven));
    assert_eq!(read_equals(tables, *read_x), Some(id(1)));
    let requirements = verdicts(&elaboration, &result, 1, requirement);
    let [(first, needs_y), (_, needs_x)] = requirements.as_slice() else {
        panic!("two requirements");
    };
    assert_kernel_checked(&elaboration, 1, *first, certificate(needs_y));
    assert_unproved(needs_x);
}

#[test]
fn a_formal_leaf_reads_as_its_one_entry_content_value() {
    // f(p: Point) -> u32 { p.x - p.x }: both reads are the one value `p.x`
    // holds at entry, so the unsigned difference cannot underflow. A
    // difference of two different leaves has no such relation.
    let difference = |left: usize, right: usize| Shaped {
        parameters: vec![point()],
        locals: Vec::new(),
        result: u32_type().into(),
        requires: Vec::new(),
        statements: vec![returns(strict(
            Strict::Subtract,
            read(place(0, point()), left),
            read(place(0, point()), right),
        ))],
    };
    let (elaboration, result) = run_places(vec![difference(0, 0)]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let reads = field_reads(&elaboration);
    let [first, second] = reads.as_slice() else {
        panic!("two reads");
    };
    let held = read_equals(tables, *first).unwrap();
    assert_eq!(read_equals(tables, *second), Some(held));
    assert!(matches!(
        tables.values.get(held).unwrap().origin,
        ValueOrigin::EntryContent { .. }
    ));
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, safety)[0];
    assert_kernel_accepted(&elaboration, 1, obligation, certificate(verdict));

    let (elaboration, result) = run_places(vec![difference(0, 1)]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let reads = field_reads(&elaboration);
    assert_ne!(read_equals(tables, reads[0]), read_equals(tables, reads[1]));
    assert_unproved(verdicts(&elaboration, &result, 1, safety)[0].1);
}

#[test]
fn a_copy_reads_as_what_its_source_held_and_an_unknown_content_reads_as_nothing() {
    // f() -> u32 {
    //     let p = Point { x: 3, y: 7 };  let q = p;
    //     bounded(q.y);              // the copy carries 7
    //     let t = make();            // a call's result: values unknown here
    //     bounded(t.y)
    // }
    let caller = Shaped {
        parameters: Vec::new(),
        locals: vec![point(), point(), point()],
        result: u32_type().into(),
        requires: Vec::new(),
        statements: vec![
            bind(0, point_of(unsigned(3), unsigned(7))),
            bind(1, place(0, point())),
            Statement::Evaluate(call(2, vec![read(place(1, point()), 1)])),
            bind(
                2,
                of_type(
                    ExpressionKind::Call {
                        callee: id(3),
                        arguments: Vec::new(),
                    },
                    point(),
                ),
            ),
            returns(call(2, vec![read(place(2, point()), 1)])),
        ],
    };
    let make = Shaped {
        parameters: Vec::new(),
        locals: Vec::new(),
        result: point(),
        requires: Vec::new(),
        statements: vec![returns(point_of(unsigned(1), unsigned(2)))],
    };
    let (elaboration, result) = run_places(vec![caller, bounded(), make]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let reads = field_reads(&elaboration);
    let [of_copy, of_result] = reads.as_slice() else {
        panic!("two reads");
    };
    let seven = constants(&elaboration)[1];
    assert_eq!(read_equals(tables, *of_copy), Some(seven));
    assert_eq!(read_equals(tables, *of_result), None);
    let requirements = verdicts(&elaboration, &result, 1, requirement);
    let [(first, through_copy), (_, through_result)] = requirements.as_slice() else {
        panic!("two requirements");
    };
    assert_kernel_checked(&elaboration, 1, *first, certificate(through_copy));
    assert_unproved(through_result);
}

#[test]
fn a_record_actual_leaves_scalar_clauses_named_by_their_own_lane() {
    // takes(p: Point, v: u32) -> u32 requires v <= 10 { v }: `v` is the
    // callee's first scalar formal though it is its second formal, and the
    // clause is decided for the caller's first scalar actual.
    let takes = Shaped {
        parameters: vec![point(), u32_type().into()],
        locals: Vec::new(),
        result: u32_type().into(),
        requires: vec![at_most(1, 10)],
        statements: vec![returns(slot(1, u32_type()))],
    };
    let caller = |argument: u128| Shaped {
        parameters: vec![point()],
        locals: Vec::new(),
        result: u32_type().into(),
        requires: Vec::new(),
        statements: vec![returns(call(
            2,
            vec![place(0, point()), unsigned(argument)],
        ))],
    };
    let takes_again = || Shaped {
        parameters: takes.parameters.clone(),
        locals: Vec::new(),
        result: takes.result,
        requires: takes.requires.clone(),
        statements: vec![returns(slot(1, u32_type()))],
    };
    let (elaboration, result) = run_places(vec![caller(7), takes_again()]);
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, requirement)[0];
    assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
    let (elaboration, result) = run_places(vec![caller(20), takes_again()]);
    assert_unproved(verdicts(&elaboration, &result, 1, requirement)[0].1);
}

/// The place reached from root `root` by `fields`, which must be interned.
fn projected(tables: &MachineTables, root: PlaceId, fields: &[u64]) -> ExactPlaceId {
    let mut place = *tables.roots.get(root).unwrap();
    for field in fields {
        let children = &tables.places.get(place).unwrap().children;
        let segment = Segment::Field(id::<StructuralFieldId>(*field));
        let found = children
            .binary_search_by_key(&segment, |(segment, _)| *segment)
            .unwrap();
        place = children[found].1;
    }
    place
}

#[test]
fn different_values_on_different_arrivals_give_no_equation() {
    // f(a: bool, b: bool) -> u32 {
    //     let p = Point { x: 1, y: 2 };
    //     let c = a && !b;       // the `!b` block is one of two ways to the join
    //     p.x
    // }
    // As lowered, the one write to `p.x` dominates the read. The subset has
    // no store yet, so the arm's write is added to the log by hand: what the
    // checker concludes must follow from the log and the graph alone.
    let program = || {
        elaborate_places(vec![Shaped {
            parameters: vec![ScalarType::Boolean.into(), ScalarType::Boolean.into()],
            locals: vec![point(), ScalarType::Boolean.into()],
            result: u32_type().into(),
            requires: Vec::new(),
            statements: vec![
                bind(2, point_of(unsigned(1), unsigned(2))),
                bind(
                    3,
                    expression(
                        ExpressionKind::ShortCircuit {
                            conjunction: true,
                            left: Box::new(slot(0, ScalarType::Boolean)),
                            right: Box::new(expression(
                                ExpressionKind::Not(Box::new(slot(1, ScalarType::Boolean))),
                                Some(ScalarType::Boolean),
                            )),
                        },
                        Some(ScalarType::Boolean),
                    ),
                ),
                returns(read(place(2, point()), 0)),
            ],
        }])
    };
    // The constants 1 and 2 the literal was built with.
    let built = constants(&program());
    let (one, two) = (built[0], built[1]);
    // What the read equals once `content` is also written to `place` after
    // the arm's one operation.
    let with_arm_write = |target: &[u64], content: Option<Content>| {
        let mut elaboration = program();
        let machine = &elaboration.module.machines[0];
        let OperationKind::EstablishRecord { .. } = machine.blocks[0].operations[2].kind else {
            panic!("the literal is the entry block's third operation");
        };
        let root = machine.blocks[0].operations[2]
            .result
            .as_structural()
            .unwrap()
            .place;
        let arm = machine
            .blocks
            .iter()
            .find(|block| {
                matches!(
                    block.operations.as_slice(),
                    [operation] if matches!(operation.kind, OperationKind::BooleanNot { .. })
                )
            })
            .unwrap()
            .operations[0]
            .id;
        let tables = elaboration.tables.machines.get_mut(id(1)).unwrap();
        if let Some(content) = content {
            let place = projected(tables, root, target);
            let write = tables.writes.push(WriteRow {
                place,
                at: ProgramPoint::AfterOperation(arm),
                content,
            });
            tables.places.get_mut(place).unwrap().writes.push(write);
        }
        discharge(&mut elaboration).unwrap();
        let read = field_reads(&elaboration)[0];
        let tables = elaboration.tables.machines.get(id(1)).unwrap();
        read_equals(tables, read)
    };
    assert_eq!(with_arm_write(&[], None), Some(one));
    // Another value on the arm: the two arrivals differ.
    assert_eq!(with_arm_write(&[1], Some(Content::Value(two))), None);
    // The same value on the arm: every arrival still supplies it.
    assert_eq!(with_arm_write(&[1], Some(Content::Value(one))), Some(one));
    // A write to the sibling `p.y` is disjoint and changes nothing.
    assert_eq!(with_arm_write(&[2], Some(Content::Value(two))), Some(one));
    // Unknown new contents of the whole record overlap its field.
    assert_eq!(with_arm_write(&[], Some(Content::Overwritten)), None);
}

#[test]
fn sibling_fields_are_disjoint_and_ancestors_overlap() {
    // far(l: Line, m: Line) -> u32 { l.to.y & l.to.x & l.from.x & m.to.y }
    let leaf = |slot: usize, outer: usize, inner: usize| {
        read(
            of_type(
                ExpressionKind::Field {
                    base: Box::new(place(slot, line())),
                    field: outer,
                },
                point(),
            ),
            inner,
        )
    };
    let and = |left, right| strict(Strict::BitwiseAnd, left, right);
    let elaboration = elaborate_places(vec![Shaped {
        parameters: vec![line(), line()],
        locals: Vec::new(),
        result: u32_type().into(),
        requires: Vec::new(),
        statements: vec![returns(and(
            and(and(leaf(0, 1, 1), leaf(0, 1, 0)), leaf(0, 0, 0)),
            leaf(1, 1, 1),
        ))],
    }]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let overlap = |left, right| {
        let of = |place| crate::places::lineage(id(1), &tables.places, place).unwrap();
        let forward = crate::places::overlap(&of(left), &of(right));
        let backward = crate::places::overlap(&of(right), &of(left));
        assert_eq!(forward, backward, "overlap is symmetric");
        forward
    };
    let l = |fields: &[u64]| projected(tables, id(1), fields);
    let m = |fields: &[u64]| projected(tables, id(2), fields);
    // A place overlaps itself, its ancestors and its descendants.
    assert!(overlap(l(&[4, 2]), l(&[4, 2])));
    assert!(overlap(l(&[]), l(&[4, 2])));
    assert!(overlap(l(&[4]), l(&[4, 2])));
    // Two fields of one record are disjoint, and so is everything beneath
    // them.
    assert!(!overlap(l(&[4, 2]), l(&[4, 1])));
    assert!(!overlap(l(&[3]), l(&[4])));
    assert!(!overlap(l(&[3, 1]), l(&[4, 1])));
    assert!(!overlap(l(&[3]), l(&[4, 2])));
    // The same path under another root is other storage.
    assert!(!overlap(l(&[4, 2]), m(&[4, 2])));
    assert!(!overlap(l(&[]), m(&[])));

    // Segments that do not prove disjoint storage overlap: two cases of one
    // sum overlay, and unequal identities alone prove nothing.
    let mut places: IdTable<ExactPlaceId, PlaceRow> = IdTable::default();
    let row = |origin| PlaceRow {
        origin,
        children: Vec::new(),
        writes: Vec::new(),
    };
    let root = places.push(row(PlaceOrigin::Root(id(1))));
    let mut child = |segment| {
        places.push(row(PlaceOrigin::Projection {
            parent: root,
            segment,
        }))
    };
    let first_case = child(Segment::Case(id(1)));
    let second_case = child(Segment::Case(id(2)));
    let field = child(Segment::Field(id(1)));
    let first_index = child(Segment::FixedIndex(0));
    let second_index = child(Segment::FixedIndex(1));
    let of = |place| crate::places::lineage(id(1), &places, place).unwrap();
    let may = |left, right| crate::places::overlap(&of(left), &of(right));
    assert!(may(first_case, second_case));
    assert!(may(first_case, field));
    assert!(may(field, first_case));
    assert!(may(first_index, second_index));
}

fn store(slot: usize, path: &[usize], value: Expression) -> Statement {
    Statement::Store {
        slot,
        path: path.to_vec(),
        value,
        source: nowhere(),
    }
}

fn declare(slot: usize, value_type: ValueType) -> Statement {
    Statement::Declare {
        slot,
        home: value_type
            .scalar()
            .map(|_| String::from("named(name(u32))")),
        source: nowhere(),
    }
}

fn and(left: Expression, right: Expression) -> Expression {
    strict(Strict::BitwiseAnd, left, right)
}

/// Machine 1's verdict on each use of storage, in block and operation order.
fn uses(result: &Discharge) -> Vec<Result<(), RefusalReason>> {
    result
        .machines
        .get(id(1))
        .unwrap()
        .uses
        .iter()
        .map(|used| used.verdict.clone().map_err(|refusal| refusal.reason))
        .collect()
}

/// Adds one row to machine 1's write log by hand, at the normal
/// continuation of `operation`.
fn log_by_hand(
    elaboration: &mut structure_elaboration::Elaboration,
    place: ExactPlaceId,
    operation: OperationId,
    content: Content,
) {
    let tables = elaboration.tables.machines.get_mut(id(1)).unwrap();
    let write = tables.writes.push(WriteRow {
        place,
        at: ProgramPoint::AfterOperation(operation),
        content,
    });
    tables.places.get_mut(place).unwrap().writes.push(write);
}

/// The one operation of the block that holds nothing but an integer
/// constant: the arm `guard -> <literal>` of the programs below.
fn arm_constant(elaboration: &structure_elaboration::Elaboration) -> (OperationId, ValueId) {
    let mut arms = elaboration.module.machines[0]
        .blocks
        .iter()
        .filter_map(|block| match block.operations.as_slice() {
            [operation] if matches!(operation.kind, OperationKind::IntegerConstant { .. }) => {
                Some((operation.id, operation.result.scalar_ref().unwrap().id))
            }
            _ => None,
        });
    let arm = arms.next().unwrap();
    assert!(arms.next().is_none(), "one arm of one constant");
    arm
}

#[test]
fn a_stored_leaf_reads_as_the_stored_value_and_its_sibling_keeps_its_content() {
    // f(a: u32) -> u32 {
    //     let mut p = Point { x: a, y: 7 };
    //     let old = p.y;
    //     p.y = 99;
    //     bounded(old);        // old is still the 7 it was read as
    //     bounded(p.y);        // p.y is now 99: not at most 10
    //     p.x                  // the sibling still holds `a`
    // }
    let caller = Shaped {
        parameters: vec![u32_type().into()],
        locals: vec![point(), u32_type().into()],
        result: u32_type().into(),
        requires: Vec::new(),
        statements: vec![
            bind(1, point_of(slot(0, u32_type()), unsigned(7))),
            bind(2, read(place(1, point()), 1)),
            store(1, &[1], unsigned(99)),
            Statement::Evaluate(call(2, vec![slot(2, u32_type())])),
            Statement::Evaluate(call(2, vec![read(place(1, point()), 1)])),
            returns(read(place(1, point()), 0)),
        ],
    };
    let (elaboration, result) = run_places(vec![caller, bounded()]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let reads = field_reads(&elaboration);
    let [old, stored, sibling] = reads.as_slice() else {
        panic!("three reads");
    };
    let built = constants(&elaboration);
    let (seven, ninety_nine) = (built[0], built[1]);
    // The read before the store equals the constructed operand, and that
    // row is still there after the store: a write never edits knowledge.
    assert_eq!(read_equals(tables, *old), Some(seven));
    // The read after the store equals the stored value, and the sibling
    // field, which the store does not overlap, still equals its operand.
    assert_eq!(read_equals(tables, *stored), Some(ninety_nine));
    assert_eq!(read_equals(tables, *sibling), Some(id(1)));
    let requirements = verdicts(&elaboration, &result, 1, requirement);
    let [(first, needs_old), (_, needs_stored)] = requirements.as_slice() else {
        panic!("two requirements");
    };
    // What was known about the value read before the store survives it.
    assert_kernel_checked(&elaboration, 1, *first, certificate(needs_old));
    assert_unproved(needs_stored);
    assert_eq!(uses(&result), [Ok(()), Ok(()), Ok(())]);
}

#[test]
fn a_store_inside_a_guarded_arm_does_not_reach_the_continuation() {
    // f(g: bool) -> u32 {
    //     let mut p = Point { x: 1, y: 2 };
    //     transition g { true -> 0 }     // the arm completes the machine
    //     let q = p;                     // a whole copy in the continuation
    //     p.x & q.x                      // a read and a read of the copy
    // }
    // The source language has no statement inside an arm, so the arm's
    // store is added to the log by hand, on the arm's one operation: what
    // the checker concludes must follow from the log and the graph alone.
    let program = || {
        elaborate_places(vec![Shaped {
            parameters: vec![ScalarType::Boolean.into()],
            locals: vec![point(), point()],
            result: u32_type().into(),
            requires: Vec::new(),
            statements: vec![
                bind(1, point_of(unsigned(1), unsigned(2))),
                guarded(slot(0, ScalarType::Boolean), unsigned(0)),
                bind(2, place(1, point())),
                returns(and(read(place(1, point()), 0), read(place(2, point()), 0))),
            ],
        }])
    };
    let read_values = |arm_store: Option<Content>| {
        let mut elaboration = program();
        let (arm, _) = arm_constant(&elaboration);
        let p = elaboration.module.machines[0].blocks[0].operations[2]
            .result
            .as_structural()
            .unwrap()
            .place;
        if let Some(content) = arm_store {
            let leaf = projected(elaboration.tables.machines.get(id(1)).unwrap(), p, &[1]);
            log_by_hand(&mut elaboration, leaf, arm, content);
        }
        let result = discharge(&mut elaboration).unwrap();
        assert!(uses(&result).iter().all(Result::is_ok));
        let tables = elaboration.tables.machines.get(id(1)).unwrap();
        let reads = field_reads(&elaboration);
        (read_equals(tables, reads[0]), read_equals(tables, reads[1]))
    };
    let one = constants(&program())[0];
    let (_, in_arm) = arm_constant(&program());
    assert_eq!(read_values(None), (Some(one), Some(one)));
    // The arm stores another value into `p.x` and then leaves the machine.
    // Neither the read nor the copy in the continuation is reached by it:
    // both still hold what the construction before the branch stored.
    assert_eq!(
        read_values(Some(Content::Value(in_arm))),
        (Some(one), Some(one))
    );
    assert_eq!(
        read_values(Some(Content::Overwritten)),
        (Some(one), Some(one))
    );
}

#[test]
fn a_store_before_a_branch_reaches_both_sides() {
    // f(g: bool) -> u32 {
    //     let mut p = Point { x: 1, y: 2 };
    //     p.x = 5;
    //     transition g { true -> p.x }   // the arm reads it
    //     let q = p;
    //     p.x & q.x                      // and so do the read and the copy
    // }
    let (elaboration, result) = run_places(vec![Shaped {
        parameters: vec![ScalarType::Boolean.into()],
        locals: vec![point(), point()],
        result: u32_type().into(),
        requires: Vec::new(),
        statements: vec![
            bind(1, point_of(unsigned(1), unsigned(2))),
            store(1, &[0], unsigned(5)),
            guarded(slot(0, ScalarType::Boolean), read(place(1, point()), 0)),
            bind(2, place(1, point())),
            returns(and(read(place(1, point()), 0), read(place(2, point()), 0))),
        ],
    }]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let five = constants(&elaboration)[2];
    let reads = field_reads(&elaboration);
    assert_eq!(reads.len(), 3);
    for read in reads {
        assert_eq!(read_equals(tables, read), Some(five));
    }
    assert!(uses(&result).iter().all(Result::is_ok));
}

#[test]
fn a_whole_assignment_replaces_every_leaf_after_constructing_its_operands() {
    // f(a: u32) -> u32 {
    //     let mut p = Point { x: 1, y: 2 };
    //     p = Point { x: p.y, y: a };     // the operand reads the old `p`
    //     let mut l = Line { from: p, to: p };
    //     l.to = Point { x: 8, y: 9 };    // replaces `l.to` whole
    //     p.x & p.y & l.to.x & l.to.y & l.from.y
    // }
    let of_line = |outer: usize, inner: usize| {
        read(
            of_type(
                ExpressionKind::Field {
                    base: Box::new(place(2, line())),
                    field: outer,
                },
                point(),
            ),
            inner,
        )
    };
    let (elaboration, result) = run_places(vec![Shaped {
        parameters: vec![u32_type().into()],
        locals: vec![point(), line()],
        result: u32_type().into(),
        requires: Vec::new(),
        statements: vec![
            bind(1, point_of(unsigned(1), unsigned(2))),
            bind(1, point_of(read(place(1, point()), 1), slot(0, u32_type()))),
            bind(
                2,
                of_type(
                    ExpressionKind::Record {
                        fields: vec![(0, place(1, point())), (1, place(1, point()))],
                    },
                    line(),
                ),
            ),
            store(2, &[1], point_of(unsigned(8), unsigned(9))),
            returns(and(
                and(
                    and(
                        and(read(place(1, point()), 0), read(place(1, point()), 1)),
                        of_line(1, 0),
                    ),
                    of_line(1, 1),
                ),
                of_line(0, 1),
            )),
        ],
    }]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let built = constants(&elaboration);
    let (two, eight, nine) = (built[1], built[2], built[3]);
    let reads = field_reads(&elaboration);
    let [operand, new_x, new_y, to_x, to_y, from_y] = reads.as_slice() else {
        panic!("six reads");
    };
    // The operand `p.y` is read from the value `p` held before the
    // assignment: construction comes first.
    assert_eq!(read_equals(tables, *operand), Some(two));
    // After it, both leaves of `p` hold the new value's operands; nothing of
    // the old value (x: 1) is left.
    assert_eq!(read_equals(tables, *new_x), Some(operand.1));
    assert_eq!(read_equals(tables, *new_y), Some(id(1)));
    // `l.to` was replaced whole by the constructed record, and `l.from`,
    // its sibling, still holds the copy of `p` it was built from.
    assert_eq!(read_equals(tables, *to_x), Some(eight));
    assert_eq!(read_equals(tables, *to_y), Some(nine));
    assert_eq!(read_equals(tables, *from_y), Some(id(1)));
    assert!(uses(&result).iter().all(Result::is_ok));
}

/// The route's one verdict rejects a program with an unestablished use: it is
/// a compile error, never an acceptance and never a hand-back to the current
/// checker. The rejection names the use and where it arises.
#[test]
fn the_verdict_rejects_a_program_with_an_unestablished_use() {
    // f() -> u32 { let mut x: u32; x }
    let elaboration = elaborate_places(vec![Shaped {
        parameters: Vec::new(),
        locals: vec![u32_type().into()],
        result: u32_type().into(),
        requires: Vec::new(),
        statements: vec![declare(0, u32_type().into()), returns(slot(0, u32_type()))],
    }]);
    let read = elaboration.module.machines[0].blocks[0]
        .operations
        .iter()
        .find(|operation| matches!(operation.kind, OperationKind::PrimitiveScalarRead { .. }))
        .unwrap()
        .id;
    let RouteVerdict::Rejected {
        unproved,
        unestablished,
    } = judge(Ok(elaboration)).unwrap()
    else {
        panic!("a program that reads unestablished storage must be rejected");
    };
    assert!(unproved.is_empty(), "{unproved:?}");
    let [used] = unestablished.as_slice() else {
        panic!("exactly the read is unestablished: {unestablished:?}");
    };
    assert_eq!(used.machine, id(1));
    assert_eq!(used.at, terminal_check_tables::Construct::Operation(read));
    assert!(used.site.is_some());
    assert!(used.message().starts_with("unestablished storage: "));
}

#[test]
fn a_read_of_a_local_nothing_was_stored_into_is_unestablished() {
    // f() -> u32 { let mut x: u32; x }
    let unwritten = |statements| Shaped {
        parameters: Vec::new(),
        locals: vec![u32_type().into()],
        result: u32_type().into(),
        requires: Vec::new(),
        statements,
    };
    let (elaboration, result) = run_places(vec![unwritten(vec![
        declare(0, u32_type().into()),
        returns(slot(0, u32_type())),
    ])]);
    assert_eq!(uses(&result), [Err(RefusalReason::Unestablished)]);
    let refusal = result
        .machines
        .get(id(1))
        .unwrap()
        .unestablished()
        .next()
        .unwrap();
    // The refusal concerns the read itself, and the zero the module stores
    // so that it runs gives the read no equation.
    let read = elaboration.module.machines[0].blocks[0]
        .operations
        .iter()
        .find(|operation| matches!(operation.kind, OperationKind::PrimitiveScalarRead { .. }))
        .unwrap();
    assert_eq!(
        refusal.concerns,
        terminal_check_tables::Construct::Operation(read.id)
    );
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    assert_eq!(
        read_equals(tables, (read.id, read.result.scalar_ref().unwrap().id)),
        None
    );
    // f() -> u32 { let mut x: u32; x = 3; x }: the store establishes it,
    // and the read equals the stored value.
    let (elaboration, result) = run_places(vec![unwritten(vec![
        declare(0, u32_type().into()),
        store(0, &[], unsigned(3)),
        returns(slot(0, u32_type())),
    ])]);
    assert_eq!(uses(&result), [Ok(())]);
    let read = elaboration.module.machines[0].blocks[0]
        .operations
        .iter()
        .find(|operation| matches!(operation.kind, OperationKind::PrimitiveScalarRead { .. }))
        .unwrap();
    let three = constants(&elaboration)[1];
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    assert_eq!(
        read_equals(tables, (read.id, read.result.scalar_ref().unwrap().id)),
        Some(three)
    );
}

#[test]
fn a_record_is_established_by_a_store_into_every_leaf_and_not_by_fewer() {
    // f() -> Point { let mut p: Point; p.x = 1; [p.y = 2;] p }
    let built = |every_leaf: bool| {
        let mut statements = vec![declare(0, point()), store(0, &[0], unsigned(1))];
        if every_leaf {
            statements.push(store(0, &[1], unsigned(2)));
        }
        statements.push(returns(place(0, point())));
        run_places(vec![Shaped {
            parameters: Vec::new(),
            locals: vec![point()],
            result: point(),
            requires: Vec::new(),
            statements,
        }])
        .1
    };
    // The whole copy that returns `p` and the return of that copy.
    assert_eq!(uses(&built(true)), [Ok(()), Ok(())]);
    assert_eq!(
        uses(&built(false)),
        [
            Err(RefusalReason::Unestablished),
            Err(RefusalReason::Unestablished)
        ]
    );
    // g() -> u32 { let mut l: Line; l.from = Point { x: 1, y: 2 }; l.to.x = 3;
    //              l.from.y & l.to.x & l.to.y }
    // A whole store into `l.from` establishes both its leaves; `l.to.x` is
    // established by its own store; `l.to.y` by nothing.
    let leaf = |outer: usize, inner: usize| {
        read(
            of_type(
                ExpressionKind::Field {
                    base: Box::new(place(0, line())),
                    field: outer,
                },
                point(),
            ),
            inner,
        )
    };
    let (_, result) = run_places(vec![Shaped {
        parameters: Vec::new(),
        locals: vec![line()],
        result: u32_type().into(),
        requires: Vec::new(),
        statements: vec![
            declare(0, line()),
            store(0, &[0], point_of(unsigned(1), unsigned(2))),
            store(0, &[1, 0], unsigned(3)),
            returns(and(and(leaf(0, 1), leaf(1, 0)), leaf(1, 1))),
        ],
    }]);
    assert_eq!(
        uses(&result),
        [
            // `l.from = Point { .. }` uses the constructed root.
            Ok(()),
            Ok(()),
            Ok(()),
            Err(RefusalReason::Unestablished)
        ]
    );
}

#[test]
fn a_store_on_one_way_to_a_join_does_not_establish_the_read_after_it() {
    // f(a: bool, b: bool) -> u32 {
    //     let mut x: u32;
    //     let c = a && !b;       // the `!b` block is one of two ways to the join
    //     x
    // }
    // Every arm of this subset completes the machine, so "assigned on one
    // arm, read after the transition" cannot be written; the short-circuit
    // join is the one place two paths meet. The store on its `!b` path is
    // added to the log by hand.
    let program = |stored_before: bool| {
        let mut statements = vec![declare(2, u32_type().into())];
        if stored_before {
            statements.push(store(2, &[], unsigned(4)));
        }
        statements.push(bind(
            3,
            expression(
                ExpressionKind::ShortCircuit {
                    conjunction: true,
                    left: Box::new(slot(0, ScalarType::Boolean)),
                    right: Box::new(expression(
                        ExpressionKind::Not(Box::new(slot(1, ScalarType::Boolean))),
                        Some(ScalarType::Boolean),
                    )),
                },
                Some(ScalarType::Boolean),
            ),
        ));
        statements.push(returns(slot(2, u32_type())));
        elaborate_places(vec![Shaped {
            parameters: vec![ScalarType::Boolean.into(), ScalarType::Boolean.into()],
            locals: vec![u32_type().into(), ScalarType::Boolean.into()],
            result: u32_type().into(),
            requires: Vec::new(),
            statements,
        }])
    };
    let judged = |stored_before: bool, on_one_way: Option<Content>| {
        let mut elaboration = program(stored_before);
        let machine = &elaboration.module.machines[0];
        let home = machine.blocks[0]
            .operations
            .iter()
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::EstablishPrimitiveLocal { .. }
                )
            })
            .unwrap()
            .result
            .as_structural()
            .unwrap()
            .place;
        let negation = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| matches!(operation.kind, OperationKind::BooleanNot { .. }))
            .unwrap();
        let negation = negation.id;
        let read = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| matches!(operation.kind, OperationKind::PrimitiveScalarRead { .. }))
            .unwrap();
        let read = (read.id, read.result.scalar_ref().unwrap().id);
        if let Some(content) = on_one_way {
            let place = *elaboration
                .tables
                .machines
                .get(id(1))
                .unwrap()
                .roots
                .get(home)
                .unwrap();
            log_by_hand(&mut elaboration, place, negation, content);
        }
        let result = discharge(&mut elaboration).unwrap();
        let tables = elaboration.tables.machines.get(id(1)).unwrap();
        (uses(&result), read_equals(tables, read))
    };
    // Some u32 value for the hand-written row to store: the zero constant
    // the home was reserved with, the first constant of either program.
    let some_value = Content::Value(constants(&program(false))[0]);
    // Nothing stored anywhere: unestablished.
    assert_eq!(judged(false, None).0, [Err(RefusalReason::Unestablished)]);
    // Stored on one of the two ways to the join only: still unestablished,
    // because the other way supplies nothing.
    assert_eq!(
        judged(false, Some(some_value)).0,
        [Err(RefusalReason::Unestablished)]
    );
    assert_eq!(
        judged(false, Some(Content::Overwritten)).0,
        [Err(RefusalReason::Unestablished)]
    );
    // Stored before the branch: established on both ways, and the read
    // equals the stored value.
    let (verdicts, equals) = judged(true, None);
    assert_eq!(verdicts, [Ok(())]);
    assert!(equals.is_some());
    // Stored before the branch and again on one way: established, but the
    // two ways supply different values, so the read has no equation.
    let (verdicts, equals) = judged(true, Some(some_value));
    assert_eq!(verdicts, [Ok(())]);
    assert_eq!(equals, None);
}

#[test]
fn every_use_of_storage_has_exactly_one_verdict() {
    // One closure with every kind of use: field reads of a formal, of a
    // literal and of an unwritten local, whole copies, a record operand, a
    // record stored into a field, record arguments and a record return.
    // Every operation and terminator that reads storage must be judged once:
    // established, which means a write or entry content reaches each leaf it
    // reads on every path, or refused as unestablished. None may be skipped.
    let taker = Shaped {
        parameters: vec![point()],
        locals: Vec::new(),
        result: point(),
        requires: Vec::new(),
        statements: vec![returns(place(0, point()))],
    };
    let caller = Shaped {
        parameters: vec![point(), ScalarType::Boolean.into()],
        locals: vec![point(), line(), point(), u32_type().into()],
        result: u32_type().into(),
        requires: Vec::new(),
        statements: vec![
            bind(2, point_of(read(place(0, point()), 0), unsigned(2))),
            declare(3, line()),
            store(3, &[0], place(2, point())),
            declare(4, point()),
            declare(5, u32_type().into()),
            guarded(
                slot(1, ScalarType::Boolean),
                read(
                    of_type(
                        ExpressionKind::Field {
                            base: Box::new(place(3, line())),
                            field: 1,
                        },
                        point(),
                    ),
                    0,
                ),
            ),
            bind(
                4,
                of_type(
                    ExpressionKind::Call {
                        callee: id(2),
                        arguments: vec![place(4, point())],
                    },
                    point(),
                ),
            ),
            returns(and(read(place(4, point()), 1), slot(5, u32_type()))),
        ],
    };
    let (elaboration, result) = run_places(vec![caller, taker]);
    let mut established = 0;
    let mut unestablished = 0;
    for (machine, lowered) in elaboration.module.machines.iter().enumerate() {
        let judged = &result.machines.get(id(machine as u64 + 1)).unwrap().uses;
        // The uses this test finds for itself, by operation kind.
        let mut expected = Vec::new();
        for block in &lowered.blocks {
            for operation in &block.operations {
                let reads = match &operation.kind {
                    OperationKind::ScalarStructuralField { .. }
                    | OperationKind::BooleanStructuralField { .. }
                    | OperationKind::PrimitiveScalarRead { .. }
                    | OperationKind::StructuralLeafCopy { .. }
                    | OperationKind::StoreStructuralField { .. } => true,
                    OperationKind::EstablishRecord { fields } => fields
                        .iter()
                        .any(|operand| operand.value.as_structural().is_some()),
                    other => !other.structural_arguments().is_empty(),
                };
                if reads {
                    expected.push(terminal_check_tables::Construct::Operation(operation.id));
                }
            }
            if let terminal_psi::Terminator::ReturnStructural { edge, .. } = &block.terminator {
                expected.push(terminal_check_tables::Construct::Edge(*edge));
            }
        }
        assert!(!expected.is_empty());
        assert_eq!(
            judged.iter().map(|used| used.at).collect::<Vec<_>>(),
            expected,
            "one verdict per use, in program order, none skipped and none twice"
        );
        for used in judged {
            match &used.verdict {
                Ok(()) => established += 1,
                Err(refusal) => {
                    assert_eq!(refusal.reason, RefusalReason::Unestablished);
                    assert_eq!(refusal.concerns, used.at);
                    unestablished += 1;
                }
            }
        }
    }
    assert!(established > 0 && unestablished > 0);
    // The obligations are judged apart from the uses and still have exactly
    // one verdict each.
    for (machine, tables) in elaboration.tables.machines.iter() {
        let verdicts = &result.machines.get(machine).unwrap().verdicts;
        assert!(tables.obligations.keys().eq(verdicts.keys()));
    }
}
