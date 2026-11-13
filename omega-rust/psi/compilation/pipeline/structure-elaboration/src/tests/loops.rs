// Call cycles lowered as loops. Each test states its machines as closed
// bodies and lowers the whole closure, because a cycle is a property of the
// call graph and not of one body.

use terminal_psi::{ScalarBlockInvariant, TerminalModule};

use super::*;
use crate::Elaboration;

fn guarded(guard: Expression, value: Expression) -> Statement {
    Statement::Arm {
        guard: Some(guard),
        target: Target::Value(value),
        source: nowhere(),
    }
}

fn positive(slot_index: usize) -> Expression {
    strict(
        Strict::Greater,
        slot(slot_index, u32_type()),
        integer(0),
        ScalarType::Boolean,
    )
}

fn less_one(slot_index: usize) -> Expression {
    strict(
        Strict::Subtract,
        slot(slot_index, u32_type()),
        integer(1),
        u32_type(),
    )
}

fn value(raw: u64) -> ScalarTerm {
    ScalarTerm::value(id(raw), u32_type())
}

fn zero() -> ScalarTerm {
    ScalarTerm::Integer {
        value: IntegerValue::Unsigned(0),
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
    }
}

fn elaborate(machines: Vec<(Body, Signature)>) -> terminal_check_tables::Checked<Elaboration> {
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
    for (body, signature) in machines {
        bodies.push(body);
        signatures.push(signature);
    }
    crate::lower_bodies(id(1), &bodies, &signatures, &IdTable::default())
}

fn machine(module: &TerminalModule, raw: u64) -> &TerminalMachine {
    &module.machines[usize::try_from(raw - 1).unwrap()]
}

/// `machine countdown(remaining: u32) -> u32 { remaining > 0 -> countdown(remaining - 1); 0 }`
fn countdown(signature: Signature) -> (Body, Signature) {
    (
        Body {
            slots: scalars(vec![u32_type()]),
            statements: vec![
                guarded(positive(0), call(1, vec![less_one(0)], Some(u32_type()))),
                returns(integer(0)),
            ],
        },
        signature,
    )
}

fn jump_of(block: &terminal_psi::Block) -> (BlockId, &[ValueId], semantic_vocabulary::EdgeId) {
    match &block.terminator {
        Terminator::Jump {
            target,
            arguments,
            edge,
            ..
        } => (*target, arguments, *edge),
        other => panic!("expected a jump, found {other:?}"),
    }
}

#[test]
fn a_machine_that_calls_itself_in_terminal_position_is_a_head_block_and_a_jump_back() {
    let elaboration = elaborate(vec![countdown(signature(
        vec![u32_type()],
        Some(u32_type()),
    ))])
    .unwrap();
    let lowered = machine(&elaboration.module, 1);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    assert_module_and_tables_agree(lowered, tables);
    // The entry holds the formal as machine parameter 1 and only jumps to
    // the head with it; the result pseudo-value is value 2.
    let entry = block(lowered, 1);
    assert!(entry.parameters.is_empty() && entry.operations.is_empty());
    let (head, entered, _) = jump_of(entry);
    assert_eq!((head, entered), (id(2), &[id::<ValueId>(1)][..]));
    // The head's one parameter stands for the formal on every arrival.
    let head = block(lowered, 2);
    let [remaining] = head.parameters.as_slice() else {
        panic!("the head has one parameter");
    };
    assert_eq!(
        tables.values.get(remaining.id).unwrap().origin,
        ValueOrigin::BlockParameter {
            block: id(2),
            position: 0
        }
    );
    // The body reads the parameter, never the machine's formal, and the
    // call is a jump back to the head carrying `remaining - 1`.
    let back = lowered
        .blocks
        .iter()
        .skip(2)
        .find_map(|block| match &block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } if *target == id(2) => Some((block, arguments.clone())),
            _ => None,
        })
        .expect("a jump back to the head");
    let [argument] = back.1.as_slice() else {
        panic!("the jump back carries one argument");
    };
    let subtract = back
        .0
        .operations
        .iter()
        .find(|operation| operation.result.scalar().map(|result| result.id) == Some(*argument))
        .expect("the argument is computed in the arm");
    assert!(matches!(
        &subtract.kind,
        OperationKind::ExactIntegerSubtract { left, .. } if *left == remaining.id
    ));
    // No call survives: the lowered form has none there.
    assert!(tables.invocations.is_empty());
    assert!(lowered.blocks.iter().all(|block| {
        block
            .operations
            .iter()
            .all(|operation| !matches!(operation.kind, OperationKind::Call { .. }))
    }));
    // A member with no `requires` has no declared invariant. Each jump into
    // its head still owes an arrival, as every edge that passes arguments
    // does; with no predicate on the head its goal is `Truth`.
    assert!(elaboration.module.scalar_block_invariants.is_empty());
    let jumps: Vec<_> = lowered
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            terminal_psi::Terminator::Jump { edge, .. } => {
                Some(ObligationKind::ArrivalInvariant { edge: *edge })
            }
            _ => None,
        })
        .collect();
    assert_eq!(jumps.len(), 2, "the entry's jump and the jump back");
    let mut expected = vec![ObligationKind::OperationSafety {
        site: terminal_check_tables::obligations::ExecutionSite::Operation(subtract.id),
    }];
    expected.extend(jumps);
    assert_eq!(obligations(tables), expected);
    assert!(lowered.ranked_scc.is_none());
}

#[test]
fn a_members_requires_is_its_heads_invariant_owed_on_every_arrival() {
    // machine countdown(remaining: u32) -> u32 requires 0 <= remaining
    let mut header = signature(vec![u32_type()], Some(u32_type()));
    header.requires = vec![Proposition::LessOrEqual(zero(), value(1))];
    let elaboration = elaborate(vec![countdown(header)]).unwrap();
    let lowered = machine(&elaboration.module, 1);
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let head = block(lowered, 2);
    let parameter = head.parameters[0].id;
    let (_, _, entering) = jump_of(block(lowered, 1));
    let back = lowered
        .blocks
        .iter()
        .skip(2)
        .find_map(|block| match &block.terminator {
            Terminator::Jump { target, edge, .. } if *target == id(2) => Some(*edge),
            _ => None,
        })
        .unwrap();
    // One row for the head: the clause over the head's parameter, and one
    // arrival per edge into it, in edge order.
    let [
        ScalarBlockInvariant {
            machine: owner,
            header: at,
            predicate,
            arrivals,
        },
    ] = elaboration.module.scalar_block_invariants.as_slice()
    else {
        panic!("one invariant row");
    };
    assert_eq!((*owner, *at), (id(1), id(2)));
    assert_eq!(
        predicate,
        &Proposition::LessOrEqual(zero(), ScalarTerm::value(parameter, u32_type()))
    );
    assert_eq!(
        arrivals.iter().map(|row| row.edge).collect::<Vec<_>>(),
        vec![entering, back]
    );
    for arrival in arrivals {
        let row = tables.obligations.get(arrival.obligation).unwrap();
        assert_eq!(
            row.kind,
            ObligationKind::ArrivalInvariant { edge: arrival.edge }
        );
        assert_eq!(row.created_at, ProgramPoint::Edge(arrival.edge));
    }
    // Every edge into a head with a row owes it: none is left out.
    let into_head = tables
        .edges
        .iter()
        .filter(|(_, row)| row.destination == Destination::Block(id(2)))
        .count();
    assert_eq!(into_head, arrivals.len());
    // The machine's own contract still states the clause over its formal.
    assert_eq!(
        lowered.contract.requires,
        vec![Proposition::LessOrEqual(zero(), value(1))]
    );
}

/// `machine left(n: u32) -> u32 { n > 0 -> right(n); 0 }`
/// `machine right(n: u32) -> u32 requires 0 < n { left(n - 1) }`
fn mutual() -> Vec<(Body, Signature)> {
    let mut right = signature(vec![u32_type()], Some(u32_type()));
    right.requires = vec![Proposition::LessThan(zero(), value(1))];
    vec![
        (
            Body {
                slots: scalars(vec![u32_type()]),
                statements: vec![
                    guarded(
                        positive(0),
                        call(2, vec![slot(0, u32_type())], Some(u32_type())),
                    ),
                    returns(integer(0)),
                ],
            },
            signature(vec![u32_type()], Some(u32_type())),
        ),
        (
            Body {
                slots: scalars(vec![u32_type()]),
                statements: vec![returns(call(1, vec![less_one(0)], Some(u32_type())))],
            },
            right,
        ),
    ]
}

#[test]
fn every_member_of_a_component_holds_the_whole_component_in_its_own_graph() {
    let elaboration = elaborate(mutual()).unwrap();
    for (raw, own_head) in [(1, 2), (2, 3)] {
        let lowered = machine(&elaboration.module, raw);
        let tables = elaboration.tables.machines.get(id(raw)).unwrap();
        assert_module_and_tables_agree(lowered, tables);
        // Heads are blocks 2 and 3, for `left` and `right` in member order,
        // whichever machine is entered; the entry jumps to its own.
        assert_eq!(jump_of(block(lowered, 1)).0, id(own_head));
        assert_eq!(block(lowered, 2).parameters.len(), 1);
        assert_eq!(block(lowered, 3).parameters.len(), 1);
        // Only `right` has a `requires`, so only its head has a row, owed by
        // the call in `left` and, when `right` is the machine entered, by
        // the entry too.
        let rows: Vec<_> = elaboration
            .module
            .scalar_block_invariants
            .iter()
            .filter(|row| row.machine == id(raw))
            .collect();
        let [row] = rows.as_slice() else {
            panic!("one row per machine");
        };
        assert_eq!(row.header, id(3));
        assert_eq!(row.arrivals.len(), if raw == 2 { 2 } else { 1 });
        assert!(tables.invocations.is_empty());
    }
    // The roster is in machine and head order, as the codec requires.
    assert!(
        elaboration
            .module
            .scalar_block_invariants
            .windows(2)
            .all(|pair| (pair[0].machine, pair[0].header) < (pair[1].machine, pair[1].header))
    );
}

#[test]
fn a_loop_with_no_way_out_never_completes_and_its_caller_does_not_continue() {
    // machine 1 { spin(); }   machine spin() { spin() }
    let caller = Body {
        slots: Vec::new(),
        statements: vec![Statement::Evaluate(call(2, Vec::new(), None))],
    };
    let spin = Body {
        slots: Vec::new(),
        statements: vec![Statement::Arm {
            guard: None,
            target: Target::Value(call(2, Vec::new(), None)),
            source: nowhere(),
        }],
    };
    let elaboration = elaborate(vec![
        (caller, signature(Vec::new(), None)),
        (spin, signature(Vec::new(), None)),
    ])
    .unwrap();
    // The caller's call has no normal successor.
    assert!(matches!(
        block(machine(&elaboration.module, 1), 1).terminator,
        Terminator::InvokeNoNormal { .. }
    ));
    // `spin` is an entry, a head and a jump from the head to itself.
    let spin = machine(&elaboration.module, 2);
    assert_eq!(spin.blocks.len(), 2);
    assert_eq!(jump_of(block(spin, 2)).0, id(2));
}

#[test]
fn what_the_lane_does_not_serve_yet_on_a_cycle_is_refused_by_name() {
    let plain = || signature(vec![u32_type()], Some(u32_type()));
    let refused = |machines| {
        let refusal = elaborate(machines).unwrap_err();
        assert_eq!(
            refusal.reason,
            terminal_check_tables::RefusalReason::Unsupported
        );
        refusal.detail
    };
    // machine f(n: u32) -> u32 { 1 + f(n) }: the call's result is used.
    let nested = Body {
        slots: scalars(vec![u32_type()]),
        statements: vec![returns(strict(
            Strict::Add,
            integer(1),
            call(1, vec![slot(0, u32_type())], Some(u32_type())),
            u32_type(),
        ))],
    };
    assert_eq!(
        refused(vec![(nested, plain())]),
        "call recursion outside a terminal position"
    );
    let mut ensured = plain();
    ensured.ensures = vec![Proposition::LessOrEqual(zero(), value(2))];
    assert_eq!(
        refused(vec![countdown(ensured)]),
        "contract ensures on a call cycle"
    );
    let mut promised = plain();
    promised.termination = Termination::Promised;
    assert_eq!(
        refused(vec![countdown(promised)]),
        "contract termination promise over a call cycle"
    );
    let mut witnessed = plain();
    witnessed.termination = Termination::Witnessed;
    assert_eq!(
        refused(vec![countdown(witnessed)]),
        "contract ranking witness"
    );
    let mut guarded_route = plain();
    guarded_route.crashes = CrashCeiling::Authored(vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::LessThan(zero(), value(1)),
        ))],
    }]);
    assert_eq!(
        refused(vec![countdown(guarded_route)]),
        "contract guarded crash route on a call cycle"
    );
    // A promise on a machine that only calls into a loop is refused too:
    // what it derives depends on the loop's ranking.
    let mut outer = signature(Vec::new(), Some(u32_type()));
    outer.termination = Termination::Promised;
    let caller = Body {
        slots: Vec::new(),
        statements: vec![returns(call(2, vec![integer(3)], Some(u32_type())))],
    };
    let mut looping = countdown(plain());
    looping.0.statements = vec![
        guarded(positive(0), call(2, vec![less_one(0)], Some(u32_type()))),
        returns(integer(0)),
    ];
    assert_eq!(
        refused(vec![(caller, outer), looping]),
        "contract termination promise over a call cycle"
    );
}

#[test]
fn a_promise_on_a_machine_that_reaches_no_cycle_is_served() {
    let mut promised = signature(Vec::new(), Some(u32_type()));
    promised.termination = Termination::Promised;
    let body = Body {
        slots: Vec::new(),
        statements: vec![returns(integer(7))],
    };
    assert!(elaborate(vec![(body, promised)]).is_ok());
}
