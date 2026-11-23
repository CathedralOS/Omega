//! Stage 05 must lower each read and each arithmetic operation to the place,
//! selector and policy typing chose for it, and must keep a machine's crash
//! routes over the formals its author named. The tests write small
//! programs, read what stage 05 produced, and change one input at a time: a
//! source twin, or one fact in stage 04's product.
//!
//! The rules: "A suffixed operator applies its policy's rule for that
//! primitive at that node" (`wiki/spec/language/numeric_values.md`, "Policy
//! operators"); "Each `crashes` clause names one cause (`Trap` or `Abort`)"
//! and its guard names the machine's contract inputs
//! (`wiki/spec/language/effects.md`, "Guarded crashes"); reads and their
//! bounds questions follow `wiki/spec/terminal-psi/structural_access.md`
//! and `wiki/spec/language/expressions.md` ("Indexing and ranges").

#[path = "../../06_lowered-program-to-checked-program/tests/support/source_handoff.rs"]
mod program;

use numerics::arithmetic::ArithmeticDomain;
use program::{Program, returned, scalar};
use semantic_vocabulary::{
    CrashCause, IntegerValue, ObligationId, PlaceId, Proposition, ScalarTerm, ValueId,
};
use source_compiler::representations::source_state::expression::{
    ExpressionHandle, ExpressionNode, MemberSelection,
};
use terminal_check_tables::obligations::ObligationKind;
use terminal_psi::{
    CrashRouteBucket, CrashRouteGuard, ExecutionSite, OperationKind, StructuralPathSegment,
    TerminalMachine, TrappingIntegerOperation,
};

fn formals(machine: &TerminalMachine) -> Vec<ValueId> {
    machine
        .contract
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect()
}

/// The place of `machine`'s structural formal at `position`.
fn structural_formal(machine: &TerminalMachine, position: usize) -> PlaceId {
    machine.contract.structural_parameters[position].place
}

const CELLS: &str = "data Cell [copy] { v: u8; w: u8; }
data Pair [copy] { left: Cell; right: Cell; }
machine get(left: Cell, right: Cell) -> u8 { left.v }
machine nested(p: Pair) -> u8 { p.left.w }
";

/// The one field read of `path`: the place it reads and the names of the
/// fields on its path. The machine returns what it read.
fn field_read(lowered: &program::Lowered<'_>, path: &str) -> (PlaceId, Vec<String>) {
    let machine = lowered.machine(path);
    let [(_, operation)] = lowered.operations(path)[..] else {
        panic!("`{path}` is one read")
    };
    let OperationKind::PrimitiveScalarRead {
        source,
        path: steps,
    } = &operation.kind
    else {
        panic!("`{path}` reads a scalar field: {operation:?}")
    };
    assert_eq!(returned(machine), scalar(operation));
    let names = steps
        .iter()
        .map(|step| {
            let StructuralPathSegment::Field(field) = step else {
                panic!("a field step: {step:?}")
            };
            lowered
                .elaboration
                .module
                .identities
                .text(*field)
                .unwrap()
                .to_owned()
        })
        .collect();
    (*source, names)
}

/// Every expression `wanted` accepts, in the order the table numbers them,
/// which is source order.
fn matching(
    program: &Program,
    wanted: impl Fn(&Program, ExpressionHandle, &ExpressionNode) -> bool,
) -> Vec<ExpressionHandle> {
    program
        .trees()
        .expression_table
        .iter_expressions()
        .filter(|(handle, node)| wanted(program, *handle, node))
        .map(|(handle, _)| handle)
        .collect()
}

/// The one member expression that reads `field` of the receiver written
/// `receiver`.
fn member_read(program: &Program, receiver: &str, field: &str) -> ExpressionHandle {
    let found = matching(program, |program, _, node| {
        matches!(node, ExpressionNode::Member(member)
            if member.member.as_str() == field && program.written_expression(member.receiver) == receiver)
    });
    let [read] = found[..] else {
        panic!("one read of `{receiver}.{field}`: {found:?}")
    };
    read
}

fn field_symbol(program: &Program, record: &str, name: &str) -> symbols::SymbolHandle {
    let trees = program.trees();
    let declared = trees.data_definition_by_name_str(record).unwrap();
    trees
        .data_members(declared)
        .iter()
        .filter_map(|member| member.as_field())
        .find(|field| field.name.as_str() == name)
        .unwrap()
        .symbol
}

#[test]
fn a_field_read_names_the_receiver_and_the_field_typing_selected() {
    let program = Program::of(CELLS);
    let lowered = program.lower();
    let get = lowered.machine("get");
    assert_eq!(
        field_read(&lowered, "get"),
        (structural_formal(get, 0), vec!["v".to_owned()])
    );
    let nested = lowered.machine("nested");
    assert_eq!(
        field_read(&lowered, "nested"),
        (
            structural_formal(nested, 0),
            vec!["left".to_owned(), "w".to_owned()]
        )
    );

    let read = member_read(&program, "left", "v");

    // One changed fact: typing selected `w` for the read. Only the field
    // step follows.
    let w = field_symbol(&program, "Cell", "w");
    let altered = program.prepared_with(|trees| {
        let ExpressionNode::Member(member) = trees.expression_table.expression_mut(read) else {
            unreachable!()
        };
        member.selection = MemberSelection::Field(w);
    });
    let lowered = program.lower_prepared(&altered).unwrap();
    let get = lowered.machine("get");
    assert_eq!(
        field_read(&lowered, "get"),
        (structural_formal(get, 0), vec!["w".to_owned()])
    );

    // Another: the receiver names `right`, which `get`'s contract also
    // reads. Only the place read follows.
    let program = Program::of(
        "data Cell [copy] { v: u8; w: u8; }
machine get(left: Cell, right: Cell) -> u8 requires right.w < 9 { left.v }
",
    );
    let read = member_read(&program, "left", "v");
    let right = member_read(&program, "right", "w");
    let altered = program.prepared_with(|trees| {
        let ExpressionNode::Member(member) = trees.expression_table.expression(read).clone() else {
            unreachable!()
        };
        let ExpressionNode::Member(other) = trees.expression_table.expression(right).clone() else {
            unreachable!()
        };
        let receiver = trees.expression_table.expression(other.receiver).clone();
        *trees.expression_table.expression_mut(member.receiver) = receiver;
    });
    let lowered = program.lower_prepared(&altered).unwrap();
    let get = lowered.machine("get");
    assert_eq!(
        field_read(&lowered, "get"),
        (structural_formal(get, 1), vec!["v".to_owned()])
    );
}

/// The index an element read of `path` selects with and the obligation it
/// owes, which must be the bounds question of that read.
fn element_read(lowered: &program::Lowered<'_>, path: &str) -> ValueId {
    let machine = lowered.machine(path);
    let reads: Vec<_> = machine
        .operations()
        .filter_map(|operation| match &operation.kind {
            OperationKind::PrimitiveScalarRead { path, .. } => {
                let [StructuralPathSegment::RuntimeIndex { index, obligation }] = path.as_slice()
                else {
                    panic!("one runtime index: {path:?}")
                };
                Some((operation, *index, *obligation))
            }
            OperationKind::ElementViewRead {
                index, obligation, ..
            } => Some((operation, *index, *obligation)),
            _ => None,
        })
        .collect();
    let [(operation, index, obligation)] = reads[..] else {
        panic!("`{path}` reads one element: {reads:?}")
    };
    assert_eq!(returned(machine), scalar(operation));
    let tables = lowered.elaboration.tables.machines.get(machine.id).unwrap();
    assert_eq!(
        tables.obligations.get(obligation).unwrap().kind,
        ObligationKind::OperationSafety {
            site: ExecutionSite::Operation(operation.id)
        }
    );
    index
}

#[test]
fn an_element_read_selects_with_its_own_index_and_owes_its_own_bounds_question() {
    let program = Program::of(
        "machine fixed(items: [u32; 4], i: u64, j: u64) -> u32 requires i < 4, j < 4 { items[i] }
machine view(items: @[u32], i: u64, j: u64) -> u32 requires i < items.length, j < items.length { items[i] }
",
    );
    let lowered = program.lower();
    for path in ["fixed", "view"] {
        let [i, _j] = formals(lowered.machine(path))[..] else {
            panic!("two scalar formals")
        };
        assert_eq!(element_read(&lowered, path), i, "{path}");
    }

    // One changed fact in each machine: the read's index names `j`.
    for (machine, other) in [("fixed", "view"), ("view", "fixed")] {
        // Expressions are numbered in source order, so `fixed`'s read and
        // `fixed`'s `j` come first.
        let position = usize::from(machine == "view");
        let reads = matching(&program, |_, _, node| {
            matches!(node, ExpressionNode::Indexed(_))
        });
        let js = matching(&program, |program, handle, node| {
            matches!(node, ExpressionNode::Name(_)) && program.written_expression(handle) == "j"
        });
        assert_eq!((reads.len(), js.len()), (2, 2));
        let (read, j) = (reads[position], js[position]);
        let altered = program.prepared_with(|trees| {
            let ExpressionNode::Indexed(indexed) = trees.expression_table.expression(read).clone()
            else {
                unreachable!()
            };
            let selector = trees.expression_table.expression(j).clone();
            *trees.expression_table.expression_mut(indexed.index) = selector;
        });
        let lowered = program.lower_prepared(&altered).unwrap();
        let [i, j] = formals(lowered.machine(machine))[..] else {
            panic!("two scalar formals")
        };
        assert_eq!(element_read(&lowered, machine), j, "{machine}");
        let [i_other, _] = formals(lowered.machine(other))[..] else {
            panic!("two scalar formals")
        };
        assert_eq!(element_read(&lowered, other), i_other, "{other}");
        assert_ne!(i, j);
    }
}

#[test]
fn an_operation_takes_the_policy_its_operator_chose() {
    let program = Program::of("machine exact(value: u8, other: u8) -> u8 { value + other }\n");
    let [sum] = matching(&program, |_, _, node| {
        matches!(node, ExpressionNode::Binary(_))
    })[..] else {
        panic!("one binary expression")
    };
    let lowered_with = |policy: Option<ArithmeticDomain>| {
        let prepared = program.prepared_with(|trees| {
            if let Some(policy) = policy {
                let ExpressionNode::Binary(binary) = trees.expression_table.expression_mut(sum)
                else {
                    unreachable!()
                };
                binary.policy = policy;
            }
        });
        let lowered = program.lower_prepared(&prepared).unwrap();
        let machine = lowered.machine("exact");
        let [value, other] = formals(machine)[..] else {
            panic!("two formals")
        };
        let [(written, operation)] = lowered.operations("exact")[..] else {
            panic!("one operation")
        };
        assert_eq!(written, "+");
        assert_eq!(returned(machine), scalar(operation));
        let tables = lowered.elaboration.tables.machines.get(machine.id).unwrap();
        let owed: Vec<_> = tables.obligations.iter().map(|(_, row)| row.kind).collect();
        (operation.kind.clone(), value, other, owed, operation.id)
    };

    // As written, `+` is exact: one partial operation owing its own safety.
    let (kind, value, other, owed, site) = lowered_with(None);
    let OperationKind::ExactIntegerAdd {
        left,
        right,
        obligation,
    } = kind
    else {
        panic!("{kind:?}")
    };
    assert_eq!((left, right), (value, other));
    assert_eq!(obligation, ObligationId::new(1).unwrap());
    assert_eq!(
        owed,
        [ObligationKind::OperationSafety {
            site: ExecutionSite::Operation(site)
        }]
    );

    // One changed fact: the operator's policy. The operation follows it, and
    // a total policy owes nothing.
    for (policy, expected) in [
        (
            ArithmeticDomain::Wrapping,
            OperationKind::WrappingIntegerAdd {
                left: value,
                right: other,
            },
        ),
        (
            ArithmeticDomain::Saturating,
            OperationKind::SaturatingIntegerAdd {
                left: value,
                right: other,
            },
        ),
    ] {
        let (kind, _, _, owed, _) = lowered_with(Some(policy));
        assert_eq!(kind, expected, "{policy:?}");
        assert!(owed.is_empty(), "{policy:?}: {owed:?}");
    }
}

/// The value a one-route guard `v == 0` compares with zero, with the route's
/// cause.
fn guard(routes: &[CrashRouteBucket]) -> (CrashCause, ValueId) {
    let [
        CrashRouteBucket {
            cause,
            alternatives,
        },
    ] = routes
    else {
        panic!("one route: {routes:?}")
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
    (*cause, *id)
}

#[test]
fn a_declared_route_keeps_its_cause_and_the_formal_its_guard_names() {
    for (route, cause, position) in [
        ("Trap right == 0", CrashCause::Trap, 1),
        ("Trap left == 0", CrashCause::Trap, 0),
        ("Abort right == 0", CrashCause::Abort, 1),
    ] {
        let program = Program::of(&format!(
            "machine divide(left: u8, right: u8) -> u8 crashes {route} {{ left }}\n"
        ));
        let lowered = program.lower();
        let machine = lowered.machine("divide");
        assert_eq!(
            guard(&machine.contract.crash_routes),
            (cause, formals(machine)[position]),
            "{route}"
        );
    }
}

#[test]
fn a_trapping_division_divides_by_its_written_operand_under_its_own_guard() {
    // The divisor and the formal the route's guard names are separate
    // inputs; each twin changes one of them.
    for (guarded, divisor) in [("x", "x"), ("y", "x"), ("x", "y")] {
        let program = Program::of(&format!(
            "machine quotient(a: u8, x: u8, y: u8) -> u8 crashes Trap {guarded} == 0 {{ a /! {divisor} }}\n"
        ));
        let lowered = program.lower();
        let machine = lowered.machine("quotient");
        let [a, x, y] = formals(machine)[..] else {
            panic!("three formals")
        };
        let by_name = |name: &str| if name == "x" { x } else { y };
        let [(written, operation)] = lowered.operations("quotient")[..] else {
            panic!("one division")
        };
        assert_eq!(written, "/!");
        assert_eq!(
            operation.kind,
            OperationKind::TrappingInteger {
                operation: TrappingIntegerOperation::Divide {
                    left: a,
                    right: by_name(divisor),
                }
            },
            "{guarded}, {divisor}"
        );
        assert_eq!(returned(machine), scalar(operation));
        assert_eq!(
            guard(&machine.contract.crash_routes),
            (CrashCause::Trap, by_name(guarded))
        );
        // The machine owes coverage of the trap at the division itself.
        let tables = lowered.elaboration.tables.machines.get(machine.id).unwrap();
        let owed: Vec<_> = tables.obligations.iter().map(|(_, row)| row.kind).collect();
        assert_eq!(
            owed,
            [ObligationKind::CrashCoverage {
                site: ExecutionSite::Operation(operation.id),
                cause: CrashCause::Trap,
            }]
        );
    }
}

#[test]
fn a_contract_index_past_a_fixed_extent_is_refused_and_its_last_index_lowers() {
    let Err(refusal) = Program::try_of(&[(
        "product/main.omg",
        "machine bad(a: [u8; 4]) requires a[4] < 5 {}\n",
    )]) else {
        panic!("stage 05 must refuse `a[4]` of a `[u8; 4]`")
    };
    assert_eq!(
        (refusal.stage, refusal.message.as_str(), refusal.at.as_str()),
        (
            "lowering",
            "Violation: contract array index 4 is outside its fixed extent 4 while reading the \
             signature of `bad`",
            "4"
        )
    );
    let program = Program::of("machine good(a: [u8; 4]) requires a[3] < 5 {}\n");
    let lowered = program.lower();
    assert_eq!(lowered.machine("good").contract.requires.len(), 1);
}
