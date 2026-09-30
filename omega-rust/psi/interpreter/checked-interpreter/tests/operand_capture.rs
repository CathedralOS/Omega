//! Calls and construction capture each owned operand before evaluating the next one.
//! Borrowed operands keep their referents rather than becoming owned snapshots.

use checked_interpreter::{BuildMachineEntry, InterpretOptions, interpret_entry};

fn assert_exit(source: &str, expected: i32) {
    let checked = crate::front_end::checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None, "{source}");
    assert_eq!(outcome.exit_code, expected, "{source}");
}

#[test]
fn record_fields_capture_owned_arrays_before_later_effects() {
    assert_exit(
        "data Storage { samples: [i32; 2]; }
         data Packet { stamp: i32; samples: [i32; 2]; }
         machine stamp(storage: &mut Storage) -> i32 { storage.samples[0] = 90; 7 }
         machine main() -> i32 {
             let mut storage: Storage = Storage { samples: [30, 40] };
             let packet: Packet = Packet { samples: storage.samples, stamp: stamp(&mut storage) };
             transition packet.samples[0] == 30 && packet.samples[1] == 40
                 && packet.stamp == 7 && storage.samples[0] == 90 {
                 true -> 70 false -> 1
             }
         }",
        70,
    );
}

#[test]
fn case_fields_capture_owned_arrays_before_later_effects() {
    assert_exit(
        "data Storage { samples: [i32; 2]; }
         data Packet { case Ready(stamp: i32, samples: [i32; 2]); }
         machine stamp(storage: &mut Storage) -> i32 { storage.samples[0] = 90; 7 }
         machine main() -> i32 {
             let mut storage: Storage = Storage { samples: [30, 40] };
             let packet: Packet = Packet::Ready { samples: storage.samples, stamp: stamp(&mut storage) };
             transition packet {
                 Packet::Ready { stamp, samples } -> inspect(samples, stamp, storage.samples[0])
             }
             state inspect(samples: [i32; 2], stamp: i32, current: i32) -> i32 {
                 transition samples[0] == 30 && samples[1] == 40 && stamp == 7 && current == 90 {
                     true -> 70 false -> 1
                 }
             }
         }",
        70,
    );
}

#[test]
fn array_elements_capture_owned_values_before_later_effects() {
    assert_exit(
        "data Storage { samples: [i32; 2]; }
         machine replace(storage: &mut Storage) -> [i32; 2] {
             storage.samples[0] = 90; [5, 6]
         }
         machine main() -> i32 {
             let mut storage: Storage = Storage { samples: [30, 40] };
             let samples: [[i32; 2]; 2] = [storage.samples, replace(&mut storage)];
             transition samples[0][0] == 30 && samples[1][0] == 5 && storage.samples[0] == 90 {
                 true -> 70 false -> 1
             }
         }",
        70,
    );
}

#[test]
fn constructor_reference_operands_keep_the_original_referent() {
    assert_exit(
        "data Handle { value: &mut i32; }
         machine set(value: &mut i32) { value = 70; }
         machine main() -> i32 {
             let mut original: i32 = 1;
             let handle: Handle = Handle { value: &mut original };
             set(&mut handle.value);
             original
         }",
        70,
    );
}

#[test]
fn call_arguments_capture_owned_values_before_later_effects() {
    assert_exit(
        "data Settings [copy] { count: i32; }
         machine stamp(settings: &mut Settings) -> i32 { settings.count = 25; 25 }
         machine observe(settings: Settings, later: i32) -> i32 { settings.count }
         machine main() -> i32 {
             let mut settings: Settings = Settings { count: 17 };
             let before: i32 = observe(settings, stamp(&mut settings));
             transition before == 17 && settings.count == 25 { true -> 42 false -> 1 }
         }",
        42,
    );
}

#[test]
fn call_arguments_accept_implicit_shared_lending() {
    assert_exit(
        "data Settings { count: i32; }
         machine observe(settings: &Settings) -> i32 { settings.count }
         machine main() -> i32 {
             let settings: Settings = Settings { count: 70 };
             observe(settings)
         }",
        70,
    );
}
