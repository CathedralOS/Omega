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

/// A store whose written value is not shown to keep a clause is not a
/// rejection: it would open an invariant window, which this route does not
/// have yet, so the closure is handed back to the current checker.
fn assert_opens_a_window(verdict: &Checked<Discharged>) {
    let refusal = verdict
        .as_ref()
        .expect_err("the store's obligation must be refused");
    assert_eq!(
        refusal.reason,
        RefusalReason::Unsupported,
        "{}",
        refusal.detail
    );
    assert!(
        refusal
            .detail
            .starts_with("a store that would open an invariant window"),
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
        cases: Vec::new(),
        requires: Vec::new(),
    });
    records.push(RecordType {
        identity: String::from("Line"),
        fields: vec![field(3, "from", point()), field(4, "to", point())],
        cases: Vec::new(),
        requires: Vec::new(),
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
    elaborate_over(machines, &records())
}

fn elaborate_over(
    machines: Vec<Shaped>,
    records: &IdTable<StructuralTypeId, RecordType>,
) -> structure_elaboration::Elaboration {
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
    structure_elaboration::lower_bodies(id(1), &bodies, &signatures, records).unwrap()
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
            // A read of typed storage may also carry its type's `requires`
            // on the read value: an order, or an equation with a literal.
            _ => None,
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

/// Elaboration refuses a dispatch with no arm for a declared case as
/// `Unestablished`. The verdict takes that as a rejection of the program, not
/// as a defect of the route and not as a hand-back, and invents no source
/// position for a closure that has no source correspondence.
#[test]
fn the_verdict_rejects_a_closure_elaboration_refused_as_unestablished() {
    let refused = terminal_check_tables::Refusal {
        reason: RefusalReason::Unestablished,
        machine: id(1),
        concerns: terminal_check_tables::Construct::Block(id(2)),
        detail: String::from("a dispatch selects no arm for a declared case"),
    };
    let RouteVerdict::Rejected {
        unproved,
        unestablished,
    } = judge(Err(refused)).unwrap()
    else {
        panic!("an unestablished closure must be rejected");
    };
    assert!(unproved.is_empty());
    let [used] = unestablished.as_slice() else {
        panic!("one refusal is one rejection: {unestablished:?}");
    };
    assert_eq!(used.at, terminal_check_tables::Construct::Block(id(2)));
    assert_eq!(used.site, None);
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
fn a_record_local_without_an_initializer_holds_the_zero_value() {
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
    // The whole copy that returns `p` and the return of that copy. A data
    // local is zero-established, so the leaf no store reached holds zero
    // and the record is established either way. (Until the owner settled
    // the data half of `local-without-initializer`, the second was
    // `Unestablished`.)
    assert_eq!(uses(&built(true)), [Ok(()), Ok(())]);
    assert_eq!(uses(&built(false)), [Ok(()), Ok(())]);
    // g() -> u32 { let mut l: Line; l.from = Point { x: 1, y: 2 }; l.to.x = 3;
    //              l.from.y & l.to.x & l.to.y }
    // A whole store into `l.from` supplies both its leaves, `l.to.x` its
    // own store, and `l.to.y` the zero the declaration established.
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
            Ok(())
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

// Sums. `Shape` is type 2 beside `Point`: `Dot(at: u32)` is case 1 with field
// 3, `Pair(l: u32, r: u32)` case 2 with fields 4 and 5, `Boxed(inner: Point)`
// case 3 with field 6, and `Empty` case 4. `Holder { shape: Shape }` is type
// 3 with field 7.

use structure_elaboration::CaseType;
use terminal_psi::Terminator;

const DOT: usize = 0;
const PAIR: usize = 1;
const BOXED: usize = 2;
const EMPTY: usize = 3;

fn shape() -> ValueType {
    ValueType::Record(id(2))
}

fn holder() -> ValueType {
    ValueType::Record(id(3))
}

fn sums() -> IdTable<StructuralTypeId, RecordType> {
    let field = |raw, spelled: &str, value_type| RecordField {
        id: id(raw),
        identity: spelled.to_owned(),
        value_type,
    };
    let case = |raw, spelled: &str, fields| CaseType {
        id: id(raw),
        identity: spelled.to_owned(),
        fields,
    };
    let scalar = ValueType::Scalar(u32_type());
    let mut records = IdTable::default();
    records.push(RecordType {
        identity: String::from("Point"),
        fields: vec![field(1, "x", scalar), field(2, "y", scalar)],
        cases: Vec::new(),
        requires: Vec::new(),
    });
    records.push(RecordType {
        identity: String::from("Shape"),
        fields: Vec::new(),
        cases: vec![
            case(1, "Dot", vec![field(3, "at", scalar)]),
            case(
                2,
                "Pair",
                vec![field(4, "l", scalar), field(5, "r", scalar)],
            ),
            case(3, "Boxed", vec![field(6, "inner", point())]),
            case(4, "Empty", Vec::new()),
        ],
        requires: Vec::new(),
    });
    records.push(RecordType {
        identity: String::from("Holder"),
        fields: vec![field(7, "shape", shape())],
        cases: Vec::new(),
        requires: Vec::new(),
    });
    records
}

fn case_of(case: usize, fields: Vec<(usize, Expression)>) -> Expression {
    of_type(ExpressionKind::Case { case, fields }, shape())
}

/// The `u32` payload field `field` of case `case` of the sum `base` names.
fn payload(base: Expression, case: usize, field: usize) -> Expression {
    of_type(
        ExpressionKind::Payload {
            base: Box::new(base),
            case,
            field,
        },
        u32_type().into(),
    )
}

fn is_case(subject: Expression, case: usize) -> Expression {
    of_type(
        ExpressionKind::IsCase {
            subject: Box::new(subject),
            case,
        },
        ScalarType::Boolean.into(),
    )
}

fn matching(
    subject: Expression,
    arms: Vec<(usize, Expression)>,
    default: Option<Expression>,
) -> Expression {
    of_type(
        ExpressionKind::Match {
            subject: Box::new(subject),
            arms,
            default: default.map(Box::new),
        },
        u32_type().into(),
    )
}

fn run_sums(machines: Vec<Shaped>) -> (structure_elaboration::Elaboration, Discharge) {
    let mut elaboration = elaborate_over(machines, &sums());
    let result = discharge(&mut elaboration).unwrap();
    (elaboration, result)
}

/// One machine over the sum types returning `u32`.
fn over_sums(
    parameters: Vec<ValueType>,
    locals: Vec<ValueType>,
    statements: Vec<Statement>,
) -> Shaped {
    Shaped {
        parameters,
        locals,
        result: u32_type().into(),
        requires: Vec::new(),
        statements,
    }
}

/// The values machine 1's knowledge equates on the edges of its dispatches:
/// each `DispatchBinding` row between two values, as (bound, held).
fn bound_payloads(tables: &MachineTables) -> Vec<(ValueId, ValueId)> {
    tables
        .knowledge
        .iter()
        .filter_map(|(_, row)| {
            let Justification::DispatchBinding { edge } = row.justification else {
                return None;
            };
            assert_eq!(row.available_from, ProgramPoint::Edge(edge));
            match tables.propositions.get(row.proposition).unwrap() {
                Proposition::Equal(
                    ScalarTerm::Value { id: bound, .. },
                    ScalarTerm::Value { id: held, .. },
                ) => Some((*bound, *held)),
                _ => None,
            }
        })
        .collect()
}

#[test]
fn a_bound_payload_is_what_its_leaf_was_constructed_with_and_nothing_from_a_sibling_case() {
    // f() -> u32 {
    //     let s = Shape::Pair { l: 30, r: 4 };
    //     match s {
    //         Shape::Pair { l, r } -> bounded(r) & bounded(l),   // r is 4, l is 30
    //         Shape::Dot { at } -> bounded(at),                  // nothing is known of `at`
    //         _ -> 0
    //     }
    // }
    let s = || place(0, shape());
    let caller = over_sums(
        Vec::new(),
        vec![shape()],
        vec![
            bind(0, case_of(PAIR, vec![(0, unsigned(30)), (1, unsigned(4))])),
            returns(matching(
                s(),
                vec![
                    (
                        PAIR,
                        and(
                            call(2, vec![payload(s(), PAIR, 1)]),
                            call(2, vec![payload(s(), PAIR, 0)]),
                        ),
                    ),
                    (DOT, call(2, vec![payload(s(), DOT, 0)])),
                ],
                Some(unsigned(0)),
            )),
        ],
    );
    let (elaboration, result) = run_sums(vec![caller, bounded()]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    assert!(
        uses(&result).iter().all(Result::is_ok),
        "{:?}",
        uses(&result)
    );
    // The dispatch is block 1's terminator. The `Pair` edge binds its two
    // parameters to the two constructing operands; the `Dot` edge binds a
    // parameter nothing is known about, because the last write to storage
    // its leaf overlays was the other case's payload.
    let blocks = &elaboration.module.machines[0].blocks;
    let Terminator::StructuralCase { cases, .. } = &blocks[0].terminator else {
        panic!("dispatch");
    };
    let parameters = |case: usize| -> Vec<ValueId> {
        let target = cases[case].target;
        blocks
            .iter()
            .find(|block| block.id == target)
            .unwrap()
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect()
    };
    let [thirty, four, ..] = constants(&elaboration)[..] else {
        panic!("two operands");
    };
    let [l, r] = parameters(PAIR)[..] else {
        panic!("two payload parameters");
    };
    assert_eq!(parameters(DOT).len(), 1);
    assert_eq!(bound_payloads(tables), [(l, thirty), (r, four)]);
    // `bounded(r)` discharges from the constructing 4 with kernel evidence;
    // `bounded(l)` is the 30 and `bounded(at)` is unknown.
    let requirements = verdicts(&elaboration, &result, 1, requirement);
    let [(first, needs_r), (_, needs_l), (_, needs_at)] = requirements.as_slice() else {
        panic!("three requirements");
    };
    assert_kernel_checked(&elaboration, 1, *first, certificate(needs_r));
    assert_unproved(needs_l);
    assert_unproved(needs_at);

    // Two payload fields of one case are disjoint.
    let leaf = |case: u64, field: u64| {
        let mut place = *tables.roots.get(id::<PlaceId>(1)).unwrap();
        for segment in [Segment::Case(id(case)), Segment::Field(id(field))] {
            let children = &tables.places.get(place).unwrap().children;
            let found = children
                .binary_search_by_key(&segment, |(segment, _)| *segment)
                .unwrap();
            place = children[found].1;
        }
        crate::places::lineage(id(1), &tables.places, place).unwrap()
    };
    assert!(!crate::places::overlap(&leaf(2, 4), &leaf(2, 5)));
}

#[test]
fn payloads_of_two_cases_under_one_sum_overlap() {
    // root.Case(1).Field(3) and root.Case(2).Field(4): the two cases overlay
    // the sum's storage, so their payload fields overlap, as each does the
    // sum itself; two fields of one case do not.
    let mut places: IdTable<ExactPlaceId, PlaceRow> = IdTable::default();
    let row = |origin| PlaceRow {
        origin,
        children: Vec::new(),
        writes: Vec::new(),
    };
    let root = places.push(row(PlaceOrigin::Root(id(1))));
    let mut child = |parent, segment| places.push(row(PlaceOrigin::Projection { parent, segment }));
    let dot = child(root, Segment::Case(id(1)));
    let pair = child(root, Segment::Case(id(2)));
    let at = child(dot, Segment::Field(id(3)));
    let l = child(pair, Segment::Field(id(4)));
    let r = child(pair, Segment::Field(id(5)));
    let of = |place| crate::places::lineage(id(1), &places, place).unwrap();
    let may = |left, right| {
        let forward = crate::places::overlap(&of(left), &of(right));
        assert_eq!(forward, crate::places::overlap(&of(right), &of(left)));
        forward
    };
    assert!(may(at, l));
    assert!(may(at, r));
    assert!(may(dot, pair));
    assert!(may(root, at));
    assert!(!may(l, r));
}

/// The verdicts of `f(parameters) -> u32 { locals; statements }` over the sum
/// types, beside `bounded` as machine 2.
fn sum_uses(
    parameters: Vec<ValueType>,
    locals: Vec<ValueType>,
    statements: Vec<Statement>,
) -> (
    structure_elaboration::Elaboration,
    Discharge,
    Vec<Result<(), RefusalReason>>,
) {
    let (elaboration, result) =
        run_sums(vec![over_sums(parameters, locals, statements), bounded()]);
    let verdicts = uses(&result);
    (elaboration, result, verdicts)
}

#[test]
fn a_payload_read_is_accepted_where_every_reaching_write_constructed_that_case() {
    let dot = || case_of(DOT, vec![(0, unsigned(3))]);
    // let s = Shape::Dot { at: 3 }; bounded(s.at)
    let (elaboration, result, verdicts) = sum_uses(
        Vec::new(),
        vec![shape()],
        vec![
            bind(0, dot()),
            returns(call(2, vec![payload(place(0, shape()), DOT, 0)])),
        ],
    );
    assert_eq!(verdicts, [Ok(())]);
    // The read is the constructing 3, so the requirement discharges.
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let [read] = field_reads(&elaboration)[..] else {
        panic!("one read");
    };
    assert_eq!(read_equals(tables, read), Some(constants(&elaboration)[0]));
    let requirements = verdicts_of(&elaboration, &result);
    assert_kernel_checked(
        &elaboration,
        1,
        requirements[0].0,
        certificate(requirements[0].1),
    );

    // let s = Shape::Dot { at: 3 }; s.l      -- built as another case
    let (_, _, verdicts) = sum_uses(
        Vec::new(),
        vec![shape()],
        vec![bind(0, dot()), returns(payload(place(0, shape()), PAIR, 0))],
    );
    assert_eq!(verdicts, [Err(RefusalReason::Unestablished)]);

    // let s = Shape::Dot { at: 3 }; let t = s; t.at   -- through a whole copy
    let (elaboration, _, verdicts) = sum_uses(
        Vec::new(),
        vec![shape(), shape()],
        vec![
            bind(0, dot()),
            bind(1, place(0, shape())),
            returns(payload(place(1, shape()), DOT, 0)),
        ],
    );
    assert_eq!(verdicts, [Ok(()), Ok(())]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let [read] = field_reads(&elaboration)[..] else {
        panic!("one read");
    };
    assert_eq!(read_equals(tables, read), Some(constants(&elaboration)[0]));

    // f(s: Shape) -> u32 { s.at }   -- which case a formal holds is not known
    let (_, _, verdicts) = sum_uses(
        vec![shape()],
        Vec::new(),
        vec![returns(payload(place(0, shape()), DOT, 0))],
    );
    assert_eq!(verdicts, [Err(RefusalReason::Unestablished)]);
}

fn verdicts_of<'a>(
    elaboration: &structure_elaboration::Elaboration,
    result: &'a Discharge,
) -> Vec<(ObligationId, &'a Checked<Discharged>)> {
    verdicts(elaboration, result, 1, requirement)
}

/// `h.shape`, the sum field of a `Holder`.
fn held(slot: usize) -> Expression {
    of_type(
        ExpressionKind::Field {
            base: Box::new(place(slot, holder())),
            field: 0,
        },
        shape(),
    )
}

#[test]
fn a_membership_test_establishes_its_case_on_its_true_edge_and_a_write_to_the_sum_retires_it() {
    // f(h: Holder) -> u32 { transition h.shape in Shape::Dot { true -> h.shape.at } 0 }
    let (_, _, verdicts) = sum_uses(
        vec![holder()],
        Vec::new(),
        vec![
            guarded(is_case(held(0), DOT), payload(held(0), DOT, 0)),
            returns(unsigned(0)),
        ],
    );
    // The test reads the sum; the read lies on the test's true edge.
    assert_eq!(verdicts, [Ok(()), Ok(())]);

    // f(h: Holder) -> u32 { transition h.shape in Shape::Dot { true -> 0 } h.shape.at }
    // The false edge refutes one case and establishes none.
    let (_, _, verdicts) = sum_uses(
        vec![holder()],
        Vec::new(),
        vec![
            guarded(is_case(held(0), DOT), unsigned(0)),
            returns(payload(held(0), DOT, 0)),
        ],
    );
    assert_eq!(verdicts, [Ok(()), Err(RefusalReason::Unestablished)]);

    // f(h: Holder) -> u32 {
    //     let t = h.shape in Shape::Dot;
    //     transition t { true -> h.shape.at }      -- nothing written since the test
    //     0
    // }
    let tested = |between: Vec<Statement>| {
        let mut statements = vec![bind(1, is_case(held(0), DOT))];
        statements.extend(between);
        statements.push(guarded(
            slot(1, ScalarType::Boolean),
            payload(held(0), DOT, 0),
        ));
        statements.push(returns(unsigned(0)));
        statements
    };
    let (_, _, verdicts) = sum_uses(
        vec![holder()],
        vec![ScalarType::Boolean.into()],
        tested(Vec::new()),
    );
    assert_eq!(verdicts, [Ok(()), Ok(())]);

    // let mut h = Holder { shape: Shape::Dot { at: 3 } };
    // let t = h.shape in Shape::Dot;
    // h.shape = Shape::Empty;                      -- a whole write to the sum
    // transition t { true -> h.shape.at }          -- the test's fact is retired
    let mut statements = vec![bind(
        0,
        of_type(
            ExpressionKind::Record {
                fields: vec![(0, case_of(DOT, vec![(0, unsigned(3))]))],
            },
            holder(),
        ),
    )];
    statements.extend(tested(vec![store(0, &[0], case_of(EMPTY, Vec::new()))]));
    let (_, _, verdicts) = sum_uses(
        Vec::new(),
        vec![holder(), ScalarType::Boolean.into()],
        statements.clone(),
    );
    assert_eq!(
        verdicts.last(),
        Some(&Err(RefusalReason::Unestablished)),
        "{verdicts:?}"
    );
    assert!(verdicts[..verdicts.len() - 1].iter().all(Result::is_ok));
    // Without the write the same read is accepted.
    statements.remove(2);
    let (_, _, verdicts) = sum_uses(
        Vec::new(),
        vec![holder(), ScalarType::Boolean.into()],
        statements,
    );
    assert!(verdicts.iter().all(Result::is_ok), "{verdicts:?}");
}

/// `f(s: Shape) -> Point { transition s { arms  _ -> default } }`.
fn boxed(arm: (usize, Expression), default: Expression) -> Shaped {
    Shaped {
        parameters: vec![shape()],
        locals: Vec::new(),
        result: point(),
        requires: Vec::new(),
        statements: vec![Statement::Dispatch {
            subject: place(0, shape()),
            arms: vec![(arm.0, Target::Value(arm.1), nowhere())],
            default: Some((Target::Value(default), nowhere())),
            source: nowhere(),
        }],
    }
}

fn inner() -> Expression {
    of_type(
        ExpressionKind::Payload {
            base: Box::new(place(0, shape())),
            case: BOXED,
            field: 0,
        },
        point(),
    )
}

#[test]
fn a_dispatch_edge_establishes_its_case_and_the_wildcard_edge_establishes_none() {
    let origin = || point_of(unsigned(0), unsigned(0));
    // transition s { Shape::Boxed { inner } -> inner   _ -> Point { x: 0, y: 0 } }
    let (_, result) = run_sums(vec![boxed((BOXED, inner()), origin())]);
    // The dispatch, the copy of the payload under its edge, and the returns.
    assert!(
        uses(&result).iter().all(Result::is_ok),
        "{:?}",
        uses(&result)
    );
    // transition s { Shape::Dot { .. } -> Point { .. }   _ -> s.inner }
    // Three cases enter the wildcard arm, so none is established there.
    let (_, result) = run_sums(vec![boxed((DOT, origin()), inner())]);
    let refused: Vec<_> = result
        .machines
        .get(id(1))
        .unwrap()
        .unestablished()
        .map(|refusal| refusal.detail.as_str())
        .collect();
    assert_eq!(
        refused,
        ["a case payload is accessed on a path that does not establish its case"]
    );
}

#[test]
fn a_dispatch_that_lists_no_edge_for_a_declared_case_is_unestablished() {
    let origin = || point_of(unsigned(0), unsigned(0));
    let mut elaboration = elaborate_over(vec![boxed((BOXED, inner()), origin())], &sums());
    let block = &mut elaboration.module.machines[0].blocks[0];
    let dispatch = Construct::Block(block.id);
    let Terminator::StructuralCase { cases, .. } = &mut block.terminator else {
        panic!("dispatch");
    };
    // Elaboration lists all four declared cases.
    assert_eq!(cases.len(), 4);
    cases.pop();
    let result = discharge(&mut elaboration).unwrap();
    let verdict = result
        .machines
        .get(id(1))
        .unwrap()
        .uses
        .iter()
        .find(|used| used.at == dispatch)
        .unwrap()
        .verdict
        .clone()
        .unwrap_err();
    assert_eq!(verdict.reason, RefusalReason::Unestablished);
    assert_eq!(
        verdict.detail,
        "a dispatch lists no edge for a declared case"
    );
}

#[test]
fn a_fact_from_one_match_arm_does_not_survive_the_join() {
    // f(s: Shape) -> u32 {
    //     let n = match s { Shape::Dot { .. } -> 3, _ -> 20 };
    //     bounded(n)                    -- n is the join's parameter: 3 or 20
    // }
    let joined = over_sums(
        vec![shape()],
        vec![u32_type().into()],
        vec![
            bind(
                1,
                matching(
                    place(0, shape()),
                    vec![(DOT, unsigned(3))],
                    Some(unsigned(20)),
                ),
            ),
            returns(call(2, vec![slot(1, u32_type())])),
        ],
    );
    let (elaboration, result) = run_sums(vec![joined, bounded()]);
    let [(_, after_join)] = verdicts_of(&elaboration, &result)[..] else {
        panic!("one requirement");
    };
    assert_unproved(after_join);
    // The same call inside the arm that produced the 3 discharges.
    let inside = over_sums(
        vec![shape()],
        Vec::new(),
        vec![returns(matching(
            place(0, shape()),
            vec![(DOT, call(2, vec![unsigned(3)]))],
            Some(unsigned(20)),
        ))],
    );
    let (elaboration, result) = run_sums(vec![inside, bounded()]);
    let [(obligation, in_arm)] = verdicts_of(&elaboration, &result)[..] else {
        panic!("one requirement");
    };
    assert_kernel_checked(&elaboration, 1, obligation, certificate(in_arm));
}

#[test]
fn a_sum_local_without_an_initializer_is_its_first_case_with_a_zeroed_payload() {
    // let z: Shape; match z { Shape::Dot { at } -> bounded(at), _ -> 0 }
    // `Dot` is the first declared case, so the zero value is `Dot` with
    // `at` zero: the dispatch is established, the payload parameter equals
    // the zero constant, and `bounded`'s requirement follows from it.
    // (Until the owner settled the data half of
    // `local-without-initializer`, the dispatch was `Unestablished`.)
    let (elaboration, result, verdicts) = sum_uses(
        Vec::new(),
        vec![shape()],
        vec![
            declare(0, shape()),
            returns(matching(
                place(0, shape()),
                vec![(DOT, call(2, vec![payload(place(0, shape()), DOT, 0)]))],
                Some(unsigned(0)),
            )),
        ],
    );
    assert_eq!(verdicts, [Ok(())]);
    let zero = constants(&elaboration)[0];
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let bound = bound_payloads(tables);
    assert_eq!(bound.len(), 1);
    assert_eq!(bound[0].1, zero);
    let required = verdicts_of(&elaboration, &result);
    assert_eq!(required.len(), 1);
    assert_kernel_checked(&elaboration, 1, required[0].0, certificate(required[0].1));
    // A payload of a case the zero value is not is not established: the
    // zero value of `Shape` is `Dot`, never `Pair`.
    let (_, _, verdicts) = sum_uses(
        Vec::new(),
        vec![shape()],
        vec![
            declare(0, shape()),
            returns(payload(place(0, shape()), PAIR, 0)),
        ],
    );
    assert_eq!(verdicts, [Err(RefusalReason::Unestablished)]);
}

// Data `requires`: each clause is a `DataDomain` obligation at every
// operation that forms or changes a value of the type.

use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralCaseSubject};

const BOUNDED: u64 = 1;
const SPAN: u64 = 2;
const WRAP: u64 = 3;
const BYTE_READ: u64 = 4;
const GATED: u64 = 5;
const PAIRED: u64 = 6;

fn data(raw: u64) -> ValueType {
    ValueType::Record(id(raw))
}

/// The `u32` field of `self` a data clause names by the field identities
/// from the value down to it.
fn own(path: Vec<CanonicalStructuralPathSegment>) -> ScalarTerm {
    ScalarTerm::IntegerField {
        root: structure_elaboration::self_place().unwrap(),
        path,
        scalar_type: integer_type(IntegerSign::Unsigned),
    }
}

fn field_of_self(raw: u64) -> ScalarTerm {
    own(vec![CanonicalStructuralPathSegment::Field(id(raw))])
}

fn number(value: u128) -> ScalarTerm {
    bound(u32_type(), IntegerValue::Unsigned(value))
}

fn self_is(case: u64) -> Proposition {
    Proposition::StructuralCaseMembership {
        subject: StructuralCaseSubject::new(
            structure_elaboration::self_place().unwrap(),
            Vec::new(),
        ),
        case: id(case),
    }
}

/// The data types of the `requires` tests:
///
/// - `Bounded { value } requires value <= 5`
/// - `Span { low, high } requires low <= high`
/// - `Wrap { span: Span, top } requires self.span.high <= top`
/// - `ByteRead { case Eof; case Byte(value) } requires (self in Eof)
///   || ((self in Byte) && self.value <= 255)`
/// - `Gated { v } requires 1 <= v`, which zero does not satisfy
/// - `Paired { neighbor, value } requires value <= 255, 1 <= value`
fn constrained() -> IdTable<StructuralTypeId, RecordType> {
    let scalar = ValueType::Scalar(u32_type());
    let field = |raw, spelled: &str, value_type| RecordField {
        id: id(raw),
        identity: spelled.to_owned(),
        value_type,
    };
    let record = |spelled: &str, fields, requires| RecordType {
        identity: spelled.to_owned(),
        fields,
        cases: Vec::new(),
        requires,
    };
    let mut records = IdTable::default();
    records.push(record(
        "Bounded",
        vec![field(1, "value", scalar)],
        vec![Proposition::LessOrEqual(field_of_self(1), number(5))],
    ));
    records.push(record(
        "Span",
        vec![field(2, "low", scalar), field(3, "high", scalar)],
        vec![Proposition::LessOrEqual(field_of_self(2), field_of_self(3))],
    ));
    records.push(record(
        "Wrap",
        vec![field(4, "span", data(SPAN)), field(5, "top", scalar)],
        vec![Proposition::LessOrEqual(
            own(vec![
                CanonicalStructuralPathSegment::Field(id(4)),
                CanonicalStructuralPathSegment::Field(id(3)),
            ]),
            field_of_self(5),
        )],
    ));
    records.push(RecordType {
        identity: String::from("ByteRead"),
        fields: Vec::new(),
        cases: vec![
            CaseType {
                id: id(1),
                identity: String::from("Eof"),
                fields: Vec::new(),
            },
            CaseType {
                id: id(2),
                identity: String::from("Byte"),
                fields: vec![field(6, "value", scalar)],
            },
        ],
        requires: vec![Proposition::Disjunction(vec![
            self_is(1),
            Proposition::Conjunction(vec![
                self_is(2),
                Proposition::LessOrEqual(
                    own(vec![
                        CanonicalStructuralPathSegment::Case(id(2)),
                        CanonicalStructuralPathSegment::Field(id(6)),
                    ]),
                    number(255),
                ),
            ]),
        ])],
    });
    records.push(record(
        "Gated",
        vec![field(7, "v", scalar)],
        vec![Proposition::LessOrEqual(number(1), field_of_self(7))],
    ));
    records.push(record(
        "Paired",
        vec![field(8, "neighbor", scalar), field(9, "value", scalar)],
        vec![
            Proposition::LessOrEqual(field_of_self(9), number(255)),
            Proposition::LessOrEqual(number(1), field_of_self(9)),
        ],
    ));
    records
}

fn run_constrained(machines: Vec<Shaped>) -> (structure_elaboration::Elaboration, Discharge) {
    let mut elaboration = elaborate_over(machines, &constrained());
    let result = discharge(&mut elaboration).unwrap();
    (elaboration, result)
}

fn domain(kind: &ObligationKind) -> bool {
    matches!(kind, ObligationKind::DataDomain { .. })
}

/// Machine 1's `DataDomain` verdicts, in obligation order.
fn domains<'a>(
    elaboration: &structure_elaboration::Elaboration,
    result: &'a Discharge,
) -> Vec<(ObligationId, &'a Checked<Discharged>)> {
    verdicts(elaboration, result, 1, domain)
}

fn literal_of(record: u64, fields: Vec<Expression>) -> Expression {
    of_type(
        ExpressionKind::Record {
            fields: fields.into_iter().enumerate().collect(),
        },
        data(record),
    )
}

/// One machine with no formal: its locals, its result and its body.
fn closed(locals: Vec<ValueType>, result: ValueType, statements: Vec<Statement>) -> Shaped {
    Shaped {
        parameters: Vec::new(),
        locals,
        result,
        requires: Vec::new(),
        statements,
    }
}

fn at_most_value(value: ValueId, limit: u128) -> Proposition {
    Proposition::LessOrEqual(ScalarTerm::value(value, u32_type()), number(limit))
}

#[test]
fn a_construction_binds_the_value_that_reaches_it_and_not_the_initializer() {
    // The stale-local witness (`fail/data/constructor_stale_local_bounds`):
    //   f() -> Bounded { let mut current = 1; current = 7;
    //                    Bounded { value: current } }
    // The clause `value <= 5` is owed over the operand the literal reads,
    // which is the reassigned `7`, never the initializer `1`.
    let built = |first: u128, second: u128| {
        run_constrained(vec![closed(
            vec![u32_type().into()],
            data(BOUNDED),
            vec![
                bind(0, unsigned(first)),
                bind(0, unsigned(second)),
                returns(literal_of(BOUNDED, vec![slot(0, u32_type())])),
            ],
        )])
    };
    let (elaboration, result) = built(1, 7);
    let owed = domains(&elaboration, &result);
    assert_eq!(owed.len(), 1, "one obligation per clause");
    assert_unproved(owed[0].1);
    // The twin with the two constants exchanged holds, and its goal names
    // the second constant.
    let (elaboration, result) = built(7, 1);
    let owed = domains(&elaboration, &result);
    assert_eq!(owed.len(), 1);
    let evidence = certificate(owed[0].1);
    assert_eq!(evidence.goal, at_most_value(constants(&elaboration)[1], 5));
    assert_kernel_checked(&elaboration, 1, owed[0].0, evidence);
}

#[test]
fn a_literal_on_a_sibling_field_is_no_evidence_for_the_bounded_field() {
    // make(v) -> Paired { Paired { neighbor: 17, value: v } }, with
    // `requires value <= 255, 1 <= value` on the type
    // (`fail/cases/constructor_subject_range_neighbor_not_evidence`).
    let make = |requires| {
        run_constrained(vec![Shaped {
            parameters: vec![u32_type().into()],
            locals: Vec::new(),
            result: data(PAIRED),
            requires,
            statements: vec![returns(literal_of(
                PAIRED,
                vec![unsigned(17), slot(0, u32_type())],
            ))],
        }])
    };
    let (elaboration, result) = make(Vec::new());
    let owed = domains(&elaboration, &result);
    assert_eq!(owed.len(), 2, "one obligation per clause");
    assert_unproved(owed[0].1);
    assert_unproved(owed[1].1);
    // With the formal bounded at entry both clauses hold, over the formal.
    let (elaboration, result) = make(vec![at_least(1, 1), at_most(1, 255)]);
    let owed = domains(&elaboration, &result);
    assert_eq!(
        certificate(owed[0].1).goal,
        at_most_value(id(1), 255),
        "the goal names the operand of the bounded field"
    );
    for (obligation, verdict) in owed {
        assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
    }
}

/// `f() -> u32 { let mut s = Span { low: 1, high: 9 }; s.low = stored; s.high }`.
fn span_store(stored: u128) -> (structure_elaboration::Elaboration, Discharge) {
    run_constrained(vec![closed(
        vec![data(SPAN)],
        u32_type().into(),
        vec![
            bind(0, literal_of(SPAN, vec![unsigned(1), unsigned(9)])),
            store(0, &[0], unsigned(stored)),
            returns(read(place(0, data(SPAN)), 1)),
        ],
    )])
}

#[test]
fn a_field_store_owes_the_clause_over_the_stored_value_and_the_other_fields_logged_value() {
    // `low <= high`: the store to `low` owes it over the stored value and
    // the value the write log holds for `high`, the construction's `9`.
    let (elaboration, result) = span_store(5);
    let owed = domains(&elaboration, &result);
    assert_eq!(owed.len(), 2, "the construction and the store");
    let (one, nine, five) = {
        let constants = constants(&elaboration);
        (constants[0], constants[1], constants[2])
    };
    let value = |value| ScalarTerm::value(value, u32_type());
    assert_eq!(
        certificate(owed[0].1).goal,
        Proposition::LessOrEqual(value(one), value(nine))
    );
    // The goal names the stored value, never the value `low` held before.
    assert_eq!(
        certificate(owed[1].1).goal,
        Proposition::LessOrEqual(value(five), value(nine))
    );
    for (obligation, verdict) in &owed {
        assert_kernel_checked(&elaboration, 1, *obligation, certificate(verdict));
    }
    // The store's obligation is created before the store: the completed
    // write's own row is not among its premises.
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let ObligationKind::DataDomain { operation, .. } =
        tables.obligations.get(owed[1].0).unwrap().kind
    else {
        panic!("a data domain obligation");
    };
    assert_ne!(
        tables.obligations.get(owed[1].0).unwrap().created_at,
        ProgramPoint::AfterOperation(operation)
    );
    // The twin that stores 12 does not keep the clause (12 <= 9): it would open a window.
    let (elaboration, result) = span_store(12);
    let owed = domains(&elaboration, &result);
    assert!(owed[0].1.is_ok());
    assert_opens_a_window(owed[1].1);
}

#[test]
fn a_clause_over_a_field_with_no_value_identity_is_unproved_and_says_so() {
    // f() -> u32 { let mut s = make(); s.low = 0; s.high }
    // `s.high` is what the call returned: contents with no value identity.
    // `0 <= high` is true of every `u32`, and still the clause cannot be
    // stated over values, so the store is not shown to keep it; never dropped.
    let (elaboration, result) = run_constrained(vec![
        closed(
            vec![data(SPAN)],
            u32_type().into(),
            vec![
                bind(
                    0,
                    of_type(
                        ExpressionKind::Call {
                            callee: id(2),
                            arguments: Vec::new(),
                        },
                        data(SPAN),
                    ),
                ),
                store(0, &[0], unsigned(0)),
                returns(read(place(0, data(SPAN)), 1)),
            ],
        ),
        closed(
            Vec::new(),
            data(SPAN),
            vec![returns(literal_of(SPAN, vec![unsigned(1), unsigned(9)]))],
        ),
    ]);
    let owed = domains(&elaboration, &result);
    assert_eq!(owed.len(), 1);
    let refusal = owed[0].1.as_ref().unwrap_err();
    assert!(
        refusal.reason == RefusalReason::Unsupported,
        "{:?}",
        refusal.reason
    );
    assert!(
        refusal.detail.contains("no value identity"),
        "{}",
        refusal.detail
    );
}

/// `f() -> u32 { let mut w = Wrap { span: Span { low: 1, high: 9 }, top: 9 };
///               w.span.high = stored; w.top }`.
fn nested_store(stored: u128) -> (structure_elaboration::Elaboration, Discharge) {
    run_constrained(vec![closed(
        vec![data(WRAP)],
        u32_type().into(),
        vec![
            bind(
                0,
                literal_of(
                    WRAP,
                    vec![
                        literal_of(SPAN, vec![unsigned(1), unsigned(9)]),
                        unsigned(9),
                    ],
                ),
            ),
            store(0, &[0, 1], unsigned(stored)),
            returns(read(place(0, data(WRAP)), 1)),
        ],
    )])
}

#[test]
fn a_nested_store_owes_the_clauses_of_every_type_that_contains_the_field() {
    // `w.span.high = 8` changes a `Span` and the `Wrap` around it: it owes
    // `Span`'s `low <= high` for `w.span`, then `Wrap`'s `span.high <= top`
    // for `w`, innermost first.
    let (elaboration, result) = nested_store(8);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
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
    let at_store: Vec<_> = domains(&elaboration, &result)
        .into_iter()
        .filter(|(obligation, _)| {
            matches!(
                tables.obligations.get(*obligation).unwrap().kind,
                ObligationKind::DataDomain { operation, .. } if operation == store
            )
        })
        .collect();
    assert_eq!(at_store.len(), 2);
    let constants = constants(&elaboration);
    let (one, top, eight) = (constants[0], constants[2], constants[3]);
    let value = |value| ScalarTerm::value(value, u32_type());
    assert_eq!(
        certificate(at_store[0].1).goal,
        Proposition::LessOrEqual(value(one), value(eight)),
        "the inner type's clause, over the stored value and the logged sibling"
    );
    assert_eq!(
        certificate(at_store[1].1).goal,
        Proposition::LessOrEqual(value(eight), value(top)),
        "the enclosing type's clause"
    );
    for (obligation, verdict) in &at_store {
        assert_kernel_checked(&elaboration, 1, *obligation, certificate(verdict));
    }
    // Storing 12 keeps `Span` (1 <= 12) and breaks `Wrap` (12 <= 9).
    let (elaboration, result) = nested_store(12);
    let owed = domains(&elaboration, &result);
    let verdicts: Vec<bool> = owed.iter().map(|(_, verdict)| verdict.is_ok()).collect();
    // The inner literal, the outer literal, then the store's two.
    assert_eq!(verdicts, [true, true, true, false]);
    assert_opens_a_window(owed[3].1);
}

#[test]
fn a_clause_that_tests_a_case_is_decided_by_the_constructed_case() {
    // `(self in Eof) || ((self in Byte) && self.value <= 255)`
    // (`fail/cases/constructor_subject_range_unproved`).
    let byte = |operand: Expression| {
        of_type(
            ExpressionKind::Case {
                case: 1,
                fields: vec![(0, operand)],
            },
            data(BYTE_READ),
        )
    };
    let make = |parameters: Vec<ValueType>, literal: Expression| {
        run_constrained(vec![Shaped {
            parameters,
            locals: Vec::new(),
            result: data(BYTE_READ),
            requires: Vec::new(),
            statements: vec![returns(literal)],
        }])
    };
    // `ByteRead::Eof`: the first disjunct is true by structure, so the
    // clause is, and the payload it names under the other case is never
    // projected.
    let (elaboration, result) = make(
        Vec::new(),
        of_type(
            ExpressionKind::Case {
                case: 0,
                fields: Vec::new(),
            },
            data(BYTE_READ),
        ),
    );
    let owed = domains(&elaboration, &result);
    assert_eq!(owed.len(), 1);
    assert_eq!(certificate(owed[0].1).goal, Proposition::Truth);
    // `ByteRead::Byte { value: 7 }`: the false disjunct drops, the true
    // conjunct drops, and what is left is over the operand value alone.
    let (elaboration, result) = make(Vec::new(), byte(unsigned(7)));
    let owed = domains(&elaboration, &result);
    let evidence = certificate(owed[0].1);
    assert_eq!(
        evidence.goal,
        at_most_value(constants(&elaboration)[0], 255)
    );
    assert_kernel_checked(&elaboration, 1, owed[0].0, evidence);
    // `ByteRead::Byte { value: v }` with `v` an unconstrained formal.
    let (elaboration, result) = make(vec![u32_type().into()], byte(slot(0, u32_type())));
    assert_unproved(domains(&elaboration, &result)[0].1);
}

/// `zero(v: u32) -> u32 requires v == 0 { v }`.
fn zero_only() -> Shaped {
    Shaped {
        parameters: vec![u32_type().into()],
        locals: Vec::new(),
        result: u32_type().into(),
        requires: vec![Proposition::Equal(formal(1, u32_type()), number(0))],
        statements: vec![returns(slot(0, u32_type()))],
    }
}

#[test]
fn a_data_local_without_an_initializer_is_established_as_zero_when_zero_satisfies_its_requires() {
    // f() -> u32 { let b: Bounded; zero(b.value) }
    // `Bounded requires value <= 5` holds of zero: the declaration owes the
    // clause over the zero constant, the read equals that constant, and
    // `zero`'s requirement follows.
    let (elaboration, result) = run_constrained(vec![
        closed(
            vec![data(BOUNDED)],
            u32_type().into(),
            vec![
                declare(0, data(BOUNDED)),
                returns(call(2, vec![read(place(0, data(BOUNDED)), 0)])),
            ],
        ),
        zero_only(),
    ]);
    let zero = constants(&elaboration)[0];
    let owed = domains(&elaboration, &result);
    assert_eq!(owed.len(), 1);
    assert_eq!(certificate(owed[0].1).goal, at_most_value(zero, 5));
    assert_kernel_checked(&elaboration, 1, owed[0].0, certificate(owed[0].1));
    assert_eq!(uses(&result), [Ok(())]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    assert_eq!(
        read_equals(tables, field_reads(&elaboration)[0]),
        Some(zero)
    );
    let required = verdicts_of(&elaboration, &result);
    assert_kernel_checked(&elaboration, 1, required[0].0, certificate(required[0].1));
}

#[test]
fn zeroed_storage_of_a_gated_type_is_not_an_established_value() {
    // f() -> u32 { let g: Gated; g.v }, with `Gated requires 1 <= v`.
    // Zero does not satisfy the clause, so the type is gated: declaring the
    // local is no error, and observing its zeroed storage is.
    let (elaboration, result) = run_constrained(vec![closed(
        vec![data(GATED)],
        u32_type().into(),
        vec![
            declare(0, data(GATED)),
            returns(read(place(0, data(GATED)), 0)),
        ],
    )]);
    let owed = domains(&elaboration, &result);
    assert_eq!(owed.len(), 1);
    let Ok(Discharged::Gated(why)) = owed[0].1 else {
        panic!("the declaration's clause is gated, found {:?}", owed[0].1);
    };
    assert!(
        matches!(why.reason, RefusalReason::Unproved(_)),
        "{:?}",
        why.reason
    );
    assert_eq!(uses(&result), [Err(RefusalReason::Unestablished)]);
    // f() -> u32 { let mut g: Gated; g.v = 3; g.v }: the store establishes
    // the value, over the stored `3`, before anything observes it.
    let (elaboration, result) = run_constrained(vec![closed(
        vec![data(GATED)],
        u32_type().into(),
        vec![
            declare(0, data(GATED)),
            store(0, &[0], unsigned(3)),
            returns(read(place(0, data(GATED)), 0)),
        ],
    )]);
    let owed = domains(&elaboration, &result);
    assert_eq!(owed.len(), 2);
    assert!(matches!(owed[0].1, Ok(Discharged::Gated(_))));
    assert_kernel_checked(&elaboration, 1, owed[1].0, certificate(owed[1].1));
    assert_eq!(uses(&result), [Ok(())]);
    // f() -> u32 { let mut g: Gated; g = Gated { v: 3 }; g.v }: the zeroed
    // storage is never observed, so nothing about the program is refused.
    let (elaboration, result) = run_constrained(vec![closed(
        vec![data(GATED)],
        u32_type().into(),
        vec![
            declare(0, data(GATED)),
            bind(0, literal_of(GATED, vec![unsigned(3)])),
            returns(read(place(0, data(GATED)), 0)),
        ],
    )]);
    assert!(
        domains(&elaboration, &result)
            .iter()
            .all(|(_, verdict)| verdict.is_ok())
    );
    assert_eq!(uses(&result), [Ok(())]);
}

#[test]
fn every_data_domain_obligation_has_exactly_one_verdict() {
    // A construction that holds, a store that breaks its clause, a gated
    // declaration and a case construction in one machine: each
    // `DataDomain` row has one verdict, of every outcome.
    let (elaboration, result) = run_constrained(vec![closed(
        vec![data(SPAN), data(GATED)],
        data(BYTE_READ),
        vec![
            bind(0, literal_of(SPAN, vec![unsigned(1), unsigned(9)])),
            store(0, &[0], unsigned(12)),
            declare(1, data(GATED)),
            returns(of_type(
                ExpressionKind::Case {
                    case: 0,
                    fields: Vec::new(),
                },
                data(BYTE_READ),
            )),
        ],
    )]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let verdicts = &result.machines.get(id(1)).unwrap().verdicts;
    assert_eq!(verdicts.len(), tables.obligations.len());
    assert!(tables.obligations.keys().eq(verdicts.keys()));
    let owed = domains(&elaboration, &result);
    assert_eq!(owed.len(), 4);
    assert!(matches!(owed[0].1, Ok(Discharged::Certificate(_))));
    assert_opens_a_window(owed[1].1);
    assert!(matches!(owed[2].1, Ok(Discharged::Gated(_))));
    assert!(matches!(owed[3].1, Ok(Discharged::Certificate(_))));
}

/// What machine 1's knowledge says of a read's value beside its equation:
/// the propositions of the read's own `OperationSemantics` rows that are
/// not an equation between two values.
fn read_premises(tables: &MachineTables, read: (OperationId, ValueId)) -> Vec<Proposition> {
    tables
        .knowledge
        .iter()
        .filter(|(_, row)| row.justification == Justification::OperationSemantics(read.0))
        .map(|(_, row)| tables.propositions.get(row.proposition).unwrap().clone())
        .filter(|proposition| {
            !matches!(
                proposition,
                Proposition::Equal(ScalarTerm::Value { .. }, ScalarTerm::Value { .. })
            )
        })
        .collect()
}

#[test]
fn a_read_of_an_established_value_carries_the_clause_of_the_one_field_it_reads() {
    // f(b: Bounded) -> u32 { bounded(b.value) }
    // `b` arrived established, so `value <= 5` holds of the read's fresh
    // value, and `bounded`'s `v <= 10` follows from it.
    let (elaboration, result) = run_constrained(vec![
        Shaped {
            parameters: vec![data(BOUNDED)],
            locals: Vec::new(),
            result: u32_type().into(),
            requires: Vec::new(),
            statements: vec![returns(call(2, vec![read(place(0, data(BOUNDED)), 0)]))],
        },
        bounded(),
    ]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let observed = field_reads(&elaboration)[0];
    assert_eq!(
        read_premises(tables, observed),
        [at_most_value(observed.1, 5)]
    );
    let required = verdicts_of(&elaboration, &result);
    assert_kernel_checked(&elaboration, 1, required[0].0, certificate(required[0].1));

    // g(s: Span, p: Paired) -> u32 { s.low & p.neighbor & p.value }
    // `low <= high` relates two fields and is no premise on one read; the
    // neighbor of the bounded field gets nothing; the bounded field gets
    // both of its clauses.
    let (elaboration, _) = run_constrained(vec![Shaped {
        parameters: vec![data(SPAN), data(PAIRED)],
        locals: Vec::new(),
        result: u32_type().into(),
        requires: Vec::new(),
        statements: vec![returns(and(
            and(
                read(place(0, data(SPAN)), 0),
                read(place(1, data(PAIRED)), 0),
            ),
            read(place(1, data(PAIRED)), 1),
        ))],
    }]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let reads = field_reads(&elaboration);
    assert_eq!(read_premises(tables, reads[0]), []);
    assert_eq!(read_premises(tables, reads[1]), []);
    assert_eq!(
        read_premises(tables, reads[2]),
        [
            at_most_value(reads[2].1, 255),
            Proposition::LessOrEqual(number(1), ScalarTerm::value(reads[2].1, u32_type())),
        ]
    );
}

#[test]
fn a_place_this_machine_wrote_lends_no_clause_to_a_read() {
    // f(b: Bounded) -> u32 { let mut c = b; c.value = 99; bounded(c.value) }
    // The store does not keep `value <= 5` and would open a window. The read after it
    // equals the stored `99` and carries no clause: a broken place supplies
    // no premise, and `bounded`'s requirement is unproved too.
    let (elaboration, result) = run_constrained(vec![
        Shaped {
            parameters: vec![data(BOUNDED)],
            locals: vec![data(BOUNDED)],
            result: u32_type().into(),
            requires: Vec::new(),
            statements: vec![
                bind(1, place(0, data(BOUNDED))),
                store(1, &[0], unsigned(99)),
                returns(call(2, vec![read(place(1, data(BOUNDED)), 0)])),
            ],
        },
        bounded(),
    ]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let observed = field_reads(&elaboration)[0];
    assert_eq!(read_premises(tables, observed), []);
    assert_eq!(
        read_equals(tables, observed),
        Some(constants(&elaboration)[0])
    );
    assert_opens_a_window(domains(&elaboration, &result)[0].1);
    assert_unproved(verdicts_of(&elaboration, &result)[0].1);
    // Without the store the copy still holds what arrived, and the read of
    // the copy carries the clause.
    let (elaboration, result) = run_constrained(vec![
        Shaped {
            parameters: vec![data(BOUNDED)],
            locals: vec![data(BOUNDED)],
            result: u32_type().into(),
            requires: Vec::new(),
            statements: vec![
                bind(1, place(0, data(BOUNDED))),
                returns(call(2, vec![read(place(1, data(BOUNDED)), 0)])),
            ],
        },
        bounded(),
    ]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let observed = field_reads(&elaboration)[0];
    assert_eq!(
        read_premises(tables, observed),
        [at_most_value(observed.1, 5)]
    );
    let required = verdicts_of(&elaboration, &result);
    assert_kernel_checked(&elaboration, 1, required[0].0, certificate(required[0].1));
}

#[test]
fn a_type_built_around_an_arrived_value_lends_no_clause_to_a_read_inside_it() {
    // f(s: Span) -> u32 { let w = Wrap { span: s, top: 9 }; w.span.high }
    // `Wrap requires span.high <= top` is owed by the literal and unproved:
    // nothing bounds what `s.high` held at entry. The read of `w.span.high`
    // reaches that same entry content, and `Wrap` is no part of the value
    // that arrived, so its clause is not lent to the read.
    let (elaboration, result) = run_constrained(vec![Shaped {
        parameters: vec![data(SPAN)],
        locals: vec![data(WRAP)],
        result: u32_type().into(),
        requires: Vec::new(),
        statements: vec![
            bind(1, literal_of(WRAP, vec![place(0, data(SPAN)), unsigned(9)])),
            returns(read(
                of_type(
                    ExpressionKind::Field {
                        base: Box::new(place(1, data(WRAP))),
                        field: 0,
                    },
                    data(SPAN),
                ),
                1,
            )),
        ],
    }]);
    assert_unproved(domains(&elaboration, &result)[0].1);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let observed = field_reads(&elaboration)[0];
    assert!(read_equals(tables, observed).is_some());
    assert_eq!(read_premises(tables, observed), []);
}

#[test]
fn a_read_of_what_a_call_returned_carries_its_types_clause() {
    // f() -> u32 { let b = make(); bounded(b.value) }
    // make() -> Bounded { Bounded { value: 3 } }
    // The caller cannot name what `make` returned, and it need not: the
    // value arrived established, so the read's value is at most 5.
    let (elaboration, result) = run_constrained(vec![
        closed(
            vec![data(BOUNDED)],
            u32_type().into(),
            vec![
                bind(
                    0,
                    of_type(
                        ExpressionKind::Call {
                            callee: id(3),
                            arguments: Vec::new(),
                        },
                        data(BOUNDED),
                    ),
                ),
                returns(call(2, vec![read(place(0, data(BOUNDED)), 0)])),
            ],
        ),
        bounded(),
        closed(
            Vec::new(),
            data(BOUNDED),
            vec![returns(literal_of(BOUNDED, vec![unsigned(3)]))],
        ),
    ]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let observed = field_reads(&elaboration)[0];
    assert_eq!(read_equals(tables, observed), None);
    assert_eq!(
        read_premises(tables, observed),
        [at_most_value(observed.1, 5)]
    );
    let required = verdicts_of(&elaboration, &result);
    assert_kernel_checked(&elaboration, 1, required[0].0, certificate(required[0].1));
}

// Entry clauses over a field of a record formal.

/// The `u32` field `field` of the callee's first record formal, as an entry
/// clause names it: a field term rooted at that formal's place.
fn formal_field(field: u64) -> ScalarTerm {
    ScalarTerm::IntegerField {
        root: id(1),
        path: vec![CanonicalStructuralPathSegment::Field(id(field))],
        scalar_type: integer_type(IntegerSign::Unsigned),
    }
}

/// `peek(p: Point) -> u32 requires p.x == 7 { p.y }`.
fn peek() -> Shaped {
    Shaped {
        parameters: vec![point()],
        locals: Vec::new(),
        result: u32_type().into(),
        requires: vec![Proposition::Equal(formal_field(1), number(7))],
        statements: vec![returns(read(place(0, point()), 1))],
    }
}

fn peeking(argument: Expression) -> Expression {
    of_type(
        ExpressionKind::Call {
            callee: id(2),
            arguments: vec![argument],
        },
        u32_type().into(),
    )
}

#[test]
fn a_requirement_over_a_field_of_a_record_formal_is_decided_by_the_actuals_leaf() {
    // f() -> u32 { peek(Point { x: first, y: 9 }) }
    // (`fail/constants/owned_record_argument_requires`, without the
    // constant.) The clause `p.x == 7` stands, at the call, for what the
    // actual's `x` leaf holds there: the literal's operand.
    let calling = |first: u128| {
        run_places(vec![
            closed(
                Vec::new(),
                u32_type().into(),
                vec![returns(peeking(point_of(unsigned(first), unsigned(9))))],
            ),
            peek(),
        ])
    };
    let (elaboration, result) = calling(7);
    let required = verdicts_of(&elaboration, &result);
    assert_eq!(required.len(), 1);
    let evidence = certificate(required[0].1);
    assert_eq!(
        evidence.goal,
        Proposition::Equal(
            ScalarTerm::value(constants(&elaboration)[0], u32_type()),
            number(7)
        ),
        "the goal names the value the actual's leaf holds, and no place"
    );
    assert_kernel_checked(&elaboration, 1, required[0].0, evidence);
    // With `first = 8` the same clause is `8 == 7`.
    let (elaboration, result) = calling(8);
    assert_unproved(verdicts_of(&elaboration, &result)[0].1);
    // g() -> u32 { peek(make()) }, make() -> Point { Point { x: 7, y: 9 } }:
    // what a call returned has no value identity in the caller, so the
    // requirement is unproved and says why.
    let (elaboration, result) = run_places(vec![
        closed(
            Vec::new(),
            u32_type().into(),
            vec![returns(peeking(of_type(
                ExpressionKind::Call {
                    callee: id(3),
                    arguments: Vec::new(),
                },
                point(),
            )))],
        ),
        peek(),
        closed(
            Vec::new(),
            point(),
            vec![returns(point_of(unsigned(7), unsigned(9)))],
        ),
    ]);
    let refusal = verdicts_of(&elaboration, &result)[0]
        .1
        .as_ref()
        .unwrap_err();
    assert!(
        matches!(refusal.reason, RefusalReason::Unproved(_)),
        "{:?}",
        refusal.reason
    );
    assert!(
        refusal.detail.contains("no value identity"),
        "{}",
        refusal.detail
    );
}

#[test]
fn an_entry_clause_over_a_formals_field_is_known_of_its_entry_content() {
    // f(p: Point) -> u32 requires p.x <= 7 { bounded(p.x) }
    // The clause is knowledge about the value `p.x` held at entry; the read
    // equals that value, so `bounded`'s `v <= 10` follows.
    let reading = |requires| {
        run_places(vec![
            Shaped {
                parameters: vec![point()],
                locals: Vec::new(),
                result: u32_type().into(),
                requires,
                statements: vec![returns(call(2, vec![read(place(0, point()), 0)]))],
            },
            bounded(),
        ])
    };
    let (elaboration, result) = reading(vec![Proposition::LessOrEqual(formal_field(1), number(7))]);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let observed = field_reads(&elaboration)[0];
    let entry = read_equals(tables, observed).unwrap();
    assert!(matches!(
        tables.values.get(entry).unwrap().origin,
        ValueOrigin::EntryContent { .. }
    ));
    let stated: Vec<_> = tables
        .knowledge
        .iter()
        .filter(|(_, row)| row.justification == Justification::EntryRequirement { clause: 0 })
        .map(|(_, row)| tables.propositions.get(row.proposition).unwrap().clone())
        .collect();
    assert_eq!(stated, [at_most_value(entry, 7)]);
    let required = verdicts_of(&elaboration, &result);
    assert_kernel_checked(&elaboration, 1, required[0].0, certificate(required[0].1));
    // Without the clause nothing bounds the field.
    let (elaboration, result) = reading(Vec::new());
    assert_unproved(verdicts_of(&elaboration, &result)[0].1);
}

mod table_faults;
