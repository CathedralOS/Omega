//! Stage 05 must lower each value stage 04 settled to that exact value: a
//! constant's read to the declaration it names, a pattern to its literal, a
//! folded operand to the one constant it lands as, and a record constant to
//! its fields in declaration order. The tests write small programs, read
//! the operations stage 05 produced, and change one input at a time: a
//! source twin, or one fact in stage 04's product.
//!
//! The spec pages: `wiki/spec/language/constants.md` ("Evaluation"),
//! `wiki/spec/language/patterns.md` ("Coverage and domain tests"),
//! `wiki/spec/language/numeric_values.md` ("Literal landing and
//! destinations") and `wiki/spec/terminal-psi/structural_access.md`
//! ("Complete record construction").

#[path = "../../06_lowered-program-to-checked-program/tests/support/source_handoff.rs"]
mod program;

use program::{Program, returned, scalar};
use semantic_vocabulary::{IntegerValue, StructuralFieldId, ValueId};
use source_compiler::representations::source_state::expression::ExpressionNode;
use terminal_psi::{
    Operation, OperationKind, RecordFieldInitializer, RecordFieldValue, StructuralTypeShape,
    TerminalMachine, Terminator,
};

fn kinds(operations: &[(&str, &Operation)]) -> Vec<OperationKind> {
    operations
        .iter()
        .map(|(_, operation)| operation.kind.clone())
        .collect()
}

/// The one integer constant `machine` computes and returns.
fn returned_constant(lowered: &program::Lowered<'_>, path: &str) -> IntegerValue {
    let [(_, operation)] = lowered.operations(path)[..] else {
        panic!("`{path}` is one constant")
    };
    let OperationKind::IntegerConstant { value } = operation.kind else {
        panic!("`{path}` returns a constant: {operation:?}")
    };
    assert_eq!(returned(lowered.machine(path)), scalar(operation));
    value
}

#[test]
fn a_constant_read_returns_the_value_of_the_declaration_it_names() {
    for (declarations, left) in [
        ("const LEFT: u8 = 17u8;\nconst RIGHT: u8 = 42u8;", 17),
        ("const RIGHT: u8 = 42u8;\nconst LEFT: u8 = 17u8;", 17),
        ("const RIGHT: u8 = 42u8;\nconst LEFT: u8 = 91u8;", 91),
    ] {
        let program = Program::of(&format!(
            "{declarations}
machine get() -> u8 {{ LEFT }}
machine other() -> u8 {{ RIGHT }}
"
        ));
        let lowered = program.lower();
        assert_eq!(
            [
                returned_constant(&lowered, "get"),
                returned_constant(&lowered, "other")
            ],
            [IntegerValue::Unsigned(left), IntegerValue::Unsigned(42)],
            "{declarations}"
        );
    }

    // The one changed fact: `get`'s read names `RIGHT`. Only `get` follows.
    let program = Program::of(
        "const LEFT: u8 = 17u8;
const RIGHT: u8 = 42u8;
machine get() -> u8 { LEFT }
machine other() -> u8 { RIGHT }
",
    );
    let is_name = |node: &ExpressionNode| matches!(node, ExpressionNode::Name(_));
    let (left, right) = (
        program.expression("LEFT", is_name),
        program.expression("RIGHT", is_name),
    );
    let unchanged = program.prepared_with(|_| {});
    let lowered = program.lower_prepared(&unchanged).unwrap();
    assert_eq!(
        returned_constant(&lowered, "get"),
        IntegerValue::Unsigned(17)
    );
    let altered = program.prepared_with(|trees| {
        let read = trees.expression_table.expression(right).clone();
        *trees.expression_table.expression_mut(left) = read;
    });
    let lowered = program.lower_prepared(&altered).unwrap();
    assert_eq!(
        [
            returned_constant(&lowered, "get"),
            returned_constant(&lowered, "other")
        ],
        [IntegerValue::Unsigned(42), IntegerValue::Unsigned(42)]
    );
}

/// `machine`'s value match: the pattern constant it compares its subject
/// with, checked to be one typed comparison of the subject followed by a
/// branch whose true arm returns 1 and false arm returns 2.
fn tested_pattern(machine: &TerminalMachine) -> IntegerValue {
    let block = |id| machine.blocks.iter().find(|block| block.id == id).unwrap();
    let [subject] = machine.contract.parameters[..] else {
        panic!("one subject")
    };
    let entry = block(machine.entry);
    let [constant, equal] = entry.operations.as_slice() else {
        panic!("one comparison: {:?}", entry.operations)
    };
    let OperationKind::IntegerConstant { value } = constant.kind else {
        panic!("the pattern is a constant: {constant:?}")
    };
    // The constant has the subject's type.
    assert_eq!(
        constant.result.scalar_ref().unwrap().scalar_type,
        subject.scalar_type
    );
    assert_eq!(
        equal.kind,
        OperationKind::IntegerEqual {
            left: subject.id,
            right: scalar(constant),
        }
    );
    let Terminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &entry.terminator
    else {
        panic!("{:?}", entry.terminator)
    };
    assert_eq!(*condition, scalar(equal));
    for (edge, arm) in [(when_true, 1), (when_false, 2)] {
        let target = block(edge.target);
        let [literal] = target.operations.as_slice() else {
            panic!("{:?}", target.operations)
        };
        assert_eq!(
            literal.kind,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(arm)
            }
        );
        assert!(
            matches!(target.terminator, Terminator::Return { value, .. } if value == scalar(literal))
        );
    }
    value
}

#[test]
fn a_value_pattern_compares_with_its_own_literal_however_it_is_grouped() {
    let program = Program::of(
        "machine plain(value: u8) -> u8 { match value { 0u8 -> 1u8, _ -> 2u8, } }
machine grouped(value: u8) -> u8 { match value { (0u8) -> 1u8, _ -> 2u8, } }
machine twice(value: u8) -> u8 { match value { ((0u8)) -> 1u8, _ -> 2u8, } }
",
    );
    let lowered = program.lower();
    let plain = lowered.machine("plain");
    assert_eq!(tested_pattern(plain), IntegerValue::Unsigned(0));
    // Grouping changes nothing stage 05 produces.
    for grouped in ["grouped", "twice"] {
        assert_eq!(
            kinds(&lowered.operations(grouped)),
            kinds(&lowered.operations("plain")),
            "{grouped}"
        );
        assert_eq!(
            tested_pattern(lowered.machine(grouped)),
            IntegerValue::Unsigned(0)
        );
    }

    // The one changed fact: the pattern's literal is `5u8`. The comparison
    // follows it, and the arms stay where they were.
    let program = Program::of(
        "machine plain(value: u8) -> u8 { match value { 0u8 -> 1u8, _ -> 2u8, } }
machine five() -> u8 { 5u8 }
",
    );
    let is_integer = |node: &ExpressionNode| matches!(node, ExpressionNode::Integer(_));
    let (pattern, five) = (
        program.expression("0u8", is_integer),
        program.expression("5u8", is_integer),
    );
    let altered = program.prepared_with(|trees| {
        let literal = trees.expression_table.expression(five).clone();
        *trees.expression_table.expression_mut(pattern) = literal;
    });
    let lowered = program.lower_prepared(&altered).unwrap();
    assert_eq!(
        tested_pattern(lowered.machine("plain")),
        IntegerValue::Unsigned(5)
    );
}

#[test]
fn a_folded_negative_operand_lands_as_one_signed_constant() {
    let program = Program::of(
        "machine a(x: i8) -> bool { x > -1 }
machine b(x: i8) -> bool { x > (-(1)) }
machine c(x: i8) -> bool { x > 1 - 2 }
machine d(x: i8, y: i8) -> bool { x > -y }
",
    );
    let lowered = program.lower();
    for name in ["a", "b", "c"] {
        let machine = lowered.machine(name);
        let [x] = machine.contract.parameters[..] else {
            panic!("one formal")
        };
        let [(_, constant), (_, less)] = lowered.operations(name)[..] else {
            panic!("`{name}` is one constant and one comparison")
        };
        assert_eq!(
            constant.kind,
            OperationKind::IntegerConstant {
                value: IntegerValue::Signed(-1)
            },
            "{name}"
        );
        assert_eq!(
            constant.result.scalar_ref().unwrap().scalar_type,
            x.scalar_type
        );
        // `x > k` is `k < x`.
        assert_eq!(
            less.kind,
            OperationKind::IntegerLessThan {
                left: scalar(constant),
                right: x.id,
            },
            "{name}"
        );
        assert_eq!(returned(machine), scalar(less));
    }
    // A negated formal is computed at run time: `0 - y`, then the comparison.
    let machine = lowered.machine("d");
    let [x, y] = machine.contract.parameters[..] else {
        panic!("two formals")
    };
    let [(_, zero), (_, negate), (_, less)] = lowered.operations("d")[..] else {
        panic!("`d` negates and compares")
    };
    assert_eq!(
        zero.kind,
        OperationKind::IntegerConstant {
            value: IntegerValue::Signed(0)
        }
    );
    assert!(matches!(
        negate.kind,
        OperationKind::ExactIntegerSubtract { left, right, .. }
            if left == scalar(zero) && right == y.id
    ));
    assert_eq!(
        less.kind,
        OperationKind::IntegerLessThan {
            left: scalar(negate),
            right: x.id,
        }
    );
}

/// The field ids of the record `name` declares, by field name.
fn record_fields(lowered: &program::Lowered<'_>, name: &str) -> Vec<(String, StructuralFieldId)> {
    let module = &lowered.elaboration.module;
    let named = format!(r"path\({name}\)");
    let declared = module
        .types
        .structural_types
        .iter()
        .find(|declared| {
            matches!(declared.shape, StructuralTypeShape::Record { .. })
                && module
                    .identities
                    .text(declared.identity)
                    .is_some_and(|identity| identity.contains(&named))
        })
        .unwrap_or_else(|| panic!("stage 05 declared no record `{name}`"));
    let StructuralTypeShape::Record { fields, .. } = &declared.shape else {
        unreachable!()
    };
    fields
        .iter()
        .map(|field| {
            (
                module.identities.text(field.identity).unwrap().to_owned(),
                field.id,
            )
        })
        .collect()
}

/// The record `machine` establishes and returns, as `(field name, value)`
/// in the order of the established fields.
fn established(lowered: &program::Lowered<'_>, path: &str) -> Vec<(String, ValueId)> {
    let fields = record_fields(lowered, "Pair");
    let machine = lowered.machine(path);
    let mut records = machine
        .operations()
        .filter_map(|operation| match &operation.kind {
            OperationKind::EstablishRecord { fields } => Some((operation, fields)),
            _ => None,
        });
    let (operation, initializers) = records.next().expect("one record");
    assert!(records.next().is_none());
    let place = operation.result.structural().unwrap().place;
    assert!(machine.blocks.iter().any(|block| matches!(
        block.terminator,
        Terminator::ReturnStructural { source, .. } if source == place
    )));
    initializers
        .iter()
        .map(|RecordFieldInitializer { field, value }| {
            let RecordFieldValue::Scalar { value, .. } = value else {
                panic!("a scalar field: {value:?}")
            };
            let (name, _) = fields.iter().find(|(_, id)| id == field).unwrap();
            (name.clone(), *value)
        })
        .collect()
}

#[test]
fn a_record_is_established_in_declaration_order_whatever_order_names_its_fields() {
    let program = Program::of(
        "data Pair [copy] { first: u8; second: u8; }
const VALUE: Pair = Pair { second: 9u8, first: 2u8 };
machine frozen() -> Pair { VALUE }
machine make(a: u8, b: u8) -> Pair { Pair { second: a, first: b } }
",
    );
    let lowered = program.lower();
    // The constant's values come from its initializer in written order, and
    // are stored in declared order.
    let operations = lowered.operations("frozen");
    let constants: Vec<_> = operations
        .iter()
        .filter_map(|(_, operation)| match operation.kind {
            OperationKind::IntegerConstant { value } => Some((value, scalar(operation))),
            _ => None,
        })
        .collect();
    let [(nine, nine_value), (two, two_value)] = constants[..] else {
        panic!("two constants: {constants:?}")
    };
    assert_eq!(
        (nine, two),
        (IntegerValue::Unsigned(9), IntegerValue::Unsigned(2))
    );
    assert_eq!(
        established(&lowered, "frozen"),
        [("first".into(), two_value), ("second".into(), nine_value)]
    );
    let make = lowered.machine("make");
    let [a, b] = make.contract.parameters[..] else {
        panic!("two formals")
    };
    assert_eq!(
        established(&lowered, "make"),
        [("first".into(), b.id), ("second".into(), a.id)]
    );

    // Twin: the literal names the formals the other way round, and only
    // which value lands in which slot changes.
    let program = Program::of(
        "data Pair [copy] { first: u8; second: u8; }
machine make(a: u8, b: u8) -> Pair { Pair { second: b, first: a } }
",
    );
    let lowered = program.lower();
    let make = lowered.machine("make");
    let [a, b] = make.contract.parameters[..] else {
        panic!("two formals")
    };
    assert_eq!(
        established(&lowered, "make"),
        [("first".into(), a.id), ("second".into(), b.id)]
    );
}
