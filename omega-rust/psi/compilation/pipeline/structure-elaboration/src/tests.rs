// Lowering tests build closed bodies directly, so each one states the exact
// scalar program it lowers; the reader's refusals are tested in `read.rs`.

use semantic_vocabulary::{
    BlockId, IdTable, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId, Proposition,
    PsiSemanticId, ScalarTerm, ScalarType, ValueId,
};
use terminal_check_tables::edges::Destination;
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{MachineTables, ProgramPoint};
use terminal_psi::{OperationKind, TerminalMachine, Terminator};

use crate::body::{Body, Expression, ExpressionKind, Signature, Statement, Strict, Target};
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
    }
}

/// Lowers machine 1 against `signatures` (machine 1 first).
fn lower(body: Body, signatures: Vec<Signature>) -> (TerminalMachine, MachineTables) {
    let mut table: IdTable<MachineId, Signature> = IdTable::default();
    for signature in signatures {
        table.push(signature);
    }
    let lowered = crate::lower::machine(id(1), &body, &table).unwrap();
    assert_module_and_tables_agree(&lowered.machine, &lowered.tables);
    (lowered.machine, lowered.tables)
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
            | Terminator::ReturnUnit { edge, .. } => vec![*edge],
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
    let mut table: IdTable<MachineId, Signature> = IdTable::default();
    table.push(signature(Vec::new(), Some(u32_type())));
    let refusal = crate::lower::machine(id(1), &body, &table).unwrap_err();
    assert_eq!(
        refusal.reason,
        terminal_check_tables::RefusalReason::Unsupported
    );
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
