use checked_interpreter::{BuildMachineEntry, InterpretOptions, interpret_entry};

#[test]
fn mathematical_integer_comparisons_preserve_each_operand_carrier() {
    let mut source = String::new();
    let mut conditions = Vec::new();
    for (index, (left_type, left, right_type, right, ordering)) in [
        ("i32", "-1", "u64", "10", -1),
        ("i64", "-1", "u64", "18446744073709551615", -1),
        ("i32", "10", "u64", "10", 0),
        ("i8", "0", "u64", "0", 0),
        (
            "i64",
            "9223372036854775807",
            "u64",
            "18446744073709551615",
            -1,
        ),
        ("u64", "18446744073709551615", "i8", "-1", 1),
        ("u8", "255", "i16", "256", -1),
        ("i16", "-129", "i8", "-128", -1),
    ]
    .into_iter()
    .enumerate()
    {
        for (relation, (operator, expected)) in [
            ("<", ordering < 0),
            ("<=", ordering <= 0),
            (">", ordering > 0),
            (">=", ordering >= 0),
            ("==", ordering == 0),
            ("!=", ordering != 0),
        ]
        .into_iter()
        .enumerate()
        {
            let name = format!("compare_{index}_{relation}");
            source.push_str(&format!("machine {name}(left: {left_type}, right: {right_type}) -> bool {{ left {operator} right }}\n"));
            conditions.push(format!("({name}({left}, {right}) == {expected})"));
        }
    }
    source.push_str(&format!(
        "machine main() -> i32 {{ transition {} {{ true -> 7 false -> 0 }} }}",
        conditions.join(" && ")
    ));
    let checked =
        crate::front_end::checked_program_result(&source).expect("mixed comparison program");
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn mixed_integer_comparison_evaluates_operands_once_left_to_right() {
    let source = "machine left(trace: &mut i32 in Wrapping) -> i32 {
        trace = trace * 10 + 1; -1
    }
    machine right(trace: &mut i32 in Wrapping) -> u64 {
        trace = trace * 10 + 2; 10
    }
    machine main() -> i32 {
        let mut trace: i32 in Wrapping = 0;
        let result: bool = left(&mut trace) < right(&mut trace);
        transition result && trace == 12 { true -> 7 false -> 0 }
    }";
    let checked =
        crate::front_end::checked_program_result(source).expect("ordered mixed comparison");
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn mixed_integer_field_and_array_operands_keep_their_carriers() {
    let source = "data Pair [copy] { negative: i32; maximum: u64; }
        machine main() -> i32 {
            let pair: Pair = Pair { negative: -1, maximum: 18446744073709551615 };
            let values: [u64; 1] = [18446744073709551615];
            transition pair.negative < pair.maximum && pair.negative != values[0] {
                true -> 7 false -> 0
            }
        }";
    let checked =
        crate::front_end::checked_program_result(source).expect("projected mixed comparison");
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}
