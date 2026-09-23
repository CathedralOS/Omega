use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use checked_interpreter::interpret_entry;

#[test]
fn resolved_calls_keep_exact_states_when_debug_spellings_collide() {
    let mut checked = crate::front_end::checked_program(
        "machine first() -> i32 { let result: i32 = chosen(); transition true { true -> result false -> 0 } state chosen() -> i32 { 3 } }
         machine second() -> i32 { let result: i32 = chosen(); transition true { true -> result false -> 0 } state chosen() -> i32 { 4 } }
         machine main() -> i32 { transition first() == 3 && second() == 4 { true -> 7 false -> 0 } }",
    );
    // Resolved symbols, not retained debug spelling, identify these calls. Make
    // both machine and state spellings collide after checking to expose a lost
    // target identity at the interpreter's resolution-to-execution boundary.
    let machines = checked.typed.machines()[..2].to_vec();
    for machine in &machines {
        for state in checked.typed.machine_states_mut(machine) {
            state.name = "shared".into();
        }
    }
    for machine in &mut checked.typed.machines_mut()[..2] {
        machine.name = "shared".into();
    }
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
fn sibling_transitions_preserve_receiver_and_arguments() {
    let checked = crate::front_end::checked_program(
        "data Counter { value: i32 in Wrapping; }
         machine Counter::advance(&mut self, amount: i32 in Wrapping) -> i32 {
             let carried: i32 in Wrapping = amount + 1;
             transition true { true -> finish(carried) false -> 0 }
             state finish(&mut self, carried: i32 in Wrapping) -> i32 {
                 self.value = self.value + carried;
                 self.value as i32
             }
         }
         machine main() -> i32 {
             let mut counter: Counter = Counter { value: 2 };
             let result: i32 = counter.advance(4);
             transition result == 7 && counter.value == 7 { true -> 7 false -> 0 }
         }",
    );
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
fn resolved_calls_evaluate_arguments_once_in_source_order() {
    let checked = crate::front_end::checked_program(
        "machine step(value: &mut i32 in Wrapping) -> i32 { value = value + 1; value as i32 }
         machine combine(left: i32 in Wrapping, right: i32 in Wrapping) -> i32 {
             (left * 10 + right) as i32
         }
         machine main() -> i32 {
             let mut count: i32 in Wrapping = 0;
             let result: i32 = combine(step(&mut count), step(&mut count));
             transition result == 12 && count == 2 { true -> 7 false -> 0 }
         }",
    );
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
fn self_recursive_tail_transition_does_not_consume_call_depth() {
    let checked = crate::front_end::checked_program(
        "machine countdown(count: u64) -> i32 {
             transition count > 0 { true -> countdown(count - 1) false -> 7 }
         }
         machine main() -> i32 { countdown(1024) }",
    );
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}
