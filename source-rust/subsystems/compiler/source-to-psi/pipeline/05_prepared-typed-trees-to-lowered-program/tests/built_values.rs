//! Stage 05 must build each value a machine returns or stores from what the
//! source wrote: a frozen constant field by field in declaration order and
//! with its own case, an omitted field from the zero of its own type, a
//! returned scalar from the constant or formal named, and a literal only in
//! the carrier its context receives. The tests write small programs and read
//! the operations stage 05 produced; where a program must be refused, they
//! pin the refusal and the text it points at.
//!
//! The spec rules: a record literal names each field, and its value is the
//! same in any written order (`wiki/spec/language/data_and_literals.md`);
//! `..` gives every omitted field its type's zero, the first case of a sum,
//! and "supplies no authority, borrowed reference, callable or proof
//! witness" (`data_and_literals.md`, "Explicit field omission");
//! "An expected type selects the carrier in which an anonymous literal
//! lands", and there is no implicit scalar conversion at a receiving type
//! (`wiki/spec/language/domains.md`, "Literals and expected types").

#[path = "../../06_lowered-program-to-checked-program/tests/support/source_handoff.rs"]
mod program;

use program::{EarlierRefusal, Lowered, Program, returned, scalar};
use semantic_vocabulary::{IeeeFloatValue, IntegerValue, StructuralFieldId};
use terminal_psi::{
    Operation, OperationKind, RecordFieldInitializer, RecordFieldValue, StructuralTypeShape,
    TerminalMachine,
};

/// Each field stage 05 declared, by its identity text.
fn field_name(lowered: &Lowered<'_>, field: StructuralFieldId) -> String {
    let module = &lowered.elaboration.module;
    module
        .types
        .structural_types
        .iter()
        .flat_map(|declared| match &declared.shape {
            StructuralTypeShape::Record { fields, .. } => fields.clone(),
            StructuralTypeShape::Sum { .. } => declared
                .shape
                .cases()
                .iter()
                .flat_map(|case| case.fields.clone())
                .collect(),
            _ => Vec::new(),
        })
        .find(|declared| declared.id == field)
        .map(|declared| {
            module
                .identities
                .text(declared.identity)
                .unwrap()
                .to_owned()
        })
        .expect("the field is declared")
}

/// Each field initializer as `(field name, integer it holds)`.
fn integer_fields(
    lowered: &Lowered<'_>,
    machine: &TerminalMachine,
    fields: &[RecordFieldInitializer],
) -> Vec<(String, IntegerValue)> {
    fields
        .iter()
        .map(|initializer| {
            let RecordFieldValue::Scalar { value, .. } = initializer.value else {
                panic!("{initializer:?}")
            };
            let constant = machine
                .operations()
                .find(|operation| {
                    operation.result.scalar_ref().map(|result| result.id) == Some(value)
                })
                .unwrap();
            let OperationKind::IntegerConstant { value } = constant.kind else {
                panic!("{constant:?}")
            };
            (field_name(lowered, initializer.field), value)
        })
        .collect()
}

fn only(machine: &TerminalMachine, select: impl Fn(&OperationKind) -> bool) -> &Operation {
    program::the_operation(machine, select)
}

/// `VALUE` writes `second` before `first`. Stage 05 fills each field with
/// the value written for that field, whatever the written order, and keeps
/// the case the constant names: `Pair::B`, the second case.
#[test]
fn a_frozen_constant_fills_each_field_by_name_and_keeps_its_case() {
    let record = Program::of(
        "data Pair [copy] { first: u8; second: u8; } \
         const VALUE: Pair = Pair { second: 9, first: 2 }; machine get() -> Pair { VALUE }",
    );
    let lowered = record.lower();
    let get = lowered.machine("get");
    let established = only(get, |kind| {
        matches!(kind, OperationKind::EstablishRecord { .. })
    });
    let OperationKind::EstablishRecord { fields } = &established.kind else {
        unreachable!()
    };
    let mut fields = integer_fields(&lowered, get, fields);
    fields.sort();
    assert_eq!(
        fields,
        [
            ("first".to_owned(), IntegerValue::Unsigned(2)),
            ("second".to_owned(), IntegerValue::Unsigned(9)),
        ]
    );

    let case = Program::of(
        "data Pair [copy] { case A(first: u8, second: u8); case B(first: u8, second: u8); } \
         const VALUE: Pair = Pair::B { second: 2, first: 9 }; machine get() -> Pair { VALUE }",
    );
    let lowered = case.lower();
    let get = lowered.machine("get");
    let established = only(get, |kind| {
        matches!(kind, OperationKind::EstablishStructuralCase { .. })
    });
    let OperationKind::EstablishStructuralCase {
        result_case,
        fields,
    } = &established.kind
    else {
        unreachable!()
    };
    assert_eq!(result_case.get(), 2, "`B` is the second case");
    let mut fields = integer_fields(&lowered, get, fields);
    fields.sort();
    assert_eq!(
        fields,
        [
            ("first".to_owned(), IntegerValue::Unsigned(9)),
            ("second".to_owned(), IntegerValue::Unsigned(2)),
        ]
    );
    record.judged(|_| {}).accepted();
    case.judged(|_| {}).accepted();
}

/// Each record of a constant array keeps its own case: the first `Entry`
/// holds `Choice::Empty`, the second `Choice::Selected { value: 7 }`, and
/// the array lists them in written order.
#[test]
fn each_record_of_a_constant_array_keeps_its_own_case() {
    let program = Program::of(
        "data Choice [copy] { case Selected(value: u8); case Empty; } \
         data Entry [copy] { choice: Choice; } \
         const ROWS: [Entry; 2] = [Entry { choice: Choice::Empty }, \
             Entry { choice: Choice::Selected { value: 7 } }]; \
         machine get() -> [Entry; 2] { ROWS }",
    );
    let lowered = program.lower();
    let operations = lowered.operations("get");
    let written: Vec<_> = operations.iter().map(|(written, _)| *written).collect();
    assert_eq!(
        written,
        [
            "Choice::Empty",
            "Entry { choice: Choice::Empty }",
            "7",
            "Choice::Selected { value: 7 }",
            "Entry { choice: Choice::Selected { value: 7 } }",
            "ROWS",
        ]
    );
    let place = |position: usize| operations[position].1.result.as_structural().unwrap().place;
    assert!(matches!(
        operations[0].1.kind,
        OperationKind::EstablishScalarCase { result_case, ref fields }
            if result_case.get() == 2 && fields.is_empty()
    ));
    assert!(matches!(
        operations[3].1.kind,
        OperationKind::EstablishStructuralCase { result_case, .. } if result_case.get() == 1
    ));
    let OperationKind::EstablishStructuralArray { elements } = &operations[5].1.kind else {
        panic!("{:?}", operations[5])
    };
    assert_eq!(
        elements
            .iter()
            .map(|element| element.place)
            .collect::<Vec<_>>(),
        [place(1), place(4)]
    );
    program.judged(|_| {}).accepted();
}

/// A machine returns the constant, literal or formal it names: `A`'s 1,
/// `true`, `OFF`'s `false`, and in `first` and `second` the formal before or
/// after an array formal, by its own identity.
#[test]
fn a_scalar_return_is_the_constant_literal_or_formal_named() {
    let program = Program::of(
        "const A: u8 = 1u8; const OFF: bool = false; \
         machine integer(unused: u8) -> u8 { A } \
         machine literal_bool() -> bool { true } \
         machine named_bool() -> bool { OFF } \
         machine first(a: u8, array: [u8; 3], b: u8) -> u8 { a } \
         machine second(a: u8, array: [u8; 3], b: u8) -> u8 { b }",
    );
    let lowered = program.lower();
    for (path, expected) in [
        (
            "integer",
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(1),
            },
        ),
        (
            "literal_bool",
            OperationKind::BooleanConstant { value: true },
        ),
        (
            "named_bool",
            OperationKind::BooleanConstant { value: false },
        ),
    ] {
        let machine = lowered.machine(path);
        let [(_, constant)] = lowered.operations(path)[..] else {
            panic!("`{path}` is one constant")
        };
        assert_eq!(constant.kind, expected, "{path}");
        assert_eq!(returned(machine), scalar(constant), "{path}");
    }
    for (path, position) in [("first", 0), ("second", 1)] {
        let machine = lowered.machine(path);
        assert_eq!(
            returned(machine),
            machine.contract.parameters[position].id,
            "{path}"
        );
    }
    program.judged(|_| {}).accepted();
}

fn refused(source: &str) -> EarlierRefusal {
    Program::try_of(&[("product/main.omg", source)])
        .err()
        .unwrap_or_else(|| panic!("the program is refused: {source}"))
}

/// A copyable record bound with `let mutable` may have a field stored into
/// it, and needs no disposal; without `mutable` the store is refused at the
/// field it writes.
#[test]
fn a_record_field_is_writable_only_through_a_mutable_binding() {
    let program = Program::of(
        "data Point [copy] { x: u8; } \
         machine make() -> u8 { let mutable p: Point = (Point { x: 1 }); p.x = 2; p.x }",
    );
    let lowered = program.lower();
    let make = lowered.machine("make");
    let stored = only(make, |kind| {
        matches!(kind, OperationKind::StructuralScalarFieldStore { .. })
    });
    let built = only(make, |kind| {
        matches!(kind, OperationKind::EstablishRecord { .. })
    });
    assert!(matches!(
        stored.kind,
        OperationKind::StructuralScalarFieldStore { destination, .. }
            if destination == built.result.as_structural().unwrap().place
    ));
    program.judged(|_| {}).accepted();

    let immutable = refused(
        "data Point [copy] { x: u8; } \
         machine make() -> u8 { let p: Point = (Point { x: 1 }); p.x = 2; p.x }",
    );
    assert_eq!((immutable.stage, immutable.at.as_str()), ("lowering", "x"));
    assert!(
        immutable
            .message
            .starts_with("Violation: writing to an immutable owned binding requires `mutable`"),
        "{immutable:?}"
    );
}

/// A linear value must be consumed. A linear local left at the end of its
/// machine, a linear field of a local record, and a linear formal left
/// unconsumed are each rejected, because no edge may dispose of them; the
/// affine twin is disposed of on the way out and accepted.
#[test]
fn a_linear_value_left_unconsumed_is_rejected() {
    Program::of(
        "data Point { x: u8; } machine make() { let mutable p: Point = (Point { x: 1 }); p.x = 2; }",
    )
    .judged(|_| {})
    .accepted();
    // The linear twin that consumes its value by a complete consuming
    // pattern is accepted.
    Program::of("data Lease [linear] { n: u8; } machine stop(l: Lease) { let { n } = move l; }")
        .judged(|_| {})
        .accepted();
    // Each is rejected at the value that is lost: the construction a local
    // holds, or, for a formal, the machine as a whole.
    for (source, at) in [
        (
            "data Point [linear] { x: u8; } machine make() { let mutable p: Point = (Point { x: 1 }); p.x = 2; }",
            "Point { x: 1 }",
        ),
        (
            "data Lease [linear] {} data Held { a: Lease; } machine keep() { let h: Held = Held { a: Lease {} }; }",
            "Held { a: Lease {} }",
        ),
        ("data Lease [linear] {} machine stop(l: Lease) {}", ""),
    ] {
        let refusal = refused(source);
        assert_eq!(
            (refusal.stage, refusal.message.as_str(), refusal.at.as_str()),
            ("lowering", UNCONSUMED, at),
            "{source}"
        );
    }
}

const UNCONSUMED: &str = "Violation: while checking control-flow edge: a linear value is still \
     held when this path leaves its storage, and nothing consumes it: every path must pass it \
     on, return it or end it by its declared disposition, and neither `_ =` nor leaving scope \
     can discard it";

/// `Cell<u8>` written in a local's annotation and in a formal is one
/// structural type: `build`'s record and `accept`'s parameter share it.
#[test]
fn one_applied_record_is_one_structural_type_wherever_it_is_written() {
    let program = Program::of(
        "data Cell<T> { value: T; } \
         machine build() { let cell: Cell<u8> = Cell { value: 7u8 }; } \
         machine accept(cell: Cell<u8>) {}",
    );
    let lowered = program.lower();
    let built = only(lowered.machine("build"), |kind| {
        matches!(kind, OperationKind::EstablishRecord { .. })
    });
    let [parameter] = lowered
        .machine("accept")
        .contract
        .structural_parameters
        .as_slice()
    else {
        panic!("one structural formal")
    };
    assert_eq!(
        built.result.as_structural().unwrap().structural_type,
        parameter.structural_type
    );
    program.judged(|_| {}).accepted();
}

/// `..` gives a sum field the first case and no payload. When only a later
/// case holds a reference, the zero is still fine; a reference in a record
/// field below `..` has no zero, and the literal is rejected.
#[test]
fn an_omitted_sum_takes_its_first_case_and_an_omitted_reference_is_rejected() {
    let program = Program::of(
        "data Slot { case Empty; case Entry(value: u8); } data Plan { id: u8; first: Slot; } \
         machine build(id: u8) -> Plan { Plan { id: id, .. } }",
    );
    let lowered = program.lower();
    let zero = only(lowered.machine("build"), |kind| {
        matches!(kind, OperationKind::EstablishScalarCase { .. })
    });
    assert!(matches!(
        zero.kind,
        OperationKind::EstablishScalarCase { result_case, ref fields }
            if result_case.get() == 1 && fields.is_empty()
    ));
    program.judged(|_| {}).accepted();

    Program::of(
        "data Slot<'r> { case Empty; case Held(at: @'r u8); } data Plan<'r> { id: u8; first: Slot<'r>; } \
         machine build<'r>(id: u8) { let plan: Plan<'r> = Plan { id: id, .. }; }",
    )
    .judged(|_| {})
    .accepted();

    let reference = refused(
        "data Reach<'r> { at: @'r u8; n: u64; } data Plan<'r> { id: u8; reach: Reach<'r>; } \
         machine build<'r>(id: u8) { let plan: Plan<'r> = Plan { id: id, .. }; }",
    );
    // data_and_literals.md, "Explicit field omission": "The marker supplies
    // no authority, borrowed reference, callable or proof witness merely
    // because storage can be zeroed." The literal is the program's error.
    assert_eq!(
        (reference.stage, reference.at.as_str()),
        ("lowering", "Plan { id: id, .. }")
    );
    assert!(
        reference.message.starts_with(
            "Violation: `..` cannot supply the field `reach`: its value holds a borrowed \
             reference, and zeroed storage supplies no borrowed reference; write the field's \
             value"
        ),
        "{reference:?}"
    );
}

/// An omitted array field is zeroed by one operation whatever its length,
/// whether it holds bytes or sums, and omitted float fields get the zero
/// bits of their own format.
#[test]
fn an_omitted_array_is_one_zeroing_and_an_omitted_float_is_its_formats_zero() {
    for source in [
        "data Buffer { length: u64; bytes: [u8; 65536]; } \
         machine build(id: u64) { let buffer: Buffer = Buffer { length: id, .. }; }",
        "data Slot { case Empty; case Entry(value: u8); } data Plan { id: u8; slots: [Slot; 70000]; } \
         machine build(id: u8) { let plan: Plan = Plan { id: id, .. }; }",
    ] {
        let program = Program::of(source);
        let lowered = program.lower();
        let zeroings = lowered
            .operations("build")
            .into_iter()
            .filter(|(_, operation)| {
                matches!(
                    operation.kind,
                    OperationKind::EstablishZeroedStructural { .. }
                )
            })
            .count();
        assert_eq!(zeroings, 1, "{source}");
        program.judged(|_| {}).accepted();
    }

    let program = Program::of(
        "data Options { count: u8; ratio: f32; scale: f64; } \
         machine build(count: u8) { let options: Options = Options { count: count, .. }; }",
    );
    let lowered = program.lower();
    let floats: Vec<_> = lowered
        .operations("build")
        .into_iter()
        .filter_map(|(_, operation)| match operation.kind {
            OperationKind::IeeeFloatConstant { value } => Some(value),
            _ => None,
        })
        .collect();
    assert_eq!(
        floats,
        [IeeeFloatValue::Binary32(0), IeeeFloatValue::Binary64(0)]
    );
    program.judged(|_| {}).accepted();
}

/// A literal whose suffix names another carrier than its context receives
/// is refused at the literal, however it is grouped, as a pattern, a local's
/// initializer, an argument or a result.
#[test]
fn a_literal_of_another_carrier_is_refused_at_the_literal() {
    for source in [
        "machine choose(x: u8) -> bool { match x { 1u16 -> true, _ -> false, } }",
        "machine choose(x: u8) -> bool { match x { (1u16) -> true, _ -> false, } }",
        "machine choose(x: u8) -> bool { match x { ((1u16)) -> true, _ -> false, } }",
        "machine set() -> u8 { let x: u8 = 1u16; x }",
        "machine take(x: u8) -> u8 { x } machine call() -> u8 { take(1u16) }",
        "machine ret() -> u8 { 1u16 }",
    ] {
        let refusal = refused(source);
        assert_eq!(
            (refusal.stage, refusal.at.as_str()),
            ("lowering", "1u16"),
            "{source}"
        );
        assert!(
            refusal.message.starts_with(
                "Violation: this literal has scalar carrier `u16` but its context requires \
                 `u8`: there is no implicit scalar conversion at a receiving type"
            ),
            "{refusal:?}"
        );
    }
    Program::of("machine choose(x: u8) -> bool { match x { 1u8 -> true, _ -> false, } }")
        .judged(|_| {})
        .accepted();
}

/// A record literal's borrowed field must hold a view of the lifetime its
/// declared type names for that field. `Pair<'a, 'b>` puts `'b` on `left`
/// and `'a` on `right`, so `Pair { left: y, right: x }` is accepted, and
/// its twin `Pair { left: x, right: y }` hands `left` a view of `'a`: nothing
/// relates two lifetimes of one machine, so the literal is rejected at the
/// field's value (`wiki/spec/language/lifetimes.md`: the language has no
/// authored outlives bounds).
#[test]
fn a_record_literal_field_of_the_other_lifetime_is_rejected() {
    let program = |fields: &str| {
        format!(
            "data Pair<'first, 'second> {{ left: @'second [u8]; right: @'first [u8]; }} \
             machine make<'a, 'b>(x: @'a [u8], y: @'b [u8]) -> Pair<'a, 'b> {{ Pair {{ {fields} }} }}"
        )
    };
    Program::of(&program("left: y, right: x"))
        .judged(|_| {})
        .accepted();
    let refusal = refused(&program("left: x, right: y"));
    assert_eq!((refusal.stage, refusal.at.as_str()), ("lowering", "x"));
    assert!(
        refusal.message.starts_with(
            "Violation: this view is of the region of another lifetime of the machine than \
             the field it initializes names: nothing relates two lifetimes of one machine"
        ),
        "{refusal:?}"
    );
}
