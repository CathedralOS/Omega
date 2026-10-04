//! Qualified static calls and service requirements resolve by the checked
//! tree's resolved symbols
//! (wiki/spec/language/machines.md#static-declarations-and-explicit-state).
//! A qualifier such as `Worker::write` or `Console::read_line` names a
//! declaration: it contributes no argument, no storage and no runtime
//! receiver. The interpreter never reads it as a same-spelled field of the
//! current instance, and a boundary requirement's host service is selected by
//! the requirement's identity, not by the receiver path.

use checked_interpreter::{BuildMachineEntry, InterpretOptions, InterpretOutcome, interpret_entry};
use checked_trees::CheckedTrees;

fn checked(source: &str) -> CheckedTrees {
    crate::front_end::checked_program(source)
}

fn run(checked: &CheckedTrees, entry: &str, stdin: &[u8]) -> InterpretOutcome {
    interpret_entry(
        checked,
        BuildMachineEntry::Name(entry),
        stdin,
        InterpretOptions::default(),
    )
}

// Leaf spellings that build facet methods and collection builtins also
// recognize. Each qualified value call runs its own declaration; none of
// those receiver-place recognizers may claim it.
const FACET_SPELLED_LEAVES: [&str; 17] = [
    "write",
    "read",
    "open",
    "close",
    "create",
    "resolve",
    "entry",
    "schema",
    "provider",
    "require",
    "complete",
    "fail",
    "path",
    "get",
    "write_line",
    "include_source",
    "len",
];

#[test]
fn qualified_value_calls_run_their_declaration_whatever_the_leaf_spelling() {
    for leaf in FACET_SPELLED_LEAVES {
        let program = checked(&format!(
            "data Cell {{ value: i32 in Wrapping; }}
             data Worker {{}}
             machine Worker::{leaf}(cell: &mut Cell, by: i32 in Wrapping) -> i32 {{
                 cell.value = cell.value + by;
                 transition {{ _ -> ((cell.value + 1) as i32) }}
             }}
             machine main() -> i32 {{
                 let mut cell: Cell;
                 let first: i32 = Worker::{leaf}(&mut cell, 30);
                 let second: i32 = Worker::{leaf}(&mut cell, 39);
                 transition first == 31 && cell.value == 69 {{
                     true -> (second)
                     false -> 1
                 }}
             }}"
        ));
        let outcome = run(&program, "main", &[]);
        assert_eq!(outcome.error, None, "{leaf}");
        assert_eq!(outcome.exit_code, 70, "{leaf}");
    }
}

#[test]
fn qualified_arguments_bind_explicit_homes_in_operand_order() {
    let program = checked(
        "data Point { value: i32; }
         machine Point::less(left: &Point, right: &Point) -> bool {
             left.value < right.value
         }
         machine main() -> i32 {
             let left: Point = Point { value: 1 };
             let right: Point = Point { value: 2 };
             let ascending: bool = Point::less(&left, &right);
             let descending: bool = Point::less(&right, &left);
             transition ascending && !descending {
                 true -> 70
                 false -> 71
             }
         }",
    );
    // Each borrow names its actual's storage; swapping or skipping an operand
    // changes the ordered result.
    let outcome = run(&program, "main", &[]);
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

const BOUNDED_CONSOLE: &str = "pub data LineReadResult {
        case Invalid;
        case LineComplete(count: u64);
        case EndOfInput(count: u64);
        case Full(count: u64);
    }
    pub boundary trait Console {
        machine read_line(out_line: &mut [u8]) -> LineReadResult reaches Console;
    }";

// `pass/calls/local_fixed_byte_array_loan_exit`: a value-position
// `Console::read_line(&mut buffer)` is a direct call to the boundary
// requirement. Its identity selects the bounded host read into the caller's
// local array; the qualifier `Console` is no receiver. Before identity
// routing only the statement position served the requirement, and this
// declined with "qualified call target `read_line` has no resolved state".
#[test]
fn qualified_requirement_value_call_reads_a_bounded_line_into_a_local() {
    let program = checked(&format!(
        "{BOUNDED_CONSOLE}
         machine main() -> i32 reaches Console {{
             let mut buffer: [u8; 4];
             let line: LineReadResult = Console::read_line(&mut buffer);
             transition line {{
                 LineReadResult::LineComplete {{ count }} -> check(count, buffer[0], buffer[1])
                 _ -> 71
             }}
             state check(count: u64, first: u8, second: u8) -> i32 {{
                 transition count == 3 && first == 97 && second == 98 {{
                     true -> 70
                     false -> 72
                 }}
             }}
         }}"
    ));
    let outcome = run(&program, "main", b"ab\ncd");
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

// A guard subject evaluates once per transition evaluation: the parser copies
// the subject call into every arm, and the host read must not consume the
// next line for a later arm.
#[test]
fn qualified_requirement_guard_subject_reads_once() {
    let program = checked(&format!(
        "{BOUNDED_CONSOLE}
         machine main() -> i32 reaches Console {{
             let mut buffer: [u8; 8];
             transition Console::read_line(&mut buffer) {{
                 LineReadResult::Full {{ count }} -> 1
                 LineReadResult::EndOfInput {{ count }} -> 2
                 LineReadResult::LineComplete {{ count }} -> check(count, buffer[4])
                 _ -> 3
             }}
             state check(count: u64, fifth: u8) -> i32 {{
                 transition count == 4 && fifth == 0 {{
                     true -> 70
                     false -> 4
                 }}
             }}
         }}"
    ));
    let outcome = run(&program, "main", b"abc\nxyz\n");
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

// A requirement the host does not serve still declines rather than resolving
// by the qualifier or the leaf spelling.
#[test]
fn unserved_qualified_requirement_value_call_declines() {
    let program = checked(
        "pub boundary trait Sensor {
             machine sample() -> i32 reaches Sensor;
         }
         machine main() -> i32 reaches Sensor { Sensor::sample() }",
    );
    let outcome = run(&program, "main", &[]);
    assert_eq!(
        outcome.error.as_deref(),
        Some("call target `sample` has no resolved state")
    );
}
