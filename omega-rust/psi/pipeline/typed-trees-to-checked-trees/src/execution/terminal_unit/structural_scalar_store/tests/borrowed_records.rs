use super::CheckedUnitEffectOperationPlan;
use crate::execution::terminal_unit::ShapeCollector;
use crate::execution::terminal_unit::structural_scalar_store::build_structural_scalar_field_store_sequence;
use crate::tests::front_end::checked_program_result;

#[test]
fn borrowed_receiver_scalar_results_survive_later_mutation_in_composed_plan() {
    let checked = checked(
        r#"
        boundary trait Console { machine write_byte(value: i32) reaches Console; }
        data Counter { value: i32; }
        machine Counter::read(&self) -> i32 { self.value }
        machine Counter::write(&mut self, value: i32) { self.value = value; }
        data Main { value: i32; source: Counter; }
        machine Main::main(&mut self) reaches Console {
            transition self.value == 0 { true -> initialized() false -> failed() }
            state initialized(&mut self) {
                self.source.value = 65;
                let saved: i32 = self.source.read();
                self.source.write(66);
                let current: i32 = self.source.read();
                self.value = saved;
                transition saved == 65 && current == 66 { true -> observed() false -> failed() }
            }
            state observed(&mut self) { Console::write_byte(self.value); }
            state failed(&mut self) { Console::write_byte(70); }
        }
    "#,
    )
    .unwrap();
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
            .any(|plan| plan.machine == machine("Counter::write")),
        "mutable scalar-parameter callee survives"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine("Counter::read"))
            .is_some()
            || checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(machine("Counter::read"))
                .is_some()
            || effects
                .machines
                .iter()
                .any(|plan| plan.machine == machine("Counter::read")
                    && (plan.scalar_result.is_some() || plan.scalar_control.is_some())),
        "shared scalar-result callee survives"
    );
    let main = checked
        .typed
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine("Main::main"))
        .unwrap();
    let initialized = &checked.typed.machine_states(main)[1];
    let mut receiver_calls = Vec::new();
    for (statement, binding) in [(1, 0), (3, 1)] {
        let root = checked
            .facts
            .values
            .scalar_computations
            .root_at(
                initialized.symbol,
                statement,
                checked_trees::CheckedScalarExpressionRole::LocalInitializer {
                    binding_ordinal: binding,
                },
            )
            .expect("each initializer retains its own whole-call computation");
        let checked_trees::CheckedScalarComputationKind::Call {
            source_call,
            target_state,
            structural_arguments,
            ..
        } = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(root.root)
            .kind
        else {
            panic!("ordinary scalar call");
        };
        let [checked_trees::CheckedScalarComputationStructuralArgument::Place(argument)] = checked
            .facts
            .values
            .scalar_computations
            .structural_arguments
            .span(structural_arguments)
            .unwrap()
        else {
            panic!("one projected receiver");
        };
        assert_eq!(argument.source_parameter_index(), Some(0));
        assert_eq!(
            argument.path,
            [checked_trees::CheckedUnitStructuralPathSegment::Field(
                "source".into()
            )]
        );
        assert_eq!(
            argument.access,
            checked_trees::CheckedStructuralAccess::SharedBorrow
        );
        receiver_calls.push((source_call, target_state));
    }
    assert_ne!(
        receiver_calls[0].0, receiver_calls[1].0,
        "mutation separates the two observations"
    );
    let first = checked.facts.flow.control.calls.get(receiver_calls[0].0);
    let later = checked.facts.flow.control.calls.get(receiver_calls[1].0);
    let typed_trees::expression::ExpressionNode::Call(first_expression) = checked
        .typed
        .expression_table
        .expression(first.authored_expression)
    else {
        panic!("call");
    };
    let typed_trees::expression::ExpressionNode::Call(later_expression) = checked
        .typed
        .expression_table
        .expression(later.authored_expression)
    else {
        panic!("call");
    };
    let target_state =
        crate::semantic_calls::find_state(&checked.typed, receiver_calls[0].1).unwrap();
    let target = &checked.typed.state_parameters(target_state)[0];
    let reconstruct = |call, expression| {
        crate::execution::terminal_unit::structural_computation_argument(
            &checked.typed,
            &checked.facts.borrow,
            main.symbol,
            initialized,
            call,
            expression,
            target,
        )
    };
    assert!(reconstruct(first, first_expression.receiver).is_some());
    assert!(
        reconstruct(first, later_expression.receiver).is_none(),
        "a later same-typed receiver occurrence cannot replace the original actual"
    );
    // The computation owner, before operand reconstruction, joins the exact
    // source expression to its statement. Equal receiver geometry alone does
    // not distinguish these two observations of the same mutable storage.
    let mut stale = checked.facts.flow.clone();
    stale
        .control
        .calls
        .get_mut(receiver_calls[0].0)
        .statement_index = later.statement_index;
    let rebuilt = crate::values::build_checked_scalar_computation_plans(
        &checked.typed,
        &checked.facts.operators,
        &stale,
        &checked.facts.borrow,
        &checked.facts.proof,
        &checked.facts.values.scalar_expressions,
        &[],
    );
    assert!(
        rebuilt
            .root_at(
                initialized.symbol,
                1,
                checked_trees::CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 }
            )
            .is_none(),
        "an earlier captured receiver cannot be replayed at the later call site"
    );
    assert!(
        rebuilt
            .root_at(
                initialized.symbol,
                3,
                checked_trees::CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 1 }
            )
            .is_some(),
        "the later call retains its own exact authored occurrence"
    );
    assert!(
        effects
            .composed_machines
            .iter()
            .any(|plan| plan.machine == machine("Main::main")),
        "saved scalar results compose with intervening mutation"
    );
}

#[test]
fn borrowed_receiver_scalar_calls_do_not_grant_shared_storage_mutation() {
    assert!(checked("data Counter { value: i32; } machine Counter::write(&mut self, value: i32) { self.value = value; } machine invalid(counter: &Counter) { counter.write(66); }").is_err());
}

fn checked(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    checked_program_result(source)
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
            None,
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
                0,
                None,
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
                0,
                None,
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
