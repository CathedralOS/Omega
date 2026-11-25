//! Stage 05 must lower each call with the arguments written, in the order
//! written and each evaluated once, and must lower each branch so that an
//! arm's operations run only on that arm and its value reaches the join or
//! the return it belongs to. The tests write small programs and read the
//! operations, blocks and edges stage 05 produced; where the checker can
//! tell a right product from a wrong one, a twin shows its verdict.
//!
//! The spec rules: arguments are evaluated left to right, once
//! (`wiki/spec/language/expressions.md`, "Evaluation schedule"); a
//! value-position `match` evaluates its subject once and only the first
//! matching arm (`expressions.md`); a non-Unit call result is discarded only
//! by `_ = call();`, "subject to the result's ordinary ownership and
//! legal-disposition obligations; the marker cannot erase linear custody"
//! (`wiki/spec/language/effects.md`, "Recoverable outcomes and strict use").

#[path = "../../06_lowered-program-to-checked-program/tests/support/source_handoff.rs"]
mod program;

use program::{Program, scalar};
use semantic_vocabulary::{BlockId, ValueId};
use terminal_psi::{Operation, OperationKind, StructuralAccess, TerminalMachine, Terminator};

fn formals(machine: &TerminalMachine) -> Vec<ValueId> {
    machine
        .contract
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect()
}

/// The block of `machine` that holds `operation`.
fn block_of(machine: &TerminalMachine, operation: &Operation) -> BlockId {
    machine
        .blocks
        .iter()
        .find(|block| block.operations.iter().any(|held| held.id == operation.id))
        .expect("the operation is in a block")
        .id
}

/// `choose(true, keep(array), offset, other)` passes its two scalars and its
/// two arrays in the order written. The array `keep` returns is a fresh
/// place passed whole, `other` is copied once and passed whole, and `keep`
/// itself receives a copy of `array`.
#[test]
fn a_call_passes_scalars_and_whole_arrays_in_the_order_written() {
    let program = Program::of(
        "machine keep(array: [[u8; 2]; 3]) -> [[u8; 2]; 3] { array } \
         machine choose(flag: bool, left: [[u8; 2]; 3], offset: u8, right: [[u8; 2]; 3]) \
             -> [[u8; 2]; 3] { right } \
         machine caller(array: [[u8; 2]; 3], other: [[u8; 2]; 3], offset: u8) -> [[u8; 2]; 3] { \
             choose(true, keep(array), offset, other) }",
    );
    let lowered = program.lower();
    let caller = lowered.machine("caller");
    let [offset] = formals(caller)[..] else {
        panic!("one scalar formal")
    };
    let [array, other] =
        [0, 1].map(|position| caller.contract.structural_parameters[position].place);
    let operations = lowered.operations("caller");
    let kinds: Vec<_> = operations
        .iter()
        .map(|(written, operation)| (*written, operation))
        .collect();
    let [
        (_, flag),
        (_, copied),
        (_, kept),
        (_, other_copy),
        (_, chosen),
    ] = kinds[..]
    else {
        panic!("five operations: {kinds:?}")
    };
    assert!(matches!(
        flag.kind,
        OperationKind::BooleanConstant { value: true }
    ));
    assert!(
        matches!(copied.kind, OperationKind::StructuralLeafCopy { source, .. } if source == array)
    );
    let place = |operation: &Operation| operation.result.as_structural().unwrap().place;
    let OperationKind::CallStructuralWithScalarArguments {
        callee,
        structural_arguments,
        ..
    } = &kept.kind
    else {
        panic!("{kept:?}")
    };
    assert_eq!(*callee, lowered.machine_id("keep"));
    assert_eq!(
        structural_arguments
            .iter()
            .map(|argument| (argument.place, argument.access))
            .collect::<Vec<_>>(),
        [(place(copied), StructuralAccess::Owned)]
    );
    assert!(
        matches!(other_copy.kind, OperationKind::StructuralLeafCopy { source, .. } if source == other)
    );
    let OperationKind::CallStructuralWithScalarArguments {
        callee,
        arguments,
        structural_arguments,
        ..
    } = &chosen.kind
    else {
        panic!("{chosen:?}")
    };
    assert_eq!(*callee, lowered.machine_id("choose"));
    assert_eq!(arguments.as_slice(), [scalar(flag), offset]);
    assert_eq!(
        structural_arguments
            .iter()
            .map(|argument| argument.place)
            .collect::<Vec<_>>(),
        [place(kept), place(other_copy)]
    );
    program.judged(|_| {}).accepted();
}

/// Two calls of a machine with no result are two operations in written
/// order, each with the one argument, and neither produces a value.
#[test]
fn unit_calls_keep_their_order_and_produce_nothing() {
    let program = Program::of(
        "machine consume(value: bool) {} \
         machine caller(flag: bool) { consume(flag); consume(flag); }",
    );
    let lowered = program.lower();
    let [flag] = formals(lowered.machine("caller"))[..] else {
        panic!("one formal")
    };
    let consume = lowered.machine_id("consume");
    let calls = lowered.operations("caller");
    assert_eq!(calls.len(), 2, "{calls:?}");
    for (_, call) in calls {
        assert!(
            matches!(&call.kind, OperationKind::CallUnit { callee, arguments, .. }
                if *callee == consume && arguments.as_slice() == [flag]),
            "{call:?}"
        );
        assert_eq!(call.result, terminal_psi::OperationResult::Unit);
    }
}

fn discard(declaration: &str) -> Result<Program, program::EarlierRefusal> {
    Program::try_of(&[(
        "product/main.omg",
        &format!(
            "data Handle {declaration} {{ v: u8; }} \
             machine make() -> Handle {{ Handle {{ v: 1u8 }} }} \
             machine caller() {{ _ = make(); }}"
        ),
    )])
}

/// `_ = leaf(x)` calls `leaf` once and drops only its scalar result. `_ =
/// make()` disposes of an affine `Handle` and is accepted, but a linear one
/// may not be discarded: the program is rejected before it reaches the
/// checker, because no edge may dispose of the linear result.
#[test]
fn a_discard_drops_one_call_result_and_cannot_drop_a_linear_one() {
    let program =
        Program::of("machine leaf(x: u8) -> u8 { x } machine caller(x: u8) { _ = leaf(x); }");
    let lowered = program.lower();
    let [x] = formals(lowered.machine("caller"))[..] else {
        panic!("one formal")
    };
    let [(_, call)] = lowered.operations("caller")[..] else {
        panic!("one call")
    };
    assert!(
        matches!(&call.kind, OperationKind::Call { callee, arguments, .. }
            if *callee == lowered.machine_id("leaf") && arguments.as_slice() == [x]),
        "{call:?}"
    );
    program.judged(|_| {}).accepted();

    discard("").unwrap().judged(|_| {}).accepted();
    let linear = discard("[linear]")
        .err()
        .expect("a linear result cannot be discarded");
    assert_eq!(
        (linear.stage, linear.message.as_str()),
        (
            "lowering",
            "Violation: while checking control-flow edge: a linear value is still held when \
             this path leaves its storage, and nothing consumes it: every path must pass it on, \
             return it or end it by its declared disposition, and neither `_ =` nor leaving \
             scope can discard it"
        )
    );
}

/// A `match` statement whose arms call Unit machines runs only the selected
/// arm's call, and every arm continues at one join that takes no value,
/// where `after` is called (`patterns.md`, "Coverage and domain tests": "An
/// intentional Unit completion is explicit, for example `_ -> {}`"). This
/// holds with a subject and without one, and for a `{}` arm.
#[test]
fn a_match_of_unit_calls_rejoins_before_the_next_call() {
    for (body, arm_calls) in [
        (
            "match flag { true -> act(1u8), false -> act(2u8), }; after();",
            2,
        ),
        ("match { flag -> act(1u8), _ -> act(2u8), }; after();", 2),
        ("match { flag -> act(1u8), _ -> {} }; after();", 1),
    ] {
        let program = Program::of(&format!(
            "machine act(x: u8) {{}} machine after() {{}} \
             machine choose(flag: bool) {{ {body} }}"
        ));
        let lowered = program.lower();
        let machine = lowered.machine("choose");
        let (act, after) = (lowered.machine_id("act"), lowered.machine_id("after"));
        let calls = lowered.operations("choose");
        let arms: Vec<_> = calls
            .iter()
            .filter(|(_, call)| matches!(&call.kind, OperationKind::CallUnit { callee, .. } if *callee == act))
            .map(|(_, call)| block_of(machine, call))
            .collect();
        assert_eq!(arms.len(), arm_calls, "{body}");
        if let [first, second] = arms[..] {
            assert_ne!(first, second, "{body}");
        }
        let [(_, last)] = calls
            .iter()
            .filter(|(_, call)| matches!(&call.kind, OperationKind::CallUnit { callee, .. } if *callee == after))
            .copied()
            .collect::<Vec<_>>()[..]
        else {
            panic!("one call of `after`: {body}")
        };
        let join = block_of(machine, last);
        let joined = machine
            .blocks
            .iter()
            .find(|block| block.id == join)
            .unwrap();
        assert!(joined.parameters.is_empty() && joined.structural_parameters.is_empty());
        // Each arm's block jumps to the join with no argument, and the arm
        // calls run before it on separate paths.
        let arrivals: Vec<_> = machine
            .blocks
            .iter()
            .filter(|block| matches!(&block.terminator, Terminator::Jump { target, .. } if *target == join))
            .map(|block| {
                let Terminator::Jump { arguments, .. } = &block.terminator else {
                    unreachable!()
                };
                assert!(arguments.is_empty(), "{body}");
                block.id
            })
            .collect();
        assert_eq!(arrivals.len(), 2, "{body}: {machine:?}");
        for arm in &arms {
            assert!(arrivals.contains(arm), "{body}: {machine:?}");
        }
        program.judged(|_| {}).accepted();
    }
}

/// The negative twins: an arm that produces a value cannot join an arm that
/// completes with Unit, in either order, and a Unit arm does not supply a
/// value the context receives ("A result-producing context still requires a
/// compatible result").
#[test]
fn a_unit_arm_and_a_value_arm_do_not_join() {
    for (body, message) in [
        (
            "match flag { true -> act(1u8), false -> 2u8, };",
            "this match arm produces a value, but an earlier arm completes with Unit",
        ),
        (
            "match flag { true -> 2u8, false -> act(1u8), };",
            "this match arm completes with Unit, but the match produces a value",
        ),
        (
            "let x: u8 = match flag { true -> act(1u8), false -> act(2u8), };",
            "this match arm completes with Unit, but the match produces a value",
        ),
    ] {
        let refusal = Program::try_of(&[(
            "product/main.omg",
            &format!("machine act(x: u8) {{}} machine choose(flag: bool) {{ {body} }}"),
        )])
        .err()
        .unwrap_or_else(|| panic!("refused: {body}"));
        assert_eq!(refusal.stage, "lowering", "{refusal:?}");
        assert!(
            refusal
                .message
                .starts_with(&format!("Violation: {message}")),
            "{body}: {refusal:?}"
        );
    }
}

/// `pair(left +% 1u8, (match flag { ... }) +% 4u8)` computes the first
/// argument before the `match` branches. Each arm computes its own sum only
/// on its own edge and hands it to the join, and the join adds 4 and then
/// calls `pair` with the two arguments in order.
#[test]
fn a_match_in_an_argument_joins_before_the_next_operand_and_the_call() {
    let program = Program::of(
        "machine pair(left: u8, right: u8) -> u8 { left +% right } \
         machine choose(flag: bool, left: u8, right: u8) -> u8 { \
             pair(left +% 1u8, (match flag { false -> left +% 2u8, true -> right +% 3u8, }) +% 4u8) }",
    );
    let lowered = program.lower();
    let machine = lowered.machine("choose");
    let [_, left, right] = formals(machine)[..] else {
        panic!("three formals")
    };
    let sums: Vec<_> = machine
        .operations()
        .filter(|operation| matches!(operation.kind, OperationKind::WrappingIntegerAdd { .. }))
        .collect();
    let [first, false_arm, true_arm, plus_four] = sums[..] else {
        panic!("four sums: {sums:?}")
    };
    let entry = machine.entry;
    assert_eq!(block_of(machine, first), entry);
    assert!(matches!(first.kind, OperationKind::WrappingIntegerAdd { left: l, .. } if l == left));
    assert!(
        matches!(false_arm.kind, OperationKind::WrappingIntegerAdd { left: l, .. } if l == left)
    );
    assert!(
        matches!(true_arm.kind, OperationKind::WrappingIntegerAdd { left: l, .. } if l == right)
    );
    let join = block_of(machine, plus_four);
    let join_block = machine
        .blocks
        .iter()
        .find(|block| block.id == join)
        .unwrap();
    let [joined] = join_block.parameters[..] else {
        panic!("the join receives one value")
    };
    assert!(
        matches!(plus_four.kind, OperationKind::WrappingIntegerAdd { left: l, .. } if l == joined.id)
    );
    let mut arrivals: Vec<_> = machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } if *target == join => Some((block.id, arguments.clone())),
            _ => None,
        })
        .collect();
    arrivals.sort();
    assert_eq!(
        arrivals,
        [
            (block_of(machine, false_arm), vec![scalar(false_arm)]),
            (block_of(machine, true_arm), vec![scalar(true_arm)]),
        ]
    );
    let call = machine
        .operations()
        .find(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .unwrap();
    assert_eq!(block_of(machine, call), join);
    assert!(matches!(&call.kind, OperationKind::Call { arguments, .. }
            if arguments.as_slice() == [scalar(first), scalar(plus_four)]));
    program.judged(|_| {}).accepted();
}

/// A `transition` whose only arm is the wildcard calls its machine with no
/// branch at all.
#[test]
fn a_wildcard_only_transition_calls_without_a_condition() {
    let program = Program::of(
        "machine leaf(x: u8) -> u8 { x } machine choose(x: u8) -> u8 { transition { _ -> leaf(x), } }",
    );
    let lowered = program.lower();
    let machine = lowered.machine("choose");
    assert!(
        machine
            .blocks
            .iter()
            .all(|block| !matches!(block.terminator, Terminator::Conditional { .. })),
        "{machine:?}"
    );
    let [(_, call)] = lowered.operations("choose")[..] else {
        panic!("one call")
    };
    assert!(
        matches!(&call.kind, OperationKind::Call { callee, arguments, .. }
            if *callee == lowered.machine_id("leaf") && arguments.as_slice() == formals(machine))
    );
}

/// The guards of a match without a subject are tested in written order, and
/// a constant guard tests the value its declaration holds: `false`, then
/// `READY` (false), then `true`.
#[test]
fn constant_guards_are_tested_in_order_with_their_declared_values() {
    let program = Program::of(
        "const READY: bool = false; \
         machine choose() -> u8 { match { false -> 1u8, READY -> 2u8, true -> 3u8, } }",
    );
    let lowered = program.lower();
    let guards: Vec<_> = lowered
        .operations("choose")
        .into_iter()
        .filter_map(|(written, operation)| match operation.kind {
            OperationKind::BooleanConstant { value } => Some((written, value)),
            _ => None,
        })
        .collect();
    assert_eq!(guards, [("false", false), ("READY", false), ("true", true)]);
    program.judged(|_| {}).accepted();
}

const HELD: &str = "data Schema<'t> { bytes: @'t [u8]; } data Held<'t> { schema: Schema<'t>; }";

/// Each arm of a returned `match` builds its own record from its own formal
/// and returns it, after releasing the reference the other formal holds.
/// When the `false` arm returns the `held` formal itself, that arm returns
/// the formal's own place.
#[test]
fn each_arm_of_a_returned_match_returns_what_its_own_formal_built() {
    let program = Program::of(&format!(
        "{HELD} machine keep<'t>(first: bool, left: Schema<'t>, right: Schema<'t>) -> Held<'t> {{ \
             match first {{ true -> Held {{ schema: left }}, false -> Held {{ schema: right }}, }} }}"
    ));
    let lowered = program.lower();
    let machine = lowered.machine("keep");
    let [left, right] =
        [0, 1].map(|position| machine.contract.structural_parameters[position].place);
    let returned_from = |moved: semantic_vocabulary::PlaceId| {
        let moves: Vec<_> = machine
            .operations()
            .filter_map(|operation| match &operation.kind {
                OperationKind::MoveStructuralValue { source } => Some((
                    source.place,
                    operation.result.as_structural().unwrap().place,
                )),
                _ => None,
            })
            .collect();
        let (_, moved_to) = moves
            .into_iter()
            .find(|(source, _)| *source == moved)
            .expect("the formal is moved");
        let record = machine
            .operations()
            .find(|operation| match &operation.kind {
                OperationKind::EstablishRecord { fields } => fields.iter().any(|field| {
                    matches!(&field.value, terminal_psi::RecordFieldValue::Structural(argument)
                        if argument.place == moved_to)
                }),
                _ => false,
            })
            .expect("a record holds the moved formal");
        let built = record.result.as_structural().unwrap().place;
        let block = block_of(machine, record);
        let terminator = &machine
            .blocks
            .iter()
            .find(|b| b.id == block)
            .unwrap()
            .terminator;
        assert!(
            matches!(terminator, Terminator::ReturnStructural { source, .. } if *source == built),
            "{terminator:?}"
        );
        block
    };
    assert_ne!(returned_from(left), returned_from(right));
    program.judged(|_| {}).accepted();

    let forwarded = Program::of(&format!(
        "{HELD} machine keep<'t>(first: bool, left: Schema<'t>, held: Held<'t>) -> Held<'t> {{ \
             match first {{ true -> Held {{ schema: left }}, false -> held, }} }}"
    ));
    let lowered = forwarded.lower();
    let machine = lowered.machine("keep");
    let held = machine.contract.structural_parameters[1].place;
    let moved: Vec<_> = machine
        .operations()
        .filter_map(|operation| match &operation.kind {
            OperationKind::MoveStructuralValue { source } if source.place == held => {
                Some(operation.result.as_structural().unwrap().place)
            }
            _ => None,
        })
        .collect();
    let [moved] = moved[..] else {
        panic!("held is moved once")
    };
    assert!(machine.blocks.iter().any(|block| matches!(
        block.terminator,
        Terminator::ReturnStructural { source, .. } if source == moved
    )));
    forwarded.judged(|_| {}).accepted();
}

fn views(access: &str) -> Program {
    bounded_views(access, "requires right.length < 1000")
}

fn bounded_views(access: &str, right: &str) -> Program {
    Program::of(&format!(
        "machine measure(n: u64) -> u8 requires n < 1000 {{ 1u8 }} \
         machine pick<'a>(flag: bool, left: @'a {access}[u32], right: @'a {access}[u32]) -> u8 \
             requires left.length < 1000 {right} {{ \
             let chosen: @'a {access}[u32] = match flag {{ true -> left, false -> right, }}; \
             measure(chosen.length) }}"
    ))
}

/// A view chosen by a `match` keeps the length of whichever view arrived,
/// whether the views are shared or exclusive. An exclusive view is affine,
/// so stage 05 moves the joined view into the local `chosen`; the join names
/// its length, and the checker relates that name to each arm's view, so
/// `measure`'s `requires n < 1000` holds when both formals are bounded. The
/// twin without the bound on `right` is rejected at the call.
#[test]
fn a_joined_view_keeps_the_length_of_the_view_that_arrived() {
    for access in ["", "mutable "] {
        views(access).judged(|_| {}).accepted();
        let unbounded = bounded_views(access, "").judged(|_| {}).rejected();
        assert_eq!(
            unbounded.unproved,
            ["CallRequirement { invocation: InvocationId(1), clause: 0 }"],
            "{access}: {unbounded:?}"
        );
    }
}

/// A `match` that chooses between two exclusive views lends each arm's
/// formal exclusively into a fresh view and moves that view to the join; the
/// shared twin copies each formal's view instead. Each arm reads its own
/// formal.
#[test]
fn an_exclusive_view_arm_lends_and_moves_and_a_shared_one_copies() {
    let exclusive = views("mutable ");
    let lowered = exclusive.lower();
    let machine = lowered.machine("pick");
    let formals: Vec<_> = machine
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect();
    let lent: Vec<_> = machine
        .operations()
        .filter_map(|operation| match &operation.kind {
            OperationKind::EstablishMutableElementView { source, .. } => {
                Some((source.place, source.access))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        lent,
        [
            (formals[0], StructuralAccess::MutableBorrow),
            (formals[1], StructuralAccess::MutableBorrow),
        ]
    );
    let shared = views("");
    let lowered = shared.lower();
    let machine = lowered.machine("pick");
    let copied: Vec<_> = machine
        .operations()
        .filter_map(|operation| match &operation.kind {
            OperationKind::StructuralLeafCopy { source, .. } => Some(*source),
            _ => None,
        })
        .collect();
    let formals: Vec<_> = machine
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect();
    assert_eq!(copied, formals);
    shared.judged(|_| {}).accepted();
}

/// An argument that is itself a call may borrow storage that the outer call
/// also lends, when both loans are shared: `finish(first(@items), @items)`.
/// Shared loans "may duplicate or reborrow" (`wiki/spec/language/ownership.md`,
/// "Borrows and aliases"), and the shared loan moves nothing, so stage 05
/// lowers the call and the checker accepts it. The twin lends `items`
/// mutably to the inner call. The lowered form forms every loan at the call,
/// so it cannot say that the inner loan ends before the outer one starts,
/// and stage 05 refuses the twin by name.
#[test]
fn a_nested_call_may_borrow_shared_what_the_outer_call_lends_shared() {
    let program = |access: &str| {
        Program::try_of(&[(
            "product/main.omg",
            &format!(
                "machine first(items: @{access}[u8; 2]) -> u8 {{ items[0] }} \
                 machine finish(value: u8, items: @[u8; 2]) -> u8 {{ items[1] }} \
                 machine caller() -> u8 {{ \
                     let mutable items: [u8; 2] = [7u8, 11u8]; \
                     finish(first(@{access}items), @items) }}"
            ),
        )])
    };
    program("").unwrap().judged(|_| {}).accepted();
    let refused = program("mutable ")
        .err()
        .expect("a mutable inner loan beside the outer loan is refused");
    assert_eq!(refused.stage, "lowering", "{refused:?}");
    assert!(
        refused
            .message
            .contains("call argument that names storage another argument lends"),
        "{refused:?}"
    );
}
