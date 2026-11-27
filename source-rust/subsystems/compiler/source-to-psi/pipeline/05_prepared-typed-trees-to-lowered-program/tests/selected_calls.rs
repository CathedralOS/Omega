//! Stage 05 must lower each call to the machine typing selected for it, with
//! the arguments written there, and must owe each precondition and crash
//! route of that callee at that call and nowhere else. The tests write small
//! programs, read the calls stage 05 produced, and change one input at a
//! time: a source twin, or one selection in stage 04's product.
//!
//! The rules: "A call names its exact callee and positional actuals. Each
//! published callee `requires` clause has one distinct caller obligation"
//! (`wiki/spec/terminal-psi/calls_and_outcomes.md`, "Call contracts"); a
//! named call evaluates "all authored arguments left to right"
//! (`wiki/spec/language/expressions.md`, "Evaluation schedule"); and across
//! calls a callee's crash guards "substitute actuals"
//! (`wiki/spec/language/effects.md`, "Guarded crashes").

#[path = "../../06_lowered-program-to-checked-program/tests/support/source_handoff.rs"]
mod program;

use program::{Program, returned, scalar};
use semantic_vocabulary::{
    CrashCause, IntegerValue, InvocationSite, MachineId, ObligationId, Proposition, ScalarTerm,
    ValueId,
};
use source_compiler::representations::source_state::expression::ExpressionNode;
use source_compiler::test_support::preparation::PreparedTypedTrees;
use terminal_check_tables::edges::{Actual, Callee};
use terminal_check_tables::obligations::ObligationKind;
use terminal_psi::{
    CrashRouteBucket, CrashRouteGuard, ExecutionSite, OperationKind, TargetMachineCallable,
    TerminalMachine,
};

/// The value ids of `machine`'s scalar formals, in declared order.
fn formals(machine: &TerminalMachine) -> Vec<ValueId> {
    machine
        .contract
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect()
}

/// A plain scalar call: the callee, the arguments and the requirements it
/// owes. Panics on any other operation.
fn call(kind: &OperationKind) -> (MachineId, &[ValueId], &[ObligationId]) {
    let OperationKind::Call {
        callee,
        arguments,
        erased_arguments,
        erased_proof_arguments,
        requirement_obligations,
        ..
    } = kind
    else {
        panic!("expected a scalar call: {kind:?}")
    };
    assert!(erased_arguments.is_empty() && erased_proof_arguments.is_empty());
    (*callee, arguments, requirement_obligations)
}

const ECHO: &str = "machine echo(x: u8) -> u8 { x }
machine after(a: u8, b: u8) -> u8 { a }
machine before(a: u8, b: u8) -> u8 { b }
machine use_it(x: u8) -> u8 { after(echo(x), echo(7u8)) }
machine other(x: u8) -> u8 { before(x, x) }
";

/// `use_it`'s four operations, checked against the order the source
/// evaluates them in, and the callee of its outer call.
fn outer_callee(program: &Program, prepared: &PreparedTypedTrees) -> MachineId {
    let lowered = program.lower_prepared(prepared).unwrap();
    let machine = lowered.machine("use_it");
    let [x] = formals(machine)[..] else {
        panic!("`use_it` takes one formal")
    };
    let echo = lowered.machine_id("echo");
    let [first, literal, second, outer] = lowered.operations("use_it")[..] else {
        panic!("two inner calls, a literal and the outer call")
    };
    assert_eq!(call(&first.1.kind), (echo, &[x][..], &[][..]));
    assert_eq!(
        literal.1.kind,
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(7)
        }
    );
    assert_eq!(
        call(&second.1.kind),
        (echo, &[scalar(literal.1)][..], &[][..])
    );
    let (callee, arguments, owed) = call(&outer.1.kind);
    assert_eq!(arguments, [scalar(first.1), scalar(second.1)]);
    assert!(owed.is_empty());
    assert_eq!(returned(machine), scalar(outer.1));

    // The checker reads the call through its invocation row, which must
    // name the same callee and the same actuals.
    let tables = lowered.elaboration.tables.machines.get(machine.id).unwrap();
    let rows: Vec<_> = tables
        .invocations
        .iter()
        .filter(|(_, row)| row.site == InvocationSite::Operation(outer.1.id))
        .map(|(_, row)| row)
        .collect();
    let [row] = rows[..] else {
        panic!("the outer call has one invocation row")
    };
    assert_eq!(
        row.callee,
        Callee::Declared(TargetMachineCallable::Machine(callee))
    );
    assert_eq!(
        row.actuals,
        [
            Actual::Value(scalar(first.1)),
            Actual::Value(scalar(second.1))
        ]
    );
    callee
}

#[test]
fn a_call_names_the_selected_callee_and_evaluates_each_argument_once_in_order() {
    let program = Program::of(ECHO);
    let lowered = program.lower();
    let (after, before) = (lowered.machine_id("after"), lowered.machine_id("before"));
    assert_eq!(outer_callee(&program, &program.prepared), after);

    // Control: stage 04 run again with nothing changed.
    assert_eq!(
        outer_callee(&program, &program.prepared_with(|_| {})),
        after
    );

    // The one changed fact: typing selected `before`, which has the same
    // signature, for the outer call. Typing records a call's selection in
    // its target, its authored selection, its binding and its static
    // application; all of them are copied from `other`'s call of `before`,
    // and the outer call keeps its own arguments. The callee follows, and
    // nothing about the arguments changes.
    let is_call = |node: &ExpressionNode| matches!(node, ExpressionNode::Call(_));
    let outer = program.expression("after(echo(x), echo(7u8))", is_call);
    let model = program.expression("before(x, x)", is_call);
    let altered = program.prepared_with(|trees| {
        let table = &mut trees.expression_table;
        let ExpressionNode::Call(mut chosen) = table.expression(model).clone() else {
            unreachable!()
        };
        let ExpressionNode::Call(call) = table.expression_mut(outer) else {
            unreachable!()
        };
        chosen.arguments = call.arguments;
        chosen.receiver = call.receiver;
        *call = chosen;
    });
    assert_eq!(outer_callee(&program, &altered), before);

    // The target alone disagrees with the other records of the selection,
    // and stage 05 refuses the call rather than choose one of them.
    let disagreeing = program.prepared_with(|trees| {
        let ExpressionNode::Call(chosen) = trees.expression_table.expression(model).clone() else {
            unreachable!()
        };
        let ExpressionNode::Call(call) = trees.expression_table.expression_mut(outer) else {
            unreachable!()
        };
        call.target_symbol = chosen.target_symbol;
    });
    let Err(refusal) = program.lower_prepared(&disagreeing) else {
        panic!("a call whose two selections disagree must be refused")
    };
    assert_eq!(
        refusal.detail,
        "call static application registers another declaration while reading the body of `use_it`"
    );
}

/// `east\pick` returns its first formal and `west\pick` its second. The two
/// have the same name and signature, so only the module says which one a
/// call names.
fn picks(main: &str) -> [(MachineId, MachineId, MachineId); 1] {
    let program = Program::try_of(&[
        (
            "product/west/pick.omg",
            "public machine pick(a: u8, b: u8) -> u8 { b }\n",
        ),
        ("product/main.omg", main),
        (
            "product/east/pick.omg",
            "public machine pick(a: u8, b: u8) -> u8 { a }\n",
        ),
    ])
    .unwrap();
    let lowered = program.lower();
    for (path, position) in [("east\\pick", 0), ("west\\pick", 1)] {
        let machine = lowered.machine(path);
        assert_eq!(returned(machine), formals(machine)[position], "{path}");
    }
    let callee = |path: &str| {
        let machine = lowered.machine(path);
        let [(_, operation)] = lowered.operations(path)[..] else {
            panic!("`{path}` is one call")
        };
        let (callee, arguments, owed) = call(&operation.kind);
        assert_eq!(arguments, formals(machine), "{path}");
        assert!(owed.is_empty());
        assert_eq!(returned(machine), scalar(operation));
        callee
    };
    [(
        callee("qualified"),
        callee("imported"),
        lowered.machine_id("east\\pick"),
    )]
}

#[test]
fn qualified_and_imported_calls_keep_the_module_of_same_spelled_machines() {
    let caller = |qualified: &str, imported: &str| {
        format!(
            "use {qualified}/pick.omg as chosen;
use {imported}/pick.omg as other;
machine qualified(a: u8, b: u8) -> u8 {{ |chosen|::pick(a, b) }}
machine imported(a: u8, b: u8) -> u8 {{ |other|::pick(a, b) }}
"
        )
    };
    let [(qualified, imported, east)] = picks(&caller("east", "west"));
    assert_eq!(qualified, east);
    assert_ne!(imported, east);
    // Exchanging the modules in the source exchanges the callees.
    let [(qualified, imported, east)] = picks(&caller("west", "east"));
    assert_ne!(qualified, east);
    assert_eq!(imported, east);
}

/// Each call of `run` with the requirement rows it owes, as
/// `(callee, clause of each owed row)`, in evaluation order.
fn owed_by_each_call(source: &str) -> Vec<(String, Vec<u32>)> {
    let program = Program::of(source);
    let lowered = program.lower();
    let machine = lowered.machine("run");
    let tables = lowered.elaboration.tables.machines.get(machine.id).unwrap();
    let name = |id: MachineId| {
        ["leaf", "checked"]
            .into_iter()
            .find(|path| lowered.machine_id(path) == id)
            .unwrap()
            .to_owned()
    };
    lowered
        .operations("run")
        .into_iter()
        .map(|(_, operation)| {
            let (callee, _, owed) = call(&operation.kind);
            let invocation = tables
                .invocations
                .iter()
                .find(|(_, row)| row.site == InvocationSite::Operation(operation.id))
                .map(|(id, row)| {
                    assert_eq!(row.requirements, owed);
                    id
                })
                .unwrap();
            let clauses = owed
                .iter()
                .map(
                    |obligation| match tables.obligations.get(*obligation).unwrap().kind {
                        ObligationKind::CallRequirement {
                            invocation: owner,
                            clause,
                        } => {
                            assert_eq!(owner, invocation);
                            clause
                        }
                        ref other => panic!("a call owes its callee's clauses: {other:?}"),
                    },
                )
                .collect();
            (name(callee), clauses)
        })
        .collect()
}

#[test]
fn a_discarded_call_owes_the_precondition_of_its_own_callee_only() {
    // `checked` requires its input to be positive; the debt belongs to the
    // outer call, whose argument is `leaf(x)`, not to `leaf(x)` itself.
    assert_eq!(
        owed_by_each_call(
            "machine leaf(x: u8) -> u8 { x }
machine checked(x: u8) -> u8 requires x > 0 { x }
machine run(x: u8) { _ = checked(leaf(x)); }
"
        ),
        [("leaf".into(), vec![]), ("checked".into(), vec![0])]
    );
    // Twin: the same clause moved to `leaf` moves the debt to the inner call.
    assert_eq!(
        owed_by_each_call(
            "machine leaf(x: u8) -> u8 requires x > 0 { x }
machine checked(x: u8) -> u8 { x }
machine run(x: u8) { _ = checked(leaf(x)); }
"
        ),
        [("leaf".into(), vec![0]), ("checked".into(), vec![])]
    );
}

#[test]
fn swapped_actuals_change_the_arguments_and_keep_one_debt_per_clause() {
    let lowered_call = |call_text: &str| {
        let source = format!(
            "machine checked(a: u8, b: u8) -> u8 requires a < 10 requires b < 20 {{ a }}
machine run(a: u8, b: u8) -> u8 requires a < 20, b < 20 {{ {call_text} }}
"
        );
        let program = Program::of(&source);
        let lowered = program.lower();
        let machine = lowered.machine("run");
        let [(_, operation)] = lowered.operations("run")[..] else {
            panic!("`run` is one call")
        };
        let (callee, arguments, owed) = call(&operation.kind);
        assert_eq!(callee, lowered.machine_id("checked"));
        let tables = lowered.elaboration.tables.machines.get(machine.id).unwrap();
        let clauses: Vec<_> = owed
            .iter()
            .map(
                |obligation| match tables.obligations.get(*obligation).unwrap().kind {
                    ObligationKind::CallRequirement { clause, .. } => clause,
                    ref other => panic!("{other:?}"),
                },
            )
            .collect();
        let [a, b] = formals(machine)[..] else {
            panic!("`run` takes two formals")
        };
        let positions: Vec<_> = arguments
            .iter()
            .map(|argument| {
                if *argument == a {
                    "a"
                } else if *argument == b {
                    "b"
                } else {
                    "?"
                }
            })
            .collect();
        (positions, clauses)
    };
    assert_eq!(lowered_call("checked(b, a)"), (vec!["b", "a"], vec![0, 1]));
    assert_eq!(lowered_call("checked(a, b)"), (vec!["a", "b"], vec![0, 1]));
}

/// The value a one-route `Trap` guard `v == 0` compares with zero.
fn guarded_value(routes: &[CrashRouteBucket]) -> ValueId {
    let [
        CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives,
        },
    ] = routes
    else {
        panic!("one trap route: {routes:?}")
    };
    let [CrashRouteGuard::Predicate(predicate)] = alternatives.as_slice() else {
        panic!("one guarded alternative: {alternatives:?}")
    };
    let Proposition::Equal(
        ScalarTerm::Value { id, .. },
        ScalarTerm::Integer {
            value: IntegerValue::Unsigned(0),
            ..
        },
    ) = predicate.proposition()
    else {
        panic!("the guard compares a value with zero: {predicate:?}")
    };
    *id
}

#[test]
fn a_callee_guard_is_instantiated_with_the_actual_the_call_passes() {
    let program = Program::of(
        "machine pick(a: u8, b: u8) -> u8 crashes Trap b == 0 { a }
machine run(x: u8, y: u8) -> u8 crashes Trap y == 0 { pick(x, y) }
machine swapped(x: u8, y: u8) -> u8 crashes Trap x == 0 { pick(y, x) }
",
    );
    let lowered = program.lower();
    // The callee states its route over its own second formal.
    let pick = lowered.machine("pick");
    assert_eq!(guarded_value(&pick.contract.crash_routes), formals(pick)[1]);
    for (caller, guarded) in [("run", 1), ("swapped", 0)] {
        let machine = lowered.machine(caller);
        let [(_, operation)] = lowered.operations(caller)[..] else {
            panic!("`{caller}` is one call")
        };
        let OperationKind::Call {
            callee,
            crash_continuations,
            ..
        } = &operation.kind
        else {
            panic!("{operation:?}")
        };
        assert_eq!(*callee, pick.id);
        // `pick`'s `b` is `y` in `run` and `x` in `swapped`: the same route
        // the caller declares over that formal.
        assert_eq!(
            guarded_value(crash_continuations),
            formals(machine)[guarded],
            "{caller}"
        );
        assert_eq!(crash_continuations, &machine.contract.crash_routes);
        let tables = lowered.elaboration.tables.machines.get(machine.id).unwrap();
        let coverage: Vec<_> = tables.obligations.iter().map(|(_, row)| row.kind).collect();
        assert_eq!(
            coverage,
            [ObligationKind::CrashCoverage {
                site: ExecutionSite::Operation(operation.id),
                cause: CrashCause::Trap,
            }],
            "{caller}"
        );
    }
}

#[test]
fn an_operator_calls_the_machine_whose_domain_holds_its_operands() {
    let lowered_callees = |km: &str, metres: &str| {
        let source = format!(
            "domain u8::Km;
domain u8::Metres;
public machine + combine(x: u8 in {km}, y: u8 in {km}) -> u8 in {km} {{ y }}
public machine + join(x: u8 in {metres}, y: u8 in {metres}) -> u8 in {metres} {{ x }}
machine km_sum(a: u8 in Km, b: u8 in Km) -> u8 in Km {{ a + b }}
machine metres_sum(a: u8 in Metres, b: u8 in Metres) -> u8 in Metres {{ b + a }}
"
        );
        let program = Program::of(&source);
        let lowered = program.lower();
        ["km_sum", "metres_sum"].map(|caller| {
            let machine = lowered.machine(caller);
            let [(written, operation)] = lowered.operations(caller)[..] else {
                panic!("`{caller}` is one operator call")
            };
            assert_eq!(written, "+");
            let (callee, arguments, _) = call(&operation.kind);
            let [a, b] = formals(machine)[..] else {
                panic!("two formals")
            };
            let expected: &[ValueId] = if caller == "km_sum" { &[a, b] } else { &[b, a] };
            assert_eq!(arguments, expected, "{caller}");
            ["combine", "join"]
                .into_iter()
                .find(|path| lowered.machine_id(path) == callee)
                .unwrap()
        })
    };
    assert_eq!(lowered_callees("Km", "Metres"), ["combine", "join"]);
    // Twin: the two declarations exchange domains, so the callees exchange.
    assert_eq!(lowered_callees("Metres", "Km"), ["join", "combine"]);
}
