use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use checked_interpreter::interpret_entry;

fn execute(source: &str) -> checked_interpreter::InterpretOutcome {
    let checked = crate::front_end::checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    )
}

fn assert_seven(source: &str) {
    let outcome = execute(source);
    assert_eq!(outcome.error, None, "{source}");
    assert_eq!(outcome.exit_code, 7, "{source}");
}

#[test]
fn array_assignments_and_record_fields_keep_element_destinations() {
    for body in [
        "let mut values: [i32; 1] = [0]; values = [7 / 2.0 * 2]; values[0]",
        "let packet: Packet = Packet { values: [0.1 * 70] }; packet.values[0]",
    ] {
        assert_seven(&format!(
            "data Packet {{ values: [i32; 1]; }} machine main() -> i32 {{ {body} }}"
        ));
    }
}

#[test]
fn ordinary_array_arguments_keep_element_destinations() {
    assert_seven(
        "machine accept(values: [i32; 1]) -> i32 { values[0] }
        machine main() -> i32 { accept([7 / 2.0 * 2]) }",
    );
    assert_seven(
        "machine main() -> i32 {
        transition { _ -> finish([0.1 * 70]) }
        state finish(values: [i32; 1]) -> i32 { values[0] }
    }",
    );
}

#[test]
fn array_returns_keep_element_destinations() {
    for body in ["[7 / 2.0 * 2]", "transition { _ -> [0.1 * 70] }"] {
        assert_seven(&format!(
            "machine make() -> [i32; 1] {{ {body} }}
            machine main() -> i32 {{ let values: [i32; 1] = make(); values[0] }}"
        ));
    }
}
