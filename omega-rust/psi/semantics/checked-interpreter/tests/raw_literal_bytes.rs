use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use checked_interpreter::interpret_entry;

#[test]
fn interpreter_writes_non_utf8_literal_bytes_exactly() {
    let checked = crate::front_end::checked_program_result(
        r#"
        boundary trait Console {
            machine write_line(text: &[u8]);
        }

        data Main<'s> { console: &'s mut Console; }

        machine Main::main(&mut self) reaches Console {
            self.console.write_line("\x80A");
        }
        "#,
    )
    .expect("check raw bytes");

    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("Main::main"),
        &[],
        InterpretOptions::default(),
    );

    assert_eq!(outcome.error, None);
    assert_eq!(outcome.stdout, [0x80, b'A', b'\n']);
}
