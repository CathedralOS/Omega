//! Stage 05 must test a `match` subject once and in the order the arms are
//! written, and must hand each case arm the payload of its own case. A case
//! edge delivers the payload fields the arm binds as parameters of the block
//! it enters, so an arm that reads `value` reads its own edge's parameter.
//! The tests write small programs and read the edges and operations stage 05
//! produced.
//!
//! The spec rules: a value-position `match` evaluates its subject once and
//! takes the first arm that matches (`wiki/spec/language/expressions.md`,
//! "Evaluation schedule"), and "A case pattern" binds the fields of the
//! case it names (`wiki/spec/language/patterns.md`, "Data and case
//! decomposition").

#[path = "../../06_lowered-program-to-checked-program/tests/support/source_handoff.rs"]
mod program;

use program::{Program, scalar};
use semantic_vocabulary::{BlockId, ValueId};
use terminal_psi::{
    OperationKind, StructuralPathSegment, TerminalAffineCleanupAction, TerminalMachine, Terminator,
};

/// The parameters of the block each case edge enters, in case order.
fn case_arms(machine: &TerminalMachine) -> Vec<(BlockId, Vec<ValueId>)> {
    let entry = machine
        .blocks
        .iter()
        .find(|block| block.id == machine.entry)
        .unwrap();
    let Terminator::StructuralCase { cases, .. } = &entry.terminator else {
        panic!("the entry dispatches on the case: {:?}", entry.terminator)
    };
    cases
        .iter()
        .map(|edge| {
            let block = machine
                .blocks
                .iter()
                .find(|block| block.id == edge.target)
                .unwrap();
            (
                block.id,
                block.parameters.iter().map(|value| value.id).collect(),
            )
        })
        .collect()
}

/// `match sample(x) { 300u16 -> 4u8, 2u16 -> 5u8, _ -> 6u8, }` calls
/// `sample` once and compares that one result with 300, then with 2. Each
/// arm hands its value to the join, where the sum is formed.
#[test]
fn one_subject_is_tested_against_each_value_in_written_order() {
    let program = Program::of(
        "machine sample(x: u16) -> u16 { x } \
         machine choose(x: u16) -> u8 { 1u8 +% match sample(x) { 300u16 -> 4u8, 2u16 -> 5u8, _ -> 6u8, } }",
    );
    let lowered = program.lower();
    let operations = lowered.operations("choose");
    let calls: Vec<_> = operations
        .iter()
        .filter(|(_, operation)| matches!(operation.kind, OperationKind::Call { .. }))
        .collect();
    let [(_, call)] = calls[..] else {
        panic!("one call of `sample`")
    };
    let tested: Vec<_> = operations
        .iter()
        .filter_map(|(written, operation)| match operation.kind {
            OperationKind::IntegerEqual { left, .. } => Some((*written, left)),
            _ => None,
        })
        .collect();
    assert_eq!(tested, [("300u16", scalar(call)), ("2u16", scalar(call))]);
    let machine = lowered.machine("choose");
    let sum = machine
        .operations()
        .find(|operation| matches!(operation.kind, OperationKind::WrappingIntegerAdd { .. }))
        .unwrap();
    let join = machine
        .blocks
        .iter()
        .find(|block| block.operations.iter().any(|held| held.id == sum.id))
        .unwrap();
    let [joined] = join.parameters[..] else {
        panic!("the join receives the arm's value")
    };
    assert!(matches!(
        sum.kind,
        OperationKind::WrappingIntegerAdd { right, .. } if right == joined.id
    ));
    let arrivals = machine
        .blocks
        .iter()
        .filter(|block| matches!(&block.terminator, Terminator::Jump { target, .. } if *target == join.id))
        .count();
    assert_eq!(arrivals, 3);
    program.judged(|_| {}).accepted();
}

/// The outer `match` of a nested one tests the value the inner `match`
/// hands its join, not the inner subject.
#[test]
fn a_match_on_a_match_tests_the_inner_join_value() {
    let program = Program::of(
        "machine sample(x: u8) -> u8 { x } \
         machine choose(x: u8) -> u8 { \
             let answer: u8 = match (match sample(x) { 7u8 -> 300u16, _ -> 301u16, }) \
                 { 300u16 -> 8u8, _ -> 9u8, }; answer }",
    );
    let lowered = program.lower();
    let machine = lowered.machine("choose");
    let tests: Vec<_> = machine
        .operations()
        .filter_map(|operation| match operation.kind {
            OperationKind::IntegerEqual { left, .. } => Some(left),
            _ => None,
        })
        .collect();
    let [inner, outer] = tests[..] else {
        panic!("two tests: {tests:?}")
    };
    let call = machine
        .operations()
        .find(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .unwrap();
    assert_eq!(inner, scalar(call));
    let join = machine
        .blocks
        .iter()
        .find(|block| block.parameters.iter().any(|value| value.id == outer))
        .expect("the outer test reads a join parameter");
    let arrivals: Vec<_> = machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } if *target == join.id => Some(arguments.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(arrivals.len(), 2, "both inner arms hand over their value");
    program.judged(|_| {}).accepted();
}

/// `Choice::A { code: 1u8, value } -> value` tests the first payload field
/// the `A` edge delivers against 1 and returns the second; `Choice::B {
/// code: 3u8, value }` in a sibling arm reads the fields its own `B` edge
/// delivers, although both cases name their fields alike.
#[test]
fn a_case_arm_binds_the_payload_its_own_edge_delivers() {
    let program = Program::of(
        "data Choice { case A(code: u8, value: u8); case B; } \
         machine first(x: @Choice) -> u8 { match x { Choice::A { code: 1u8, value } -> value, _ -> 0u8, } }",
    );
    let lowered = program.lower();
    let machine = lowered.machine("first");
    let arms = case_arms(machine);
    let [code, value] = arms[0].1[..] else {
        panic!("the `A` edge delivers two fields: {arms:?}")
    };
    assert!(machine.operations().any(|operation| matches!(
        operation.kind,
        OperationKind::IntegerEqual { left, .. } if left == code
    )));
    assert!(
        machine.blocks.iter().any(
            |block| matches!(block.terminator, Terminator::Return { value: returned, .. } if returned == value)
        ),
        "the arm returns the second field"
    );
    program.judged(|_| {}).accepted();

    let siblings = Program::of(
        "data Choice { case A(code: u8, value: u8); case B(code: u8, value: u8); } \
         machine first(x: @Choice) -> bool { let result: bool = match x { \
             Choice::A { code: 1u8, value } -> value == 2u8, \
             Choice::B { code: 3u8, value } -> value == 4u8, _ -> false, }; result }",
    );
    let lowered = siblings.lower();
    let machine = lowered.machine("first");
    let arms = case_arms(machine);
    let compared: Vec<_> = machine
        .operations()
        .filter_map(|operation| match operation.kind {
            OperationKind::IntegerEqual { left, .. } => Some(left),
            _ => None,
        })
        .collect();
    assert_eq!(
        compared,
        [arms[0].1[0], arms[0].1[1], arms[1].1[0], arms[1].1[1]],
        "each arm tests `code` and `value` of its own case"
    );
    siblings.judged(|_| {}).accepted();
}

/// Each case arm adds its own payload to itself, whether the subject is an
/// owned formal, a borrowed one or a field of a borrowed record. The owned
/// subject is disposed of on each case edge once its payload is taken; a
/// borrowed one is not.
#[test]
fn each_case_arm_reads_its_own_payload_from_the_subject_written() {
    let choice =
        "data Choice { case Left(x: u16); case Right(x: u16); } data Holder { c: Choice; }";
    let arms = "{ Choice::Left { x as n } -> n +% n, Choice::Right { x as n } -> n +% n, }";
    for (parameter, subject, owned, through_field) in [
        ("c: Choice", "c", true, false),
        ("c: @Choice", "c", false, false),
        ("box: @Holder", "box.c", false, true),
    ] {
        let program = Program::of(&format!(
            "{choice} machine f({parameter}) -> u16 {{ match {subject} {arms} }}"
        ));
        let lowered = program.lower();
        let machine = lowered.machine("f");
        let formal = machine.contract.structural_parameters[0].place;
        let entry = machine
            .blocks
            .iter()
            .find(|block| block.id == machine.entry)
            .unwrap();
        let Terminator::StructuralCase {
            source,
            path,
            cases,
        } = &entry.terminator
        else {
            panic!("{parameter}: {:?}", entry.terminator)
        };
        assert_eq!(*source, formal, "{parameter}");
        assert_eq!(
            matches!(path.as_slice(), [StructuralPathSegment::Field(_)]),
            through_field,
            "{parameter}: {path:?}"
        );
        for edge in cases {
            assert_eq!(
                edge.cleanup_actions == [TerminalAffineCleanupAction::DiscardRoot(formal)],
                owned,
                "{parameter}: {edge:?}"
            );
            let block = machine
                .blocks
                .iter()
                .find(|block| block.id == edge.target)
                .unwrap();
            let [payload] = block.parameters[..] else {
                panic!("one payload field")
            };
            let sum = block.operations.first().expect("the arm adds");
            assert_eq!(
                sum.kind,
                OperationKind::WrappingIntegerAdd {
                    left: payload.id,
                    right: payload.id
                },
                "{parameter}"
            );
        }
        program.judged(|_| {}).accepted();
    }
}
