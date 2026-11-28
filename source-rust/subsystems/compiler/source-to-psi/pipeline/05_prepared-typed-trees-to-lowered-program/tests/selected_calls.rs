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

#[test]
fn scope_exit_restores_each_hidden_mutable_reference_in_an_array() {
    let source = "
data File { value: u8; }
data Socket { other: u8; value: u8; }
data Handle<'a, some State> { state: @'a mutable State; }
machine main() {
    let mutable file: File = File { value: 17u8 };
    let mutable socket: Socket = Socket { other: 0u8, value: 42u8 };
    let handles: [Handle; 2] = [
        Handle { state: @mutable file }, Handle { state: @mutable socket }
    ];
}";
    let program = Program::of(source);
    let lowered = program.elaboration();
    let sites = &lowered.module.execution.packages.sites;
    let witnesses: Vec<_> = sites
        .iter()
        .filter_map(|site| match &site.event {
            terminal_psi::TerminalPackageEvent::Construct { witnesses } => Some(witnesses),
            _ => None,
        })
        .collect();
    assert_eq!(witnesses.len(), 2);
    assert_ne!(witnesses[0], witnesses[1]);
    let mut released = Vec::new();
    for operation in program.machine("main").operations() {
        if let OperationKind::ReleaseReference { path, .. } = &operation.kind
            && let [
                terminal_psi::StructuralPathSegment::FixedIndex(index),
                terminal_psi::StructuralPathSegment::Field(_),
            ] = path.as_slice()
        {
            released.push(*index);
        }
    }
    released.sort_unstable();
    assert_eq!(released, [0, 1]);
}

#[test]
fn calls_invalidate_the_actual_lenders_of_two_hidden_mutable_fields() {
    let source = "
data File { byte: u8; }
data Socket { unused: u8; byte: u8; }
data Read<S> = machine(state: @mutable S) -> u8 reaches none crashes none;
data Reader<'a, some S> { state: @'a mutable S; read: Read<S>; }
machine read_file(state: @mutable File) -> u8 reaches none crashes none { state.byte }
machine read_socket(state: @mutable Socket) -> u8 reaches none crashes none { state.byte }
machine read(reader: @mutable Reader) -> u8 { (reader.read)(reader.state) }
machine main() -> bool {
    let mutable file: File = File { byte: 17u8 };
    let mutable socket: Socket = Socket { unused: 0u8, byte: 42u8 };
    let mutable readers: [Reader; 2] = [
        Reader { state: @mutable file, read: read_file },
        Reader { state: @mutable socket, read: read_socket }
    ];
    let first: u8 = read(@mutable readers[0]);
    let second: u8 = read(@mutable readers[1]);
    first == 17u8 && second == 42u8
}";
    let program = Program::of(source);
    // A write through readers[0] gives no information about readers[1],
    // even though both array elements have the same public type.
    use terminal_psi::{StructuralPathSegment::FixedIndex, call_structural_arguments};
    use terminal_semantics::descriptor_lifetimes::descriptor_location;
    use terminal_semantics::descriptor_results::{
        DescriptorAnalysis, terminal_borrowed_storage_graph_from,
    };
    let module = &program.elaboration().module;
    let state = program
        .machine("read")
        .operations()
        .find_map(|op| op.kind.callable_invocation()?.structural_arguments.first())
        .unwrap();
    let main = program.machine("main");
    let types = &module.types.structural_types;
    let graph = terminal_borrowed_storage_graph_from(module, [main.id], &[]).unwrap();
    let mut references = DescriptorAnalysis::with_borrowed_storage_slots(types, &graph);
    let mut lenders = Vec::new();
    for block in &main.blocks {
        for (ordinal, operation) in block.operations.iter().enumerate() {
            let Some(argument) = call_structural_arguments(&operation.kind).first() else {
                continue;
            };
            let [FixedIndex(selected)] = argument.path.as_slice() else {
                panic!("a fixed array element")
            };
            let root = main.structural_place_type(argument.place).unwrap();
            for index in 0..2 {
                let mut path = vec![FixedIndex(index)];
                path.extend_from_slice(&state.path);
                let location =
                    descriptor_location(types, argument.place, root, &path, None).unwrap();
                let origins = terminal_semantics::packages::local_call_write_origins(
                    module,
                    main,
                    (block.id, ordinal.try_into().unwrap()),
                    &mut references,
                    &location,
                );
                if index == *selected {
                    lenders.push(origins.expect("the actual argument has an exact lender"));
                } else {
                    assert!(origins.is_none(), "the other array element was never lent");
                }
            }
        }
    }
    assert_eq!(lenders.len(), 2);
    assert_ne!(lenders[0], lenders[1]);
    program.judged(|_| ()).accepted();

    let mixed = source
        .replace(
            "read_file(state: @mutable File)",
            "read_file(state: @mutable u8)",
        )
        .replacen("{ state.byte }", "{ $state }", 1)
        .replace(
            "let mutable file: File = File { byte: 17u8 };",
            "let mutable file: u8 = 17u8;",
        );
    Program::of(&mixed).judged(|_| ()).accepted();

    // Borrow the original File again while its Reader will still invoke it.
    let overlapping = source
        .replace(
            "machine main()",
            "machine overwrite(value: @mutable File) { value.byte = 99u8; }\nmachine main()",
        )
        .replace(
            "let first: u8",
            "overwrite(@mutable file);\n    let first: u8",
        );
    let disjoint = overlapping.replace(
        "overwrite(@mutable file);",
        "let mutable unrelated: File = File { byte: 9u8 }; overwrite(@mutable unrelated);",
    );
    Program::of(&disjoint).judged(|_| ()).accepted();
    match Program::of(&overlapping).judged(|_| ()) {
        program::Verdict::Refused(refusal) => assert_eq!(
            refusal.reason,
            terminal_check_tables::RefusalReason::Conflict
        ),
        program::Verdict::Rejected(rejection) => assert!(!rejection.conflicts.is_empty()),
        other => panic!("expected conflicting use of the retained File, got {other:?}"),
    }
}

/// Rebuilding the public package borrows both hidden fields from the same
/// opening. Its plain tag has no dependence on a hidden type.
#[test]
fn repacking_shared_fields_uses_the_opening_of_their_source() {
    const SOURCE: &str = "data Pair<'a, some S> { first: @'a S; second: @'a S; tag: u8; }
        machine repack<'a>(p: @Pair<'a>) -> Pair<'a> {
            Pair { first: p.first, second: p.second, tag: p.tag }
        }";
    let program = Program::of(SOURCE);
    let lowered = program.lower();
    let operations = lowered.operations("repack");
    let sites = &program.elaboration().module.execution.packages.sites;
    let mut projected = Vec::new();
    for site in sites {
        if let terminal_psi::TerminalPackageEvent::Project { opening, field } = &site.event {
            let (_, op) = operations
                .iter()
                .find(|(_, op)| op.id == site.operation)
                .expect("a projected field must have its actual read");
            assert!(matches!(op.kind, OperationKind::StructuralLeafCopy { .. }));
            projected.push((*opening, *field));
        }
    }
    let [(first, first_field), (second, second_field)] = projected.as_slice() else {
        panic!("expected two hidden reference projections, found {projected:?}");
    };
    assert_eq!(first, second);
    assert_ne!(first_field, second_field);
    assert_eq!(sites.len(), 3);
    assert!(sites.iter().any(|site| matches!(
        site.event,
        terminal_psi::TerminalPackageEvent::Construct { .. }
    )));
    program.judged(|_| {}).accepted();

    let crossed = SOURCE
        .replace("p: @Pair<'a>", "p: @Pair<'a>, q: @Pair<'a>")
        .replace("second: p.second", "second: q.second");
    let refusal = Program::try_of(&[("product/main.omg", &crossed)])
        .err()
        .unwrap();
    assert_eq!(refusal.stage, "typing");
    assert_eq!(refusal.at, "second");
    assert!(refusal.message.contains("conflicting generic arguments"));
}

#[test]
fn a_dependent_field_update_survives_publication_and_independent_replay() {
    use source_compiler::test_support::checking::{RouteVerdict, judge};
    use terminal_psi::TerminalPackageEvent;
    const SAVED_INPUT: &str = "OMEGA_TEST_SAVED_DEPENDENT_FIELD_UPDATE";
    if let Some(path) = std::env::var_os(SAVED_INPUT) {
        let bytes = std::fs::read(path).unwrap();
        let decoded = terminal_codec::decode_module(&bytes).unwrap();
        terminal_codec::sealing::prepare_received_execution(&decoded).unwrap();
        assert!(
            decoded
                .execution
                .packages
                .sites
                .iter()
                .any(|s| matches!(s.event, TerminalPackageEvent::Store { .. }))
        );
        return;
    }
    let source = "
data File { value: u8; }
data Pair<'a, some S> { first: @'a S; second: @'a S; }
machine update(p: @mutable Pair) { p.first = p.second; }
machine main() {
    let a: File = File { value: 11u8 };
    let b: File = File { value: 42u8 };
    let mutable pair: Pair = Pair { first: @a, second: @b };
    update(@mutable pair);
}";
    let program = Program::of(source);
    let lowered = &program.elaboration().module;
    let (index, site) = lowered
        .execution
        .packages
        .sites
        .iter()
        .enumerate()
        .find(|(_, s)| matches!(s.event, TerminalPackageEvent::Store { .. }))
        .expect("the dependent assignment emits an opening at its ordinary store");
    assert!(matches!(
        lowered
            .machine(site.machine)
            .unwrap()
            .operation(site.operation)
            .unwrap()
            .kind,
        OperationKind::StoreStructuralField { .. }
    ));
    let RouteVerdict::Accepted(accepted) = judge(Ok(program.elaboration().clone())).unwrap() else {
        panic!(
            "same-opening source update must reach publication: {:?}",
            program.judged(|_| ())
        );
    };
    let module = accepted.prepare_publication().unwrap().module().clone();
    let bytes = terminal_codec::encode_module(&module).unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "omega-package-update-{}-{nonce}.psi",
        std::process::id()
    ));
    {
        use std::io::Write;
        std::fs::File::create_new(&path)
            .unwrap()
            .write_all(&bytes)
            .unwrap();
    }
    let receiver = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "a_dependent_field_update_survives_publication_and_independent_replay",
        ])
        .env(SAVED_INPUT, &path)
        .output();
    std::fs::remove_file(&path).unwrap();
    let receiver = receiver.unwrap();
    assert!(
        receiver.status.success(),
        "{}",
        String::from_utf8_lossy(&receiver.stdout)
    );
    let source_field = lowered
        .execution
        .packages
        .sites
        .iter()
        .find_map(|site| match site.event {
            TerminalPackageEvent::Project { field, .. } => Some(field),
            _ => None,
        })
        .unwrap();
    let mut crossed = module.clone();
    let TerminalPackageEvent::Store { field, .. } =
        &mut crossed.execution.packages.sites[index].event
    else {
        unreachable!()
    };
    assert_ne!(*field, source_field);
    *field = source_field;
    assert!(
        terminal_codec::decode_module(&terminal_codec::encode_module(&crossed).unwrap()).is_err()
    );
    let mut missing = module;
    missing.execution.packages.sites.remove(index);
    assert!(
        terminal_codec::decode_module(&terminal_codec::encode_module(&missing).unwrap()).is_err()
    );
}
