use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use checked_interpreter::interpret_entry;

#[test]
fn record_field_countdown_executes_through_renamed_state_arrivals() {
    let countdown = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/omega/pass/termination/measure_field_named_arrival/main.omg"
    ));
    check_countdown(countdown, "Countdown { remaining: 5 }, 5");
}

#[test]
fn record_field_countdown_executes_a_computed_initial_arrival() {
    let countdown = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/omega/pass/termination/measure_field_computed_arrival/main.omg"
    ));
    check_countdown(countdown, "Countdown { remaining: 5, limit: 5 }");
}

fn check_countdown(countdown: &str, arguments: &str) {
    let source = format!(
        "{countdown} machine main() -> i32 {{
            let result: u64 = walk({arguments});
            transition result == 0 {{ true -> 7 false -> 0 }}
        }}"
    );
    let checked =
        crate::front_end::checked_program_result(&source).expect("checked field arrivals");
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}
