// Lowering tests build closed bodies directly, so each one states the exact
// scalar program it lowers; the reader's refusals are tested in `read.rs`.

use semantic_vocabulary::{
    BlockId, CrashCause, IdTable, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    Proposition, PsiSemanticId, ScalarTerm, ScalarType, ValueId,
};
use terminal_check_tables::bindings::{Content, PlaceOrigin, Segment};
use terminal_check_tables::edges::{Actual, Destination};
use terminal_check_tables::obligations::{CrashSite, ObligationKind};
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{MachineTables, ProgramPoint};
use terminal_psi::{
    CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, OperationKind, TerminalMachine,
    Terminator,
};

use crate::body::{
    Body, CrashCeiling, Expression, ExpressionKind, Signature, Statement, Strict, Target, ValueType,
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
        result: result.map(ValueType::Scalar),
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
        parameters: scalars(parameters),
        result: result.map(ValueType::Scalar),
        requires: Vec::new(),
        ensures: Vec::new(),
        crashes: CrashCeiling::Authored(Vec::new()),
    }
}

/// Scalar slot or formal types as the body holds them.
fn scalars(types: Vec<ScalarType>) -> Vec<ValueType> {
    types.into_iter().map(ValueType::Scalar).collect()
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
    lower_records(body, &signature_table, &outcome_table, &IdTable::default())
}

/// Lowers machine 1 with the record types its body and signatures name.
fn lower_records(
    body: Body,
    signatures: &IdTable<MachineId, Signature>,
    outcomes: &IdTable<MachineId, Outcome>,
    records: &IdTable<semantic_vocabulary::StructuralTypeId, crate::body::RecordType>,
) -> terminal_check_tables::Checked<crate::lower::Lowered> {
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    bodies.push(body);
    let declared = crate::lower::places::declare(id(1), records, &bodies)?;
    let body = bodies.get(id(1)).unwrap();
    crate::lower::machine(id(1), body, signatures, outcomes, &declared.records)
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
            | Terminator::ReturnStructural { edge, .. }
            | Terminator::ReturnUnit { edge, .. }
            | Terminator::Crash { edge, .. }
            | Terminator::InvokeNoNormal { edge, .. } => vec![*edge],
            Terminator::StructuralCase { cases, .. } => {
                cases.iter().map(|case| case.edge).collect()
            }
            other => panic!("unexpected terminator {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(edges.len(), tables.edges.len());
    for edge in edges {
        assert!(tables.edges.get(edge).is_some());
    }
    // Every structural place the machine declares is a root row, and the
    // root table holds nothing else.
    assert_eq!(machine.structural_places.len(), tables.roots.len());
    for place in &machine.structural_places {
        let root = *tables.roots.get(place.id).unwrap();
        assert_eq!(
            tables.places.get(root).unwrap().origin,
            PlaceOrigin::Root(place.id)
        );
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
        slots: scalars(vec![u32_type(), u32_type()]),
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
        slots: scalars(vec![u32_type()]),
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
        slots: scalars(vec![ScalarType::Boolean]),
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
    // One published identity per clause first, then one guarantee per (exit,
    // clause) naming it.
    let guarantees = obligations(&tables);
    let mut expected = vec![
        ObligationKind::PublishedGuarantee { clause: 0 },
        ObligationKind::PublishedGuarantee { clause: 1 },
    ];
    expected.extend(exits.iter().flat_map(|exit| {
        [0, 1].map(|clause| ObligationKind::NormalGuarantee {
            exit: *exit,
            clause,
            published: id(u64::from(clause) + 1),
        })
    }));
    assert_eq!(guarantees, expected);
    // The module's clauses carry the published identities, not an exit's.
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
        slots: scalars(vec![u32_type()]),
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
        slots: scalars(vec![ScalarType::Boolean, ScalarType::Boolean]),
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
        slots: scalars(vec![ScalarType::Boolean]),
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
        slots: scalars(vec![u32_type()]),
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
        slots: scalars(vec![u32_type()]),
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
        slots: scalars(vec![u32_type()]),
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
    // The clause carries its published identity; the one return edge owes a
    // guarantee that names it.
    assert_eq!(
        tables.obligations.get(contract.obligation).unwrap().kind,
        ObligationKind::PublishedGuarantee { clause: 0 }
    );
    assert!(tables.obligations.iter().any(|(_, row)| {
        row.kind
            == ObligationKind::NormalGuarantee {
                exit: id(1),
                clause: 0,
                published: contract.obligation,
            }
    }));
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

/// An authored `crashes none` is a promise, not a request to infer: a body
/// with no crash publishes no route, and a body that reaches a crash, by its
/// own `crash` statement or through a callee's route, is refused by name
/// instead of being given the route its author forbade.
#[test]
fn a_body_that_declares_crashes_none_and_can_crash_is_refused() {
    use crate::outcome::outcomes;
    let run = |statements: Vec<Statement>, callee_statements: Vec<Statement>| {
        let mut bodies: IdTable<MachineId, Body> = IdTable::default();
        let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
        for (statements, crashes) in [
            (statements, CrashCeiling::DeclaredNone),
            (callee_statements, CrashCeiling::Inferred),
        ] {
            bodies.push(Body {
                slots: Vec::new(),
                statements,
            });
            signatures.push(Signature {
                crashes,
                ..signature(Vec::new(), None)
            });
        }
        outcomes(&bodies, &signatures).map(|table| table.get(id(1)).unwrap().clone())
    };
    let quiet = || Vec::new();
    let crashing = || vec![crash_arm(None, CrashCause::Trap)];
    let calls = || vec![Statement::Evaluate(call(2, Vec::new(), None))];

    assert_eq!(run(quiet(), quiet()).unwrap(), returns_normally());
    assert_eq!(run(calls(), quiet()).unwrap(), returns_normally());
    for refusal in [
        run(crashing(), quiet()).unwrap_err(),
        run(calls(), crashing()).unwrap_err(),
    ] {
        assert_eq!(
            (refusal.reason, refusal.machine, refusal.detail.as_str()),
            (
                terminal_check_tables::RefusalReason::Unsupported,
                id(1),
                "contract `crashes none` on a body with a crash route"
            )
        );
    }
}

/// The right operand of `&&` or `||` may be skipped, so a short-circuit whose
/// right operand never returns still completes; its causes stay published. A
/// call cycle handed to `lower_bodies` directly is refused as recursion.
#[test]
fn a_skipped_operand_cannot_end_the_machine_and_a_cycle_is_refused() {
    use crate::outcome::outcomes;
    // machine 1(a: bool) -> bool { a && f() }  with f = `{ crash Trap; }`
    let caller = Body {
        slots: scalars(vec![ScalarType::Boolean]),
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

// Records by structure. `Point { x: u32, y: u32 }` is record 1 with fields 1
// and 2; `Line { from: Point, to: Point }` is record 2 with fields 3 and 4.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, StructuralFieldId, StructuralPlaceKind,
    StructuralTypeId,
};
use terminal_check_tables::ExactPlaceId;
use terminal_psi::{RecordFieldValue, StructuralAccess, StructuralArgument};

use crate::body::{RecordField, RecordType};

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

fn member(base: Expression, field: usize, result: ValueType) -> Expression {
    of_type(
        ExpressionKind::Field {
            base: Box::new(base),
            field,
        },
        result,
    )
}

fn literal(fields: Vec<(usize, Expression)>, result: ValueType) -> Expression {
    of_type(ExpressionKind::Record { fields }, result)
}

fn bind(slot: usize, value: Expression) -> Statement {
    Statement::Bind { slot, value }
}

fn shaped(parameters: Vec<ValueType>, result: Option<ValueType>) -> Signature {
    Signature {
        parameters,
        result,
        ..signature(Vec::new(), None)
    }
}

/// Lowers machine 1 over `Point` and `Line`, every machine returning
/// normally with no crash route.
fn lower_places(body: Body, signatures: Vec<Signature>) -> (TerminalMachine, MachineTables) {
    let mut signature_table: IdTable<MachineId, Signature> = IdTable::default();
    let mut outcome_table: IdTable<MachineId, Outcome> = IdTable::default();
    for signature in signatures {
        signature_table.push(signature);
        outcome_table.push(returns_normally());
    }
    let lowered = lower_records(body, &signature_table, &outcome_table, &records()).unwrap();
    assert_module_and_tables_agree(&lowered.machine, &lowered.tables);
    (lowered.machine, lowered.tables)
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

/// Every write to `place`, as its content and the operation it follows.
fn written(tables: &MachineTables, place: ExactPlaceId) -> Vec<(Content, ProgramPoint)> {
    tables
        .places
        .get(place)
        .unwrap()
        .writes
        .iter()
        .map(|write| {
            let row = tables.writes.get(*write).unwrap();
            assert_eq!(row.place, place);
            (row.content, row.at)
        })
        .collect()
}

fn produced(operation: &terminal_psi::Operation) -> PlaceId {
    operation.result.as_structural().unwrap().place
}

#[test]
fn a_record_literal_evaluates_its_operands_in_authored_order_then_establishes_once() {
    // make() -> Point { Point { y: g(), x: 3 } }: `y`'s operand is authored
    // first, so the call runs before the constant, and the establishment
    // comes after both with its roster in declaration order.
    let body = Body {
        slots: Vec::new(),
        statements: vec![returns(literal(
            vec![(1, call(2, Vec::new(), Some(u32_type()))), (0, integer(3))],
            point(),
        ))],
    };
    let (machine, tables) = lower_places(
        body,
        vec![
            shaped(Vec::new(), Some(point())),
            signature(Vec::new(), Some(u32_type())),
        ],
    );
    let entry = block(&machine, 1);
    let [called, constant, established] = entry.operations.as_slice() else {
        panic!("three operations");
    };
    assert!(matches!(called.kind, OperationKind::Call { .. }));
    assert!(matches!(
        constant.kind,
        OperationKind::IntegerConstant { .. }
    ));
    let y = called.result.scalar().unwrap().id;
    let x = constant.result.scalar().unwrap().id;
    let OperationKind::EstablishRecord { fields } = &established.kind else {
        panic!("establishment");
    };
    let roster: Vec<_> = fields
        .iter()
        .map(|field| match field.value {
            RecordFieldValue::Scalar {
                value,
                range_obligation: None,
            } => (field.field, value),
            _ => panic!("plain scalar initializer"),
        })
        .collect();
    assert_eq!(roster, [(id(1), x), (id(2), y)]);
    // The machine's result place is root 1; the literal is the root its
    // operation produced, and the machine returns that root.
    let root = produced(established);
    assert_eq!(
        machine
            .structural_places
            .iter()
            .map(|place| place.kind)
            .collect::<Vec<_>>(),
        [
            StructuralPlaceKind::Result,
            StructuralPlaceKind::OperationResult {
                producer: established.id,
                structural_type: id(1),
            },
        ]
    );
    assert!(matches!(
        entry.terminator,
        Terminator::ReturnStructural { source, .. } if source == root
    ));
    // One write per scalar leaf, the operand itself, after the operation.
    let after = ProgramPoint::AfterOperation(established.id);
    assert_eq!(
        written(&tables, projected(&tables, root, &[1])),
        [(Content::Value(x), after)]
    );
    assert_eq!(
        written(&tables, projected(&tables, root, &[2])),
        [(Content::Value(y), after)]
    );
    assert_eq!(tables.writes.len(), 2);
    assert!(tables.obligations.is_empty());
}

#[test]
fn a_nested_read_interns_one_place_per_projection_and_reuses_it() {
    // far(l: Line) -> u32 { l.to.y & l.to.y }
    let read = || member(member(place(0, line()), 1, point()), 1, u32_type().into());
    let body = Body {
        slots: vec![line()],
        statements: vec![returns(strict(
            Strict::BitwiseAnd,
            read(),
            read(),
            u32_type(),
        ))],
    };
    let (machine, tables) = lower_places(body, vec![shaped(vec![line()], Some(u32_type().into()))]);
    let formal = machine.structural_parameters[0].place;
    assert_eq!(
        machine.parameter_order,
        [terminal_psi::BoundaryParameterKind::Structural]
    );
    assert!(machine.parameters.is_empty());
    let reads: Vec<_> = block(&machine, 1)
        .operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            OperationKind::ScalarStructuralField {
                source,
                path,
                field,
            } => Some((*source, path.clone(), *field)),
            _ => None,
        })
        .collect();
    // Both reads name the whole formal, the carrier path `to` and the final
    // field `y`, all by dense identity.
    let expected = (
        formal,
        vec![CanonicalStructuralPathSegment::Field(id(4))],
        id::<StructuralFieldId>(2),
    );
    assert_eq!(reads, [expected.clone(), expected]);
    // The root, `l.to` and `l.to.y`: three places for two reads.
    assert_eq!(tables.places.len(), 3);
    let to = projected(&tables, formal, &[4]);
    let leaf = projected(&tables, formal, &[4, 2]);
    assert_eq!(
        tables.places.get(leaf).unwrap().origin,
        PlaceOrigin::Projection {
            parent: to,
            segment: Segment::Field(id(2)),
        }
    );
    // What a formal holds at entry is not a write: it is one value per leaf,
    // minted at the first read and shared by the second.
    assert!(tables.writes.is_empty());
    let entry_contents: Vec<_> = tables
        .values
        .iter()
        .filter_map(|(value, row)| match row.origin {
            ValueOrigin::EntryContent { place } => Some((value, place, row.scalar_type)),
            _ => None,
        })
        .collect();
    assert_eq!(entry_contents.len(), 1);
    assert_eq!(
        (entry_contents[0].1, entry_contents[0].2),
        (leaf, u32_type())
    );
    // A record formal takes no value identity: with no scalar formal the
    // result pseudo-value is value 1.
    assert_eq!(
        tables.values.get(id(1)).unwrap().origin,
        ValueOrigin::MachineResult
    );
}

#[test]
fn a_whole_copy_logs_one_row_on_its_fresh_root_and_names_its_source() {
    // f(a: u32, p: Point) -> u32 {
    //     let q = Point { x: a, y: 7 };  let r = q;   // a copy of values
    //     let s = p;                                   // of entry contents
    //     let t = g();  let u = t;                     // of unknown contents
    //     r.x
    // }
    let body = Body {
        slots: vec![
            u32_type().into(),
            point(),
            point(),
            point(),
            point(),
            point(),
            point(),
        ],
        statements: vec![
            bind(
                2,
                literal(vec![(0, slot(0, u32_type())), (1, integer(7))], point()),
            ),
            bind(3, place(2, point())),
            bind(4, place(1, point())),
            bind(
                5,
                of_type(
                    ExpressionKind::Call {
                        callee: id(2),
                        arguments: Vec::new(),
                    },
                    point(),
                ),
            ),
            bind(6, place(5, point())),
            returns(member(place(3, point()), 0, u32_type().into())),
        ],
    };
    let (machine, tables) = lower_places(
        body,
        vec![
            shaped(vec![u32_type().into(), point()], Some(u32_type().into())),
            shaped(Vec::new(), Some(point())),
        ],
    );
    let operations = &block(&machine, 1).operations;
    let kinds: Vec<_> = operations
        .iter()
        .map(|operation| match &operation.kind {
            OperationKind::IntegerConstant { .. } => "constant",
            OperationKind::EstablishRecord { .. } => "establish",
            OperationKind::StructuralLeafCopy { path, .. } if path.is_empty() => "copy",
            OperationKind::CallStructuralWithScalarArguments { .. } => "call",
            OperationKind::ScalarStructuralField { .. } => "read",
            other => panic!("unexpected operation {other:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "constant",
            "establish",
            "copy",
            "copy",
            "call",
            "copy",
            "read"
        ]
    );
    let a: ValueId = id(1);
    let seven = operations[0].result.scalar().unwrap().id;
    let formal = machine.structural_parameters[0].place;
    let (q, r, s, t, u) = (
        produced(&operations[1]),
        produced(&operations[2]),
        produced(&operations[3]),
        produced(&operations[4]),
        produced(&operations[5]),
    );
    let copy_source = |operation: &terminal_psi::Operation| match &operation.kind {
        OperationKind::StructuralLeafCopy { source, .. } => *source,
        other => panic!("copy expected, found {other:?}"),
    };
    assert_eq!(
        [&operations[2], &operations[3], &operations[5]].map(copy_source),
        [q, formal, t]
    );
    let after = |position: usize| ProgramPoint::AfterOperation(operations[position].id);
    // Lowering does not say what a copy's leaves hold: it would have to
    // ask what the source holds at that point, which only the graph answers
    // (the checker resolves it; `lowered-discharge`, `places`). Each copy
    // logs one row on its fresh root, and its operation names the source.
    let root = |place| *tables.roots.get(place).unwrap();
    let _ = (a, seven);
    for (copy, position) in [(r, 2), (s, 3), (u, 5)] {
        assert_eq!(
            written(&tables, root(copy)),
            [(Content::Overwritten, after(position))]
        );
        for (_, leaf) in &tables.places.get(root(copy)).unwrap().children {
            assert!(written(&tables, *leaf).is_empty());
        }
    }
    // Copying the formal names its storage, so each of its leaves has its
    // one entry content value, minted for that leaf and never written there.
    for field in [1, 2] {
        let leaf = projected(&tables, formal, &[field]);
        assert!(written(&tables, leaf).is_empty());
        let values = tables
            .values
            .iter()
            .filter(|(_, row)| row.origin == ValueOrigin::EntryContent { place: leaf });
        assert_eq!(values.count(), 1);
    }
    // A call's record result has no value identity: nothing is invented.
    assert_eq!(
        written(&tables, root(t)),
        [(Content::Overwritten, after(4))]
    );
    // The read names `r`'s root directly; a local has no place of its own.
    assert!(matches!(
        &operations[6].kind,
        OperationKind::ScalarStructuralField { source, path, field }
            if *source == r && path.is_empty() && *field == id(1)
    ));
}

#[test]
fn a_record_argument_is_passed_whole_and_owned_and_its_home_is_retired() {
    // f(p: Point) -> u32 { g(p, 5) }, with g(Point, u32) -> u32.
    let body = Body {
        slots: vec![point()],
        statements: vec![returns(call(
            2,
            vec![place(0, point()), integer(5)],
            Some(u32_type()),
        ))],
    };
    let (machine, tables) = lower_places(
        body,
        vec![
            shaped(vec![point()], Some(u32_type().into())),
            shaped(vec![point(), u32_type().into()], Some(u32_type().into())),
        ],
    );
    let entry = block(&machine, 1);
    let [copied, constant, called] = entry.operations.as_slice() else {
        panic!("three operations");
    };
    // Passing by value copies the formal into a root nothing else names.
    let formal = machine.structural_parameters[0].place;
    let argument = produced(copied);
    assert!(matches!(
        &copied.kind,
        OperationKind::StructuralLeafCopy { source, path } if *source == formal && path.is_empty()
    ));
    let five = constant.result.scalar().unwrap().id;
    let OperationKind::CallStructuralScalar {
        callee,
        arguments,
        structural_arguments,
        ..
    } = &called.kind
    else {
        panic!("a scalar-result call with a record argument");
    };
    assert_eq!(*callee, id::<MachineId>(2));
    assert_eq!(arguments, &[five]);
    assert_eq!(
        structural_arguments,
        &[StructuralArgument::whole(argument, StructuralAccess::Owned)]
    );
    // The invocation row keeps the authored order across both lanes.
    let invocation = tables.invocations.get(id(1)).unwrap();
    assert_eq!(
        invocation.actuals,
        [
            Actual::Place(*tables.roots.get(argument).unwrap()),
            Actual::Value(five)
        ]
    );
    // The consumed home is retired on the edge into the continuation, which
    // returns the call's result.
    let Terminator::Jump {
        target,
        cleanup_actions,
        ..
    } = &entry.terminator
    else {
        panic!("continuation edge");
    };
    assert_eq!(
        cleanup_actions,
        &[terminal_psi::TerminalAffineCleanupAction::RetireHome(
            argument
        )]
    );
    let result = called.result.scalar().unwrap().id;
    assert!(matches!(
        block(&machine, target.get()).terminator,
        Terminator::Return { value, .. } if value == result
    ));
}

#[test]
fn a_projected_record_is_copied_out_by_its_path() {
    // ends(l: Line) -> Point { l.to }
    let body = Body {
        slots: vec![line()],
        statements: vec![returns(member(place(0, line()), 1, point()))],
    };
    let (machine, tables) = lower_places(body, vec![shaped(vec![line()], Some(point()))]);
    let [copied] = block(&machine, 1).operations.as_slice() else {
        panic!("one copy");
    };
    let formal = machine.structural_parameters[0].place;
    let OperationKind::StructuralLeafCopy { source, path } = &copied.kind else {
        panic!("copy");
    };
    assert_eq!((*source, path.len()), (formal, 1));
    // The copy logs one row on its fresh root; its operation names the
    // source place by root and path. Naming `l.to` mints the entry content
    // of each leaf beneath it.
    let copy = produced(copied);
    assert_eq!(
        written(&tables, *tables.roots.get(copy).unwrap()),
        [(
            Content::Overwritten,
            ProgramPoint::AfterOperation(copied.id)
        )]
    );
    for field in [1, 2] {
        let from = projected(&tables, formal, &[4, field]);
        let values = tables
            .values
            .iter()
            .filter(|(_, row)| row.origin == ValueOrigin::EntryContent { place: from });
        assert_eq!(values.count(), 1);
    }
}

#[test]
fn a_module_with_records_declares_them_and_encodes() {
    // make(a: u32) -> Line { Line { to: Point { x: a, y: 7 }, from: ... } }
    let inner = |x: Expression| literal(vec![(0, x), (1, integer(7))], point());
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    bodies.push(Body {
        slots: vec![u32_type().into()],
        statements: vec![returns(literal(
            vec![(1, inner(slot(0, u32_type()))), (0, inner(integer(1)))],
            line(),
        ))],
    });
    let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
    signatures.push(shaped(vec![u32_type().into()], Some(line())));
    let elaboration = crate::lower_bodies(id(1), &bodies, &signatures, &records()).unwrap();
    let module = &elaboration.module;
    assert_eq!(module.structural_types.len(), 2);
    assert_eq!(module.native_layouts.records().count(), 2);
    assert!(
        module
            .structural_types
            .iter()
            .all(|declaration| declaration.copyable)
    );
    terminal_codec::encode_module(module).unwrap();
    // A scalar module declares nothing.
    let mut scalar_bodies: IdTable<MachineId, Body> = IdTable::default();
    scalar_bodies.push(Body {
        slots: Vec::new(),
        statements: vec![returns(integer(1))],
    });
    let mut scalar_signatures: IdTable<MachineId, Signature> = IdTable::default();
    scalar_signatures.push(signature(Vec::new(), Some(u32_type())));
    let scalar = crate::lower_bodies(
        id(1),
        &scalar_bodies,
        &scalar_signatures,
        &IdTable::default(),
    )
    .unwrap();
    assert!(scalar.module.structural_types.is_empty());
    assert_eq!(scalar.module.native_layouts.records().count(), 0);
}

fn store(slot: usize, path: &[usize], value: Expression) -> Statement {
    Statement::Store {
        slot,
        path: path.to_vec(),
        value,
        source: nowhere(),
    }
}

fn declare(slot: usize, home: Option<&str>) -> Statement {
    Statement::Declare {
        slot,
        home: home.map(str::to_owned),
        source: nowhere(),
    }
}

#[test]
fn a_field_store_is_one_operation_and_one_row_on_its_exact_leaf() {
    // f() -> u32 {
    //     let p = Point { x: 1, y: 2 };
    //     let mut l = Line { from: p, to: p };
    //     l.from.x = 5;
    //     l.to = p;
    //     l.from.x
    // }
    let body = Body {
        slots: vec![point(), line()],
        statements: vec![
            bind(0, literal(vec![(0, integer(1)), (1, integer(2))], point())),
            bind(
                1,
                literal(vec![(0, place(0, point())), (1, place(0, point()))], line()),
            ),
            store(1, &[0, 0], integer(5)),
            store(1, &[1], place(0, point())),
            returns(member(
                member(place(1, line()), 0, point()),
                0,
                u32_type().into(),
            )),
        ],
    };
    let (machine, tables) = lower_places(body, vec![shaped(Vec::new(), Some(u32_type().into()))]);
    let operations = &block(&machine, 1).operations;
    let kinds: Vec<_> = operations
        .iter()
        .map(|operation| match &operation.kind {
            OperationKind::IntegerConstant { .. } => "constant",
            OperationKind::EstablishRecord { .. } => "establish",
            OperationKind::StructuralLeafCopy { .. } => "copy",
            OperationKind::StructuralScalarFieldStore { .. } => "store",
            OperationKind::StoreStructuralField { .. } => "replace",
            OperationKind::ScalarStructuralField { .. } => "read",
            other => panic!("unexpected operation {other:?}"),
        })
        .collect();
    // The stored value is evaluated before its store, and the record value
    // is constructed (here copied whole) before it replaces the field.
    assert_eq!(
        kinds,
        [
            "constant",
            "constant",
            "establish",
            "copy",
            "copy",
            "establish",
            "constant",
            "store",
            "copy",
            "replace",
            "read"
        ]
    );
    let l = produced(&operations[5]);
    let five = operations[6].result.scalar().unwrap().id;
    let after = |position: usize| ProgramPoint::AfterOperation(operations[position].id);
    // `l.from.x = 5`: the path is the carrier `from`, the field the leaf
    // `x`, and a plain field carries no range obligation.
    assert!(matches!(
        &operations[7].kind,
        OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            value,
            range_obligation: None,
        } if *destination == l && path.len() == 1 && *field == id(1) && *value == five
    ));
    assert!(matches!(
        operations[7].result,
        terminal_psi::OperationResult::Unit
    ));
    // One row, on exactly `l.from.x`. Its sibling `l.from.y` is not even a
    // place yet, and `l.from` keeps the one row its construction wrote.
    assert_eq!(
        written(&tables, projected(&tables, l, &[3, 1])),
        [(Content::Value(five), after(7))]
    );
    let from = projected(&tables, l, &[3]);
    assert_eq!(tables.places.get(from).unwrap().children.len(), 1);
    assert_eq!(written(&tables, from), [(Content::Overwritten, after(5))]);
    // `l.to = p`: the ordinary structural replacement of the field by an
    // already constructed root, not member stores. Its row says only that
    // `l.to` has new contents; the operation names where they came from.
    let copied = produced(&operations[8]);
    assert!(matches!(
        &operations[9].kind,
        OperationKind::StoreStructuralField {
            disposition: terminal_psi::StructuralStoreDisposition::Vacant,
            destination,
            path,
            field: Some(field),
            value,
        } if *destination == l && path.is_empty() && *field == id(4)
            && value.place == copied && value.path.is_empty()
    ));
    let to = projected(&tables, l, &[4]);
    assert_eq!(
        written(&tables, to),
        [
            (Content::Overwritten, after(5)),
            (Content::Overwritten, after(9))
        ]
    );
    assert!(tables.places.get(to).unwrap().children.is_empty());
    // No store mints an obligation.
    assert!(tables.obligations.is_empty());
}

#[test]
fn a_whole_assignment_constructs_its_value_then_names_the_fresh_root() {
    // f(a: u32) -> u32 { let mut p = Point { x: 1, y: 2 }; p = Point { x: p.y, y: a }; p.x }
    let body = Body {
        slots: vec![u32_type().into(), point()],
        statements: vec![
            bind(1, literal(vec![(0, integer(1)), (1, integer(2))], point())),
            bind(
                1,
                literal(
                    vec![
                        (0, member(place(1, point()), 1, u32_type().into())),
                        (1, slot(0, u32_type())),
                    ],
                    point(),
                ),
            ),
            returns(member(place(1, point()), 0, u32_type().into())),
        ],
    };
    let (machine, tables) = lower_places(
        body,
        vec![shaped(vec![u32_type().into()], Some(u32_type().into()))],
    );
    let operations = &block(&machine, 1).operations;
    let (first, second) = (produced(&operations[2]), produced(&operations[4]));
    assert_ne!(first, second);
    // The operand `p.y` reads the old root, before the new one exists.
    let old_y = operations[3].result.scalar().unwrap().id;
    assert!(matches!(
        &operations[3].kind,
        OperationKind::ScalarStructuralField { source, field, .. }
            if *source == first && *field == id(2)
    ));
    // The new root's leaves hold exactly the constructed operands.
    let after = ProgramPoint::AfterOperation(operations[4].id);
    assert_eq!(
        written(&tables, projected(&tables, second, &[1])),
        [(Content::Value(old_y), after)]
    );
    assert_eq!(
        written(&tables, projected(&tables, second, &[2])),
        [(Content::Value(id(1)), after)]
    );
    // From the assignment on, the local names the new root.
    assert!(matches!(
        &operations[5].kind,
        OperationKind::ScalarStructuralField { source, field, .. }
            if *source == second && *field == id(1)
    ));
    // Nothing was written to the old root after its construction.
    assert_eq!(written(&tables, projected(&tables, first, &[1])).len(), 1);
}

#[test]
fn a_scalar_local_without_an_initializer_logs_nothing_and_a_data_local_logs_its_zero() {
    // f() -> u32 {
    //     let mut x: u32;  let mut p: Point;
    //     x = 3;  p.x = x;
    //     x
    // }
    let body = || Body {
        slots: vec![u32_type().into(), point()],
        statements: vec![
            declare(0, Some("named(name(u32))")),
            declare(1, None),
            store(0, &[], integer(3)),
            store(1, &[0], slot(0, u32_type())),
            returns(slot(0, u32_type())),
        ],
    };
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    bodies.push(body());
    let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
    signatures.push(shaped(Vec::new(), Some(u32_type().into())));
    let elaboration = crate::lower_bodies(id(1), &bodies, &signatures, &records()).unwrap();
    let machine = &elaboration.module.machines[0];
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let operations = &block(machine, 1).operations;
    let kinds: Vec<_> = operations
        .iter()
        .map(|operation| match &operation.kind {
            OperationKind::IntegerConstant { .. } => "constant",
            OperationKind::EstablishPrimitiveLocal { .. } => "home",
            OperationKind::EstablishZeroedStructural {} => "zeroed",
            OperationKind::WriteOnlyPrimitiveStore { .. } => "write",
            OperationKind::PrimitiveScalarRead { .. } => "read",
            OperationKind::StructuralScalarFieldStore { .. } => "store",
            other => panic!("unexpected operation {other:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "constant", "home", "constant", "constant", "zeroed", "constant", "write", "read",
            "store", "read"
        ]
    );
    let (x, p) = (produced(&operations[1]), produced(&operations[4]));
    // The scalar home has the primitive structural type declared after the
    // two records, and the module with it still encodes.
    let home = operations[1]
        .result
        .as_structural()
        .unwrap()
        .structural_type;
    assert_eq!(home, id(3));
    assert!(matches!(
        elaboration.module.structural_types[2].shape,
        terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type)
            if scalar_type == u32_type()
    ));
    terminal_codec::encode_module(&elaboration.module).unwrap();
    // The scalar declaration writes no row: the zero the module stores so
    // that it runs is not content the checker may rely on.
    let three = operations[5].result.scalar().unwrap().id;
    let x_root = *tables.roots.get(x).unwrap();
    assert_eq!(
        written(tables, x_root),
        [(
            Content::Value(three),
            ProgramPoint::AfterOperation(operations[6].id)
        )]
    );
    // The data declaration is zero-established: one row for the root's new
    // contents, then on each scalar leaf the zero constant emitted for it
    // just before the establishment. (Until the owner settled the data half
    // of `local-without-initializer`, it wrote no row.)
    let zeroed = ProgramPoint::AfterOperation(operations[4].id);
    assert_eq!(
        written(tables, *tables.roots.get(p).unwrap()),
        [(Content::Overwritten, zeroed)]
    );
    let zero = |position: usize| operations[position].result.scalar().unwrap().id;
    assert_eq!(
        written(tables, projected(tables, p, &[2])),
        [(Content::Value(zero(3)), zeroed)]
    );
    // Each use of `x` is a fresh read of its home, and the store into `p.x`
    // stores that read's value after the zero the declaration logged.
    let read = operations[7].result.scalar().unwrap().id;
    assert!(matches!(
        &operations[7].kind,
        OperationKind::PrimitiveScalarRead { source, path } if *source == x && path.is_empty()
    ));
    assert_eq!(
        written(tables, projected(tables, p, &[1])),
        [
            (Content::Value(zero(2)), zeroed),
            (
                Content::Value(read),
                ProgramPoint::AfterOperation(operations[8].id)
            )
        ]
    );
    assert_eq!(tables.writes.len(), 5);
    // `Point` has no `requires`, so its zero establishment owes nothing.
    assert!(tables.obligations.is_empty());
    // A scalar home with no type identity cannot be declared.
    let mut nameless: IdTable<MachineId, Body> = IdTable::default();
    nameless.push(Body {
        statements: vec![declare(0, None), returns(integer(1))],
        ..body()
    });
    let refusal = crate::lower_bodies(id(1), &nameless, &signatures, &records()).unwrap_err();
    assert_eq!(
        refusal.reason,
        terminal_check_tables::RefusalReason::Unsupported
    );
    assert_eq!(
        refusal.detail,
        "spelling scalar home without a type identity"
    );
}

// Sums by structure. `Shape` is type 2 beside `Point`: `Dot(at: u32)` is case
// 1 with field 3, `Pair(l: u32, r: u32)` case 2 with fields 4 and 5,
// `Boxed(inner: Point)` case 3 with field 6, and `Empty` case 4.

use semantic_vocabulary::StructuralCaseId;
use terminal_check_tables::RefusalReason;
use terminal_psi::{StructuralPathSegment, StructuralTypeShape};

use crate::body::CaseType;

const DOT: usize = 0;
const PAIR: usize = 1;
const BOXED: usize = 2;
const EMPTY: usize = 3;

fn shape() -> ValueType {
    ValueType::Record(id(2))
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
    records
}

fn case_of(case: usize, fields: Vec<(usize, Expression)>) -> Expression {
    of_type(ExpressionKind::Case { case, fields }, shape())
}

fn payload(base: Expression, case: usize, field: usize, result: ValueType) -> Expression {
    of_type(
        ExpressionKind::Payload {
            base: Box::new(base),
            case,
            field,
        },
        result,
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

fn try_lower_sums(
    body: Body,
    signatures: Vec<Signature>,
) -> terminal_check_tables::Checked<crate::lower::Lowered> {
    let mut signature_table: IdTable<MachineId, Signature> = IdTable::default();
    let mut outcome_table: IdTable<MachineId, Outcome> = IdTable::default();
    for signature in signatures {
        signature_table.push(signature);
        outcome_table.push(returns_normally());
    }
    lower_records(body, &signature_table, &outcome_table, &sums())
}

fn lower_sums(body: Body, signatures: Vec<Signature>) -> (TerminalMachine, MachineTables) {
    let lowered = try_lower_sums(body, signatures).unwrap();
    assert_module_and_tables_agree(&lowered.machine, &lowered.tables);
    (lowered.machine, lowered.tables)
}

/// The place reached from root `root` by `segments`, which must be interned.
fn projected_by(tables: &MachineTables, root: PlaceId, segments: &[Segment]) -> ExactPlaceId {
    let mut place = *tables.roots.get(root).unwrap();
    for segment in segments {
        let children = &tables.places.get(place).unwrap().children;
        let found = children
            .binary_search_by_key(segment, |(segment, _)| *segment)
            .unwrap();
        place = children[found].1;
    }
    place
}

fn case_segment(raw: u64) -> Segment {
    Segment::Case(id::<StructuralCaseId>(raw))
}

fn field_segment(raw: u64) -> Segment {
    Segment::Field(id::<StructuralFieldId>(raw))
}

#[test]
fn a_case_literal_evaluates_its_operands_in_authored_order_then_establishes_once() {
    // make() -> Shape { Shape::Pair { r: g(), l: 3 } }: `r`'s operand is
    // authored first, so the call runs before the constant, and the one
    // establishment follows both with its roster in declaration order.
    let body = Body {
        slots: Vec::new(),
        statements: vec![returns(case_of(
            PAIR,
            vec![(1, call(2, Vec::new(), Some(u32_type()))), (0, integer(3))],
        ))],
    };
    let (machine, tables) = lower_sums(
        body,
        vec![
            shaped(Vec::new(), Some(shape())),
            signature(Vec::new(), Some(u32_type())),
        ],
    );
    let entry = block(&machine, 1);
    let [called, constant, established] = entry.operations.as_slice() else {
        panic!("three operations");
    };
    assert!(matches!(called.kind, OperationKind::Call { .. }));
    let r = called.result.scalar().unwrap().id;
    let l = constant.result.scalar().unwrap().id;
    let OperationKind::EstablishStructuralCase {
        result_case,
        fields,
    } = &established.kind
    else {
        panic!("case establishment");
    };
    assert_eq!(*result_case, id(2));
    let roster: Vec<_> = fields
        .iter()
        .map(|field| match field.value {
            RecordFieldValue::Scalar {
                value,
                range_obligation: None,
            } => (field.field, value),
            _ => panic!("plain scalar initializer"),
        })
        .collect();
    assert_eq!(roster, [(id(4), l), (id(5), r)]);
    // The root is written whole, then each payload leaf with its operand,
    // all at the establishment. No row names the case: the operation does.
    let root = produced(established);
    let after = ProgramPoint::AfterOperation(established.id);
    assert_eq!(
        written(&tables, projected_by(&tables, root, &[])),
        [(Content::Overwritten, after)]
    );
    let leaf = |field| projected_by(&tables, root, &[case_segment(2), field_segment(field)]);
    assert_eq!(written(&tables, leaf(4)), [(Content::Value(l), after)]);
    assert_eq!(written(&tables, leaf(5)), [(Content::Value(r), after)]);
    assert_eq!(tables.writes.len(), 3);

    // A case without payload is `EstablishScalarCase` with no field, and
    // one row on its root.
    let (machine, tables) = lower_sums(
        Body {
            slots: Vec::new(),
            statements: vec![returns(case_of(EMPTY, Vec::new()))],
        },
        vec![shaped(Vec::new(), Some(shape()))],
    );
    let [established] = block(&machine, 1).operations.as_slice() else {
        panic!("one operation");
    };
    assert!(matches!(
        &established.kind,
        OperationKind::EstablishScalarCase { result_case, fields }
            if *result_case == id(4) && fields.is_empty()
    ));
    assert_eq!(tables.writes.len(), 1);
}

/// `weigh(s: Shape) -> u32 { transition s { arms } }` over the formal.
fn weigh(arms: Vec<(usize, Expression)>, default: Option<Expression>) -> Body {
    let arm = |value: Expression| (Target::Value(value), nowhere());
    Body {
        slots: vec![shape()],
        statements: vec![Statement::Dispatch {
            subject: place(0, shape()),
            arms: arms
                .into_iter()
                .map(|(case, value)| (case, Target::Value(value), nowhere()))
                .collect(),
            default: default.map(arm),
            source: nowhere(),
        }],
    }
}

fn weighs() -> Vec<Signature> {
    vec![shaped(vec![shape()], Some(u32_type().into()))]
}

#[test]
fn a_dispatch_lists_every_declared_case_and_binds_scalar_payloads_as_block_parameters() {
    // transition s { Pair { l, r } -> r   Dot { at } -> at   _ -> 0 }
    let scalar = ValueType::Scalar(u32_type());
    let subject = || place(0, shape());
    let (machine, tables) = lower_sums(
        weigh(
            vec![
                (PAIR, payload(subject(), PAIR, 1, scalar)),
                (DOT, payload(subject(), DOT, 0, scalar)),
            ],
            Some(integer(0)),
        ),
        weighs(),
    );
    let entry = block(&machine, 1);
    assert!(entry.operations.is_empty(), "the subject is not copied");
    let Terminator::StructuralCase {
        source,
        path,
        cases,
    } = &entry.terminator
    else {
        panic!("dispatch");
    };
    assert_eq!(*source, id::<PlaceId>(1));
    assert!(path.is_empty());
    // One edge per declared case, in declaration order, whatever order the
    // arms were authored in; the cases no arm names share the wildcard's
    // block.
    let roster: Vec<_> = cases
        .iter()
        .map(|edge| (edge.case, edge.target, edge.payload_fields.clone()))
        .collect();
    let (pair, dot, rest) = (id::<BlockId>(2), id::<BlockId>(3), id::<BlockId>(4));
    assert_eq!(
        roster,
        [
            (id(1), dot, vec![id(3)]),
            (id(2), pair, vec![id(4), id(5)]),
            (id(3), rest, Vec::new()),
            (id(4), rest, Vec::new()),
        ]
    );
    // Each scalar payload field is one parameter of its arm's block, and
    // the arm's value is that parameter: no read is emitted.
    for (arm, bound, returned) in [(pair, 2, 1), (dot, 1, 0)] {
        let arm = block(&machine, arm.get());
        assert_eq!(arm.parameters.len(), bound);
        assert!(arm.operations.is_empty());
        let parameter = arm.parameters[returned].id;
        assert_eq!(
            tables.values.get(parameter).unwrap().origin,
            ValueOrigin::BlockParameter {
                block: arm.id,
                position: returned as u32,
            }
        );
        assert!(matches!(
            arm.terminator,
            Terminator::Return { value, .. } if value == parameter
        ));
    }
    assert!(block(&machine, rest.get()).parameters.is_empty());

    // A declared case with no arm and no wildcard is a compile error of
    // the program, reported where the roster is built.
    let missing = try_lower_sums(
        weigh(
            vec![(PAIR, integer(1)), (DOT, integer(2)), (BOXED, integer(3))],
            None,
        ),
        weighs(),
    )
    .unwrap_err();
    assert_eq!(missing.reason, RefusalReason::Unestablished);
    assert_eq!(
        missing.detail,
        "a dispatch selects no arm for a declared case"
    );
    // A wildcard no case reaches would never be lowered: refused by name.
    let dead = try_lower_sums(
        weigh(
            vec![
                (PAIR, integer(1)),
                (DOT, integer(2)),
                (BOXED, integer(3)),
                (EMPTY, integer(4)),
            ],
            Some(integer(5)),
        ),
        weighs(),
    )
    .unwrap_err();
    assert_eq!(dead.reason, RefusalReason::Unsupported);
    assert_eq!(dead.detail, "transition wildcard arm that no case reaches");
}

#[test]
fn a_value_match_joins_its_arms_with_the_result_as_one_block_parameter() {
    // f(s: Shape) -> u32 { (match s { Dot { at } -> at, _ -> 7 }) & 1 }
    let scalar = ValueType::Scalar(u32_type());
    let matched = of_type(
        ExpressionKind::Match {
            subject: Box::new(place(0, shape())),
            arms: vec![(DOT, payload(place(0, shape()), DOT, 0, scalar))],
            default: Some(Box::new(integer(7))),
        },
        scalar,
    );
    let (machine, tables) = lower_sums(
        Body {
            slots: vec![shape()],
            statements: vec![returns(strict(
                Strict::BitwiseAnd,
                matched,
                integer(1),
                u32_type(),
            ))],
        },
        weighs(),
    );
    assert!(matches!(
        block(&machine, 1).terminator,
        Terminator::StructuralCase { .. }
    ));
    // Blocks: 2 the `Dot` arm, 3 the wildcard arm, 4 the join.
    let join = block(&machine, 4);
    let [result] = join.parameters.as_slice() else {
        panic!("one result parameter");
    };
    assert_eq!(
        tables.values.get(result.id).unwrap().origin,
        ValueOrigin::BlockParameter {
            block: join.id,
            position: 0,
        }
    );
    let arrival = |raw| match &block(&machine, raw).terminator {
        Terminator::Jump {
            target, arguments, ..
        } => {
            assert_eq!(*target, join.id);
            arguments.clone()
        }
        other => panic!("an arm jumps to the join, found {other:?}"),
    };
    // The `Dot` arm passes its payload parameter, the wildcard its constant.
    assert_eq!(arrival(2), [block(&machine, 2).parameters[0].id]);
    let seven = block(&machine, 3).operations[0].result.scalar().unwrap().id;
    assert_eq!(arrival(3), [seven]);
    // What follows the match runs in the join and uses its parameter.
    assert!(matches!(
        join.operations.last().unwrap().kind,
        OperationKind::IntegerBitwiseAnd { left, .. } if left == result.id
    ));
}

#[test]
fn a_case_test_observes_the_sum_in_place_and_a_payload_read_ends_its_carrier_in_the_case() {
    // f(s: Shape) -> u32 { transition s in Shape::Dot { true -> s.at } 9 }
    let scalar = ValueType::Scalar(u32_type());
    let (machine, _) = lower_sums(
        Body {
            slots: vec![shape()],
            statements: vec![
                Statement::Arm {
                    guard: Some(is_case(place(0, shape()), DOT)),
                    target: Target::Value(payload(place(0, shape()), DOT, 0, scalar)),
                    source: nowhere(),
                },
                returns(integer(9)),
            ],
        },
        weighs(),
    );
    let [test] = block(&machine, 1).operations.as_slice() else {
        panic!("one operation");
    };
    assert!(matches!(
        &test.kind,
        OperationKind::StructuralCaseMembership { source, path, case }
            if *source == id::<PlaceId>(1) && path.is_empty() && *case == id(1)
    ));
    // Outside a dispatch on that place the payload is read, and the read's
    // carrier is the one `Case` step.
    let [read] = block(&machine, 2).operations.as_slice() else {
        panic!("one read");
    };
    assert!(matches!(
        &read.kind,
        OperationKind::ScalarStructuralField { source, path, field }
            if *source == id::<PlaceId>(1)
                && path.as_slice() == [CanonicalStructuralPathSegment::Case(id(1))]
                && *field == id(3)
    ));
}

#[test]
fn a_data_payload_is_copied_out_by_its_case_and_field() {
    // f(s: Shape) -> Point { transition s { Boxed { inner } -> inner  _ -> .. } }
    let (machine, _) = lower_sums(
        Body {
            slots: vec![shape()],
            statements: vec![Statement::Dispatch {
                subject: place(0, shape()),
                arms: vec![(
                    BOXED,
                    Target::Value(payload(place(0, shape()), BOXED, 0, point())),
                    nowhere(),
                )],
                default: Some((
                    Target::Value(literal(vec![(0, integer(0)), (1, integer(0))], point())),
                    nowhere(),
                )),
                source: nowhere(),
            }],
        },
        vec![shaped(vec![shape()], Some(point()))],
    );
    // A data payload field is no block parameter; the arm copies it whole.
    let arm = block(&machine, 2);
    assert!(arm.parameters.is_empty());
    let [copy] = arm.operations.as_slice() else {
        panic!("one copy");
    };
    let OperationKind::StructuralLeafCopy { source, path } = &copy.kind else {
        panic!("whole copy");
    };
    // Root 1 is the formal, root 2 the machine's result place.
    assert_eq!(*source, id::<PlaceId>(1));
    assert!(matches!(
        path.as_slice(),
        [StructuralPathSegment::Case(case), StructuralPathSegment::Field(_)] if *case == id(3)
    ));
}

#[test]
fn each_sum_construct_outside_the_slice_is_refused_by_name_in_lowering() {
    let scalar = ValueType::Scalar(u32_type());
    // A crash inside a case arm: its site guard could not state the case.
    let crash = try_lower_sums(
        Body {
            slots: vec![shape()],
            statements: vec![Statement::Dispatch {
                subject: place(0, shape()),
                arms: vec![(DOT, Target::Crash(CrashCause::Trap), nowhere())],
                default: Some((Target::Value(integer(0)), nowhere())),
                source: nowhere(),
            }],
        },
        weighs(),
    )
    .unwrap_err();
    assert_eq!(crash.reason, RefusalReason::Unsupported);
    assert_eq!(crash.detail, "transition crash in a case arm");
    // A field read through a data payload would put the `Case` step inside
    // the read's carrier.
    let inner = payload(place(0, shape()), BOXED, 0, point());
    let beneath = try_lower_sums(
        Body {
            slots: vec![shape()],
            statements: vec![returns(member(inner, 0, scalar))],
        },
        weighs(),
    )
    .unwrap_err();
    assert_eq!(beneath.reason, RefusalReason::Unsupported);
    assert_eq!(
        beneath.detail,
        "expression field read beneath a case payload"
    );
}

#[test]
fn a_module_with_a_sum_declares_its_cases_and_encodes() {
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    bodies.push(Body {
        slots: vec![u32_type().into()],
        statements: vec![returns(case_of(DOT, vec![(0, slot(0, u32_type()))]))],
    });
    let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
    signatures.push(shaped(vec![u32_type().into()], Some(shape())));
    let elaboration = crate::lower_bodies(id(1), &bodies, &signatures, &sums()).unwrap();
    let module = &elaboration.module;
    let StructuralTypeShape::Sum { cases } = &module.structural_types[1].shape else {
        panic!("a sum");
    };
    let declared: Vec<_> = cases
        .iter()
        .map(|case| {
            (
                case.id,
                case.fields.iter().map(|field| field.id).collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(
        declared,
        [
            (id(1), vec![id(3)]),
            (id(2), vec![id(4), id(5)]),
            (id(3), vec![id(6)]),
            (id(4), Vec::new()),
        ]
    );
    // The decoder wants a layout row per record and none per sum.
    assert_eq!(module.native_layouts.records().count(), 1);
    terminal_codec::encode_module(module).unwrap();
}

// Data `requires`: lowering mints one `DataDomain` obligation per clause an
// operation owes, before the operation.

/// `Point` with `requires x <= y`, `Line` with `requires from.x <= to.x`
/// and with a second clause `from.y <= to.y`, and `Shape` unconstrained.
fn constrained() -> IdTable<StructuralTypeId, RecordType> {
    let own = |fields: &[u64]| ScalarTerm::IntegerField {
        root: crate::self_place().unwrap(),
        path: fields
            .iter()
            .map(|field| CanonicalStructuralPathSegment::Field(id(*field)))
            .collect(),
        scalar_type: match u32_type() {
            ScalarType::Integer(integer) => integer,
            other => panic!("u32 is an integer, found {other:?}"),
        },
    };
    let mut records = records();
    records.get_mut(id(1)).unwrap().requires = vec![Proposition::LessOrEqual(own(&[1]), own(&[2]))];
    records.get_mut(id(2)).unwrap().requires = vec![
        Proposition::LessOrEqual(own(&[3, 1]), own(&[4, 1])),
        Proposition::LessOrEqual(own(&[3, 2]), own(&[4, 2])),
    ];
    records
}

#[test]
fn each_operation_that_forms_or_changes_a_constrained_value_owes_its_clauses() {
    // f() -> u32 {
    //     let mut l: Line;                       // zero: Line 2, from 1, to 1
    //     let p: Point = Point { x: 1, y: 2 };   // Point 1
    //     l.to = p;                              // Line 2
    //     l.from.x = 1;                          // Point 1, then Line 2
    //     1
    // }
    let body = Body {
        slots: vec![line(), point()],
        statements: vec![
            declare(0, None),
            bind(1, literal(vec![(0, integer(1)), (1, integer(2))], point())),
            store(0, &[1], place(1, point())),
            store(0, &[0, 0], integer(1)),
            returns(integer(1)),
        ],
    };
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    bodies.push(body);
    let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
    signatures.push(shaped(Vec::new(), Some(u32_type().into())));
    let elaboration = crate::lower_bodies(id(1), &bodies, &signatures, &constrained()).unwrap();
    // The clauses travel beside the module, by the type's identity.
    assert_eq!(elaboration.domains.get(id(1)).unwrap().len(), 1);
    assert_eq!(elaboration.domains.get(id(2)).unwrap().len(), 2);
    let machine = &elaboration.module.machines[0];
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let operations = &block(machine, 1).operations;
    let owing = |select: fn(&OperationKind) -> bool| {
        let operation = operations
            .iter()
            .find(|operation| select(&operation.kind))
            .unwrap()
            .id;
        let before = operations
            .iter()
            .position(|candidate| candidate.id == operation)
            .unwrap();
        let clauses: Vec<u32> = tables
            .obligations
            .iter()
            .filter_map(|(_, row)| match row.kind {
                ObligationKind::DataDomain {
                    operation: owner,
                    clause,
                } if owner == operation => {
                    // Created at the point before the operation, never
                    // after it.
                    assert_eq!(
                        row.created_at,
                        ProgramPoint::AfterOperation(operations[before - 1].id)
                    );
                    Some(clause)
                }
                _ => None,
            })
            .collect();
        clauses
    };
    assert_eq!(
        owing(|kind| matches!(kind, OperationKind::EstablishZeroedStructural {})),
        [0, 1, 2, 3]
    );
    assert_eq!(
        owing(|kind| matches!(kind, OperationKind::EstablishRecord { .. })),
        [0]
    );
    assert_eq!(
        owing(|kind| matches!(kind, OperationKind::StoreStructuralField { .. })),
        [0, 1]
    );
    assert_eq!(
        owing(|kind| matches!(kind, OperationKind::StructuralScalarFieldStore { .. })),
        [0, 1, 2]
    );
    assert_eq!(tables.obligations.len(), 10);
    terminal_codec::encode_module(&elaboration.module).unwrap();
}

#[test]
fn an_entry_clause_over_a_record_formals_field_encodes_in_the_module_contract() {
    // peek(p: Point) -> u32 requires p.x == 7 { p.y }
    // The clause is a field term rooted at the formal's place, the first
    // place of the machine, and the module that carries it encodes.
    let field = ScalarTerm::IntegerField {
        root: id(1),
        path: vec![CanonicalStructuralPathSegment::Field(id(1))],
        scalar_type: match u32_type() {
            ScalarType::Integer(integer) => integer,
            other => panic!("u32 is an integer, found {other:?}"),
        },
    };
    let seven = ScalarTerm::integer(
        match u32_type() {
            ScalarType::Integer(integer) => integer,
            other => panic!("u32 is an integer, found {other:?}"),
        },
        semantic_vocabulary::IntegerValue::Unsigned(7),
    )
    .unwrap();
    let clause = crate::canonical::equal(field, seven, id(1)).unwrap();
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    bodies.push(Body {
        slots: vec![point()],
        statements: vec![returns(member(place(0, point()), 1, u32_type().into()))],
    });
    let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
    signatures.push(Signature {
        requires: vec![clause.clone()],
        ..shaped(vec![point()], Some(u32_type().into()))
    });
    let elaboration = crate::lower_bodies(id(1), &bodies, &signatures, &records()).unwrap();
    let machine = &elaboration.module.machines[0];
    assert_eq!(machine.contract.requires, [clause]);
    assert_eq!(machine.structural_parameters[0].place, id::<PlaceId>(1));
    terminal_codec::encode_module(&elaboration.module).unwrap();
}
