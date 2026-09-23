use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use checked_interpreter::interpret_entry;

#[test]
fn unsigned_identity_views_check_and_execute_each_countdown() {
    let countdown = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/omega/pass/termination/unsigned_identity_measure_rank_range/main.omg"
    ));
    for carrier in ["u8", "u16", "u32", "u64"] {
        let source = format!(
            "{} machine main() -> i32 {{
                let result: {carrier} = walk(5);
                transition result == 0 {{ true -> 7 false -> 0 }}
            }}",
            countdown.replace("u32", carrier),
        );
        let checked = crate::front_end::checked_program_result(&source)
            .expect("checked unsigned identity view");
        let outcome = interpret_entry(
            &checked,
            BuildMachineEntry::Name("main"),
            &[],
            InterpretOptions::default(),
        );
        assert_eq!(outcome.error, None, "{source}");
        assert_eq!(outcome.exit_code, 7, "{source}");
    }
}
