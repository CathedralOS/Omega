#[test]
fn multiarm_reference_result_keeps_every_caller_write_destination() {
    let program = crate::front_end::typed_program(
        r#"
        data Cell { value: u64; untouched: u64; }
        data Main { first: Cell; second: Cell; untouched: Cell; }
        machine choose(first: &mut Cell, second: &mut Cell, selected: bool) -> &mut Cell {
            transition {
                selected -> (first)
                _ -> (second)
            }
        }
        machine Main::inspect(&mut self, selected: bool) {
            let alias: &mut Cell = choose(&mut self.first, &mut self.second, selected);
            alias.value = 7;
        }
        "#,
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::inspect")
        .expect("caller");
    let frames = crate::CallFrameResolver::new(&program)
        .expect("frame resolver")
        .inferred_machine_state_write_frames(machine);
    assert_eq!(frames.len(), 1);
    assert_eq!(
        frames[0].complete_paths(),
        Some(
            [
                "self.first.value".to_owned(),
                "self.second.value".to_owned()
            ]
            .as_slice()
        ),
        "the returned loan can reach either field, but not their siblings",
    );
}

#[test]
fn multiarm_reference_result_keeps_indexed_collection_origin_coarse() {
    let program = crate::front_end::typed_program(
        r#"
        data Cell { value: u64; }
        data Main { cells: [Cell; 2]; untouched: Cell; }
        machine choose(cells: &mut [Cell; 2], selected: bool) -> &mut Cell {
            let slots: &mut [Cell] = cells.as_mut_slice();
            transition {
                selected -> &mut slots[0]
                _ -> &mut slots[1]
            }
        }
        machine Main::inspect(&mut self, selected: bool) {
            let alias: &mut Cell = choose(&mut self.cells, selected);
            alias.value = 7;
        }
        "#,
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::inspect")
        .expect("caller");
    let frames = crate::CallFrameResolver::new(&program)
        .expect("frame resolver")
        .inferred_machine_state_write_frames(machine);
    assert_eq!(frames.len(), 1);
    assert_eq!(
        frames[0].complete_paths(),
        Some(["self.cells".to_owned()].as_slice()),
        "an indexed referent retains its whole collection root",
    );
}

#[test]
fn multiarm_reference_result_composes_a_method_frame_across_every_origin() {
    let program = crate::front_end::typed_program(
        r#"
        data Cell { value: u64; untouched: u64; }
        data Main { first: Cell; second: Cell; untouched: Cell; }
        machine choose(first: &mut Cell, second: &mut Cell, selected: bool) -> &mut Cell {
            transition {
                selected -> (first)
                _ -> (second)
            }
        }
        machine Cell::replace(&mut self) {
            self.value = 7;
        }
        machine Main::inspect(&mut self, selected: bool) {
            let alias: &mut Cell = choose(&mut self.first, &mut self.second, selected);
            alias.replace();
        }
        "#,
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::inspect")
        .expect("caller");
    let frames = crate::CallFrameResolver::new(&program)
        .expect("frame resolver")
        .inferred_machine_state_write_frames(machine);
    assert_eq!(frames.len(), 1);
    assert_eq!(
        frames[0].complete_paths(),
        Some(
            [
                "self.first.value".to_owned(),
                "self.second.value".to_owned()
            ]
            .as_slice()
        ),
        "the method frame is instantiated at every proven receiver origin",
    );
}

#[test]
fn multiarm_reference_result_does_not_invent_projected_receiver_origins() {
    let program = crate::front_end::typed_program(
        r#"
        data Value { number: u64; }
        data Cell { value: Value; }
        data Main { first: Cell; second: Cell; }
        machine choose(first: &mut Cell, second: &mut Cell, selected: bool) -> &mut Cell {
            transition {
                selected -> (first)
                _ -> (second)
            }
        }
        machine Value::replace(&mut self) {
            self.number = 7;
        }
        machine Main::inspect(&mut self, selected: bool) {
            let alias: &mut Cell = choose(&mut self.first, &mut self.second, selected);
            alias.value.replace();
        }
        "#,
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::inspect")
        .expect("caller");
    let frames = crate::CallFrameResolver::new(&program)
        .expect("frame resolver")
        .inferred_machine_state_write_frames(machine);
    assert_eq!(frames.len(), 1);
    assert!(
        !frames[0].is_complete(),
        "a projected divergent receiver needs its own origin proof",
    );
}

#[test]
fn multiarm_reference_result_rejects_an_untraceable_return_arm() {
    let program = crate::front_end::typed_program(
        r#"
        data Cell { value: u64; }
        data Main { first: Cell; second: Cell; }
        boundary data Token;
        boundary trait Device {
            machine reference(value: &mut Cell, metadata: Token) -> &mut Cell;
        }
        machine choose(
            first: &mut Cell, second: &mut Cell, metadata: Token, selected: bool
        ) -> &mut Cell {
            transition {
                selected -> (first)
                _ -> (Device::reference(second, metadata))
            }
        }
        machine Main::inspect(&mut self, metadata: Token, selected: bool) {
            let alias: &mut Cell = choose(&mut self.first, &mut self.second, metadata, selected);
            alias.value = 7;
        }
        "#,
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::inspect")
        .expect("caller");
    let frames = crate::CallFrameResolver::new(&program)
        .expect("frame resolver")
        .inferred_machine_state_write_frames(machine);
    assert_eq!(frames.len(), 1);
    assert!(
        !frames[0].is_complete(),
        "a proven arm cannot hide another arm's opaque referent",
    );
}
