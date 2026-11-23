//! Stage 05 must read and write the place the author wrote: the current
//! value of a reassigned local, the copy a binding took before the
//! reassignment, the referent behind `$`, the field a borrow names however
//! the receiver is spelled, and the parameter a body returns. The tests
//! write small programs, read what stage 05 produced, and compare it with a
//! source twin that changes one word.
//!
//! The rules: an assignment replaces the stored value and later reads see
//! it (`wiki/spec/language/ownership.md`, and the destination-first
//! schedule in `wiki/spec/language/expressions.md`); `$r` reads or writes
//! the referent of `r` (`wiki/spec/language/expressions.md`, "Explicit
//! dereference"); a whole array is a value, so a binding copies it
//! (`wiki/spec/terminal-psi/structural_access.md`); a nested machine is a
//! machine called with explicit arguments
//! (`wiki/spec/language/machines.md`, "Nested machines and identity").

#[path = "../../06_lowered-program-to-checked-program/tests/support/source_handoff.rs"]
mod program;

use program::{Program, returned, scalar};
use semantic_vocabulary::{IeeeFloatValue, IntegerValue, PlaceId, ValueId};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralArgument, StructuralPathSegment,
    TerminalMachine, Terminator,
};

fn formals(machine: &TerminalMachine) -> Vec<ValueId> {
    machine
        .contract
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect()
}

fn structural_formal(machine: &TerminalMachine, position: usize) -> PlaceId {
    machine.contract.structural_parameters[position].place
}

/// The place a structural operation establishes.
fn established(operation: &Operation) -> PlaceId {
    let OperationResult::Structural(result) = &operation.result else {
        panic!("the operation establishes no place: {operation:?}")
    };
    result.place
}

fn kinds(lowered: &program::Lowered<'_>, path: &str) -> Vec<(String, OperationKind)> {
    lowered
        .operations(path)
        .into_iter()
        .map(|(written, operation)| (written.to_owned(), operation.kind.clone()))
        .collect()
}

#[test]
fn a_reassigned_local_is_read_with_its_new_value_and_an_earlier_binding_keeps_the_old() {
    for (sum, expect_before_is_x) in [("before +% local", true), ("local +% before", false)] {
        let program = Program::of(&format!(
            "machine run(x: u8) -> u8 {{ let mutable local: u8 = x; let before: u8 = local; local = 1u8; {sum} }}\n"
        ));
        let lowered = program.lower();
        let machine = lowered.machine("run");
        let [x] = formals(machine)[..] else {
            panic!("one formal")
        };
        let [(one_text, one), (add_text, add)] = lowered.operations("run")[..] else {
            panic!(
                "a constant and an addition: {:?}",
                lowered.operations("run")
            )
        };
        assert_eq!((one_text, add_text), ("1u8", "+%"));
        assert_eq!(
            one.kind,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(1)
            }
        );
        // `before` still holds `x`; `local` now holds the assigned 1.
        let (left, right) = if expect_before_is_x {
            (x, scalar(one))
        } else {
            (scalar(one), x)
        };
        assert_eq!(
            add.kind,
            OperationKind::WrappingIntegerAdd { left, right },
            "{sum}"
        );
        assert_eq!(returned(machine), scalar(add));
    }
}

#[test]
fn an_earlier_copy_of_an_array_local_survives_its_reassignment() {
    let program = Program::of(
        "machine run(left: [u8; 2], right: [u8; 2], i: u64) -> u8 requires i < 2 { let mutable row: [u8; 2] = left; let old: [u8; 2] = row; row = right; old[i] +% row[i] }\n",
    );
    let lowered = program.lower();
    let machine = lowered.machine("run");
    let (left, right) = (structural_formal(machine, 0), structural_formal(machine, 1));
    let [i] = formals(machine)[..] else {
        panic!("one scalar formal")
    };
    let operations = lowered.operations("run");
    let [
        (_, row_copy),
        (_, old_copy),
        (_, right_copy),
        (_, store),
        (_, old_read),
        (_, row_read),
        (_, add),
    ] = operations[..]
    else {
        panic!("three copies, a store, two reads and an addition: {operations:?}")
    };
    let row = established(row_copy);
    let old = established(old_copy);
    let new = established(right_copy);
    assert_eq!(
        row_copy.kind,
        OperationKind::StructuralLeafCopy {
            source: left,
            path: vec![]
        }
    );
    // `let old = row` takes its own copy of the row as it stands.
    assert_eq!(
        old_copy.kind,
        OperationKind::StructuralLeafCopy {
            source: row,
            path: vec![]
        }
    );
    assert_eq!(
        right_copy.kind,
        OperationKind::StructuralLeafCopy {
            source: right,
            path: vec![]
        }
    );
    // `row = right` replaces the row, not the copy `old` took.
    let OperationKind::StoreStructuralField {
        destination,
        path,
        field: None,
        value:
            StructuralArgument {
                place: stored,
                path: stored_path,
                ..
            },
        ..
    } = &store.kind
    else {
        panic!("the assignment stores the row: {store:?}")
    };
    assert_eq!(
        (
            *destination,
            path.as_slice(),
            *stored,
            stored_path.as_slice()
        ),
        (row, &[][..], new, &[][..])
    );
    let element = |operation: &Operation| {
        let OperationKind::PrimitiveScalarRead { source, path } = &operation.kind else {
            panic!("an element read: {operation:?}")
        };
        let [StructuralPathSegment::RuntimeIndex { index, .. }] = path.as_slice() else {
            panic!("one runtime index: {path:?}")
        };
        (*source, *index)
    };
    assert_eq!(element(old_read), (old, i));
    assert_eq!(element(row_read), (row, i));
    assert_eq!(
        add.kind,
        OperationKind::WrappingIntegerAdd {
            left: scalar(old_read),
            right: scalar(row_read)
        }
    );
    assert_eq!(returned(machine), scalar(add));
}

#[test]
fn a_dereference_reads_and_writes_the_referent_of_the_reference_it_names() {
    for (stored, read) in [("at", "at"), ("other", "at"), ("at", "other")] {
        let program = Program::of(&format!(
            "machine update(at: @mutable u8, other: @mutable u8) -> u8 {{ ${stored} = 2u8; ${read} }}\n"
        ));
        let lowered = program.lower();
        let machine = lowered.machine("update");
        let place = |name: &str| structural_formal(machine, usize::from(name == "other"));
        let [(_, two), (_, store), (_, load)] = lowered.operations("update")[..] else {
            panic!("a constant, a store and a read")
        };
        assert_eq!(
            store.kind,
            OperationKind::WriteOnlyPrimitiveStore {
                destination: place(stored),
                path: vec![],
                value: scalar(two),
                range_obligation: None,
            },
            "${stored} = 2u8"
        );
        assert_eq!(
            load.kind,
            OperationKind::PrimitiveScalarRead {
                source: place(read),
                path: vec![]
            },
            "${read}"
        );
        assert_eq!(returned(machine), scalar(load));
    }
}

#[test]
fn a_field_borrow_names_the_same_place_through_either_dereference_spelling() {
    let program = Program::of(
        "data Cell { mark: u8; other: u8; }
machine take(m: @u8) {}
machine one(cell: @Cell) { take(@cell.mark); }
machine two(cell: @Cell) { take(@($cell).mark); }
machine three(cell: @Cell) { take(@cell.other); }
",
    );
    let lowered = program.lower();
    let borrowed = |name: &str| {
        let machine = lowered.machine(name);
        let [(_, call)] = lowered.operations(name)[..] else {
            panic!("`{name}` is one call")
        };
        let OperationKind::CallUnit {
            callee,
            structural_arguments,
            ..
        } = &call.kind
        else {
            panic!("a unit call: {call:?}")
        };
        assert_eq!(*callee, lowered.machine_id("take"));
        let [
            StructuralArgument {
                place,
                path,
                access,
            },
        ] = structural_arguments.as_slice()
        else {
            panic!("one borrowed argument")
        };
        assert_eq!(*place, structural_formal(machine, 0), "{path:?}");
        let [StructuralPathSegment::Field(field)] = path.as_slice() else {
            panic!("one field step: {path:?}")
        };
        (
            lowered
                .elaboration
                .module
                .identities
                .text(*field)
                .unwrap()
                .to_owned(),
            *access,
            kinds(&lowered, name),
        )
    };
    let one = borrowed("one");
    let two = borrowed("two");
    let three = borrowed("three");
    assert_eq!(one.0, "mark");
    assert_eq!(three.0, "other");
    // The two spellings give the same operations, not merely the same field.
    assert_eq!(
        one.2.iter().map(|(_, kind)| kind).collect::<Vec<_>>(),
        two.2.iter().map(|(_, kind)| kind).collect::<Vec<_>>()
    );
    assert_eq!(one.1, two.1);
}

#[test]
fn a_nested_machine_is_called_with_the_argument_written_and_returns_its_own_formal() {
    for (argument, position) in [("x", 0), ("z", 1)] {
        let program = Program::of(&format!(
            "machine outer(x: u8, z: u8) -> u8 {{ inner({argument}) machine inner(y: u8) -> u8 {{ y }} }}\n"
        ));
        let lowered = program.lower();
        let outer = lowered.machine("outer");
        let inner = lowered.machine("outer\\inner");
        let [(written, call)] = lowered.operations("outer")[..] else {
            panic!("`outer` is one call")
        };
        assert_eq!(written, format!("inner({argument})"));
        let OperationKind::Call {
            callee, arguments, ..
        } = &call.kind
        else {
            panic!("a scalar call: {call:?}")
        };
        assert_eq!(*callee, inner.id);
        assert_eq!(arguments.as_slice(), [formals(outer)[position]]);
        assert_eq!(returned(outer), scalar(call));
        assert!(lowered.operations("outer\\inner").is_empty());
        assert_eq!(returned(inner), formals(inner)[0]);
    }
}

/// `text` with the number in each `StructuralTypeId(n)` removed.
fn without_type_numbers(text: &str) -> String {
    const MARK: &str = "StructuralTypeId(";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(MARK) {
        out.push_str(&rest[..at + MARK.len()]);
        rest = rest[at + MARK.len()..].trim_start_matches(|c: char| c.is_ascii_digit());
    }
    out.push_str(rest);
    out
}

#[test]
fn a_returned_record_names_the_parameter_the_body_names_in_either_file_order() {
    // `select` names the record its file imports from `cell.omg`
    // (`wiki/spec/language/modules.md`, "Names and imports"), so reading
    // that file first changes the order every declaration is met in.
    let cell = ("product/cell.omg", "public data Cell [copy] { v: u8; }\n");
    let returned_place = |files: &[(&str, &str)], position: usize| {
        let program = Program::try_of(files).unwrap_or_else(|refusal| panic!("{refusal:?}"));
        let lowered = program.lower();
        let machine = lowered.machine("select");
        let [Terminator::ReturnStructural { source, .. }] = machine
            .blocks
            .iter()
            .map(|block| &block.terminator)
            .collect::<Vec<_>>()[..]
        else {
            panic!("one structural return")
        };
        // A copyable record is returned as a copy of the formal it names.
        let copy = machine
            .operations()
            .find(|operation| {
                matches!(&operation.result, OperationResult::Structural(result) if result.place == *source)
            })
            .expect("the returned place is established in the body");
        assert_eq!(
            copy.kind,
            OperationKind::StructuralLeafCopy {
                source: structural_formal(machine, position),
                path: vec![]
            }
        );
        // Stage 05 numbers record types in the order it meets them, so only
        // that numbering may differ between the two file orders.
        without_type_numbers(&format!(
            "{:?} {:?}",
            machine.blocks, machine.contract.structural_parameters
        ))
    };
    for (body, position) in [("a", 0), ("b", 1)] {
        let select = format!(
            "use cell.omg;\nmachine select(a: |cell|::Cell, b: |cell|::Cell) -> |cell|::Cell {{ {body} }}\n"
        );
        let main = ("product/main.omg", select.as_str());
        let forward = returned_place(&[main, cell], position);
        let backward = returned_place(&[cell, main], position);
        assert_eq!(forward, backward, "{body}");
    }
}

#[test]
fn public_machines_get_the_effect_bounds_an_explicit_none_states() {
    let program = Program::of(
        "public machine implicit() {}
public machine explicit() crashes none blocks false suspends false {}
machine private_one() {}
machine private_explicit() crashes none blocks false suspends false {}
",
    );
    let lowered = program.lower();
    let bounds = |path: &str| {
        let contract = &lowered.machine(path).contract;
        (
            contract.declared_crash_routes.clone(),
            contract.declared_suspension,
            contract.declared_blocking,
        )
    };
    let closed = (Some(vec![]), Some(false), Some(false));
    assert_eq!(bounds("implicit"), closed);
    assert_eq!(bounds("explicit"), closed);
    assert_eq!(bounds("private_explicit"), closed);
    // A private machine with no clause leaves its effects to inference.
    assert_eq!(bounds("private_one"), (None, None, None));
}

#[test]
fn a_frozen_nested_array_lowers_its_elements_in_order() {
    for (written, expected) in [
        ("[[4u8, 9u8], [2u8, 7u8]]", [4u128, 9, 2, 7]),
        ("[[2u8, 7u8], [4u8, 9u8]]", [2u128, 7, 4, 9]),
    ] {
        let program = Program::of(&format!(
            "const A: [[u8; 2]; 2] = {written};\nmachine f() -> [[u8; 2]; 2] {{ A }}\n"
        ));
        let lowered = program.lower();
        let machine = lowered.machine("f");
        let operations = lowered.operations("f");
        let [
            (_, a),
            (_, b),
            (_, first),
            (_, c),
            (_, d),
            (_, second),
            (_, outer),
        ] = operations[..]
        else {
            panic!("four constants, two rows and the array: {operations:?}")
        };
        let values: Vec<_> = [a, b, c, d]
            .iter()
            .map(|operation| match operation.kind {
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(value),
                } => value,
                ref other => panic!("an element constant: {other:?}"),
            })
            .collect();
        assert_eq!(values, expected, "{written}");
        assert_eq!(
            first.kind,
            OperationKind::EstablishScalarArray {
                elements: vec![scalar(a), scalar(b)]
            }
        );
        assert_eq!(
            second.kind,
            OperationKind::EstablishScalarArray {
                elements: vec![scalar(c), scalar(d)]
            }
        );
        let OperationKind::EstablishStructuralArray { elements } = &outer.kind else {
            panic!("the outer array: {outer:?}")
        };
        let rows: Vec<_> = elements
            .iter()
            .map(|element| (element.place, element.path.clone()))
            .collect();
        assert_eq!(
            rows,
            [(established(first), vec![]), (established(second), vec![])]
        );
        let [Terminator::ReturnStructural { source, .. }] = machine
            .blocks
            .iter()
            .map(|block| &block.terminator)
            .collect::<Vec<_>>()[..]
        else {
            panic!("one structural return")
        };
        assert_eq!(*source, established(outer));
    }
}

#[test]
fn a_float_literal_lowers_to_its_own_bits() {
    for (written, bits) in [
        ("1.0", 0x3f80_0000u32),
        ("2.0", 0x4000_0000),
        ("-0.5", 0xbf00_0000),
    ] {
        let program = Program::of(&format!("machine one() -> f32 {{ {written} }}\n"));
        let lowered = program.lower();
        let machine = lowered.machine("one");
        let operations = lowered.operations("one");
        let constant = operations
            .iter()
            .find_map(|(_, operation)| match &operation.kind {
                OperationKind::IeeeFloatConstant { value } => Some((operation, *value)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("a float constant: {operations:?}"));
        assert_eq!(constant.1, IeeeFloatValue::Binary32(bits), "{written}");
        if written != "-0.5" {
            assert_eq!(returned(machine), scalar(constant.0));
        }
    }
}
