use super::{checked_program, frame};

/// A reference-typed receiver field writes under its own path: a borrow moved
/// field-to-field keeps a finite caller-visible frame because every later
/// write through the stored borrow already composes beneath `self.<field>`.
/// The checked frame records `self.alias` and `frame::matches` admits it —
/// the write-frame-agreement gate no longer stops the body at the store.
#[test]
fn reference_field_store_keeps_the_receiver_path_in_a_complete_write_frame() {
    for access in ["&'a mut [u8]", "&'a [u8]"] {
        let source = format!(
            r#"
            data Main<'a> {{
                view: {access};
                alias: {access};
            }}
            machine Main::run(&mut self) {{
                self.alias = self.view;
            }}
        "#
        );
        let checked = checked_program(&source);
        let program = &checked.typed;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let write_frame = checked
            .facts
            .mutation
            .for_machine(machine.symbol)
            .unwrap()
            .state_write_frames
            .iter()
            .find(|entry| entry.state == state.symbol)
            .expect("state write frame")
            .frame
            .clone();
        assert_eq!(write_frame.paths(), ["self.alias"], "access: {access}");
        assert!(
            frame::matches(program, machine, state, &write_frame, None),
            "access: {access}"
        );
    }
}
