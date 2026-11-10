// Lowering tests build closed bodies directly, so each one states the exact
// scalar program it lowers; the reader's refusals are tested in `read.rs`.

use semantic_vocabulary::{
    BlockId, CrashCause, IdTable, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    Proposition, PsiSemanticId, ScalarTerm, ScalarType, ValueId,
};
use terminal_check_tables::edges::Destination;
use terminal_check_tables::obligations::{CrashSite, ObligationKind};
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{MachineTables, ProgramPoint};
use terminal_psi::{
    CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, OperationKind, TerminalMachine,
    Terminator,
};

use crate::body::{
    Body, CrashCeiling, Expression, ExpressionKind, Signature, Statement, Strict, Target,
};
use crate::outcome::Outcome;
use crate::{SourceOrigin, Unpositioned};

fn u32_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap())
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

fn integer(value: u128) -> Expression {
    expression(
        ExpressionKind::Integer(IntegerValue::Unsigned(value)),
        Some(u32_type()),
    )
}

fn slot(slot: usize, scalar_type: ScalarType) -> Expression {
    expression(ExpressionKind::Slot(slot), Some(scalar_type))
}

fn call(callee: u64, arguments: Vec<Expression>, result: Option<ScalarType>) -> Expression {
    expression(
        ExpressionKind::Call {
            callee: id(callee),
            arguments,
        },
        result,
    )
}

fn strict(
    operation: Strict,
    left: Expression,
    right: Expression,
    result: ScalarType,
) -> Expression {
    expression(
        ExpressionKind::Strict {
            operation,
            left: Box::new(left),
            right: Box::new(right),
        },
        Some(result),
    )
}

fn returns(value: Expression) -> Statement {
    Statement::Arm {
        guard: None,
        target: Target::Value(value),
        source: nowhere(),
    }
}

fn signature(parameters: Vec<ScalarType>, result: Option<ScalarType>) -> Signature {
    Signature {
        parameters,
        result,
        requires: Vec::new(),
        ensures: Vec::new(),
        crashes: CrashCeiling::Authored(Vec::new()),
    }
}

/// A machine that may return and publishes no crash route.
fn returns_normally() -> Outcome {
    Outcome {
        completes_normally: true,
        crash_routes: Vec::new(),
    }
}

/// Lowers machine 1 against `signatures` (machine 1 first), every machine
/// returning normally with no crash route.
fn lower(body: Body, signatures: Vec<Signature>) -> (TerminalMachine, MachineTables) {
    let outcomes = signatures.iter().map(|_| returns_normally()).collect();
    lower_with(body, signatures, outcomes)
}

/// Lowers machine 1 against `signatures` and `outcomes` (machine 1 first).
fn lower_with(
    body: Body,
    signatures: Vec<Signature>,
    outcomes: Vec<Outcome>,
) -> (TerminalMachine, MachineTables) {
    let lowered = try_lower(body, signatures, outcomes).unwrap();
    assert_module_and_tables_agree(&lowered.machine, &lowered.tables);
    (lowered.machine, lowered.tables)
}

fn try_lower(
    body: Body,
    signatures: Vec<Signature>,
    outcomes: Vec<Outcome>,
) -> terminal_check_tables::Checked<crate::lower::Lowered> {
    let mut signature_table: IdTable<MachineId, Signature> = IdTable::default();
    for signature in signatures {
        signature_table.push(signature);
    }
    let mut outcome_table: IdTable<MachineId, Outcome> = IdTable::default();
    for outcome in outcomes {
        outcome_table.push(outcome);
    }
    crate::lower::machine(id(1), &body, &signature_table, &outcome_table)
}

/// Every value, edge and obligation the machine names has a table row.
fn assert_module_and_tables_agree(machine: &TerminalMachine, tables: &MachineTables) {
    for block in &machine.blocks {
        for parameter in &block.parameters {
            assert!(tables.values.get(parameter.id).is_some());
        }
        for operation in &block.operations {
            if let Some(result) = operation.result.scalar() {
                let row = tables.values.get(result.id).unwrap();
                assert_eq!(row.origin, ValueOrigin::Operation(operation.id));
                assert_eq!(row.scalar_type, result.scalar_type);
            }
        }
    }
    let edges = machine
        .blocks
        .iter()
        .flat_map(|block| match &block.terminator {
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.edge, when_false.edge],
            Terminator::Jump { edge, .. }
            | Terminator::Return { edge, .. }
            | Terminator::ReturnUnit { edge, .. }
            | Terminator::Crash { edge, .. }
            | Terminator::InvokeNoNormal { edge, .. } => vec![*edge],
            other => panic!("unexpected terminator {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(edges.len(), tables.edges.len());
    for edge in edges {
        assert!(tables.edges.get(edge).is_some());
    }
}

fn block(machine: &TerminalMachine, raw: u64) -> &terminal_psi::Block {
    machine
        .blocks
        .iter()
        .find(|block| block.id == id::<BlockId>(raw))
        .unwrap()
}

fn obligations(tables: &MachineTables) -> Vec<ObligationKind> {
    (1..=tables.obligations.len() as u64)
        .map(|raw| tables.obligations.get(id(raw)).unwrap().kind)
        .collect()
}

#[test]
fn strict_operands_and_call_arguments_evaluate_left_to_right() {
    // machine 1() -> u32 { f(2) + g() }  with f = 2, g = 3
    let body = Body {
        slots: Vec::new(),
        statements: vec![returns(strict(
            Strict::Add,
            call(2, vec![integer(2)], Some(u32_type())),
            call(3, Vec::new(), Some(u32_type())),
            u32_type(),
        ))],
    };
    let (machine, tables) = lower(
        body,
        vec![
            signature(Vec::new(), Some(u32_type())),
            signature(vec![u32_type()], Some(u32_type())),
            signature(Vec::new(), Some(u32_type())),
        ],
    );
    let operations = &block(&machine, 1).operations;
    assert_eq!(operations.len(), 4);
    let OperationKind::IntegerConstant { .. } = operations[0].kind else {
        panic!("the argument comes first")
    };
    let literal = operations[0].result.scalar().unwrap().id;
    let OperationKind::Call {
        callee, arguments, ..
    } = &operations[1].kind
    else {
        panic!("then the left call")
    };
    assert_eq!(
        (*callee, arguments.as_slice()),
        (id(2), [literal].as_slice())
    );
    let OperationKind::Call { callee, .. } = &operations[2].kind else {
        panic!("then the right call")
    };
    assert_eq!(*callee, id(3));
    let OperationKind::ExactIntegerAdd {
        left,
        right,
        obligation,
    } = operations[3].kind
    else {
        panic!("then the addition")
    };
    assert_eq!(left, operations[1].result.scalar().unwrap().id);
    assert_eq!(right, operations[2].result.scalar().unwrap().id);
    // The addition is partial: one safety obligation, created before it.
    let row = tables.obligations.get(obligation).unwrap();
    assert_eq!(
        row.kind,
        ObligationKind::OperationSafety {
            operation: operations[3].id
        }
    );
    assert_eq!(
        row.created_at,
        ProgramPoint::AfterOperation(operations[2].id)
    );
}

#[test]
fn greater_swaps_operands_without_reordering_evaluation() {
    // machine 1(a: u32, b: u32) -> bool { a > b }
    let body = Body {
        slots: vec![u32_type(), u32_type()],
        statements: vec![returns(strict(
            Strict::Greater,
            slot(0, u32_type()),
            slot(1, u32_type()),
            ScalarType::Boolean,
        ))],
    };
    let (machine, _) = lower(
        body,
        vec![signature(
            vec![u32_type(), u32_type()],
            Some(ScalarType::Boolean),
        )],
    );
    let OperationKind::IntegerLessThan { left, right } = block(&machine, 1).operations[0].kind
    else {
        panic!("a > b is b < a")
    };
    assert_eq!((left, right), (id::<ValueId>(2), id::<ValueId>(1)));
}

fn short_circuit(conjunction: bool) -> (TerminalMachine, MachineTables) {
    // machine 1() -> bool { f() && g() } (or `||`)
    let body = Body {
        slots: Vec::new(),
        statements: vec![returns(expression(
            ExpressionKind::ShortCircuit {
                conjunction,
                left: Box::new(call(2, Vec::new(), Some(ScalarType::Boolean))),
                right: Box::new(call(3, Vec::new(), Some(ScalarType::Boolean))),
            },
            Some(ScalarType::Boolean),
        ))],
    };
    lower(
        body,
        vec![
            signature(Vec::new(), Some(ScalarType::Boolean)),
            signature(Vec::new(), Some(ScalarType::Boolean)),
            signature(Vec::new(), Some(ScalarType::Boolean)),
        ],
    )
}

#[test]
fn conjunction_evaluates_the_right_operand_only_on_the_true_edge() {
    let (machine, tables) = short_circuit(true);
    let entry = block(&machine, 1);
    let left = entry.operations[0].result.scalar().unwrap().id;
    let Terminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &entry.terminator
    else {
        panic!("the left operand decides")
    };
    assert_eq!(*condition, left);
    // True: evaluate the right operand, which is the only call in block 2.
    assert_eq!((when_true.target, when_true.arguments.len()), (id(2), 0));
    let right_block = block(&machine, 2);
    let [right_call] = right_block.operations.as_slice() else {
        panic!("block 2 holds only the right call")
    };
    // False: the join, carrying the false left value as the result.
    assert_eq!(
        (when_false.target, when_false.arguments.as_slice()),
        (id(3), [left].as_slice())
    );
    let Terminator::Jump {
        target, arguments, ..
    } = &right_block.terminator
    else {
        panic!("the right block jumps to the join")
    };
    assert_eq!(
        (*target, arguments.as_slice()),
        (id(3), [right_call.result.scalar().unwrap().id].as_slice())
    );
    let join = block(&machine, 3);
    let [result] = join.parameters.as_slice() else {
        panic!("the join's one parameter is the result")
    };
    let Terminator::Return { value, .. } = join.terminator else {
        panic!("the machine returns the joined value")
    };
    assert_eq!(value, result.id);
    assert_eq!(
        tables.values.get(result.id).unwrap().origin,
        ValueOrigin::BlockParameter {
            block: id(3),
            position: 0
        }
    );
}

#[test]
fn disjunction_evaluates_the_right_operand_only_on_the_false_edge() {
    let (machine, _) = short_circuit(false);
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &block(&machine, 1).terminator
    else {
        panic!("the left operand decides")
    };
    assert_eq!((when_true.target, when_true.arguments.len()), (id(3), 1));
    assert_eq!((when_false.target, when_false.arguments.len()), (id(2), 0));
}

#[test]
fn every_call_owes_one_requirement_per_callee_requires_clause() {
    // machine 1(x: u32) { f(x); f(x); }  with f requiring two clauses
    let body = Body {
        slots: vec![u32_type()],
        statements: vec![
            Statement::Evaluate(call(2, vec![slot(0, u32_type())], None)),
            Statement::Evaluate(call(2, vec![slot(0, u32_type())], None)),
        ],
    };
    let callee = Signature {
        requires: vec![Proposition::Truth, Proposition::Falsehood],
        ..signature(vec![u32_type()], None)
    };
    let (machine, tables) = lower(body, vec![signature(vec![u32_type()], None), callee]);
    assert_eq!(tables.invocations.len(), 2);
    let mut seen = Vec::new();
    for (position, operation) in block(&machine, 1).operations.iter().enumerate() {
        let OperationKind::CallUnit {
            requirement_obligations,
            ..
        } = &operation.kind
        else {
            panic!("both statements are Unit calls")
        };
        let invocation = id(position as u64 + 1);
        let row = tables.invocations.get(invocation).unwrap();
        assert_eq!(&row.requirements, requirement_obligations);
        for (clause, obligation) in requirement_obligations.iter().enumerate() {
            assert_eq!(
                tables.obligations.get(*obligation).unwrap().kind,
                ObligationKind::CallRequirement {
                    invocation,
                    clause: clause as u32
                }
            );
            seen.push(*obligation);
        }
    }
    assert_eq!(seen.len(), 4);
    assert_eq!(obligations(&tables).len(), 4);
}

#[test]
fn every_normal_exit_owes_one_guarantee_per_ensures_clause() {
    // machine 1(a: bool) -> u32 ensures P ensures Q
    //     { transition { a -> 1  _ -> 2 } }
    let body = Body {
        slots: vec![ScalarType::Boolean],
        statements: vec![
            Statement::Arm {
                guard: Some(slot(0, ScalarType::Boolean)),
                target: Target::Value(integer(1)),
                source: nowhere(),
            },
            returns(integer(2)),
        ],
    };
    let machine_signature = Signature {
        ensures: vec![Proposition::Truth, Proposition::Falsehood],
        ..signature(vec![ScalarType::Boolean], Some(u32_type()))
    };
    let (machine, tables) = lower(body, vec![machine_signature]);
    let exits = (1..=tables.edges.len() as u64)
        .map(id)
        .filter(|edge| tables.edges.get(*edge).unwrap().destination == Destination::Return)
        .collect::<Vec<_>>();
    assert_eq!(exits.len(), 2);
    let guarantees = obligations(&tables);
    let expected = exits
        .iter()
        .flat_map(|exit| {
            [0, 1].map(|clause| ObligationKind::NormalGuarantee {
                exit: *exit,
                clause,
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(guarantees, expected);
    // The module's clauses carry the first exit's identities.
    let clauses = machine
        .contract
        .ensures
        .iter()
        .map(|clause| clause.obligation)
        .collect::<Vec<_>>();
    assert_eq!(clauses, vec![id::<ObligationId>(1), id(2)]);
    // The guarded arm branches; the false edge continues to the last arm.
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &block(&machine, 1).terminator
    else {
        panic!("the guard branches")
    };
    assert_eq!((when_true.target, when_false.target), (id(2), id(3)));
}

#[test]
fn a_unit_machine_completes_after_its_last_statement() {
    let body = Body {
        slots: vec![u32_type()],
        statements: vec![Statement::Bind {
            slot: 0,
            value: integer(4),
        }],
    };
    let (machine, tables) = lower(body, vec![signature(vec![u32_type()], None)]);
    assert!(matches!(
        block(&machine, 1).terminator,
        Terminator::ReturnUnit { .. }
    ));
    assert!(tables.places.is_empty() && tables.writes.is_empty());
}

#[test]
fn every_exact_operation_is_partial_by_the_policy_catalog() {
    use numerics::arithmetic::ArithmeticDomain;
    use numerics::integer_policy::{IntegerPolicyPrimitive as P, integer_policy_bridge};
    // Lowering mints an `OperationSafety` obligation for exactly these.
    for primitive in [
        P::Add,
        P::Subtract,
        P::Multiply,
        P::Divide,
        P::Remainder,
        P::ShiftLeft,
        P::ShiftRight,
    ] {
        assert!(
            !integer_policy_bridge(primitive, ArithmeticDomain::Exact)
                .formation_conditions
                .is_empty()
        );
    }
}

#[test]
fn a_value_machine_that_falls_off_its_end_is_refused() {
    let body = Body {
        slots: Vec::new(),
        statements: Vec::new(),
    };
    let refusal = try_lower(
        body,
        vec![signature(Vec::new(), Some(u32_type()))],
        vec![returns_normally()],
    )
    .unwrap_err();
    assert_eq!(
        refusal.reason,
        terminal_check_tables::RefusalReason::Unsupported
    );
}

fn boolean_slot(slot: usize) -> Expression {
    expression(ExpressionKind::Slot(slot), Some(ScalarType::Boolean))
}

fn crash_arm(guard: Option<Expression>, cause: CrashCause) -> Statement {
    Statement::Arm {
        guard,
        target: Target::Crash(cause),
        source: nowhere(),
    }
}

fn decision(value: ValueId, taken: bool) -> CrashPredicateTerm {
    CrashPredicateTerm::new(Proposition::Equal(
        ScalarTerm::Value {
            id: value,
            scalar_type: ScalarType::Boolean,
        },
        ScalarTerm::Boolean(taken),
    ))
}

fn crash_sites(machine: &TerminalMachine) -> Vec<(BlockId, CrashCause, Vec<CrashPredicateTerm>)> {
    machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Crash {
                cause, site_guard, ..
            } => Some((block.id, *cause, site_guard.clone())),
            _ => None,
        })
        .collect()
}

/// `crash Trap;` behind a guard is a `Crash` terminator with its cause, and
/// its site guard is the exact incoming conjunction: the earlier arm's guard
/// was false and this arm's guard was true.
#[test]
fn a_crash_is_a_terminator_with_its_cause_and_incoming_conjunction() {
    // machine 1(a: bool, b: bool) { transition { a -> {}  b -> crash Trap  _ -> crash Abort } }
    let body = Body {
        slots: vec![ScalarType::Boolean, ScalarType::Boolean],
        statements: vec![
            Statement::Arm {
                guard: Some(boolean_slot(0)),
                target: Target::Unit,
                source: nowhere(),
            },
            crash_arm(Some(boolean_slot(1)), CrashCause::Trap),
            crash_arm(None, CrashCause::Abort),
        ],
    };
    let (machine, _) = lower(
        body,
        vec![signature(
            vec![ScalarType::Boolean, ScalarType::Boolean],
            None,
        )],
    );
    let (a, b) = (id::<ValueId>(1), id::<ValueId>(2));
    // Block 1 branches on a (2 arm, 3 rest); block 3 on b (4 arm, 5 rest).
    assert_eq!(
        crash_sites(&machine),
        vec![
            (
                id(4),
                CrashCause::Trap,
                vec![decision(a, false), decision(b, true)]
            ),
            (
                id(5),
                CrashCause::Abort,
                vec![decision(a, false), decision(b, false)]
            ),
        ]
    );
    for (_, _, guard) in crash_sites(&machine) {
        // Canonical: each `Equal` names the value before the literal.
        for term in &guard {
            let Proposition::Equal(ScalarTerm::Value { .. }, ScalarTerm::Boolean(_)) =
                term.proposition()
            else {
                panic!("a decision is the branch value equal to a Boolean")
            };
        }
    }
}

/// Each crash edge has no successor and no conserved obligation to give a
/// fate, and each crash site owes one `CrashCoverage` per cause, created
/// before its terminator.
#[test]
fn every_crash_site_owes_one_coverage_per_cause_on_a_no_successor_edge() {
    let body = Body {
        slots: vec![ScalarType::Boolean],
        statements: vec![
            crash_arm(Some(boolean_slot(0)), CrashCause::Trap),
            crash_arm(None, CrashCause::Abort),
        ],
    };
    let (machine, tables) = lower(body, vec![signature(vec![ScalarType::Boolean], None)]);
    let mut coverage = Vec::new();
    for (site, cause, _) in crash_sites(&machine) {
        let Terminator::Crash { edge, .. } = block(&machine, site.get()).terminator else {
            unreachable!()
        };
        let row = tables.edges.get(edge).unwrap();
        assert_eq!(
            (row.source, row.destination, row.dispositions.len()),
            (site, Destination::NoNormal, 0)
        );
        coverage.push(ObligationKind::CrashCoverage {
            site: CrashSite::Block(site),
            cause,
        });
    }
    assert_eq!(obligations(&tables), coverage);
    for raw in 1..=tables.obligations.len() as u64 {
        let row = tables.obligations.get(id(raw)).unwrap();
        let ObligationKind::CrashCoverage {
            site: CrashSite::Block(site),
            ..
        } = row.kind
        else {
            panic!("a crash terminator's site is its block")
        };
        assert_eq!(row.created_at, ProgramPoint::BlockEntry(site));
    }
}

fn routes(causes: &[CrashCause]) -> Vec<CrashRouteBucket> {
    causes
        .iter()
        .map(|cause| CrashRouteBucket {
            cause: *cause,
            alternatives: vec![CrashRouteGuard::Truth],
        })
        .collect()
}

/// A terminal call of a machine with no normal completion is the block's
/// `InvokeNoNormal` terminator. Its crash continuations are the callee's
/// routes with formals replaced by the actuals, and the site owes one
/// `CrashCoverage` per cause besides the callee's `requires` clause.
#[test]
fn a_call_with_no_normal_completion_is_a_no_successor_crash_site() {
    // machine 1(x: u32) { transition { _ -> f(x) } }
    // with f(y: u32) requires P crashes Trap when y < 3 crashes Abort
    let body = Body {
        slots: vec![u32_type()],
        statements: vec![returns(call(2, vec![slot(0, u32_type())], None))],
    };
    let formal = ScalarTerm::Value {
        id: id(1),
        scalar_type: u32_type(),
    };
    let three = ScalarTerm::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
        value: IntegerValue::Unsigned(3),
    };
    let guarded = CrashRouteGuard::Predicate(CrashPredicateTerm::new(Proposition::LessThan(
        formal,
        three.clone(),
    )));
    let callee_routes = vec![
        CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![guarded],
        },
        CrashRouteBucket {
            cause: CrashCause::Abort,
            alternatives: vec![CrashRouteGuard::Truth],
        },
    ];
    let callee = Signature {
        requires: vec![Proposition::Truth],
        ..signature(vec![u32_type()], None)
    };
    let (machine, tables) = lower_with(
        body,
        vec![signature(vec![u32_type()], None), callee],
        vec![
            returns_normally(),
            Outcome {
                completes_normally: false,
                crash_routes: callee_routes,
            },
        ],
    );
    let entry = block(&machine, 1);
    let Terminator::InvokeNoNormal {
        edge,
        callee,
        arguments,
        requirement_obligations,
        crash_continuations,
        ..
    } = &entry.terminator
    else {
        panic!("the call is the terminator")
    };
    assert_eq!((*callee, arguments.as_slice()), (id(2), [id(1)].as_slice()));
    // The caller's x is value 1 too, so the substituted guard reads the same
    // here; the actual it names is the caller's value.
    assert_eq!(
        crash_continuations,
        &vec![
            CrashRouteBucket {
                cause: CrashCause::Trap,
                alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
                    Proposition::LessThan(
                        ScalarTerm::Value {
                            id: arguments[0],
                            scalar_type: u32_type()
                        },
                        three
                    )
                ))],
            },
            CrashRouteBucket {
                cause: CrashCause::Abort,
                alternatives: vec![CrashRouteGuard::Truth],
            },
        ]
    );
    let row = tables.edges.get(*edge).unwrap();
    assert_eq!(row.destination, Destination::NoNormal);
    let invocation = tables.invocations.get(id(1)).unwrap();
    assert_eq!(
        invocation.site,
        semantic_vocabulary::InvocationSite::InvokeNoNormal(*edge)
    );
    assert_eq!(&invocation.requirements, requirement_obligations);
    assert_eq!(
        obligations(&tables),
        vec![
            ObligationKind::CallRequirement {
                invocation: id(1),
                clause: 0
            },
            ObligationKind::CrashCoverage {
                site: CrashSite::Block(id(1)),
                cause: CrashCause::Trap
            },
            ObligationKind::CrashCoverage {
                site: CrashSite::Block(id(1)),
                cause: CrashCause::Abort
            },
        ]
    );
    // No normal exit, so no return edge and no guarantee.
    assert_eq!(tables.edges.len(), 1);
}

/// A call of a machine that may return and may crash stays an operation in
/// the middle of its block. It carries the callee's routes with formals
/// replaced by the actuals, and that operation, not the block, is the crash
/// site: one `CrashCoverage` per cause, created before the call.
#[test]
fn a_call_that_may_return_or_crash_owes_coverage_at_its_operation() {
    // machine 1(x: u32) -> u32 { f(x) + 1 }
    // with f(y: u32) -> u32 crashes Trap when y < 3 crashes Abort
    let body = Body {
        slots: vec![u32_type()],
        statements: vec![returns(strict(
            Strict::Add,
            call(2, vec![slot(0, u32_type())], Some(u32_type())),
            integer(1),
            u32_type(),
        ))],
    };
    let three = ScalarTerm::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
        value: IntegerValue::Unsigned(3),
    };
    let guarded = |value: ValueId| {
        CrashRouteGuard::Predicate(CrashPredicateTerm::new(Proposition::LessThan(
            ScalarTerm::Value {
                id: value,
                scalar_type: u32_type(),
            },
            three.clone(),
        )))
    };
    let callee_routes = vec![
        CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![guarded(id(1))],
        },
        CrashRouteBucket {
            cause: CrashCause::Abort,
            alternatives: vec![CrashRouteGuard::Truth],
        },
    ];
    let (machine, tables) = lower_with(
        body,
        vec![
            signature(vec![u32_type()], Some(u32_type())),
            signature(vec![u32_type()], Some(u32_type())),
        ],
        vec![
            Outcome {
                completes_normally: true,
                crash_routes: routes(&[CrashCause::Trap, CrashCause::Abort]),
            },
            Outcome {
                completes_normally: true,
                crash_routes: callee_routes,
            },
        ],
    );
    // One block: the call does not end it.
    assert_eq!(machine.blocks.len(), 1);
    let entry = block(&machine, 1);
    let call_operation = &entry.operations[0];
    let OperationKind::Call {
        callee,
        arguments,
        crash_continuations,
        ..
    } = &call_operation.kind
    else {
        panic!("the call is the block's first operation")
    };
    assert_eq!((*callee, arguments.as_slice()), (id(2), [id(1)].as_slice()));
    assert_eq!(
        crash_continuations,
        &vec![
            CrashRouteBucket {
                cause: CrashCause::Trap,
                alternatives: vec![guarded(arguments[0])],
            },
            CrashRouteBucket {
                cause: CrashCause::Abort,
                alternatives: vec![CrashRouteGuard::Truth],
            },
        ]
    );
    assert!(matches!(entry.terminator, Terminator::Return { .. }));
    let site = CrashSite::Operation(call_operation.id);
    let coverage: Vec<_> = obligations(&tables)
        .into_iter()
        .filter(|kind| matches!(kind, ObligationKind::CrashCoverage { .. }))
        .collect();
    assert_eq!(
        coverage,
        vec![
            ObligationKind::CrashCoverage {
                site,
                cause: CrashCause::Trap
            },
            ObligationKind::CrashCoverage {
                site,
                cause: CrashCause::Abort
            },
        ]
    );
    for raw in 1..=tables.obligations.len() as u64 {
        let row = tables.obligations.get(id(raw)).unwrap();
        if matches!(row.kind, ObligationKind::CrashCoverage { .. }) {
            // The call is the first operation, so the point before it is
            // the block entry.
            assert_eq!(row.created_at, ProgramPoint::BlockEntry(id(1)));
        }
    }
}

/// A call with no normal completion inside an expression has no terminator
/// to be, so it is refused, not lowered as an operation that returns.
#[test]
fn a_no_normal_call_inside_an_expression_is_refused() {
    // machine 1() -> u32 { f() + 1 }
    let body = Body {
        slots: Vec::new(),
        statements: vec![returns(strict(
            Strict::Add,
            call(2, Vec::new(), Some(u32_type())),
            integer(1),
            u32_type(),
        ))],
    };
    let refusal = try_lower(
        body,
        vec![
            signature(Vec::new(), Some(u32_type())),
            signature(Vec::new(), Some(u32_type())),
        ],
        vec![
            returns_normally(),
            Outcome {
                completes_normally: false,
                crash_routes: routes(&[CrashCause::Trap]),
            },
        ],
    )
    .unwrap_err();
    assert_eq!(refusal.detail, "call no-normal call inside an expression");
}

/// `ensures result == 7` on `machine(x: u32) -> u32`: the result
/// pseudo-value is value 2, right after the formal, with its own row; the
/// clause names it and every exit owes the guarantee.
#[test]
fn ensures_over_result_name_the_result_pseudo_value() {
    let result = ScalarTerm::Value {
        id: id(2),
        scalar_type: u32_type(),
    };
    let seven = ScalarTerm::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
        value: IntegerValue::Unsigned(7),
    };
    let clause = Proposition::Equal(result, seven);
    let body = Body {
        slots: vec![u32_type()],
        statements: vec![returns(integer(7))],
    };
    let (machine, tables) = lower(
        body,
        vec![Signature {
            ensures: vec![clause.clone()],
            ..signature(vec![u32_type()], Some(u32_type()))
        }],
    );
    let terminal_psi::TerminalMachineResult::Scalar(declared) = machine.result else {
        panic!("a scalar machine")
    };
    assert_eq!(declared.id, id::<ValueId>(2));
    let row = tables.values.get(declared.id).unwrap();
    assert_eq!(
        (row.origin, row.scalar_type),
        (ValueOrigin::MachineResult, u32_type())
    );
    let [contract] = machine.contract.ensures.as_slice() else {
        panic!("one clause")
    };
    assert_eq!(contract.proposition, clause);
    assert_eq!(
        tables.obligations.get(contract.obligation).unwrap().kind,
        ObligationKind::NormalGuarantee {
            exit: id(1),
            clause: 0
        }
    );
    // The literal 7 is the first body value, after the result.
    assert_eq!(
        tables.values.get(id(3)).unwrap().origin,
        ValueOrigin::Operation(id(1))
    );
}

/// A machine publishes what it may do: an authored ceiling as written, none
/// for a public body without a clause, and for a private body without one
/// each cause its body may reach, as `Truth`. A body whose every path crashes
/// has no normal completion.
#[test]
fn outcomes_follow_the_ceiling_or_the_body() {
    use crate::outcome::outcomes;
    // machine 1 calls 2; 2 is `{ crash Trap; }`.
    let caller = Body {
        slots: Vec::new(),
        statements: vec![Statement::Evaluate(call(2, Vec::new(), None))],
    };
    let crashing = Body {
        slots: Vec::new(),
        statements: vec![crash_arm(None, CrashCause::Trap)],
    };
    let run = |first: CrashCeiling, second: CrashCeiling| {
        let mut bodies: IdTable<MachineId, Body> = IdTable::default();
        bodies.push(caller.clone());
        bodies.push(crashing.clone());
        let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
        for crashes in [first, second] {
            signatures.push(Signature {
                crashes,
                ..signature(Vec::new(), None)
            });
        }
        let table = outcomes(&bodies, &signatures).unwrap();
        [1, 2].map(|raw| table.get(id(raw)).unwrap().clone())
    };
    let [caller_outcome, callee_outcome] = run(CrashCeiling::Inferred, CrashCeiling::Inferred);
    assert_eq!(
        callee_outcome,
        Outcome {
            completes_normally: false,
            crash_routes: routes(&[CrashCause::Trap]),
        }
    );
    assert_eq!(caller_outcome, callee_outcome);
    let [caller_outcome, callee_outcome] = run(
        CrashCeiling::Authored(routes(&[CrashCause::Abort])),
        CrashCeiling::Authored(Vec::new()),
    );
    assert!(callee_outcome.crash_routes.is_empty());
    assert_eq!(caller_outcome.crash_routes, routes(&[CrashCause::Abort]));
}

/// The right operand of `&&` or `||` may be skipped, so a short-circuit whose
/// right operand never returns still completes; its causes stay published. A
/// call cycle handed to `lower_bodies` directly is refused as recursion.
#[test]
fn a_skipped_operand_cannot_end_the_machine_and_a_cycle_is_refused() {
    use crate::outcome::outcomes;
    // machine 1(a: bool) -> bool { a && f() }  with f = `{ crash Trap; }`
    let caller = Body {
        slots: vec![ScalarType::Boolean],
        statements: vec![returns(expression(
            ExpressionKind::ShortCircuit {
                conjunction: true,
                left: Box::new(boolean_slot(0)),
                right: Box::new(call(2, Vec::new(), Some(ScalarType::Boolean))),
            },
            Some(ScalarType::Boolean),
        ))],
    };
    let crashing = Body {
        slots: Vec::new(),
        statements: vec![crash_arm(None, CrashCause::Trap)],
    };
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    bodies.push(caller);
    bodies.push(crashing);
    let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
    signatures.push(Signature {
        crashes: CrashCeiling::Inferred,
        ..signature(vec![ScalarType::Boolean], Some(ScalarType::Boolean))
    });
    signatures.push(Signature {
        crashes: CrashCeiling::Inferred,
        ..signature(Vec::new(), Some(ScalarType::Boolean))
    });
    let table = outcomes(&bodies, &signatures).unwrap();
    assert_eq!(
        table.get(id(1)).unwrap(),
        &Outcome {
            completes_normally: true,
            crash_routes: routes(&[CrashCause::Trap]),
        }
    );

    // machine 1 { f() }, machine 2 { g() }, machine 3 { f() }
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
    for callee in [2, 3, 2] {
        bodies.push(Body {
            slots: Vec::new(),
            statements: vec![Statement::Evaluate(call(callee, Vec::new(), None))],
        });
        signatures.push(signature(Vec::new(), None));
    }
    assert_eq!(
        outcomes(&bodies, &signatures).unwrap_err().detail,
        "call recursion"
    );
}
