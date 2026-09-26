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
fn runtime_subject_reaches_the_specialized_callee() {
    let outcome = execute(
        "machine pick<Count: u32>(base: u32) -> i64 { Count as i64 } \
         machine main() -> i64 { let n: u32 = 3; pick<n>(7) }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 3);
}

#[test]
fn reassigned_source_captures_the_current_value() {
    let outcome = execute(
        "machine pick<Count: u32>(base: u32) -> i64 { Count as i64 } \
         machine main() -> i64 { let mut n: u32 = 3; n = 5; pick<n>(7) }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 5);
}

#[test]
fn runtime_subject_discharges_a_requires_contract() {
    let outcome = execute(
        "machine pick<Count: u32>(base: u32) -> i64
         requires
             Count <= 10;
         { Count as i64 } \
         machine main() -> i64 { let n: u32 = 3; pick<n>(7) }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 3);
}

#[test]
fn forwarded_subject_reaches_the_nested_callee() {
    let outcome = execute(
        "machine pick<Count: u32>(base: u32) -> i64 { Count as i64 } \
         machine forward<K: u32>(base: u32) -> i64 { pick<K>(base) } \
         machine main() -> i64 { let n: u32 = 3; forward<n>(7) }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 3);
}

#[test]
fn distinct_runtime_subjects_share_one_machine_body() {
    // Both calls target the same carrier specialization; each replays its own
    // captured subject.
    let outcome = execute(
        "machine pick<Count: u32>(base: u32) -> i64 { Count as i64 } \
         machine main() -> i64 { \
             let n: u32 = 3; \
             let m: u32 = 4; \
             let a: i64 = pick<n>(7); \
             let b: i64 = pick<m>(7); \
             match a == 3i64 && b == 4i64 { true -> 7i64, false -> 0i64 } \
         }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn indexed_attached_field_replays_the_captured_subject() {
    // `self.values[Count]` under `requires Count <= 7`: the write lands at the
    // runtime-selected slot and the read returns it. `n` is reassigned between
    // calls, a second subject `m` shares the one specialized body, and a
    // static `<3>` argument reads the slot the first call wrote.
    let outcome = execute_entry(
        "data Main { values: [u32; 8]; } \
         machine Main::at<Count: u32>(&self) -> u32
         requires
             Count <= 7;
         { self.values[Count] } \
         machine Main::put<Count: u32>(&mut self, v: u32)
         requires
             Count <= 7;
         { self.values[Count] = v; } \
         machine Main::main(&mut self) -> i64 { \
             let mut n: u32 = 3; \
             self.put<n>(10); \
             n = 5; \
             self.put<n>(20); \
             let a: u32 = self.at<n>(); \
             let m: u32 = 4; \
             self.put<m>(7); \
             let b: u32 = self.at<m>(); \
             let c: u32 = self.at<3>(); \
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
