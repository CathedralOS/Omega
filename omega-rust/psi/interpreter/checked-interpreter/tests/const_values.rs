use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use checked_interpreter::interpret_entry;

fn assert_seven(source: &str) {
    let checked = crate::front_end::checked_program_with_generic_data_result(source)
        .expect("checked const values");
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None, "{source}");
    assert_eq!(outcome.exit_code, 7, "{source}");
}

#[test]
fn structured_const_instances_execute_distinct_integer_and_boolean_fields() {
    assert_seven("data Config [copy] { count: u8; enabled: bool; }
        const Values::FIRST: Config = Config { count: 2, enabled: true };
        const Values::SECOND: Config = Config { enabled: false, count: 3 };
        data Witness<const N: Config> { case Only; }
        data Scenario { first: Witness<Values::FIRST>; second: Witness<Values::SECOND>; }
        machine value<const N: Config>(witness: &Witness<N>) -> Config { N }
        machine Scenario::run(scenario: &Scenario) -> i32 {
            let first_value: Config = value(&scenario.first);
            let second_value: Config = value(&scenario.second);
            transition first_value.count == 2 && first_value.enabled && second_value.count == 3 && !second_value.enabled {
                true -> 7 false -> 0
            }
        }
        machine main() -> i32 {
            let scenario: Scenario = Scenario { first: Witness::Only, second: Witness::Only };
            Scenario::run(&scenario)
        }");
}

#[test]
fn explicit_const_arguments_execute_scalars_arrays_records_and_cases() {
    assert_seven("data Config [copy] { count: u8; enabled: bool; }
        data Choice [copy] { case First(value: u8); case Second(value: u8); }
        const Values::NEGATIVE: i32 = -2;
        const Values::CONFIG: Config = Config { enabled: true, count: 3 };
        const Values::ARRAY: [u8; 2] = [2, 3];
        const Values::CHOICE: Choice = Choice::Second { value: 4 };
        machine boolean<const N: bool>() -> bool { N }
        machine integer<const N: i32>() -> i32 { N }
        machine record<const N: Config>() -> Config { N }
        machine array<const N: [u8; 2]>() -> [u8; 2] { N }
        machine choice<const N: Choice>() -> Choice { N }
        machine check_choice(selected: Choice) -> bool {
            transition selected {
                Choice::First { value } -> false
                Choice::Second { value } -> (value == 4)
            }
        }
        machine main() -> i32 {
            let enabled: bool = boolean<true>();
            let disabled: bool = boolean<false>();
            let negative: i32 = integer<Values::NEGATIVE>();
            let config: Config = record<Values::CONFIG>();
            let items: [u8; 2] = array<Values::ARRAY>();
            let selected: Choice = choice<Values::CHOICE>();
            let correct_choice: bool = check_choice(selected);
            transition enabled && !disabled && negative == -2 && config.enabled && config.count == 3 && items[0] == 2 && items[1] == 3 && correct_choice {
                true -> 7 false -> 0
            }
        }");
}

#[test]
fn specialized_result_temporaries_execute_each_initializer_once() {
    assert_seven(
        "machine value<const N: u64>(calls: &mut i32 in Wrapping) -> u64[0..=N] {
            calls = calls + 1; N
        }
        machine forward(calls: &mut i32 in Wrapping) -> u64 {
            let first: u64 = value<2>(calls);
            value<3>(calls)
        }
        machine main() -> i32 {
            let mut calls: i32 in Wrapping = 0;
            let selected: u64 = forward(&mut calls);
            transition calls == 2 && selected == 3 { true -> 7 false -> 0 }
        }",
    );
}
