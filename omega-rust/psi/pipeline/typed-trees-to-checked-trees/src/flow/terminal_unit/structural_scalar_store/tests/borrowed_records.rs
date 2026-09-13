use super::*;

fn checked(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::lower_typed_trees(typed)
}

#[test]
fn borrowed_record_store_selects_the_exact_destination_among_other_inputs() {
    for (parameters, destination_position) in [
        ("destination: &mut Counter, source: &Counter", 0),
        ("source: &Counter, destination: &mut Counter", 1),
        ("source: &Counter, extra: i32, destination: &mut Counter", 2),
    ] {
        let checked = checked(&format!(
            "data Counter {{ value: i32; }} machine copy_counter({parameters}) {{ destination.value = source.value; }}"
        )).unwrap();
        let program = &checked.typed;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "copy_counter")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let mut shapes = ShapeCollector::new(program);
        let (structural, scalar) =
            super::super::super::free_structural_scalar_signature(program, &mut shapes, state, &[])
                .unwrap();
        let stores = build_structural_scalar_field_store_sequence(
            program,
            &checked.facts,
            machine,
            state,
            &structural,
            &scalar,
            0,
        )
        .expect("read one borrowed record and write the independently selected destination");
        let [CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store)] = stores.as_slice()
        else {
            panic!("one field store");
        };
        assert_eq!(
            store.destination,
            checked_trees::CheckedStructuralScalarFieldStoreDestination::Parameter {
                position: destination_position
            }
        );
        assert!(store.carrier_path.is_empty());
        assert_eq!(store.field_identity, "value");

        let mut wrong_root = structural.clone();
        let other_position = structural
            .iter()
            .find(|parameter| parameter.position != destination_position)
            .unwrap()
            .position;
        wrong_root
            .iter_mut()
            .find(|parameter| parameter.position == destination_position)
            .unwrap()
            .position = other_position;
        assert!(
            build_structural_scalar_field_store_sequence(
                program,
                &checked.facts,
                machine,
                state,
                &wrong_root,
                &scalar,
                0
            )
            .is_none()
        );
        let mut shared_destination = structural.clone();
        shared_destination
            .iter_mut()
            .find(|parameter| parameter.position == destination_position)
            .unwrap()
            .access = checked_trees::CheckedStructuralAccess::SharedBorrow;
        assert!(
            build_structural_scalar_field_store_sequence(
                program,
                &checked.facts,
                machine,
                state,
                &shared_destination,
                &scalar,
                0
            )
            .is_none()
        );
    }
}

#[test]
fn borrowed_record_copy_rejects_shared_write_and_overlapping_arguments() {
    assert!(checked("data Counter { value: i32; } machine invalid(destination: &Counter, source: &Counter) { destination.value = source.value; }").is_err());
    assert!(checked(r#"
        data Counter { value: i32; }
        machine copy_counter(destination: &mut Counter, source: &Counter) { destination.value = source.value; }
        machine invalid(counter: &mut Counter) { copy_counter(&mut counter, &counter); }
    "#).is_err());
    assert!(checked(r#"
        data Counter { value: i32; }
        data Holder { counter: Counter; }
        machine copy_counter(source: &Counter, destination: &mut Counter) { destination.value = source.value; }
        machine Holder::invalid(&mut self) { copy_counter(&self.counter, &mut self.counter); }
    "#).is_err());
}

#[test]
fn borrowed_record_copy_retains_ordinary_callee_and_composed_receiver_plans() {
    let checked = checked(r#"
        boundary trait Console { machine write_byte(value: i32) reaches Console; }
        data Counter { value: i32; }
        machine copy_counter(source: &Counter, destination: &mut Counter) { destination.value = source.value; }
        data Main { value: i32; source: Counter; destination: Counter; }
        machine Main::main(&mut self) reaches Console {
            transition self.value == 0 { true -> initialized() false -> failed() }
            state initialized(&mut self) {
                self.source.value = 65;
                copy_counter(&self.source, &mut self.destination);
                self.value = self.destination.value;
                transition self.value == 65 { true -> observed() false -> failed() }
            }
            state observed(&mut self) { Console::write_byte(self.value); }
            state failed(&mut self) { Console::write_byte(70); }
        }
    "#).unwrap();
    let machine = |name| {
        checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap()
            .symbol
    };
    let effects = &checked.facts.flow.terminal_unit_effects;
    assert!(
        effects
            .machines
            .iter()
            .any(|plan| plan.machine == machine("copy_counter")),
        "ordinary copy callee must survive candidate closure"
    );
    assert!(
        effects
            .composed_machines
            .iter()
            .any(|plan| plan.machine == machine("Main::main")),
        "composed receiver must retain the copy and following observation"
    );
}
