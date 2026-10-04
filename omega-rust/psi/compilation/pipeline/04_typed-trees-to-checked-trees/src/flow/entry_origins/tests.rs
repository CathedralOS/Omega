use super::*;
use crate::tests::front_end::typed_program;

#[test]
fn entry_storage_origins_keep_named_state_alternatives_and_repeated_writes() {
    let program = typed_program(
        "machine exercise(left: &mut i64, right: &mut i64, flag: bool) {
             transition flag { true -> write(left) false -> write(right) }
             state write(renamed: &mut i64) { renamed = 128; renamed = 0; }
         }",
    );
    let machine = &program.machines()[0];
    let states = program.machine_states(machine);
    let frames = CallFrameResolver::new(&program).unwrap();
    let solved = machine_storage_origins(&program, &frames, machine);
    for statement in program
        .statement_table
        .statements(states[1].statement_nodes)
    {
        let sources = frames
            .assignment_destination_origins(machine, statement)
            .unwrap();
        assert_eq!(sources.len(), 1);
        let origins =
            entry_storage_origins(&program, machine, &states[1], &solved, &sources[0]).unwrap();
        assert_eq!(origins.len(), 2);
        for parameter in &program.state_parameters(&states[0])[..2] {
            assert!(origins.iter().any(|origin| origin.root == parameter.symbol));
        }
    }
    assert!(
        state_origins(&program, machine, &states[1], &solved).is_empty(),
        "alternative storage must not become one stable value"
    );
}

#[test]
fn entry_storage_origins_transport_rebound_alias_and_private_storage() {
    let program = typed_program(
        "machine exercise(left: &mut i64, right: &mut i64) {
             let mut selected: &mut i64 = left; selected = right;
             transition { _ -> renamed(selected) }
             state renamed(output: &mut i64) { output = 127; }
         }",
    );
    let machine = &program.machines()[0];
    let states = program.machine_states(machine);
    let frames = CallFrameResolver::new(&program).unwrap();
    let solved = machine_storage_origins(&program, &frames, machine);
    let source = root_storage(&program, program.state_parameters(&states[1])[0].symbol);
    let origins = entry_storage_origins(&program, machine, &states[1], &solved, &source).unwrap();
    assert_eq!(origins.len(), 1);
    assert_eq!(
        origins[0].root,
        program.state_parameters(&states[0])[1].symbol
    );
    let mut unknown = source;
    unknown.root = SymbolHandle::invalid();
    assert!(entry_storage_origins(&program, machine, &states[1], &solved, &unknown).is_none());
}

#[test]
fn entry_storage_origins_preserve_projected_recast_geometry() {
    let program = typed_program(
        "data Carrier { value: i64; } data Storage { bytes: [u8; 8]; }
         machine exercise(storage: &mut Storage) {
             let view: &mut Carrier = &mut storage.bytes[0] as &mut Carrier;
             transition { _ -> write(&mut view.value) }
             state write(output: &mut i64) { output = 127; }
         }",
    );
    let machine = &program.machines()[0];
    let states = program.machine_states(machine);
    let frames = CallFrameResolver::new(&program).unwrap();
    let solved = machine_storage_origins(&program, &frames, machine);
    let source = root_storage(&program, program.state_parameters(&states[1])[0].symbol);
    let origins = entry_storage_origins(&program, machine, &states[1], &solved, &source).unwrap();
    assert_eq!(origins.len(), 1);
    assert_eq!(
        origins[0].root,
        program.state_parameters(&states[0])[0].symbol
    );
    let (_, selectors) = origins[0]
        .representation
        .as_ref()
        .expect("logical view retained");
    assert!(matches!(
        selectors.as_slice(),
        [facts::PlaceSegment::Field { .. }]
    ));
    assert_ne!(origins[0].segments, *selectors);
}

#[test]
fn entry_storage_origins_keep_unknown_predecessor_among_known_alternatives() {
    let program = typed_program(
        "machine mutate(input: &mut i64) -> i64 { input = 0; 0 }
         machine exercise(left: &mut i64, right: &mut i64, flag: bool) {
             transition flag {
                 true -> write(0, left)
                 false -> write(mutate(right), right)
             }
             state write(number: i64, output: &mut i64) { output = number; }
         }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .unwrap();
    let states = program.machine_states(machine);
    let frames = CallFrameResolver::new(&program).unwrap();
    let solved = machine_storage_origins(&program, &frames, machine);
    let source = root_storage(&program, program.state_parameters(&states[1])[1].symbol);
    assert!(
        entry_storage_origins(&program, machine, &states[1], &solved, &source).is_none(),
        "the known predecessor cannot erase an unknown address"
    );
}

#[test]
fn entry_storage_origins_converge_on_identity_cycle() {
    let program = typed_program(
        "machine exercise(input: &mut i64, flag: bool) -> i64 {
             transition { _ -> write(input, flag) }
             state write(output: &mut i64, again: bool) -> i64 {
                 output = 127;
                 transition again { true -> write(output, again) false -> 0 }
             }
         }",
    );
    let machine = &program.machines()[0];
    let states = program.machine_states(machine);
    let frames = CallFrameResolver::new(&program).unwrap();
    let solved = machine_storage_origins(&program, &frames, machine);
    let source = root_storage(&program, program.state_parameters(&states[1])[0].symbol);
    let origins = entry_storage_origins(&program, machine, &states[1], &solved, &source).unwrap();
    assert_eq!(origins.len(), 1);
    assert_eq!(
        origins[0].root,
        program.state_parameters(&states[0])[0].symbol
    );
    let mut growing = source.clone();
    growing
        .segments
        .push(facts::PlaceSegment::FixedIndex { index: 0 });
    assert!(!frames.storage_origin_substitutions_are_finite(&[(source.root, growing)]));
}
