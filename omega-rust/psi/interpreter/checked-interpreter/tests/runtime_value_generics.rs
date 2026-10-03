use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use checked_interpreter::interpret_entry;

fn execute(source: &str) -> checked_interpreter::InterpretOutcome {
    execute_entry(source, "main")
}

fn execute_entry(source: &str, entry: &str) -> checked_interpreter::InterpretOutcome {
    let checked = crate::front_end::checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    interpret_entry(
        &checked,
        BuildMachineEntry::Name(entry),
        &[],
        InterpretOptions::default(),
    )
}

#[test]
fn indexed_local_record_replays_the_captured_subject() {
    // `context.values[Count]` under `requires Count <= 7`: the write lands at the
    // runtime-selected slot and the read returns it. `n` is reassigned between
    // calls, a second subject `m` shares the one specialized body, and a
    // static `<3>` argument reads the slot the first call wrote.
    let outcome = execute_entry(
        "data Main { values: [u32; 8]; } \
         machine Main::at<Count: u32>(context: &Main) -> u32
         requires
             Count <= 7;
         { context.values[Count] } \
         machine Main::put<Count: u32>(context: &mut Main, value: u32)
         requires
             Count <= 7;
         { context.values[Count] = value; } \
         machine Main::main() -> i64 { \
             let mut context: Main; \
             let mut n: u32 = 3; \
             Main::put<n>(&mut context, 10); \
             n = 5; \
             Main::put<n>(&mut context, 20); \
             let a: u32 = Main::at<n>(&context); \
             let m: u32 = 4; \
             Main::put<m>(&mut context, 7); \
             let b: u32 = Main::at<m>(&context); \
             let c: u32 = Main::at<3>(&context); \
             (a as i64) + (b as i64) + (c as i64) \
         }",
        "Main::main",
    );
    // values[5] = 20, values[4] = 7, values[3] = 10.
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 37);
}

#[test]
fn forwarded_requires_subject_replays_through_the_nested_callee() {
    // `forward<K>` discharges `bounded<K>`'s `Count <= 10` with its own
    // `K <= 10` obligation, so the forwarded subject reaches the inner callee.
    let outcome = execute(
        "machine bounded<Count: u32>(base: u32) -> u32
         requires
             Count <= 10;
         { Count } \
         machine forward<K: u32>(base: u32) -> u32
         requires
             K <= 10;
         { bounded<K>(base) } \
         machine main() -> i64 { let n: u32 = 3; forward<n>(7) as i64 }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 3);
}

#[test]
fn forwarded_mixed_type_and_value_binders_preserve_their_separate_bindings() {
    let outcome = execute(
        "machine bounded<Count: u32, Element [copy]>(value: Element) -> u32
         requires Count <= 10;
         { Count }
         machine forward<Value [copy], K: u32>(value: Value) -> u32
         requires K <= 10;
         { bounded<K, Value>(value) }
         machine main() -> i64 {
             let count: u32 = 3;
             forward<i64, count>(17) as i64
         }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 3);
}

#[test]
fn forwarding_a_value_binder_does_not_discharge_its_missing_requirement() {
    let diagnostics = crate::front_end::checked_program_result(
        "machine bounded<Count: u32>(base: u32) -> u32
         requires Count <= 10;
         { Count }
         machine forward<K: u32>(base: u32) -> u32 { bounded<K>(base) }
         machine main() -> i64 { let count: u32 = 3; forward<count>(7) as i64 }",
    )
    .expect_err("the generic body cannot borrow the concrete caller's bound");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "expected an undischarged requires obligation: {diagnostics:#?}",
    );
    assert!(diagnostics.iter().all(|diagnostic| {
        !diagnostic
            .message
            .contains("invalid static type argument correspondence")
    }));
}
